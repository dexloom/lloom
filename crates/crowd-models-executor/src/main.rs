//! Crowd Models Executor
//! 
//! A service provider node that executes LLM requests and reports usage to the blockchain.

mod config;
mod llm_client;
mod blockchain;

use anyhow::Result;
use clap::Parser;
use config::{ExecutorConfig, LlmBackendConfig};
use crowd_models_core::{
    identity::Identity,
    network::{LlmP2pBehaviour, LlmP2pEvent, helpers},
    protocol::{LlmRequest, LlmResponse, ServiceRole, UsageRecord},
};
use futures::StreamExt;
use libp2p::{
    kad::{self, Record},
    request_response::{self, ResponseChannel},
    swarm::{SwarmEvent, Swarm},
    Multiaddr, SwarmBuilder,
};
use llm_client::LlmClient;
use blockchain::BlockchainClient;
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    signal,
    sync::mpsc,
    time::{interval, Interval},
};
use tracing::{debug, error, info, warn};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Private key (hex encoded) for identity
    #[arg(long, env = "CROWD_MODELS_PRIVATE_KEY")]
    private_key: Option<String>,
    
    /// Bootstrap nodes to connect to
    #[arg(long, value_delimiter = ',')]
    bootstrap_nodes: Vec<String>,
    
    /// Path to configuration file
    #[arg(long, default_value = "config.toml")]
    config: String,
    
    /// OpenAI API key (overrides config)
    #[arg(long, env = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
    
    /// Sepolia RPC URL
    #[arg(long, env = "SEPOLIA_RPC_URL", default_value = "https://rpc.sepolia.org")]
    rpc_url: String,
    
    /// Accounting contract address on Sepolia
    #[arg(long, env = "ACCOUNTING_CONTRACT")]
    contract_address: Option<String>,

    /// P2P port to listen on
    #[arg(short = 'p', long, default_value = "9001")]
    port: u16,

    /// Enable debug logging
    #[arg(short = 'd', long)]
    debug: bool,
}

/// Executor state and runtime data
struct ExecutorState {
    identity: Identity,
    config: ExecutorConfig,
    llm_clients: HashMap<String, LlmClient>,
    usage_records: Vec<UsageRecord>,
    pending_requests: HashMap<request_response::RequestId, ResponseChannel<LlmResponse>>,
    blockchain_client: Option<BlockchainClient>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize logging
    let log_level = if args.debug { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(log_level.parse()?)
                .add_directive("libp2p=info".parse()?),
        )
        .init();
    
    info!("Starting Crowd Models Executor...");
    info!("Config file: {}", args.config);
    info!("RPC URL: {}", args.rpc_url);
    
    // Load or generate identity
    let identity = match args.private_key {
        Some(key) => {
            info!("Loading identity from private key");
            Identity::from_str(&key)?
        }
        None => {
            info!("Generating ephemeral identity");
            Identity::generate()
        }
    };
    
    info!("Node identity: PeerId={}", identity.peer_id);
    info!("EVM address: {}", identity.evm_address);
    
    // Load configuration
    let mut config = if std::path::Path::new(&args.config).exists() {
        info!("Loading configuration from {}", args.config);
        ExecutorConfig::from_file(&args.config)?
    } else {
        info!("Configuration file not found, using defaults");
        ExecutorConfig::default()
    };
    
    // Override config with CLI arguments
    if let Some(contract) = args.contract_address {
        config.blockchain.contract_address = Some(contract);
    }
    config.blockchain.rpc_url = args.rpc_url;
    config.network.port = args.port;
    config.network.bootstrap_nodes = args.bootstrap_nodes;
    
    // Override OpenAI API key if provided
    if let Some(api_key) = args.openai_api_key {
        for backend in &mut config.llm_backends {
            if backend.name == "openai" {
                backend.api_key = Some(api_key.clone());
                break;
            }
        }
    }
    
    // Initialize LLM clients
    let mut llm_clients = HashMap::new();
    for backend_config in &config.llm_backends {
        match LlmClient::new(backend_config.clone()) {
            Ok(client) => {
                llm_clients.insert(backend_config.name.clone(), client);
                info!("Initialized LLM client for backend: {}", backend_config.name);
            }
            Err(e) => {
                warn!("Failed to initialize LLM client for {}: {}", backend_config.name, e);
            }
        }
    }
    
    if llm_clients.is_empty() {
        return Err(anyhow::anyhow!("No LLM clients could be initialized"));
    }
    
    // Create network behaviour
    let behaviour = LlmP2pBehaviour::new(&identity)?;
    
