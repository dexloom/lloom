//! Crowd Models Client
//! 
//! A CLI tool for interacting with the Crowd Models P2P network to request LLM services.

use anyhow::{Result, anyhow};
use clap::Parser;
use crowd_models_core::{
    identity::Identity,
    network::{LlmP2pBehaviour, LlmP2pEvent, helpers},
    protocol::{LlmRequest, LlmResponse, ServiceRole},
};
use futures::StreamExt;
use libp2p::{
    kad::{self, QueryResult},
    request_response::{self, RequestId},
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use tokio::time::{timeout, sleep};
use tracing::{debug, info, warn, error};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Private key (hex encoded) for identity
    #[arg(long, env = "CROWD_MODELS_PRIVATE_KEY")]
    private_key: Option<String>,
    
    /// Bootstrap nodes to connect to (accountant nodes)
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
}

/// Client state for tracking the request lifecycle
#[derive(Default)]
struct ClientState {
    discovered_executors: HashSet<PeerId>,
    pending_request: Option<(RequestId, PeerId)>,
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
    
    info!("Starting Crowd Models Client...");
    info!("Model: {}", args.model);
    info!("Prompt: {}", args.prompt);
    
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
    
    // Connect to bootstrap nodes
    for addr in &bootstrap_addrs {
        if let Err(e) = swarm.dial(addr.clone()) {
            warn!("Failed to dial bootstrap node {}: {}", addr, e);
        } else {
            info!("Dialing bootstrap node: {}", addr);
        }
    }
    
    // Subscribe to gossipsub topics
    helpers::subscribe_topic(&mut swarm, "crowd-models/announcements")?;
    
    let mut client_state = ClientState::default();
    
    // Run the client with timeout
    let result = timeout(
        Duration::from_secs(args.timeout_secs),
        run_client(&mut swarm, &args, &mut client_state)
    ).await;
    
    match result {
        Ok(Ok(response)) => {
            if let Some(error) = &response.error {
                error!("Request failed: {}", error);
                std::process::exit(1);
            } else {
                println!("Model: {}", response.model_used);
                println!("Tokens: {}", response.token_count);
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
    swarm: &mut Swarm<LlmP2pBehaviour>,
    args: &Args,
    state: &mut ClientState,
) -> Result<LlmResponse> {
    info!("Phase 1: Discovering executors...");
    
    // Wait for initial connections
    sleep(Duration::from_secs(2)).await;
    
    // Query for executors
    let executor_key = ServiceRole::Executor.to_kad_key();
    swarm.behaviour_mut().kademlia.get_providers(executor_key.into());
    
    let mut discovery_timeout = tokio::time::interval(Duration::from_secs(10));
    discovery_timeout.tick().await; // Skip first immediate tick
    
    // Discovery phase
    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                handle_swarm_event(swarm, event, state).await;
                
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
                    };
                    
                    info!("Phase 3: Sending request to executor: {}", selected_executor);
                    
                    // Send the request
                    match swarm.behaviour_mut().request_response.send_request(&selected_executor, request) {
                        Ok(request_id) => {
                            state.pending_request = Some((request_id, selected_executor));
                            state.discovery_complete = true;
                        }
                        Err(e) => {
                            return Err(anyhow!("Failed to send request: {}", e));
                        }
                    }
                }
                
                // Check if we received a response
                if let Some(response) = &state.response_received {
                    return Ok(response.clone());
                }
            }
            _ = discovery_timeout.tick() => {
                if state.discovered_executors.is_empty() {
                    return Err(anyhow!("No executors found after discovery timeout"));
                }
            }
        }
    }
}

/// Handle swarm events
async fn handle_swarm_event(
    swarm: &mut Swarm<LlmP2pBehaviour>,
    event: SwarmEvent<LlmP2pEvent>,
    state: &mut ClientState,
) {
    match event {
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            info!("Connected to peer: {}", peer_id);
        }
        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
            debug!("Connection closed with {}: {:?}", peer_id, cause);
        }
        SwarmEvent::Behaviour(LlmP2pEvent::Kademlia(kad::Event::OutboundQueryProgressed {
            result: QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders { providers, .. })),
            ..
        })) => {
            for provider in providers {
                if state.discovered_executors.insert(provider) {
                    info!("Discovered executor: {}", provider);
                }
            }
        }
        SwarmEvent::Behaviour(LlmP2pEvent::Kademlia(kad::Event::OutboundQueryProgressed {
            result: QueryResult::GetProviders(Ok(kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. })),
            ..
        })) => {
            debug!("Kademlia provider query finished");
        }
        SwarmEvent::Behaviour(LlmP2pEvent::RequestResponse(
            request_response::Event::Message { 
                message: request_response::Message::Response { response, request_id },
                peer,
            }
        )) => {
            if let Some((pending_id, expected_peer)) = &state.pending_request {
                if request_id == *pending_id && peer == *expected_peer {
                    info!("Received response from {}: {} tokens", peer, response.token_count);
                    state.response_received = Some(response);
                }
            }
        }
        SwarmEvent::Behaviour(LlmP2pEvent::RequestResponse(
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
        SwarmEvent::Behaviour(LlmP2pEvent::Gossipsub(libp2p::gossipsub::Event::Message { message, .. })) => {
            debug!("Received gossipsub message on topic {:?}", message.topic);
        }
        _ => {}
    }
}