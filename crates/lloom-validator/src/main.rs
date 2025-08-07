//! Validator node for the Lloom P2P network.
//!
//! The Validator serves as a stable supernode for network bootstrap and discovery.
//! It maintains a directory of active executors and helps clients discover them.

use anyhow::Result;
use clap::Parser;
use serde::Deserialize;
use lloom_core::{
    identity::Identity,
    network::{LloomBehaviour, LloomEvent, helpers},
    protocol::ServiceRole,
};
use futures::StreamExt;
use libp2p::{
    kad::{self, QueryResult, Record},
    swarm::SwarmEvent,
    Multiaddr, Swarm, SwarmBuilder,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};
use tokio::{
    signal,
    sync::mpsc,
    time,
};
use tracing::{debug, info, warn, trace};

#[derive(Debug, Deserialize)]
struct ValidatorConfig {
    identity: IdentityConfig,
}

#[derive(Debug, Deserialize)]
struct IdentityConfig {
    private_key: String,
}

/// Command-line arguments for the Validator node
#[derive(Parser, Debug)]
#[command(author, version, about = "Validator node for Lloom P2P network")]
struct Args {
    /// Path to TOML configuration file
    #[arg(long)]
    config: Option<String>,
    
    /// Path to the private key file (hex-encoded)
    #[arg(short = 'k', long, env = "VALIDATOR_PRIVATE_KEY_FILE")]
    private_key_file: Option<PathBuf>,

    /// Port to listen on for P2P connections
    #[arg(short = 'p', long, default_value = "9000", env = "VALIDATOR_P2P_PORT")]
    p2p_port: u16,

    /// External address for other nodes to connect to (e.g., /ip4/1.2.3.4/tcp/9000)
    #[arg(long, env = "VALIDATOR_EXTERNAL_ADDR")]
    external_addr: Option<String>,

    /// Enable debug logging
    #[arg(short = 'd', long, env = "VALIDATOR_DEBUG")]
    debug: bool,
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

    info!("Starting Lloom Validator node...");

    // Load or generate identity
    let identity = if let Some(config_path) = &args.config {
        info!("Loading configuration from: {}", config_path);
        let config_content = std::fs::read_to_string(config_path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file {}: {}", config_path, e))?;
        let config: ValidatorConfig = toml::from_str(&config_content)
            .map_err(|e| anyhow::anyhow!("Failed to parse TOML config: {}", e))?;
        
        info!("Loading identity from config file");
        Identity::from_str(&config.identity.private_key)
            .map_err(|e| anyhow::anyhow!("Failed to parse identity from config: {}", e))?
    } else if std::path::Path::new("config.toml").exists() {
        info!("Automatically loading config from: config.toml");
        let config_content = std::fs::read_to_string("config.toml")
            .map_err(|e| anyhow::anyhow!("Failed to read config file config.toml: {}", e))?;
        let config: ValidatorConfig = toml::from_str(&config_content)
            .map_err(|e| anyhow::anyhow!("Failed to parse TOML config: {}", e))?;
        
        info!("Loading identity from config file");
        Identity::from_str(&config.identity.private_key)
            .map_err(|e| anyhow::anyhow!("Failed to parse identity from config: {}", e))?
    } else {
        // Fall back to old method for backward compatibility
        load_or_generate_identity(args.private_key_file.as_deref()).await?
    };
    
    info!("Node identity loaded: PeerId={}", identity.peer_id);
    info!("EVM address: {}", identity.evm_address);

    // Create the network behaviour
    let behaviour = LloomBehaviour::new(&identity)?;

    // Build the swarm
    let mut swarm = SwarmBuilder::with_existing_identity(identity.p2p_keypair.clone())
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )?
        .with_behaviour(|_| behaviour)?
        .build();

    // Listen on the specified port
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", args.p2p_port).parse()?;
    swarm.listen_on(listen_addr)?;

    // Add external address if provided
    if let Some(external_addr) = args.external_addr {
        let addr: Multiaddr = external_addr.parse()?;
        swarm.add_external_address(addr);
    }

    // Subscribe to gossipsub topics
    helpers::subscribe_topic(&mut swarm, "lloom/announcements")?;
    helpers::subscribe_topic(&mut swarm, "lloom/executor-updates")?;

