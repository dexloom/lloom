//! Lloom Executor
//!
//! A service provider node that executes LLM requests and reports usage to the blockchain.

mod config;
mod llm_client;
mod blockchain;

use anyhow::Result;
use clap::Parser;
use config::ExecutorConfig;
use lloom_core::{
    identity::Identity,
    network::{LloomBehaviour, LloomEvent, helpers},
    protocol::{
        LlmRequest, LlmResponse, ServiceRole, UsageRecord, RequestMessage, ResponseMessage,
        constants::MAX_MESSAGE_AGE_SECS, ModelAnnouncement, ModelDescriptor, ModelCapabilities,
        AnnouncementType, ModelPricing
    },
    signing::{SignableMessage},
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
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    signal,
    sync::mpsc,
    time::interval,
};
use tracing::{debug, error, info, trace, warn};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Private key (hex encoded) for identity
    #[arg(long, env = "LLOOM_PRIVATE_KEY")]
    private_key: Option<String>,
    
    /// Bootstrap nodes to connect to
    #[arg(long, value_delimiter = ',')]
    bootstrap_nodes: Vec<String>,
    
    /// Path to configuration file
    #[arg(long)]
    config: Option<String>,
    
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
    
    /// Enable message signing (default: true)
    #[arg(long, default_value = "true")]
    enable_signing: bool,

    /// Test model health and exclude failed models
    #[arg(long)]
    test: bool,
}

/// Helper function to create model descriptors from executor configuration
async fn create_model_descriptors(
    config: &ExecutorConfig,
    llm_clients: &HashMap<String, LlmClient>,
) -> Vec<ModelDescriptor> {
    let mut descriptors = Vec::new();
    
    for backend_config in &config.llm_backends {
        let client = match llm_clients.get(&backend_config.name) {
            Some(client) => client,
            None => {
                warn!("No LLM client found for backend: {}", backend_config.name);
                continue;
            }
        };
        
        for model_id in &backend_config.supported_models {
            let mut capabilities = ModelCapabilities {
                max_context_length: 4096, // Default context length
                features: vec!["chat".to_string(), "completion".to_string()],
                architecture: None,
                model_size: None,
                performance: None,
                metadata: std::collections::HashMap::new(),
            };
            
            // Try to get model-specific information if available
            if client.is_lmstudio_backend() {
                // For LMStudio, we could potentially get more detailed model info
                capabilities.features.push("streaming".to_string());
                capabilities.metadata.insert(
                    "backend_type".to_string(),
                    serde_json::Value::String("lmstudio".to_string())
                );
            }
            
            let descriptor = ModelDescriptor {
                model_id: model_id.clone(),
                backend_type: backend_config.name.clone(),
                capabilities,
                is_available: true,
                pricing: Some(ModelPricing {
                    input_token_price: "500000000000000".to_string(), // 0.0005 ETH per token
                    output_token_price: "1000000000000000".to_string(), // 0.001 ETH per token
                    minimum_fee: None,
                }),
            };
            
            descriptors.push(descriptor);
        }
    }
    
    info!("Created {} model descriptors for announcement", descriptors.len());
    descriptors
}

