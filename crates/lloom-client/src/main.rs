//! Lloom Client
//!
//! A CLI tool for interacting with the Lloom P2P network to request LLM services.

use anyhow::{Result, anyhow};
use clap::Parser;
use serde::Deserialize;
use lloom_core::{
    identity::Identity,
    network::{LloomBehaviour, LloomEvent, helpers},
    protocol::{
        LlmRequest, LlmResponse, ServiceRole, RequestMessage, ResponseMessage,
        constants::MAX_MESSAGE_AGE_SECS, ModelQuery, ModelQueryResponse, ModelQueryType,
        QueryFilters, QueryResult as ProtocolQueryResult, ModelEntry, ExecutorEntry
    },
    signing::{SignableMessage},
};
use futures::StreamExt;
use libp2p::{
    kad::{self, QueryResult},
    request_response::{self, OutboundRequestId},
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use std::{
    collections::HashSet,
    time::Duration,
};
use tokio::time::{timeout, sleep};
use tracing::{debug, info, warn, error};

#[derive(Debug, Deserialize)]
struct ClientConfig {
    identity: IdentityConfig,
    network: NetworkConfig,
}

#[derive(Debug, Deserialize)]
struct IdentityConfig {
    private_key: String,
}

#[derive(Debug, Deserialize)]
struct NetworkConfig {
    bootstrap_nodes: Vec<String>,
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to TOML configuration file
    #[arg(long)]
    config: Option<String>,
    
    /// Private key (hex encoded) for identity
    #[arg(long, env = "LLOOM_PRIVATE_KEY")]
    private_key: Option<String>,
    
    /// Bootstrap nodes to connect to (validator nodes)
    #[arg(long, value_delimiter = ',')]
    bootstrap_nodes: Vec<String>,
    
    /// Model to use for the request
    #[arg(long, default_value = "gpt-3.5-turbo")]
    model: String,
    
    /// Prompt to send to the model (not required when using discovery flags)
    #[arg(long)]
    prompt: Option<String>,
    
    /// Optional system prompt
    #[arg(long)]
    system_prompt: Option<String>,
    
    /// Temperature for generation (0.0 to 2.0)
    #[arg(long)]
    temperature: Option<f32>,
    
    /// Maximum tokens to generate
    #[arg(long)]
    max_tokens: Option<u32>,
    
    /// Timeout for the entire operation in seconds
    #[arg(long, default_value = "120")]
    timeout_secs: u64,
    
    /// Enable debug logging
    #[arg(short = 'd', long)]
    debug: bool,
    
    /// Enable message signing (default: true)
    #[arg(long, default_value = "true")]
    enable_signing: bool,

    /// Discover and list all available models
    #[arg(long)]
    discover_models: bool,

    /// Query for executors supporting a specific model
    #[arg(long)]
    query_model: Option<String>,

    /// Run a demo query with predefined settings (connects to default validator, uses gpt-oss:20b model)
    #[arg(long)]
    demo: bool,
}

/// Client state for tracking the request lifecycle
#[derive(Default)]
struct ClientState {
    discovered_executors: HashSet<PeerId>,
    pending_request: Option<(OutboundRequestId, PeerId)>,
    response_received: Option<LlmResponse>,
    discovery_complete: bool,
}

/// Model discovery cache for client-side model information
#[derive(Debug, Default)]
struct ModelDiscoveryCache {
    /// Cached model entries from validators
    models: std::collections::HashMap<String, ModelEntry>,
    
    /// Cached executor entries
    executors: std::collections::HashMap<libp2p::PeerId, ExecutorEntry>,
    
    /// Last cache update time
    last_updated: Option<std::time::Instant>,
    
    /// Cache TTL (5 minutes)
    ttl: std::time::Duration,
}

impl ModelDiscoveryCache {
    fn new() -> Self {
        Self {
            models: std::collections::HashMap::new(),
            executors: std::collections::HashMap::new(),
            last_updated: None,
            ttl: std::time::Duration::from_secs(300),
        }
    }
    
    fn is_expired(&self) -> bool {
        match self.last_updated {
            Some(last_update) => last_update.elapsed() > self.ttl,
            None => true,
        }
    }
    
    fn update(&mut self, response: &ModelQueryResponse) {
        match &response.result {
            ProtocolQueryResult::ModelList(models) => {
                for model in models {
                    self.models.insert(model.model_id.clone(), model.clone());
                    for executor_id_str in &model.executors {
                        if let Ok(executor_id) = executor_id_str.parse::<libp2p::PeerId>() {
                            // Create basic executor entry if not exists
                            self.executors.entry(executor_id).or_insert_with(|| ExecutorEntry {
                                peer_id: executor_id_str.clone(),
                                evm_address: Default::default(), // Will be filled later
                                is_connected: true,
                                last_seen: response.timestamp,
                                reliability_score: None,
                            });
                        }
                    }
                }
            }
            ProtocolQueryResult::ExecutorList(executors) => {
                for executor in executors {
                    if let Ok(executor_id) = executor.peer_id.parse::<libp2p::PeerId>() {
                        self.executors.insert(executor_id, executor.clone());
                    }
                }
            }
            _ => {}
        }
        self.last_updated = Some(std::time::Instant::now());
    }
    
    fn clear(&mut self) {
        self.models.clear();
        self.executors.clear();
        self.last_updated = None;
    }
}

/// Discover all available models in the network
async fn discover_models(
    swarm: &mut Swarm<LloomBehaviour>,
    identity: &Identity,
    cache: &mut ModelDiscoveryCache,
) -> Result<Vec<ModelEntry>> {
    info!("Starting model discovery...");
    
    // Return cached results if still valid
    if !cache.is_expired() && !cache.models.is_empty() {
        info!("Using cached model discovery results");
        return Ok(cache.models.values().cloned().collect());
    }

    // Create model query for all models
    let query = ModelQuery {
        query_type: ModelQueryType::ListAllModels,
        filters: Some(QueryFilters {
            backend_type: None,
            min_context_length: None,
            required_features: None,
            max_price: None,
            only_available: true,
            min_success_rate: None,
        }),
        limit: Some(100),
        offset: None,
        query_id: uuid::Uuid::new_v4().to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    // Send query to connected validators and collect responses
    send_model_query(swarm, identity, query, cache).await
}

/// Find executors supporting a specific model
async fn find_executors_for_model(
    swarm: &mut Swarm<LloomBehaviour>,
    identity: &Identity,
    cache: &mut ModelDiscoveryCache,
    model_name: &str,
) -> Result<Vec<ExecutorEntry>> {
    info!("Finding executors for model: {}", model_name);

    // Check cache first
    if !cache.is_expired() {
        if let Some(model_entry) = cache.models.get(model_name) {
            let executors: Vec<ExecutorEntry> = model_entry.executors.iter()
                .filter_map(|executor_id_str| {
                    executor_id_str.parse::<libp2p::PeerId>().ok()
                        .and_then(|id| cache.executors.get(&id).cloned())
                })
                .collect();
            
            if !executors.is_empty() {
                info!("Found {} cached executors for model {}", executors.len(), model_name);
                return Ok(executors);
            }
        }
    }

    // Create query for specific model
    let query = ModelQuery {
        query_type: ModelQueryType::FindModel(model_name.to_string()),
        filters: Some(QueryFilters {
            backend_type: None,
            min_context_length: None,
            required_features: None,
            max_price: None,
            only_available: true,
            min_success_rate: None,
        }),
        limit: Some(50),
        offset: None,
        query_id: uuid::Uuid::new_v4().to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    // Send query and extract executors from response
    let models = send_model_query(swarm, identity, query, cache).await?;
    
    let executors: Vec<ExecutorEntry> = models.into_iter()
        .filter(|m| m.model_id == model_name)
        .flat_map(|m| m.executors)
        .filter_map(|executor_id_str| {
            executor_id_str.parse::<libp2p::PeerId>().ok()
                .and_then(|id| cache.executors.get(&id).cloned())
        })
        .collect();

    info!("Found {} executors for model {}", executors.len(), model_name);
    Ok(executors)
}

/// List all models with their capabilities
async fn list_all_models(
    swarm: &mut Swarm<LloomBehaviour>,
    identity: &Identity,
    cache: &mut ModelDiscoveryCache,
) -> Result<Vec<ModelEntry>> {
    discover_models(swarm, identity, cache).await
}

/// Send a model query to validators and wait for responses
async fn send_model_query(
    swarm: &mut Swarm<LloomBehaviour>,
    identity: &Identity,
    query: ModelQuery,
    cache: &mut ModelDiscoveryCache,
) -> Result<Vec<ModelEntry>> {
    use std::collections::HashSet;
    
    // Clear cache for fresh query
    cache.clear();
    
    // Find connected validators
    let connected_peers: Vec<libp2p::PeerId> = swarm.connected_peers().cloned().collect();
    if connected_peers.is_empty() {
        return Err(anyhow!("No connected peers for model discovery"));
    }

    info!("Sending model query to {} connected peers", connected_peers.len());

    // Sign and prepare the query
    let signed_query = query.sign_blocking(&identity.wallet)
        .map_err(|e| anyhow!("Failed to sign model query: {}", e))?;
    
    let request_message = RequestMessage::ModelQuery(signed_query);
    
    // Send queries to all connected peers (assuming they are validators)
    let mut request_ids = Vec::new();
    for peer_id in &connected_peers {
        match swarm.behaviour_mut().request_response.send_request(peer_id, request_message.clone()) {
            request_id => {
                info!("Sent model query to validator {}", peer_id);
                request_ids.push((request_id, *peer_id));
            }
        }
    }

    if request_ids.is_empty() {
        return Err(anyhow!("Failed to send any model queries"));
    }

    // Wait for responses with timeout
    let query_timeout = Duration::from_secs(5);
    let start_time = std::time::Instant::now();
    let mut responses_received = 0;
    let expected_responses = request_ids.len();

    while start_time.elapsed() < query_timeout && responses_received < expected_responses {
        if let Ok(event) = tokio::time::timeout(Duration::from_millis(100), swarm.select_next_some()).await {
            if let SwarmEvent::Behaviour(LloomEvent::RequestResponse(
                libp2p::request_response::Event::Message {
                    message: libp2p::request_response::Message::Response { response, .. },
                    peer,
                    ..
                }
            )) = event {
                if let ResponseMessage::ModelQueryResponse(signed_response) = response {
                    info!("Received model query response from {}", peer);
                    
                    // Verify signature if signing is enabled
                    match signed_response.verify_with_time_window(MAX_MESSAGE_AGE_SECS) {
                        Ok(_) => {
                            cache.update(&signed_response.payload);
                            responses_received += 1;
                        }
                        Err(e) => {
                            warn!("Model query response signature verification failed: {}", e);
                        }
                    }
                }
            }
        }
    }

    if responses_received == 0 {
        warn!("No model query responses received within timeout");
        return Err(anyhow!("Model discovery timeout - no responses received"));
    }

    info!("Model discovery completed: {} responses received", responses_received);
    Ok(cache.models.values().cloned().collect())
}

/// Handle discovery commands (--discover-models or --query-model)
async fn handle_discovery_commands(
    swarm: &mut Swarm<LloomBehaviour>,
    args: &Args,
    identity: &Identity,
    cache: &mut ModelDiscoveryCache,
) -> Result<()> {
    // Wait for initial connections
    info!("Waiting for connections to stabilize...");
    sleep(Duration::from_secs(3)).await;
    
    // Bootstrap Kademlia DHT
    if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
        warn!("Failed to bootstrap Kademlia: {:?}", e);
    }
    
    // Wait for bootstrap to complete
    sleep(Duration::from_secs(2)).await;

    if args.discover_models {
        info!("Discovering all available models...");
        match discover_models(swarm, identity, cache).await {
            Ok(models) => {
                display_models(&models);
            }
            Err(e) => {
                error!("Model discovery failed: {}", e);
                return Err(e);
            }
        }
    }

    if let Some(ref model_name) = args.query_model {
        info!("Finding executors for model: {}", model_name);
        match find_executors_for_model(swarm, identity, cache, model_name).await {
            Ok(executors) => {
                display_executors_for_model(model_name, &executors);
            }
            Err(e) => {
                error!("Executor discovery failed for model {}: {}", model_name, e);
                return Err(e);
            }
        }
    }

    Ok(())
}

/// Display models in a formatted table
fn display_models(models: &[ModelEntry]) {
    if models.is_empty() {
        println!("No models found in the network");
        return;
    }

    println!("\n📋 Available Models:");
    println!("┌─────────────────────────┬─────────────┬──────────────────┬────────────┐");
    println!("│ Model ID                │ Executors   │ Context Length   │ Price      │");
    println!("├─────────────────────────┼─────────────┼──────────────────┼────────────┤");
    
    for model in models {
        let executor_count = model.executors.len();
        let context_length = model.capabilities.max_context_length;
        let price = model.avg_pricing.as_ref()
            .map(|p| format!("{} wei", &p.input_token_price))
            .unwrap_or_else(|| "N/A".to_string());
        
        println!("│ {:23} │ {:11} │ {:16} │ {:10} │",
                 truncate_string(&model.model_id, 23),
                 executor_count,
                 if context_length > 0 { context_length.to_string() } else { "N/A".to_string() },
                 truncate_string(&price, 10));
    }
    
    println!("└─────────────────────────┴─────────────┴──────────────────┴────────────┘");
    println!("Total: {} models found", models.len());
}

/// Display executors for a specific model
fn display_executors_for_model(model_name: &str, executors: &[ExecutorEntry]) {
    if executors.is_empty() {
        println!("No executors found for model: {}", model_name);
        return;
    }

    println!("\n🔧 Executors for model '{}':", model_name);
    println!("┌─────────────────────────────────────────────────┬─────────────┬──────────────┐");
    println!("│ Peer ID                                         │ Connected   │ Reliability  │");
    println!("├─────────────────────────────────────────────────┼─────────────┼──────────────┤");
    
    for executor in executors {
        let connected = if executor.is_connected { "✓" } else { "✗" };
        let reliability = executor.reliability_score
            .map(|score| format!("{:.1}%", score * 100.0))
            .unwrap_or_else(|| "N/A".to_string());
        
        println!("│ {:47} │ {:11} │ {:12} │",
                 truncate_string(&executor.peer_id, 47),
                 connected,
                 reliability);
    }
    
    println!("└─────────────────────────────────────────────────┴─────────────┴──────────────┘");
    println!("Total: {} executors found", executors.len());
}

/// Helper function to truncate strings for display
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
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
                .add_directive("libp2p=warn".parse()?),
        )
        .init();
    
    // Load configuration from file if provided, or check for default config.toml
    let (final_private_key, final_bootstrap_nodes) = if let Some(config_path) = &args.config {
        info!("Loading configuration from: {}", config_path);
        let config_content = std::fs::read_to_string(config_path)
            .map_err(|e| anyhow!("Failed to read config file {}: {}", config_path, e))?;
        let config: ClientConfig = toml::from_str(&config_content)
            .map_err(|e| anyhow!("Failed to parse TOML config: {}", e))?;
        
        // Command-line arguments override config file values
        let private_key = args.private_key.clone().unwrap_or(config.identity.private_key);
        let bootstrap_nodes = if args.bootstrap_nodes.is_empty() {
            config.network.bootstrap_nodes
        } else {
            args.bootstrap_nodes.clone()
        };
        
        (Some(private_key), bootstrap_nodes)
    } else if std::path::Path::new("config.toml").exists() {
        info!("Automatically loading config from: config.toml");
        let config_content = std::fs::read_to_string("config.toml")
            .map_err(|e| anyhow!("Failed to read config file config.toml: {}", e))?;
        let config: ClientConfig = toml::from_str(&config_content)
            .map_err(|e| anyhow!("Failed to parse TOML config: {}", e))?;
        
        // Command-line arguments override config file values
        let private_key = args.private_key.clone().unwrap_or(config.identity.private_key);
        let bootstrap_nodes = if args.bootstrap_nodes.is_empty() {
            config.network.bootstrap_nodes
        } else {
            args.bootstrap_nodes.clone()
        };
        
        (Some(private_key), bootstrap_nodes)
    } else {
        (args.private_key.clone(), args.bootstrap_nodes.clone())
    };
    
    // Handle demo mode - override settings with demo defaults
    let (final_bootstrap_nodes, final_model, final_prompt) = if args.demo {
        println!("🚀 Running Lloom Demo!");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📡 Connecting to default validator: /ip4/67.220.95.247/tcp/3099");
        println!("🤖 Using model: gpt-oss:20b");
        println!("💬 Sending prompt: Please introduce yourself!");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        let demo_bootstrap = vec!["/ip4/67.220.95.247/tcp/3099".to_string()];
        let demo_model = "gpt-oss:20b".to_string();
        let demo_prompt = Some("Please introduce yourself!".to_string());
        
        (demo_bootstrap, demo_model, demo_prompt)
    } else {
        (final_bootstrap_nodes, args.model.clone(), args.prompt.clone())
    };
    
    // Validate bootstrap nodes are provided
    if final_bootstrap_nodes.is_empty() {
        return Err(anyhow!("At least one bootstrap node is required (via --bootstrap-nodes or config file)"));
    }
    
    info!("Starting Lloom Client with signing {}", if args.enable_signing { "enabled" } else { "disabled" });
    info!("Model: {}", final_model);
    if let Some(ref prompt) = final_prompt {
        info!("Prompt: {}", prompt);
    }
    
    // Load or generate identity
    let identity = match &final_private_key {
        Some(key) => {
            info!("Loading identity from private key");
            Identity::from_str(key)?
        }
        None => {
            info!("Generating ephemeral identity");
            Identity::generate()
        }
    };
    
    info!("Client identity: PeerId={}", identity.peer_id);
    info!("EVM address: {}", identity.evm_address);
    
    // Parse bootstrap nodes
    let bootstrap_addrs: Result<Vec<Multiaddr>> = final_bootstrap_nodes
        .iter()
        .map(|addr_str| addr_str.parse().map_err(Into::into))
        .collect();
    let bootstrap_addrs = bootstrap_addrs?;
    
    info!("Bootstrap nodes: {:?}", bootstrap_addrs);
    
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
    
    // Connect to bootstrap nodes
    for addr in &bootstrap_addrs {
        if let Err(e) = swarm.dial(addr.clone()) {
            warn!("DEBUG: Failed to dial bootstrap node {}: {}", addr, e);
        } else {
            info!("DEBUG: Successfully initiated dial to bootstrap node: {}", addr);
        }
    }
    
    // Subscribe to gossipsub topics
    helpers::subscribe_topic(&mut swarm, "lloom/announcements")?;
    helpers::subscribe_topic(&mut swarm, "lloom/executor-announcements")?;
    
    let mut client_state = ClientState::default();
    let mut discovery_cache = ModelDiscoveryCache::new();
    
    // Handle model discovery commands first (but not in demo mode)
    if !args.demo && (args.discover_models || args.query_model.is_some()) {
        let discovery_result = timeout(
            Duration::from_secs(args.timeout_secs),
            handle_discovery_commands(&mut swarm, &args, &identity, &mut discovery_cache)
        ).await;
        
        match discovery_result {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(e)) => {
                error!("Discovery command failed: {}", e);
                std::process::exit(1);
            }
            Err(_) => {
                error!("Discovery command timed out after {} seconds", args.timeout_secs);
                std::process::exit(1);
            }
        }
    }

    // Require prompt for normal operation (demo provides its own prompt)
    if final_prompt.is_none() && !args.discover_models && args.query_model.is_none() && !args.demo {
        return Err(anyhow!("Prompt is required when not using discovery commands (--discover-models, --query-model, or --demo)"));
    }
    
    // Create a modified args struct for demo mode
    let mut runtime_args = args.clone();
    if args.demo {
        runtime_args.model = final_model;
        runtime_args.prompt = final_prompt;
    }
    
    // Run the client with timeout
    let result = timeout(
        Duration::from_secs(args.timeout_secs),
        run_client(&mut swarm, &runtime_args, &mut client_state, &identity)
    ).await;
    
    match result {
        Ok(Ok(response)) => {
            if let Some(error) = &response.error {
                error!("Request failed: {}", error);
                std::process::exit(1);
            } else {
                println!("Model: {}", response.model_used);
                println!("Inbound Tokens: {}", response.inbound_tokens);
                println!("Outbound Tokens: {}", response.outbound_tokens);
                println!("Total Cost: {}", response.total_cost);
                println!("---");
                println!("{}", response.content);
            }
        }
        Ok(Err(e)) => {
            error!("Client error: {}", e);
            std::process::exit(1);
        }
        Err(_) => {
            error!("Request timed out after {} seconds", args.timeout_secs);
            std::process::exit(1);
        }
    }
    
    Ok(())
}