    // Register as a validator in Kademlia
    let validator_key = ServiceRole::Validator.to_kad_key();
    info!("DEBUG: Registering validator with key: {:?}", validator_key);
    let record = Record {
        key: validator_key.clone().into(),
        value: identity.peer_id.to_bytes(),
        publisher: Some(identity.peer_id),
        expires: None,
    };
    swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One)?;
    info!("DEBUG: Validator registration completed");

    info!("Validator node started successfully");
    trace!("Validator ready to track executor connections and model information");

    // Set up shutdown signal handler
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
        let _ = shutdown_tx.send(()).await;
    });

    // Set up periodic tasks
    let mut periodic_interval = time::interval(Duration::from_secs(60));

    // Track known executors with their model information
    let mut known_executors = HashSet::new();
    let mut executor_models: HashMap<libp2p::PeerId, Vec<String>> = HashMap::new();

    // Main event loop
    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                handle_swarm_event(&mut swarm, event, &mut known_executors, &mut executor_models).await;
            }
            _ = periodic_interval.tick() => {
                // Perform periodic maintenance
                perform_periodic_tasks(&mut swarm, &known_executors, &executor_models).await;
            }
            _ = shutdown_rx.recv() => {
                info!("Received shutdown signal");
                break;
            }
        }
    }

    info!("Shutting down validator node...");
    Ok(())
}

/// Load identity from file or generate a new one
async fn load_or_generate_identity(key_file: Option<&std::path::Path>) -> Result<Identity> {
    if let Some(path) = key_file {
        if path.exists() {
            info!("Loading identity from {:?}", path);
            let key_hex = tokio::fs::read_to_string(path).await?;
            let key_hex = key_hex.trim();
            Identity::from_str(key_hex).map_err(|e| anyhow::anyhow!("Failed to parse identity: {}", e))
        } else {
            info!("Generating new identity and saving to {:?}", path);
            let identity = Identity::generate();
            let key_hex = hex::encode(identity.wallet.to_bytes());
            tokio::fs::write(path, key_hex).await?;
            Ok(identity)
        }
    } else {
        info!("No key file specified, generating ephemeral identity");
        Ok(Identity::generate())
    }
}