/// Send model announcement via gossipsub (since no direct validator connection exists yet)
async fn announce_models_to_network(
    swarm: &mut Swarm<LloomBehaviour>,
    identity: &Identity,
    config: &ExecutorConfig,
    llm_clients: &HashMap<String, LlmClient>,
    announcement_type: AnnouncementType,
) -> Result<()> {
    let model_descriptors = create_model_descriptors(config, llm_clients).await;
    
    if model_descriptors.is_empty() {
        warn!("No models available for announcement");
        return Ok(());
    }
    
    let announcement = ModelAnnouncement {
        executor_peer_id: identity.peer_id.to_string(),
        executor_address: identity.evm_address,
        models: model_descriptors.clone(),
        announcement_type,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        nonce: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64, // Use nanoseconds as nonce for uniqueness
        protocol_version: 1,
    };
    
    // Sign the announcement
    let signed_announcement = announcement.sign_blocking(&identity.wallet)?;
    
    // Publish via gossipsub to model announcement topic
    let topic = libp2p::gossipsub::IdentTopic::new("lloom/model-announcements");
    let message_data = serde_json::to_vec(&signed_announcement)
        .map_err(|e| anyhow::anyhow!("Failed to serialize announcement: {}", e))?;
    
    debug!("Serialized model announcement: {} bytes (SignedMessage<ModelAnnouncement>)",
           message_data.len());
    trace!("Signed announcement signer: {}", signed_announcement.signer);
    
    match swarm.behaviour_mut().gossipsub.publish(topic, message_data) {
        Ok(_) => {
            info!(
                "Successfully announced {} models via gossipsub (type: {:?})",
                model_descriptors.len(),
                announcement.announcement_type
            );
        }
        Err(e) => {
            error!("Failed to publish model announcement: {:?}", e);
            return Err(anyhow::anyhow!("Failed to publish announcement: {:?}", e));
        }
    }
    
    Ok(())
}

/// Executor state and runtime data
struct ExecutorState {
    identity: Identity,
    config: ExecutorConfig,
    llm_clients: HashMap<String, LlmClient>,
    usage_records: Vec<UsageRecord>,
    #[allow(dead_code)]
    pending_requests: HashMap<request_response::OutboundRequestId, ResponseChannel<ResponseMessage>>,
    blockchain_client: Option<BlockchainClient>,
    enable_signing: bool,
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
    
