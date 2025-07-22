//! Accountant node for the Crowd Models P2P network.
//! 
//! The Accountant serves as a stable supernode for network bootstrap and discovery.
//! It maintains a directory of active executors and helps clients discover them.

use anyhow::Result;
use clap::Parser;
use crowd_models_core::{
    identity::Identity,
    network::{LlmP2pBehaviour, LlmP2pEvent, helpers},
    protocol::ServiceRole,
};
use futures::StreamExt;
use libp2p::{
    kad::{self, QueryResult, Record},
    swarm::SwarmEvent,
    Multiaddr, Swarm, SwarmBuilder,
};
use std::{
    collections::HashSet,
    path::PathBuf,
    time::Duration,
};
use tokio::{
    signal,
    sync::mpsc,
    time,
};
use tracing::{debug, info, warn};

/// Command-line arguments for the Accountant node
#[derive(Parser, Debug)]
#[command(author, version, about = "Accountant node for Crowd Models P2P network")]
struct Args {
    /// Path to the private key file (hex-encoded)
    #[arg(short = 'k', long, env = "ACCOUNTANT_PRIVATE_KEY_FILE")]
    private_key_file: Option<PathBuf>,

    /// Port to listen on for P2P connections
    #[arg(short = 'p', long, default_value = "9000", env = "ACCOUNTANT_P2P_PORT")]
    p2p_port: u16,

    /// External address for other nodes to connect to (e.g., /ip4/1.2.3.4/tcp/9000)
    #[arg(long, env = "ACCOUNTANT_EXTERNAL_ADDR")]
    external_addr: Option<String>,

    /// Enable debug logging
    #[arg(short = 'd', long, env = "ACCOUNTANT_DEBUG")]
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

    info!("Starting Crowd Models Accountant node...");

    // Load or generate identity
    let identity = load_or_generate_identity(args.private_key_file.as_deref()).await?;
    info!("Node identity loaded: PeerId={}", identity.peer_id);
    info!("EVM address: {}", identity.evm_address);

    // Create the network behaviour
    let behaviour = LlmP2pBehaviour::new(&identity)?;

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
    helpers::subscribe_topic(&mut swarm, "crowd-models/announcements")?;
    helpers::subscribe_topic(&mut swarm, "crowd-models/executor-updates")?;

    // Register as an accountant in Kademlia
    let accountant_key = ServiceRole::Accountant.to_kad_key();
    let record = Record {
        key: accountant_key.clone().into(),
        value: identity.peer_id.to_bytes(),
        publisher: Some(identity.peer_id),
        expires: None,
    };
    swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One)?;

    info!("Accountant node started successfully");

    // Set up shutdown signal handler
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
        let _ = shutdown_tx.send(()).await;
    });

    // Set up periodic tasks
    let mut periodic_interval = time::interval(Duration::from_secs(60));

    // Track known executors
    let mut known_executors = HashSet::new();

    // Main event loop
    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                handle_swarm_event(&mut swarm, event, &mut known_executors).await;
            }
            _ = periodic_interval.tick() => {
                // Perform periodic maintenance
                perform_periodic_tasks(&mut swarm, &known_executors).await;
            }
            _ = shutdown_rx.recv() => {
                info!("Received shutdown signal");
                break;
            }
        }
    }

    info!("Shutting down accountant node...");
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
    _swarm: &mut Swarm<LlmP2pBehaviour>,
    event: SwarmEvent<LlmP2pEvent>,
    known_executors: &mut HashSet<libp2p::PeerId>,
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
        SwarmEvent::Behaviour(LlmP2pEvent::Kademlia(kad::Event::OutboundQueryProgressed {
            result: QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(record))),
            ..
        })) => {
            if record.record.key.as_ref() == ServiceRole::Executor.to_kad_key() {
                if let Ok(peer_id) = libp2p::PeerId::from_bytes(&record.record.value) {
                    if known_executors.insert(peer_id) {
                        info!("Discovered new executor: {}", peer_id);
                    }
                }
            }
        }
        SwarmEvent::Behaviour(LlmP2pEvent::Kademlia(kad::Event::InboundRequest { 
            request: kad::InboundRequest::GetRecord { .. }, 
            .. 
        })) => {
            debug!("Received Kademlia GetRecord request");
        }
        SwarmEvent::Behaviour(LlmP2pEvent::Gossipsub(libp2p::gossipsub::Event::Message {
            propagation_source,
            message,
            ..
        })) => {
            debug!(
                "Received gossipsub message from {} on topic {:?}",
                propagation_source, message.topic
            );
        }
        _ => {}
    }
}

/// Perform periodic maintenance tasks
async fn perform_periodic_tasks(
    swarm: &mut Swarm<LlmP2pBehaviour>,
    known_executors: &HashSet<libp2p::PeerId>,
) {
    // Refresh our accountant registration
    let accountant_key = ServiceRole::Accountant.to_kad_key();
    let peer_id = *swarm.local_peer_id();
    let record = Record {
        key: accountant_key.into(),
        value: peer_id.to_bytes(),
        publisher: Some(peer_id),
        expires: None,
    };
    
    if let Err(e) = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
        warn!("Failed to refresh accountant registration: {:?}", e);
    }

    // Query for executors periodically
    swarm.behaviour_mut()
        .kademlia
        .get_record(ServiceRole::Executor.to_kad_key().into());

    info!("Periodic maintenance completed. Known executors: {}", known_executors.len());
}