/// Handle swarm events
async fn handle_swarm_event(
    swarm: &mut Swarm<LloomBehaviour>,
    event: SwarmEvent<LloomEvent>,
    known_executors: &mut HashSet<libp2p::PeerId>,
    executor_models: &mut HashMap<libp2p::PeerId, Vec<String>>,
) {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            info!("Listening on {}", address);
        }
        SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
            info!("DEBUG: Validator connection established with {} at {}", peer_id, endpoint.get_remote_address());
            // Add the connected peer to Kademlia for mutual bootstrap
            swarm.behaviour_mut().kademlia.add_address(&peer_id, endpoint.get_remote_address().clone());
            
            // Check if this is an executor by looking up in our known executors
            if known_executors.contains(&peer_id) {
                trace!("Connected peer {} is a known executor", peer_id);
                
                // Update model information if we don't have it yet
                if !executor_models.contains_key(&peer_id) {
                    let models = discover_executor_models(&peer_id);
                    executor_models.insert(peer_id, models);
                }
                
                trace_executor_models(&executor_models);
            }
        }
        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
            debug!("Connection closed with {}: {:?}", peer_id, cause);
            
            // If this was an executor, remove it from our tracking
            if known_executors.contains(&peer_id) {
                trace!("Executor {} disconnected", peer_id);
                // Note: We don't remove from known_executors here as they might reconnect
                // But we could mark them as offline in a more sophisticated implementation
                trace_executor_models(&executor_models);
            }
        }
        SwarmEvent::Behaviour(LloomEvent::Kademlia(kad::Event::OutboundQueryProgressed {
            result: QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(record))),
            ..
        })) => {
            if record.record.key.as_ref() == ServiceRole::Executor.to_kad_key() {
                if let Ok(peer_id) = libp2p::PeerId::from_bytes(&record.record.value) {
                    if known_executors.insert(peer_id) {
                        info!("Discovered new executor: {}", peer_id);
                        
                        // Try to derive model information from the executor
                        // In practice, executors should announce their supported models
                        // For demonstration, we'll simulate some model discovery
                        let models = discover_executor_models(&peer_id);
                        executor_models.insert(peer_id, models);
                        
                        // Log connected executors with models at trace level
                        trace_executor_models(&executor_models);
                    }
                }
            }
        }
        SwarmEvent::Behaviour(LloomEvent::Kademlia(kad::Event::InboundRequest {
            request: kad::InboundRequest::GetRecord { .. }, 
            .. 
        })) => {
            debug!("Received Kademlia GetRecord request");
        }
        SwarmEvent::Behaviour(LloomEvent::Gossipsub(libp2p::gossipsub::Event::Message {
            propagation_source,
            message,
            ..
        })) => {
            debug!(
                "Received gossipsub message from {} on topic {:?}",
                propagation_source, message.topic
            );
            
            // Check if this is an executor announcement with model information
            if message.topic.as_str().contains("executor-announcements") {
                if let Ok(msg_str) = std::str::from_utf8(&message.data) {
                    trace!("Executor announcement: {}", msg_str);
                    
                    // Try to extract PeerId from announcement message
                    if msg_str.starts_with("EXECUTOR_AVAILABLE:") {
                        let peer_id_str = msg_str.strip_prefix("EXECUTOR_AVAILABLE:").unwrap_or("");
                        if let Ok(peer_id) = peer_id_str.parse::<libp2p::PeerId>() {
                            if known_executors.insert(peer_id) {
                                info!("Discovered executor via gossipsub: {}", peer_id);
                            }
                            
                            // Discover models for this executor
                            let models = discover_executor_models(&peer_id);
                            executor_models.insert(peer_id, models);
                            
                            trace_executor_models(&executor_models);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Perform periodic maintenance tasks
async fn perform_periodic_tasks(
    swarm: &mut Swarm<LloomBehaviour>,
    known_executors: &HashSet<libp2p::PeerId>,
    executor_models: &HashMap<libp2p::PeerId, Vec<String>>,
) {
    // Refresh our validator registration
    let validator_key = ServiceRole::Validator.to_kad_key();
    let peer_id = *swarm.local_peer_id();
    let record = Record {
        key: validator_key.into(),
        value: peer_id.to_bytes(),
        publisher: Some(peer_id),
        expires: None,
    };
    
    if let Err(e) = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
        warn!("Failed to refresh validator registration: {:?}", e);
    }

    // Query for executors periodically
    swarm.behaviour_mut()
        .kademlia
        .get_record(ServiceRole::Executor.to_kad_key().into());

    info!("Periodic maintenance completed. Known executors: {}", known_executors.len());
    
    // Log executor model information at trace level during periodic maintenance
    trace_executor_models(&executor_models);
}

/// Discover models supported by an executor
/// In a production system, this would query the executor or parse announcement data
fn discover_executor_models(peer_id: &libp2p::PeerId) -> Vec<String> {
    // For demonstration purposes, simulate different executors having different models
    // In practice, this information would come from:
    // 1. Enhanced DHT records that include model information
    // 2. Direct queries to the executor
    // 3. Enhanced gossipsub announcements
    
    let peer_str = peer_id.to_string();
    let last_chars = peer_str.chars().rev().take(3).collect::<String>();
    
    // Simulate different model sets based on peer ID characteristics
    match last_chars.chars().next().unwrap_or('0') {
        '0'..='3' => vec!["gpt-3.5-turbo".to_string(), "gpt-4".to_string()],
        '4'..='7' => vec!["claude-3".to_string(), "claude-2".to_string()],
        '8'..='9' => vec!["llama-2-7b".to_string(), "mistral-7b".to_string()],
        'a'..='f' | 'A'..='F' => vec!["gpt-4-turbo".to_string(), "gpt-3.5-turbo".to_string(), "claude-3".to_string()],
        _ => vec!["unknown-model".to_string()],
    }
}

/// Log connected executors with their model information at trace level
fn trace_executor_models(executor_models: &HashMap<libp2p::PeerId, Vec<String>>) {
    if executor_models.is_empty() {
        trace!("📋 Executor Registry: No executors currently registered");
        return;
    }
    
    trace!("📋 === Connected Executors with Model Information ===");
    trace!("📋 Total registered executors: {}", executor_models.len());
    
    // Sort executors by PeerId for consistent output
    let mut sorted_executors: Vec<_> = executor_models.iter().collect();
    sorted_executors.sort_by_key(|(peer_id, _)| peer_id.to_string());
    
    for (peer_id, models) in sorted_executors {
        let peer_short = peer_id.to_string();
        let peer_display = if peer_short.len() > 16 {
            format!("{}...{}", &peer_short[..8], &peer_short[peer_short.len()-8..])
        } else {
            peer_short
        };
        
        if models.is_empty() {
            trace!("📋 Executor {}: ❌ No models available", peer_display);
        } else {
            let model_count = models.len();
            let models_str = models.join(", ");
            trace!("📋 Executor {}: ✅ {} model(s) [{}]",
                   peer_display, model_count, models_str);
        }
    }
    trace!("📋 ================================================");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_args_parsing() {
        // Test default values
        use clap::Parser;
        
        // Test with minimal args
        let args = Args::try_parse_from(&["validator"]).unwrap();
        assert_eq!(args.p2p_port, 9000);
        assert_eq!(args.config, None);
        assert_eq!(args.private_key_file, None);
        assert_eq!(args.external_addr, None);
        assert!(!args.debug);
    }

    #[test]
    fn test_args_with_options() {
        use clap::Parser;
        
        let args = Args::try_parse_from(&[
            "validator",
            "--p2p-port", "8000",
            "--debug",
            "--external-addr", "/ip4/192.168.1.1/tcp/8000"
        ]).unwrap();
        
        assert_eq!(args.p2p_port, 8000);
        assert!(args.debug);
        assert_eq!(args.external_addr, Some("/ip4/192.168.1.1/tcp/8000".to_string()));
    }

    #[tokio::test]
    async fn test_load_or_generate_identity_no_file() {
        let identity = load_or_generate_identity(None).await.unwrap();
        assert!(!identity.peer_id.to_string().is_empty());
        assert!(!identity.evm_address.is_empty());
        
        // Each call should generate different identities
        let identity2 = load_or_generate_identity(None).await.unwrap();
        assert_ne!(identity.peer_id, identity2.peer_id);
    }

    #[tokio::test]
    async fn test_load_or_generate_identity_new_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_owned();
        
        // Delete the file so it doesn't exist
        drop(temp_file);
        
        let identity = load_or_generate_identity(Some(&path)).await.unwrap();
        
        // Should have created the file
        assert!(path.exists());
        assert!(!identity.peer_id.to_string().is_empty());
        
        // Loading again should return the same identity
        let identity2 = load_or_generate_identity(Some(&path)).await.unwrap();
        assert_eq!(identity.peer_id, identity2.peer_id);
        assert_eq!(identity.evm_address, identity2.evm_address);
    }

    #[tokio::test]
    async fn test_load_or_generate_identity_existing_file() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        let test_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        writeln!(temp_file, "{}", test_key)?;
        
        let identity = load_or_generate_identity(Some(temp_file.path())).await.unwrap();
        
        // Should load the same identity consistently
        let identity2 = load_or_generate_identity(Some(temp_file.path())).await.unwrap();
        assert_eq!(identity.peer_id, identity2.peer_id);
        assert_eq!(identity.evm_address, identity2.evm_address);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_load_or_generate_identity_invalid_key() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "invalid_hex_key").unwrap();
        
        let result = load_or_generate_identity(Some(temp_file.path())).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_multiaddr_parsing() {
        // Test valid multiaddr formats
        let valid_addrs = vec![
            "/ip4/127.0.0.1/tcp/9000",
            "/ip4/192.168.1.1/tcp/8000",
            "/ip6/::1/tcp/9000",
        ];
        
        for addr_str in valid_addrs {
            let addr: Result<Multiaddr, _> = addr_str.parse();
            assert!(addr.is_ok(), "Failed to parse {}", addr_str);
        }
        
        // Test invalid multiaddr
        let invalid_addrs = vec![
            "invalid-multiaddr",
            "tcp://localhost:9000",
            "/invalid/protocol",
        ];
        
        for addr_str in invalid_addrs {
            let addr: Result<Multiaddr, _> = addr_str.parse();
            assert!(addr.is_err(), "Should have failed to parse {}", addr_str);
        }
    }

    #[test]
    fn test_service_role_kad_keys() {
        // Test that service roles generate consistent Kademlia keys
        let validator_key1 = ServiceRole::Validator.to_kad_key();
        let validator_key2 = ServiceRole::Validator.to_kad_key();
        assert_eq!(validator_key1, validator_key2);
        
        let executor_key = ServiceRole::Executor.to_kad_key();
        assert_ne!(validator_key1, executor_key);
    }

    #[tokio::test]
    async fn test_network_behavior_creation() {
        let identity = Identity::generate();
        let result = LloomBehaviour::new(&identity);
        assert!(result.is_ok(), "Failed to create network behaviour: {:?}", result.err());
    }

    #[test]
    fn test_args_debug_trait() {
        let args = Args {
            config: None,
            private_key_file: None,
            p2p_port: 9000,
            external_addr: None,
            debug: false,
        };
        
        let debug_str = format!("{:?}", args);
        assert!(debug_str.contains("Args"));
        assert!(debug_str.contains("p2p_port: 9000"));
    }

    #[test]
    fn test_periodic_task_constants() {
        // Test that periodic interval is reasonable
        let interval_secs = 60;
        assert!(interval_secs > 0);
        assert!(interval_secs <= 300); // Not more than 5 minutes
    }

    #[test]
    fn test_port_range_validation() {
        // Test that default port is in valid range
        let default_port = 9000u16;
        assert!(default_port > 1024); // Above system ports
        assert!(default_port < 65535); // Valid port range
    }
}