    // Display current debug/log level
    let effective_log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| log_level.to_string());
    info!("🔧 Debug/Log Level: {}", effective_log_level);
    
    info!("Starting Lloom Executor with signing {}", if args.enable_signing { "enabled" } else { "disabled" });
    if let Some(config_path) = &args.config {
        info!("Config file: {}", config_path);
    } else if std::path::Path::new("config.toml").exists() {
        info!("Config file: config.toml (auto-detected)");
    } else {
        info!("Config file: none (using defaults)");
    }
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
    let config_file = if let Some(config_path) = &args.config {
        info!("Loading configuration from: {}", config_path);
        config_path.clone()
    } else if std::path::Path::new("config.toml").exists() {
        info!("Automatically loading config from: config.toml");
        "config.toml".to_string()
    } else {
        info!("No config file specified and config.toml not found, using defaults");
        String::new()
    };
    
    let mut config = if !config_file.is_empty() && std::path::Path::new(&config_file).exists() {
        ExecutorConfig::from_file(&config_file)?
    } else {
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
    for backend_config in &mut config.llm_backends {
        match LlmClient::new(backend_config.clone()) {
            Ok(client) => {
                // For LMStudio backends, try to discover available models
                if client.is_lmstudio_backend() {
                    match client.discover_lmstudio_models().await {
                        Ok(discovered_models) => {
                            if !discovered_models.is_empty() {
                                info!("Discovered {} models from LMStudio: {:?}",
                                      discovered_models.len(), discovered_models);
                                // Update the backend config with discovered models if none were specified
                                if backend_config.supported_models.is_empty() ||
                                   backend_config.supported_models == vec!["llama-2-7b-chat", "mistral-7b-instruct", "your-loaded-model"] {
                                    backend_config.supported_models = discovered_models;
                                    info!("Updated {} backend with discovered models", backend_config.name);
                                }
                            } else {
                                warn!("No models discovered from LMStudio backend {}", backend_config.name);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to discover models from LMStudio backend {}: {}", backend_config.name, e);
                        }
                    }
                }
                
                llm_clients.insert(backend_config.name.clone(), client);
                info!("Initialized LLM client for backend: {} with models: {:?}",
                      backend_config.name, backend_config.supported_models);
            }
            Err(e) => {
                warn!("Failed to initialize LLM client for {}: {}", backend_config.name, e);
            }
        }
    }
    
    if llm_clients.is_empty() {
        return Err(anyhow::anyhow!("No LLM clients could be initialized"));
    }

    // If --test flag is provided, perform model health checks
    if args.test {
        info!("Running model health checks...");
        match test_model_health(&llm_clients, &mut config, args.debug).await {
            Ok(healthy_backends) => {
                if healthy_backends.is_empty() {
                    return Err(anyhow::anyhow!(
                        "No models are available after testing. All configured models failed health checks.\n\
                         Please check your model configurations and ensure the services are running."
                    ));
                }
                
                // Update config to only include healthy backends
                config.llm_backends = healthy_backends;
                info!("Updated configuration with {} healthy backend(s)", config.llm_backends.len());
                
                // Update llm_clients to only include healthy ones
                let healthy_names: std::collections::HashSet<String> = config.llm_backends.iter()
                    .map(|b| b.name.clone())
                    .collect();
                llm_clients.retain(|name, _| healthy_names.contains(name));
                
                println!("Model health checks completed. Continuing with healthy models only.\n");
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to perform model health checks: {}", e));
            }
        }
    }
    
    // Create network behaviour
    let behaviour = LloomBehaviour::new(&identity)?;
    
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
        info!("DEBUG: Attempting to bootstrap with {} nodes", config.network.bootstrap_nodes.len());
        for addr_str in &config.network.bootstrap_nodes {
            match addr_str.parse::<Multiaddr>() {
                Ok(addr) => {
                    info!("DEBUG: Attempting to dial bootstrap node: {}", addr);
                    if let Err(e) = swarm.dial(addr.clone()) {
                        warn!("DEBUG: Failed to dial bootstrap node {}: {}", addr, e);
                    } else {
                        info!("DEBUG: Successfully initiated dial to bootstrap node: {}", addr);
                    }
                }
                Err(e) => {
                    warn!("DEBUG: Invalid bootstrap address '{}': {}", addr_str, e);
                }
            }
        }
    }
    
    // Subscribe to gossipsub topics
    helpers::subscribe_topic(&mut swarm, "lloom/announcements")?;
    helpers::subscribe_topic(&mut swarm, "lloom/executor-updates")?;
    helpers::subscribe_topic(&mut swarm, "lloom/executor-announcements")?;
    
    info!("DEBUG: Waiting for network stabilization before registering...");
    
    // Wait a bit for connections to stabilize
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // Register as executor in Kademlia (as provider, not record)
    let executor_key = ServiceRole::Executor.to_kad_key();
    info!("DEBUG: Registering as executor provider with key: {:?} (as string: {})",
          executor_key, String::from_utf8_lossy(&executor_key));
    info!("DEBUG: My PeerID: {}", identity.peer_id);
    info!("DEBUG: Supported models: {:?}", config.get_all_supported_models());
    
    match swarm.behaviour_mut().kademlia.start_providing(executor_key.into()) {
        Ok(_) => info!("DEBUG: ✅ Started providing executor service"),
        Err(e) => error!("DEBUG: ❌ Failed to start providing: {:?}", e),
    }
    
    // Also put a record for backwards compatibility
    let record = Record {
        key: ServiceRole::Executor.to_kad_key().into(),
        value: identity.peer_id.to_bytes(),
        publisher: Some(identity.peer_id),
        expires: None,
    };
    match swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
        Ok(_) => info!("DEBUG: ✅ Put executor record in DHT"),
        Err(e) => error!("DEBUG: ❌ Failed to put record: {:?}", e),
    }
    
    info!("DEBUG: ✅ Executor registration completed");
    
    // Subscribe to model announcement topics for discovery
    helpers::subscribe_topic(&mut swarm, "lloom/model-announcements")?;
    helpers::subscribe_topic(&mut swarm, "lloom/model-queries")?;
    
    // Send initial model announcement to network
    if let Err(e) = announce_models_to_network(
        &mut swarm,
        &identity,
        &config,
        &llm_clients,
        AnnouncementType::Initial,
    ).await {
        error!("Failed to send initial model announcement: {}", e);
    } else {
        info!("✅ Initial model announcement sent successfully");
    }
    
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
        enable_signing: args.enable_signing,
    };
    
    // Set up timers
    let mut announce_interval = interval(Duration::from_secs(config.network.announce_interval_secs));
    let mut batch_interval = interval(Duration::from_secs(config.blockchain.batch_interval_secs));
    
    // Model announcement heartbeat timer (every 30 seconds)
    let mut model_heartbeat_interval = interval(Duration::from_secs(30));
    model_heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    
    // Track previous model state for change detection
    let mut previous_models = create_model_descriptors(&config, &executor_state.llm_clients).await;
    
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
            _ = model_heartbeat_interval.tick() => {
                // Send periodic model heartbeat
                if let Err(e) = announce_models_to_network(
                    &mut swarm,
                    &executor_state.identity,
                    &executor_state.config,
                    &executor_state.llm_clients,
                    AnnouncementType::Heartbeat,
                ).await {
                    error!("Failed to send model heartbeat: {}", e);
                } else {
                    debug!("Sent model heartbeat successfully");
                }
                
                // Check for model updates (particularly for LMStudio dynamic discovery)
                let current_models = create_model_descriptors(&executor_state.config, &executor_state.llm_clients).await;
                if current_models != previous_models {
                    info!("Model configuration changed: {} -> {} models", previous_models.len(), current_models.len());
                    
                    if let Err(e) = announce_models_to_network(
                        &mut swarm,
                        &executor_state.identity,
                        &executor_state.config,
                        &executor_state.llm_clients,
                        AnnouncementType::Update,
                    ).await {
                        error!("Failed to send model update announcement: {}", e);
                    } else {
                        info!("Successfully sent model update announcement");
                    }
                    
                    previous_models = current_models;
                }
            }
            _ = batch_interval.tick() => {
                submit_usage_batch(&mut executor_state).await;
            }
            _ = shutdown_rx.recv() => {
                info!("Received shutdown signal");
                
                // Send removal announcement before shutting down
                if let Err(e) = announce_models_to_network(
                    &mut swarm,
                    &executor_state.identity,
                    &executor_state.config,
                    &executor_state.llm_clients,
                    AnnouncementType::Removal,
                ).await {
                    error!("Failed to send removal announcement: {}", e);
                } else {
                    info!("Sent removal announcement before shutdown");
                }
                
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
    swarm: &mut Swarm<LloomBehaviour>,
    event: SwarmEvent<LloomEvent>,
    state: &mut ExecutorState,
) {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            info!("Listening on {}", address);
        }
        SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
            info!("DEBUG: Connection established with {} at {}", peer_id, endpoint.get_remote_address());
            // Add the connected peer to Kademlia for mutual bootstrap
            swarm.behaviour_mut().kademlia.add_address(&peer_id, endpoint.get_remote_address().clone());
        }
        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
            debug!("Connection closed with {}: {:?}", peer_id, cause);
        }
        SwarmEvent::Behaviour(LloomEvent::RequestResponse(
            request_response::Event::Message { message, peer, .. }
        )) => {
            match message {
                request_response::Message::Request { request, channel, .. } => {
                    handle_request_message(swarm, request, channel, peer, state).await;
                }
                request_response::Message::Response { response, .. } => {
                    debug!("Received unexpected response: {:?}", response);
                }
            }
        }
        SwarmEvent::Behaviour(LloomEvent::Kademlia(kad_event)) => {
            match kad_event {
                kad::Event::OutboundQueryProgressed { result, .. } => {
                    match result {
                        kad::QueryResult::StartProviding(Ok(kad::AddProviderOk { key })) => {
                            info!("DEBUG: Successfully started providing for key: {:?}", key);
                        }
                        kad::QueryResult::StartProviding(Err(e)) => {
                            error!("DEBUG: Failed to start providing: {:?}", e);
                        }
                        kad::QueryResult::PutRecord(Ok(kad::PutRecordOk { key })) => {
                            info!("DEBUG: Successfully put record for key: {:?}", key);
                        }
                        kad::QueryResult::PutRecord(Err(e)) => {
                            error!("DEBUG: Failed to put record: {:?}", e);
                        }
                        _ => {
                            debug!("DEBUG: Other Kademlia query result: {:?}", result);
                        }
                    }
                }
                kad::Event::InboundRequest { request } => {
                    info!("DEBUG: Received inbound Kademlia request: {:?}", request);
                }
                kad::Event::RoutingUpdated { peer, .. } => {
                    info!("DEBUG: Kademlia routing updated with peer: {}", peer);
                }
                _ => {
                    debug!("DEBUG: Other Kademlia event: {:?}", kad_event);
                }
            }
        }
        SwarmEvent::Behaviour(LloomEvent::Gossipsub(libp2p::gossipsub::Event::Message {
            message,
            ..
        })) => {
            debug!("Received gossipsub message on topic {:?}", message.topic);
        }
        _ => {}
    }
}