/// Main client logic
async fn run_client(
    swarm: &mut Swarm<LloomBehaviour>,
    args: &Args,
    state: &mut ClientState,
    identity: &Identity,
) -> Result<LlmResponse> {
    info!("Phase 1: Discovering executors...");
    
    // Wait longer for initial connections and DHT to stabilize
    info!("DEBUG: Waiting for DHT to stabilize...");
    sleep(Duration::from_secs(5)).await;
    
    // Bootstrap Kademlia DHT first
    info!("DEBUG: Bootstrapping Kademlia DHT...");
    if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
        warn!("DEBUG: Failed to bootstrap Kademlia: {:?}", e);
    }
    
    // Wait for bootstrap to complete
    sleep(Duration::from_secs(3)).await;
    
    // Query for executors multiple times with delays - more attempts with longer intervals
    let executor_key = ServiceRole::Executor.to_kad_key();
    info!("DEBUG: Looking for executors using key: {:?} (as string: {})", executor_key, String::from_utf8_lossy(&executor_key));
    
    for attempt in 1..=8 {
        info!("DEBUG: Attempt {}/8: Starting provider query for executor key", attempt);
        swarm.behaviour_mut().kademlia.get_providers(executor_key.clone().into());
        
        // Also try to get records for backwards compatibility
        info!("DEBUG: Attempt {}/8: Starting record query for executor key", attempt);
        swarm.behaviour_mut().kademlia.get_record(executor_key.clone().into());
        
        // Process events immediately after each query
        for _ in 0..10 {
            if let Ok(event) = tokio::time::timeout(Duration::from_millis(100), swarm.select_next_some()).await {
                handle_swarm_event(swarm, event, state, args, identity).await;
                if !state.discovered_executors.is_empty() {
                    info!("DEBUG: Early discovery success - found {} executors", state.discovered_executors.len());
                    break;
                }
            }
        }
        
        if !state.discovered_executors.is_empty() {
            break;
        }
        
        if attempt < 8 {
            info!("DEBUG: Waiting 2 seconds before next attempt...");
            sleep(Duration::from_secs(2)).await;
        }
    }
    
    info!("DEBUG: Completed all discovery attempts, found {} executors so far",
          state.discovered_executors.len());
    
    let mut discovery_timeout = tokio::time::interval(Duration::from_secs(60));
    discovery_timeout.tick().await; // Skip first immediate tick
    
    // Discovery phase
    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                handle_swarm_event(swarm, event, state, args, identity).await;
                
                // Check if we found executors and can proceed
                if !state.discovered_executors.is_empty() && !state.discovery_complete {
                    info!("Phase 2: Found {} executors, selecting one...", state.discovered_executors.len());
                    
                    // Select first available executor (could be improved with latency testing)
                    let selected_executor = *state.discovered_executors.iter().next().unwrap();
                    
                    // Prepare LLM request
                    let request = LlmRequest {
                        model: args.model.clone(),
                        prompt: args.prompt.as_ref().unwrap().clone(),
                        system_prompt: args.system_prompt.clone(),
                        temperature: args.temperature,
                        max_tokens: args.max_tokens,
                        executor_address: selected_executor.to_string(),
                        inbound_price: "500000000000000".to_string(), // 0.0005 ETH per token
                        outbound_price: "1000000000000000".to_string(), // 0.001 ETH per token
                        nonce: 1,
                        deadline: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() + 300, // 5 minutes from now
                    };
                    
                    info!("Phase 3: Sending request to executor: {}", selected_executor);
                    
                    // Send the request (with or without signing based on configuration)
                    let request_message = if args.enable_signing {
                        // Sign the request before sending
                        match request.sign_blocking(&identity.wallet) {
                            Ok(signed_request) => {
                                info!("Successfully signed request with timestamp: {}", signed_request.timestamp);
                                RequestMessage::SignedLlmRequest(signed_request)
                            }
                            Err(e) => {
                                error!("Failed to sign request: {}, falling back to unsigned", e);
                                RequestMessage::LlmRequest(request)
                            }
                        }
                    } else {
                        info!("Signing disabled, sending unsigned request");
                        RequestMessage::LlmRequest(request)
                    };
                    
                    let request_id = swarm.behaviour_mut().request_response.send_request(&selected_executor, request_message);
                    state.pending_request = Some((request_id, selected_executor));
                    state.discovery_complete = true;
                }
                
                // Check if we received a response
                if let Some(response) = &state.response_received {
                    return Ok(response.clone());
                }
            }
            _ = discovery_timeout.tick() => {
                if state.discovered_executors.is_empty() {
                    error!("DEBUG: ❌ DISCOVERY TIMEOUT - No executors found after 60 seconds");
                    error!("DEBUG: This suggests either:");
                    error!("DEBUG: 1. No executors are running or registered");
                    error!("DEBUG: 2. Network connectivity issues prevent discovery");
                    error!("DEBUG: 3. Executors are not advertising the '{}' model", args.model);
                    return Err(anyhow!("No executors found after discovery timeout"));
                } else {
                    info!("DEBUG: Discovery timeout reached but we have {} executors", state.discovered_executors.len());
                }
            }
        }
    }
}