    // Build swarm
    let mut swarm = SwarmBuilder::with_existing_identity(identity.p2p_keypair.clone())
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )?
        .with_behaviour(|_| behaviour)?
        .build();
    
    // Listen on specified port
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", config.network.port).parse()?;
    swarm.listen_on(listen_addr)?;
    
    // Add external address if configured
    if let Some(external_addr) = &config.network.external_address {
        let addr: Multiaddr = external_addr.parse()?;
        swarm.add_external_address(addr);
    }
    
    // Bootstrap with known nodes
    if !config.network.bootstrap_nodes.is_empty() {
        let bootstrap_peers: Result<Vec<_>> = config.network.bootstrap_nodes
            .iter()
            .map(|addr_str| -> Result<(libp2p::PeerId, Multiaddr)> {
                let addr: Multiaddr = addr_str.parse()?;
                // Extract peer ID from multiaddr if present, or use a dummy one for now
                // In a real implementation, you'd need the actual peer IDs
                Ok((identity.peer_id, addr))
            })
            .collect();
            
        if let Ok(peers) = bootstrap_peers {
            helpers::bootstrap_kademlia(&mut swarm, peers);
        }
    }
    
    // Subscribe to gossipsub topics
    helpers::subscribe_topic(&mut swarm, "crowd-models/announcements")?;
    helpers::subscribe_topic(&mut swarm, "crowd-models/executor-updates")?;
    
    // Register as executor in Kademlia
    let executor_key = ServiceRole::Executor.to_kad_key();
    let record = Record {
        key: executor_key.clone().into(),
        value: identity.peer_id.to_bytes(),
        publisher: Some(identity.peer_id),
        expires: None,
    };
    swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One)?;
    
    // Initialize blockchain client
    let blockchain_client = match BlockchainClient::new(identity.clone(), config.blockchain.clone()).await {
        Ok(client) => {
            info!("Blockchain client initialized successfully");
            
            // Perform health check
            if let Err(e) = client.health_check().await {
                warn!("Blockchain health check failed: {}", e);
            }
            
            Some(client)
        }
        Err(e) => {
            warn!("Failed to initialize blockchain client: {}", e);
            warn!("Usage records will not be submitted to blockchain");
            None
        }
    };

    info!("Executor node started successfully");
    info!("Supported models: {:?}", config.get_all_supported_models());
    
    // Initialize executor state
    let mut executor_state = ExecutorState {
        identity,
        config: config.clone(),
        llm_clients,
        usage_records: Vec::new(),
        pending_requests: HashMap::new(),
        blockchain_client,
    };
    
    // Set up timers
    let mut announce_interval = interval(Duration::from_secs(config.network.announce_interval_secs));
    let mut batch_interval = interval(Duration::from_secs(config.blockchain.batch_interval_secs));
    
    // Set up shutdown signal handler
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
        let _ = shutdown_tx.send(()).await;
    });
    
    // Main event loop
    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                handle_swarm_event(&mut swarm, event, &mut executor_state).await;
            }
            _ = announce_interval.tick() => {
                announce_executor(&mut swarm, &executor_state).await;
            }
            _ = batch_interval.tick() => {
                submit_usage_batch(&mut executor_state).await;
            }
            _ = shutdown_rx.recv() => {
                info!("Received shutdown signal");
                break;
            }
        }
    }
    
    info!("Shutting down executor node...");
    
    // Submit any remaining usage records
    if !executor_state.usage_records.is_empty() {
        info!("Submitting remaining {} usage records", executor_state.usage_records.len());
        submit_usage_batch(&mut executor_state).await;
    }
    
    Ok(())
}

/// Handle swarm events
async fn handle_swarm_event(
    swarm: &mut Swarm<LlmP2pBehaviour>,
    event: SwarmEvent<LlmP2pEvent>,
    state: &mut ExecutorState,
) {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            info!("Listening on {}", address);
        }
        SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
            debug!("Connection established with {} at {}", peer_id, endpoint.get_remote_address());
        }
        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
            debug!("Connection closed with {}: {:?}", peer_id, cause);
        }
        SwarmEvent::Behaviour(LlmP2pEvent::RequestResponse(
            request_response::Event::Message { message, peer, .. }
        )) => {
            match message {
                request_response::Message::Request { request, channel, .. } => {
                    info!("Received LLM request from {}: model={}", peer, request.model);
                    handle_llm_request(swarm, request, channel, peer, state).await;
                }
                request_response::Message::Response { response, .. } => {
                    debug!("Received unexpected response: {:?}", response);
                }
            }
        }
        SwarmEvent::Behaviour(LlmP2pEvent::Kademlia(kad::Event::OutboundQueryProgressed {
            result,
            ..
        })) => {
            debug!("Kademlia query result: {:?}", result);
        }
        SwarmEvent::Behaviour(LlmP2pEvent::Gossipsub(libp2p::gossipsub::Event::Message {
            message,
            ..
        })) => {
            debug!("Received gossipsub message on topic {:?}", message.topic);
        }
        _ => {}
    }
}