/// Handle an incoming request message (both signed and unsigned)
async fn handle_request_message(
    swarm: &mut Swarm<LloomBehaviour>,
    request_message: RequestMessage,
    channel: ResponseChannel<ResponseMessage>,
    client_peer: libp2p::PeerId,
    state: &mut ExecutorState,
) {
    match request_message {
        RequestMessage::LlmRequest(request) => {
            info!("Received unsigned LLM request from {}: model={}", client_peer, request.model);
            if state.enable_signing {
                warn!("⚠️  Received unsigned request while signing is enabled from peer: {}", client_peer);
                warn!("Consider enabling signing on client for improved security");
            }
            handle_llm_request(swarm, request, channel, client_peer, state, None).await;
        }
        RequestMessage::SignedLlmRequest(signed_request) => {
            info!("Received signed LLM request from {}: model={}", client_peer, signed_request.payload.model);
            
            let signer_address = if state.enable_signing {
                // Verify the signature with time window for replay protection
                match signed_request.verify_with_time_window(MAX_MESSAGE_AGE_SECS) {
                    Ok(signer_address) => {
                        info!("✓ Request signature verified from signer: {}", signer_address);
                        Some(signer_address)
                    }
                    Err(e) => {
                        error!("✗ Request signature verification failed: {}", e);
                        warn!("Processing request anyway but logging security issue");
                        
                        // Send error response for invalid signature
                        let error_response = LlmResponse {
                            content: String::new(),
                            inbound_tokens: 0,
                            outbound_tokens: 0,
                            total_cost: "0".to_string(),
                            model_used: signed_request.payload.model.clone(),
                            error: Some(format!("Signature verification failed: {}", e)),
                        };
                        
                        let response_message = if state.enable_signing {
                            // Sign the error response
                            match error_response.sign_blocking(&state.identity.wallet) {
                                Ok(signed_response) => {
                                    info!("✓ Signed error response with timestamp: {}", signed_response.timestamp);
                                    ResponseMessage::SignedLlmResponse(signed_response)
                                }
                                Err(sign_err) => {
                                    error!("Failed to sign error response: {}, sending unsigned", sign_err);
                                    ResponseMessage::LlmResponse(error_response)
                                }
                            }
                        } else {
                            ResponseMessage::LlmResponse(error_response)
                        };
                        
                        if let Err(e) = swarm.behaviour_mut().request_response.send_response(channel, response_message) {
                            error!("Failed to send error response: {:?}", e);
                        }
                        return;
                    }
                }
            } else {
                info!("Signature verification disabled, processing signed request without verification");
                None
            };
            
            handle_llm_request(swarm, signed_request.payload, channel, client_peer, state, signer_address).await;
        }
        RequestMessage::ModelAnnouncement(signed_announcement) => {
            // Log model announcements received from other executors
            debug!("Received model announcement from {}: {} models",
                   client_peer, signed_announcement.payload.models.len());
            // For now, just log - could be used for peer discovery in the future
        }
        RequestMessage::ModelQuery(signed_query) => {
            // Log model queries - typically from clients looking for available models
            debug!("Received model query from {}: {:?}", client_peer, signed_query.payload.query_type);
            // For now, just log - could be used to respond with our model availability
        }
        RequestMessage::ModelUpdate(signed_update) => {
            // Log model updates from other executors
            debug!("Received model update from {}: {:?}",
                   client_peer, signed_update.payload.update_type);
            // For now, just log - could be used for dynamic model discovery
        }
    }
}

