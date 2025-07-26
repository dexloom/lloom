//! Lloom Client
//!
//! A CLI tool for interacting with the Lloom P2P network to request LLM services.

use anyhow::{Result, anyhow};
use clap::Parser;
use lloom_core::{
    identity::Identity,
    network::{LloomBehaviour, LloomEvent, helpers},
    protocol::{LlmRequest, LlmResponse, ServiceRole, RequestMessage, ResponseMessage, constants::MAX_MESSAGE_AGE_SECS},
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

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Private key (hex encoded) for identity
    #[arg(long, env = "LLOOM_PRIVATE_KEY")]
    private_key: Option<String>,
    
    /// Bootstrap nodes to connect to (validator nodes)
    #[arg(long, value_delimiter = ',', required = true)]
    bootstrap_nodes: Vec<String>,
    
    /// Model to use for the request
    #[arg(long, default_value = "gpt-3.5-turbo")]
    model: String,
    
    /// Prompt to send to the model
    #[arg(long, required = true)]
    prompt: String,
    
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
}

/// Client state for tracking the request lifecycle
#[derive(Default)]
struct ClientState {
    discovered_executors: HashSet<PeerId>,
    pending_request: Option<(OutboundRequestId, PeerId)>,
    response_received: Option<LlmResponse>,
    discovery_complete: bool,
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
    
    info!("Starting Lloom Client with signing {}", if args.enable_signing { "enabled" } else { "disabled" });
    info!("Model: {}", args.model);
    info!("Prompt: {}", args.prompt);
    
    // Load or generate identity
    let identity = match &args.private_key {
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
    let bootstrap_addrs: Result<Vec<Multiaddr>> = args.bootstrap_nodes
        .iter()
        .map(|addr_str| addr_str.parse().map_err(Into::into))
        .collect();
    let bootstrap_addrs = bootstrap_addrs?;
    
    if bootstrap_addrs.is_empty() {
        return Err(anyhow!("At least one bootstrap node is required"));
    }
    
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
    
    // Run the client with timeout
    let result = timeout(
        Duration::from_secs(args.timeout_secs),
        run_client(&mut swarm, &args, &mut client_state, &identity)
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
                        prompt: args.prompt.clone(),
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
            result: QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders { providers, .. })),
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
            result: QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(record))),
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
            result: QueryResult::GetProviders(Ok(kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. })),
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
        assert_eq!(args.prompt, "Hello world");
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
        assert_eq!(args.prompt, "Test prompt");
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
        // Missing prompt
        let result = Args::try_parse_from(&[
            "client",
            "--bootstrap-nodes", "/ip4/127.0.0.1/tcp/9000"
        ]);
        assert!(result.is_err());
        
        // Missing bootstrap nodes
        let result = Args::try_parse_from(&[
            "client",
            "--prompt", "Hello world"
        ]);
        assert!(result.is_err());
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
            private_key: None,
            bootstrap_nodes: vec!["/ip4/127.0.0.1/tcp/9000".to_string()],
            model: "gpt-3.5-turbo".to_string(),
            prompt: "Hello".to_string(),
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            timeout_secs: 120,
            debug: false,
            enable_signing: true,
        };
        
        let debug_str = format!("{:?}", args);
        assert!(debug_str.contains("Args"));
        assert!(debug_str.contains("gpt-3.5-turbo"));
        assert!(debug_str.contains("Hello"));
    }
}