/// Handle swarm events
async fn handle_swarm_event(
    _swarm: &mut Swarm<LloomBehaviour>,
    event: SwarmEvent<LloomEvent>,
    state: &mut ClientState,
    args: &Args,
    _identity: &Identity,
) {
    match event {
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            info!("DEBUG: Connected to peer: {}", peer_id);
        }
        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
            debug!("Connection closed with {}: {:?}", peer_id, cause);
        }
        SwarmEvent::Behaviour(LloomEvent::Kademlia(kad::Event::OutboundQueryProgressed {
            result: kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders { providers, .. })),
            ..
        })) => {
            info!("DEBUG: ✅ Found {} executor providers via Kademlia", providers.len());
            for provider in providers {
                if state.discovered_executors.insert(provider) {
                    info!("DEBUG: ✅ NEW executor discovered: {}", provider);
                } else {
                    info!("DEBUG: ⚠️  Already known executor: {}", provider);
                }
            }
            info!("DEBUG: Total known executors now: {}", state.discovered_executors.len());
        }
        SwarmEvent::Behaviour(LloomEvent::Kademlia(kad::Event::OutboundQueryProgressed {
            result: kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(record))),
            ..
        })) => {
            if record.record.key.as_ref() == ServiceRole::Executor.to_kad_key() {
                if let Ok(peer_id) = libp2p::PeerId::from_bytes(&record.record.value) {
                    if state.discovered_executors.insert(peer_id) {
                        info!("DEBUG: Discovered executor from record: {}", peer_id);
                    }
                }
            }
        }
        SwarmEvent::Behaviour(LloomEvent::Kademlia(kad::Event::OutboundQueryProgressed {
            result: kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. })),
            ..
        })) => {
            debug!("Kademlia provider query finished");
        }
        SwarmEvent::Behaviour(LloomEvent::RequestResponse(
            request_response::Event::Message {
                message: request_response::Message::Response { response, request_id },
                peer,
                connection_id: _,
            }
        )) => {
            if let Some((pending_id, expected_peer)) = &state.pending_request {
                if request_id == *pending_id && peer == *expected_peer {
                    // Handle both signed and unsigned responses
                    let llm_response = match &response {
                        ResponseMessage::LlmResponse(resp) => {
                            info!("Received unsigned response from {}: {} inbound + {} outbound tokens",
                                  peer, resp.inbound_tokens, resp.outbound_tokens);
                            Some(resp.clone())
                        }
                        ResponseMessage::SignedLlmResponse(signed_resp) => {
                            info!("Received signed response from {}", peer);
                            
                            // Verify the signature if signing is enabled
                            if args.enable_signing {
                                match signed_resp.verify_with_time_window(MAX_MESSAGE_AGE_SECS) {
                                    Ok(signer_address) => {
                                        info!("✓ Response signature verified from signer: {}", signer_address);
                                        info!("Response content: {} inbound + {} outbound tokens",
                                              signed_resp.payload.inbound_tokens, signed_resp.payload.outbound_tokens);
                                        Some(signed_resp.payload.clone())
                                    }
                                    Err(e) => {
                                        error!("✗ Response signature verification failed: {}", e);
                                        warn!("Response may be tampered with or from untrusted source");
                                        // Still process the response but log the security issue
                                        info!("Processing unverified response: {} inbound + {} outbound tokens",
                                              signed_resp.payload.inbound_tokens, signed_resp.payload.outbound_tokens);
                                        Some(signed_resp.payload.clone())
                                    }
                                }
                            } else {
                                info!("Signature verification disabled, processing response: {} inbound + {} outbound tokens",
                                      signed_resp.payload.inbound_tokens, signed_resp.payload.outbound_tokens);
                                Some(signed_resp.payload.clone())
                            }
                        }
                        ResponseMessage::ModelQueryResponse(_) => {
                            debug!("Received model query response from {}", peer);
                            None // Not handled by client
                        }
                        ResponseMessage::AcknowledgmentResponse(_) => {
                            debug!("Received acknowledgment response from {}", peer);
                            None // Not handled by client
                        }
                    };
                    
                    if let Some(resp) = llm_response {
                        state.response_received = Some(resp);
                    }
                }
            }
        }
        SwarmEvent::Behaviour(LloomEvent::RequestResponse(
            request_response::Event::OutboundFailure { request_id, error, peer, .. }
        )) => {
            if let Some((pending_id, expected_peer)) = &state.pending_request {
                if request_id == *pending_id && peer == *expected_peer {
                    error!("Request failed: {:?}", error);
                    // Try next executor if available
                    state.discovered_executors.remove(expected_peer);
                    state.pending_request = None;
                    state.discovery_complete = false; // Allow retry with different executor
                }
            }
        }
        SwarmEvent::Behaviour(LloomEvent::Gossipsub(libp2p::gossipsub::Event::Message { message, .. })) => {
            debug!("Received gossipsub message on topic {:?}", message.topic);
            
            // Handle executor announcements
            if message.topic.as_str() == "lloom/executor-announcements" {
                if let Ok(msg_str) = std::str::from_utf8(&message.data) {
                    if let Some(peer_id_str) = msg_str.strip_prefix("EXECUTOR_AVAILABLE:") {
                        if let Ok(peer_id) = peer_id_str.parse::<PeerId>() {
                            if state.discovered_executors.insert(peer_id) {
                                info!("DEBUG: Discovered executor via gossipsub: {}", peer_id);
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use lloom_core::protocol::{LlmRequest, LlmResponse};
    use libp2p::PeerId;

    #[test]
    fn test_args_parsing() {
        // Test minimal required args
        let args = Args::try_parse_from(&[
            "client",
            "--bootstrap-nodes", "/ip4/127.0.0.1/tcp/9000",
            "--prompt", "Hello world"
        ]).unwrap();
        
        assert_eq!(args.bootstrap_nodes, vec!["/ip4/127.0.0.1/tcp/9000"]);
        assert_eq!(args.prompt, Some("Hello world".to_string()));
        assert_eq!(args.model, "gpt-3.5-turbo"); // default
        assert_eq!(args.timeout_secs, 120); // default
        assert_eq!(args.private_key, None);
        assert_eq!(args.system_prompt, None);
        assert_eq!(args.temperature, None);
        assert_eq!(args.max_tokens, None);
        assert!(!args.debug);
    }

    #[test]
    fn test_args_parsing_full() {
        let args = Args::try_parse_from(&[
            "client",
            "--bootstrap-nodes", "/ip4/127.0.0.1/tcp/9000,/ip4/192.168.1.1/tcp/8000",
            "--prompt", "Test prompt",
            "--model", "gpt-4",
            "--system-prompt", "You are helpful",
            "--temperature", "0.7",
            "--max-tokens", "100",
            "--timeout-secs", "60",
            "--private-key", "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            "--debug"
        ]).unwrap();
        
        assert_eq!(args.bootstrap_nodes.len(), 2);
        assert_eq!(args.bootstrap_nodes[0], "/ip4/127.0.0.1/tcp/9000");
        assert_eq!(args.bootstrap_nodes[1], "/ip4/192.168.1.1/tcp/8000");
        assert_eq!(args.prompt, Some("Test prompt".to_string()));
        assert_eq!(args.model, "gpt-4");
        assert_eq!(args.system_prompt, Some("You are helpful".to_string()));
        assert_eq!(args.temperature, Some(0.7));
        assert_eq!(args.max_tokens, Some(100));
        assert_eq!(args.timeout_secs, 60);
        assert!(args.private_key.is_some());
        assert!(args.debug);
    }

    #[test]
    fn test_args_missing_required() {
        // Missing prompt (should succeed because prompt is optional at parse time)
        let result = Args::try_parse_from(&[
            "client",
            "--bootstrap-nodes", "/ip4/127.0.0.1/tcp/9000"
        ]);
        assert!(result.is_ok());
        
        // Missing bootstrap nodes (should succeed because bootstrap nodes can come from config)
        let result = Args::try_parse_from(&[
            "client",
            "--prompt", "Hello world"
        ]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_client_state_default() {
        let state = ClientState::default();
        assert!(state.discovered_executors.is_empty());
        assert_eq!(state.pending_request, None);
        assert_eq!(state.response_received, None);
        assert!(!state.discovery_complete);
    }

    #[test]
    fn test_client_state_operations() {
        let mut state = ClientState::default();
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();
        
        // Add executors
        state.discovered_executors.insert(peer1);
        state.discovered_executors.insert(peer2);
        assert_eq!(state.discovered_executors.len(), 2);
        assert!(state.discovered_executors.contains(&peer1));
        assert!(state.discovered_executors.contains(&peer2));
        
        // Note: OutboundRequestId doesn't have a public constructor, so we skip that part of the test
        // In real usage, it's returned by send_request()
        
        // Set response
        let response = LlmResponse {
            content: "Test response".to_string(),
            inbound_tokens: 5,
            outbound_tokens: 5,
            total_cost: "10000000000000000".to_string(),
            model_used: "gpt-3.5-turbo".to_string(),
            error: None,
        };
        state.response_received = Some(response.clone());
        assert_eq!(state.response_received, Some(response));
        
        // Mark discovery complete
        state.discovery_complete = true;
        assert!(state.discovery_complete);
    }

    #[test]
    fn test_identity_generation_with_key() {
        let test_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let identity = Identity::from_str(test_key).unwrap();
        
        // Should generate consistent identity from same key
        let identity2 = Identity::from_str(test_key).unwrap();
        assert_eq!(identity.peer_id, identity2.peer_id);
        assert_eq!(identity.evm_address, identity2.evm_address);
    }

    #[test]
    fn test_identity_generation_random() {
        let identity1 = Identity::generate();
        let identity2 = Identity::generate();
        
        // Should generate different identities
        assert_ne!(identity1.peer_id, identity2.peer_id);
        assert_ne!(identity1.evm_address, identity2.evm_address);
        assert!(!identity1.peer_id.to_string().is_empty());
        assert!(!identity1.evm_address.is_empty());
    }

    #[test]
    fn test_bootstrap_addr_parsing() {
        let valid_addrs = vec![
            "/ip4/127.0.0.1/tcp/9000".to_string(),
            "/ip4/192.168.1.1/tcp/8000".to_string(),
            "/ip6/::1/tcp/9000".to_string(),
        ];
        
        let result: Result<Vec<libp2p::Multiaddr>, _> = valid_addrs
            .iter()
            .map(|addr_str| addr_str.parse())
            .collect();
        
        assert!(result.is_ok());
        let addrs = result.unwrap();
        assert_eq!(addrs.len(), 3);
    }

    #[test]
    fn test_bootstrap_addr_parsing_invalid() {
        let invalid_addrs = vec![
            "invalid-addr".to_string(),
            "http://localhost:9000".to_string(),
        ];
        
        let result: Result<Vec<libp2p::Multiaddr>, _> = invalid_addrs
            .iter()
            .map(|addr_str| addr_str.parse())
            .collect();
        
        assert!(result.is_err());
    }

    #[test]
    fn test_llm_request_creation() {
        let request = LlmRequest {
            model: "gpt-4".to_string(),
            prompt: "Test prompt".to_string(),
            system_prompt: Some("System message".to_string()),
            temperature: Some(0.8),
            max_tokens: Some(150),
            executor_address: "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string(),
            inbound_price: "500000000000000".to_string(),
            outbound_price: "1000000000000000".to_string(),
            nonce: 1,
            deadline: 1234567890,
        };
        
        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.prompt, "Test prompt");
        assert_eq!(request.system_prompt, Some("System message".to_string()));
        assert_eq!(request.temperature, Some(0.8));
        assert_eq!(request.max_tokens, Some(150));
    }

    #[test]
    fn test_llm_response_creation() {
        let response = LlmResponse {
            content: "Generated text".to_string(),
            inbound_tokens: 10,
            outbound_tokens: 15,
            total_cost: "25000000000000000".to_string(),
            model_used: "gpt-3.5-turbo".to_string(),
            error: None,
        };
        
        assert_eq!(response.content, "Generated text");
        assert_eq!(response.model_used, "gpt-3.5-turbo");
        assert_eq!(response.inbound_tokens, 10);
        assert_eq!(response.outbound_tokens, 15);
        assert_eq!(response.error, None);
        
        // Test with error
        let error_response = LlmResponse {
            content: String::new(),
            inbound_tokens: 0,
            outbound_tokens: 0,
            total_cost: "0".to_string(),
            model_used: "gpt-3.5-turbo".to_string(),
            error: Some("API error".to_string()),
        };
        
        assert!(error_response.error.is_some());
        assert_eq!(error_response.error.unwrap(), "API error");
    }

    #[test]
    fn test_timeout_values() {
        // Test reasonable timeout values
        let timeouts = [30, 60, 120, 300];
        for timeout in timeouts {
            let duration = Duration::from_secs(timeout);
            assert!(duration.as_secs() > 0);
            assert!(duration.as_secs() <= 300); // Max 5 minutes seems reasonable
        }
    }

    #[test]
    fn test_network_behavior_creation() {
        let identity = Identity::generate();
        let result = LloomBehaviour::new(&identity);
        assert!(result.is_ok(), "Failed to create network behaviour: {:?}", result.err());
    }

    #[test]
    fn test_service_role_executor_key() {
        let key1 = ServiceRole::Executor.to_kad_key();
        let key2 = ServiceRole::Executor.to_kad_key();
        assert_eq!(key1, key2); // Should be deterministic
        
        let validator_key = ServiceRole::Validator.to_kad_key();
        assert_ne!(key1, validator_key); // Different roles should have different keys
    }

    #[test]
    fn test_args_debug_trait() {
        let args = Args {
            config: None,
            private_key: None,
            bootstrap_nodes: vec!["/ip4/127.0.0.1/tcp/9000".to_string()],
            model: "gpt-3.5-turbo".to_string(),
            prompt: Some("Hello".to_string()),
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            timeout_secs: 120,
            debug: false,
            enable_signing: true,
            discover_models: false,
            query_model: None,
            demo: false,
        };
        
        let debug_str = format!("{:?}", args);
        assert!(debug_str.contains("Args"));
        assert!(debug_str.contains("gpt-3.5-turbo"));
        assert!(debug_str.contains("Hello"));
    }
    
    #[test]
    fn test_demo_flag() {
        let args = Args::try_parse_from(&[
            "client",
            "--demo"
        ]).unwrap();
        
        assert!(args.demo);
        assert!(!args.discover_models);
        assert_eq!(args.query_model, None);
        assert_eq!(args.model, "gpt-3.5-turbo"); // Default before demo override
    }
    
    #[test]
    fn test_demo_with_other_args() {
        let args = Args::try_parse_from(&[
            "client",
            "--demo",
            "--debug",
            "--timeout-secs", "60"
        ]).unwrap();
        
        assert!(args.demo);
        assert!(args.debug);
        assert_eq!(args.timeout_secs, 60);
    }
}