/// Handle an incoming LLM request
async fn handle_llm_request(
    _swarm: &mut Swarm<LloomBehaviour>,
    request: LlmRequest,
    channel: ResponseChannel<ResponseMessage>,
    _client_peer: libp2p::PeerId,
    state: &mut ExecutorState,
    verified_signer: Option<alloy::primitives::Address>,
) {
    let model = request.model.clone();
    info!("DEBUG: 🎯 Received LLM request for model: '{}'", model);
    info!("DEBUG: Available models: {:?}", state.config.get_all_supported_models());
    
    // Find the appropriate backend for this model
    let backend_name = match state.config.find_backend_for_model(&model) {
        Some(backend) => {
            info!("DEBUG: ✅ Found backend '{}' for model '{}'", backend.name, model);
            backend.name.clone()
        }
        None => {
            error!("DEBUG: ❌ No backend found for model '{}'. Available models: {:?}",
                   model, state.config.get_all_supported_models());
            let error_response = LlmResponse {
                content: String::new(),
                inbound_tokens: 0,
                outbound_tokens: 0,
                total_cost: "0".to_string(),
                model_used: model.clone(),
                error: Some(format!("Model {} not supported", model)),
            };
            
            let response_message = if state.enable_signing {
                match error_response.sign_blocking(&state.identity.wallet) {
                    Ok(signed_response) => {
                        info!("✓ Signed error response with timestamp: {}", signed_response.timestamp);
                        ResponseMessage::SignedLlmResponse(signed_response)
                    }
                    Err(sign_err) => {
                        error!("Failed to sign error response: {}, sending unsigned", sign_err);
                        ResponseMessage::LlmResponse(error_response)
                    }
                }
            } else {
                ResponseMessage::LlmResponse(error_response)
            };
            
            if let Err(e) = _swarm.behaviour_mut().request_response.send_response(channel, response_message) {
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
                inbound_tokens: 0,
                outbound_tokens: 0,
                total_cost: "0".to_string(),
                model_used: model.clone(),
                error: Some(format!("Backend {} not available", backend_name)),
            };
            
            let response_message = if state.enable_signing {
                match error_response.sign_blocking(&state.identity.wallet) {
                    Ok(signed_response) => {
                        info!("✓ Signed error response with timestamp: {}", signed_response.timestamp);
                        ResponseMessage::SignedLlmResponse(signed_response)
                    }
                    Err(sign_err) => {
                        error!("Failed to sign error response: {}, sending unsigned", sign_err);
                        ResponseMessage::LlmResponse(error_response)
                    }
                }
            } else {
                ResponseMessage::LlmResponse(error_response)
            };
            
            if let Err(e) = _swarm.behaviour_mut().request_response.send_response(channel, response_message) {
                error!("Failed to send error response: {:?}", e);
            }
            return;
        }
    };
    
    // Execute the LLM request with LMStudio enhancements if available
    match llm_client.lmstudio_chat_completion(
        &request.model,
        &request.prompt,
        request.system_prompt.as_deref(),
        request.temperature,
        request.max_tokens,
    ).await {
        Ok((content, token_count, stats, model_info)) => {
            let mut log_msg = format!("LLM request completed: {} tokens used", token_count);
            
            // Log LMStudio-specific performance metrics if available
            if let Some(stats) = &stats {
                if let Some(tps) = stats.tokens_per_second {
                    log_msg.push_str(&format!(", {:.2} tokens/sec", tps));
                }
                if let Some(ttft) = stats.time_to_first_token {
                    log_msg.push_str(&format!(", {:.3}s to first token", ttft));
                }
            }
            
            if let Some(model_info) = &model_info {
                if let Some(arch) = &model_info.architecture {
                    log_msg.push_str(&format!(", architecture: {}", arch));
                }
            }
            
            info!("{}", log_msg);
            
            let response = LlmResponse {
                content,
                inbound_tokens: (token_count / 2) as u64,  // Rough estimate - could be improved
                outbound_tokens: (token_count / 2) as u64,
                total_cost: format!("{}", (token_count as u64) * 1000000000000000u64), // 0.001 ETH per token
                model_used: model.clone(),
                error: None,
            };
            
            // Sign response if signing is enabled
            let response_message = if state.enable_signing {
                match response.sign_blocking(&state.identity.wallet) {
                    Ok(signed_response) => {
                        info!("✓ Signed response with timestamp: {}", signed_response.timestamp);
                        ResponseMessage::SignedLlmResponse(signed_response)
                    }
                    Err(sign_err) => {
                        error!("Failed to sign response: {}, sending unsigned", sign_err);
                        ResponseMessage::LlmResponse(response)
                    }
                }
            } else {
                ResponseMessage::LlmResponse(response)
            };
            
            // Send response
            if let Err(e) = _swarm.behaviour_mut().request_response.send_response(channel, response_message) {
                error!("Failed to send response: {:?}", e);
            } else {
                // Record usage for blockchain submission
                // Use verified signer address if available, otherwise use placeholder
                let client_address = verified_signer.unwrap_or(state.identity.evm_address);
                let usage_record = UsageRecord {
                    client_address,
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
                inbound_tokens: 0,
                outbound_tokens: 0,
                total_cost: "0".to_string(),
                model_used: model,
                error: Some(e.to_string()),
            };
            
            let response_message = if state.enable_signing {
                match error_response.sign_blocking(&state.identity.wallet) {
                    Ok(signed_response) => {
                        info!("✓ Signed error response with timestamp: {}", signed_response.timestamp);
                        ResponseMessage::SignedLlmResponse(signed_response)
                    }
                    Err(sign_err) => {
                        error!("Failed to sign error response: {}, sending unsigned", sign_err);
                        ResponseMessage::LlmResponse(error_response)
                    }
                }
            } else {
                ResponseMessage::LlmResponse(error_response)
            };
            
            if let Err(e) = _swarm.behaviour_mut().request_response.send_response(channel, response_message) {
                error!("Failed to send error response: {:?}", e);
            }
        }
    }
}

/// Announce executor availability
async fn announce_executor(
    swarm: &mut Swarm<LloomBehaviour>,
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
    
    // Also announce via gossipsub for better discovery in small networks
    let announcement_msg = format!("EXECUTOR_AVAILABLE:{}", state.identity.peer_id);
    let topic = libp2p::gossipsub::IdentTopic::new("lloom/executor-announcements");
    
    if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic, announcement_msg.as_bytes()) {
        warn!("Failed to publish executor announcement via gossipsub: {:?}", e);
    } else {
        info!("DEBUG: Published executor announcement via gossipsub: {}", state.identity.peer_id);
    }
}