/// Handle an incoming LLM request
async fn handle_llm_request(
    _swarm: &mut Swarm<LlmP2pBehaviour>,
    request: LlmRequest,
    channel: ResponseChannel<LlmResponse>,
    client_peer: libp2p::PeerId,
    state: &mut ExecutorState,
) {
    let model = request.model.clone();
    
    // Find the appropriate backend for this model
    let backend_name = match state.config.find_backend_for_model(&model) {
        Some(backend) => backend.name.clone(),
        None => {
            let error_response = LlmResponse {
                content: String::new(),
                token_count: 0,
                model_used: model.clone(),
                error: Some(format!("Model {} not supported", model)),
            };
            
            if let Err(e) = _swarm.behaviour_mut().request_response.send_response(channel, error_response) {
                error!("Failed to send error response: {:?}", e);
            }
            return;
        }
    };
    
    // Get the LLM client
    let llm_client = match state.llm_clients.get(&backend_name) {
        Some(client) => client,
        None => {
            let error_response = LlmResponse {
                content: String::new(),
                token_count: 0,
                model_used: model.clone(),
                error: Some(format!("Backend {} not available", backend_name)),
            };
            
            if let Err(e) = _swarm.behaviour_mut().request_response.send_response(channel, error_response) {
                error!("Failed to send error response: {:?}", e);
            }
            return;
        }
    };
    
    // Execute the LLM request
    match llm_client.chat_completion(
        &request.model,
        &request.prompt,
        request.system_prompt.as_deref(),
        request.temperature,
        request.max_tokens,
    ).await {
        Ok((content, token_count)) => {
            info!("LLM request completed: {} tokens used", token_count);
            
            let response = LlmResponse {
                content,
                token_count,
                model_used: model.clone(),
                error: None,
            };
            
            // Send response
            if let Err(e) = _swarm.behaviour_mut().request_response.send_response(channel, response) {
                error!("Failed to send response: {:?}", e);
            } else {
                // Record usage for blockchain submission
                // TODO: In a real implementation, we need to map the peer_id to the client's EVM address
                // For now, we'll use a placeholder approach
                let usage_record = UsageRecord {
                    client_address: state.identity.evm_address, // Placeholder - should be actual client address
                    model: model.clone(),
                    token_count,
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                };
                state.usage_records.push(usage_record);
            }
        }
        Err(e) => {
            error!("LLM request failed: {}", e);
            
            let error_response = LlmResponse {
                content: String::new(),
                token_count: 0,
                model_used: model,
                error: Some(e.to_string()),
            };
            
            if let Err(e) = _swarm.behaviour_mut().request_response.send_response(channel, error_response) {
                error!("Failed to send error response: {:?}", e);
            }
        }
    }
}

/// Announce executor availability
async fn announce_executor(
    swarm: &mut Swarm<LlmP2pBehaviour>,
    state: &ExecutorState,
) {
    // Re-announce in Kademlia
    let executor_key = ServiceRole::Executor.to_kad_key();
    let record = Record {
        key: executor_key.into(),
        value: state.identity.peer_id.to_bytes(),
        publisher: Some(state.identity.peer_id),
        expires: None,
    };
    
    if let Err(e) = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
        warn!("Failed to announce executor: {:?}", e);
    } else {
        debug!("Announced executor availability");
    }
}

/// Submit usage records to blockchain
async fn submit_usage_batch(state: &mut ExecutorState) {
    if state.usage_records.is_empty() {
        debug!("No usage records to submit");
        return;
    }
    
    info!("Submitting {} usage records to blockchain", state.usage_records.len());
    
    if let Some(ref blockchain_client) = state.blockchain_client {
        // Take the records to submit
        let records_to_submit = std::mem::take(&mut state.usage_records);
        
        // Submit to blockchain
        match blockchain_client.submit_usage_batch(records_to_submit).await {
            Ok(failed_records) => {
                if failed_records.is_empty() {
                    info!("All usage records submitted successfully");
                } else {
                    warn!("{} records failed to submit, will retry later", failed_records.len());
                    // Add failed records back to the queue
                    state.usage_records.extend(failed_records);
                }
            }
            Err(e) => {
                error!("Failed to submit usage batch: {}", e);
                // Add records back to the queue for retry
                state.usage_records = std::mem::take(&mut state.usage_records);
            }
        }
    } else {
        warn!("Blockchain client not available, discarding {} usage records", state.usage_records.len());
        state.usage_records.clear();
    }
}