/// Test all configured LLM backends and return only the healthy ones
async fn test_model_health(
    llm_clients: &HashMap<String, LlmClient>,
    config: &ExecutorConfig,
    verbose: bool,
) -> Result<Vec<config::LlmBackendConfig>> {
    use tracing::trace;
    
    let test_prompt = "Please introduce yourself";
    let mut healthy_backends = Vec::new();
    let mut failed_count = 0;

    if config.llm_backends.is_empty() {
        return Err(anyhow::anyhow!("No LLM backends configured in the configuration file"));
    }

    println!("Testing {} LLM backend(s) with prompt: \"{}\"",
             config.llm_backends.len(), test_prompt);
    println!("{}", "-".repeat(60));
    
    for backend_config in &config.llm_backends {
        if let Some(client) = llm_clients.get(&backend_config.name) {
            println!("Testing backend '{}' ({}) with {} model(s)...",
                     backend_config.name,
                     backend_config.endpoint,
                     backend_config.supported_models.len());
            
            let mut backend_healthy = false;
            let mut tested_models = 0;
            let mut successful_models = 0;
            
            // Test each model in the backend
            for model in &backend_config.supported_models {
                print!("  • Testing model '{}' ... ", model);
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
                tested_models += 1;
                
                trace!("═══════════════════════════════════════════════════════════════════════");
                trace!("🧪 Starting health test for model: {}", model);
                trace!("Backend: {} ({})", backend_config.name, backend_config.endpoint);
                trace!("═══════════════════════════════════════════════════════════════════════");
                
                let start_time = std::time::Instant::now();
                match client.lmstudio_chat_completion(
                    model,
                    test_prompt,
                    None, // system_prompt
                    Some(0.1), // low temperature for consistent responses
                    Some(1000), // small max_tokens for quick testing
                ).await {
                    Ok((content, _token_count, _stats, _model_info)) => {
                        let elapsed = start_time.elapsed();
                        trace!("⏱️  Request completed in: {:.3}s", elapsed.as_secs_f64());
                        
                        let response_trimmed = content.trim();
                        if !response_trimmed.is_empty() {
                            println!("✓ PASS");
                            trace!("✅ Model test PASSED for '{}'", model);
                            trace!("📊 Response stats: {} tokens, {:.3}s elapsed", _token_count, elapsed.as_secs_f64());
                            
                            if verbose {
                                let preview = if response_trimmed.len() > 80 {
                                    format!("{}...", &response_trimmed[..80])
                                } else {
                                    response_trimmed.to_string()
                                };
                                println!("    Response: {}", preview);
                            }
                            successful_models += 1;
                            backend_healthy = true;
                        } else {
                            println!("✗ FAIL (empty response)");
                            trace!("❌ Model test FAILED for '{}': empty response", model);
                            failed_count += 1;
                        }
                    }
                    Err(e) => {
                        let elapsed = start_time.elapsed();
                        println!("✗ FAIL");
                        trace!("❌ Model test FAILED for '{}': {} (after {:.3}s)", model, e, elapsed.as_secs_f64());
                        if verbose {
                            println!("    Error: {}", e);
                        }
                        failed_count += 1;
                    }
                }
            }
            
            if backend_healthy {
                println!("  ✅ Backend '{}': {}/{} models working",
                         backend_config.name, successful_models, tested_models);
                healthy_backends.push(backend_config.clone());
            } else {
                println!("  ❌ Backend '{}': 0/{} models working",
                         backend_config.name, tested_models);
            }
        } else {
            println!("⚠️  Backend '{}' not found in initialized clients", backend_config.name);
            failed_count += 1;
        }
        println!(); // Add spacing between backends
    }
    
    println!("{}", "-".repeat(60));
    println!("Model health check results:");
    println!("  ✓ Healthy backends: {}", healthy_backends.len());
    println!("  ✗ Failed tests: {}", failed_count);
    
    if !healthy_backends.is_empty() {
        println!("  Available backends for execution:");
        for backend in &healthy_backends {
            println!("    - {} ({} models)", backend.name, backend.supported_models.len());
            if verbose {
                for model in &backend.supported_models {
                    println!("      • {}", model);
                }
            }
        }
    } else {
        println!("  ⚠️  No healthy backends found!");
    }
    
    Ok(healthy_backends)
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
