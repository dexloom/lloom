//! Network behavior and event handling for the Crowd Models P2P network.
//! 
//! This module defines the composite libp2p NetworkBehaviour that combines
//! various protocols for discovery, communication, and messaging.

use libp2p::{
    gossipsub::{self, MessageAuthenticity, ValidationMode},
    kad::{self, store::MemoryStore},
    request_response::{self, ProtocolSupport},
    swarm::NetworkBehaviour,
    StreamProtocol,
};
use std::time::Duration;

use crate::protocol::{LlmRequest, LlmResponse, constants::LLM_PROTOCOL};
use crate::error::Result;

/// The custom event type that the behaviour will emit to the Swarm owner.
#[derive(Debug)]
pub enum LlmP2pEvent {
    RequestResponse(request_response::Event<LlmRequest, LlmResponse>),
    Kademlia(kad::Event),
    Gossipsub(gossipsub::Event),
}

// Implement From<T> for LlmP2pEvent for each inner event type
impl From<request_response::Event<LlmRequest, LlmResponse>> for LlmP2pEvent {
    fn from(event: request_response::Event<LlmRequest, LlmResponse>) -> Self {
        LlmP2pEvent::RequestResponse(event)
    }
}

impl From<kad::Event> for LlmP2pEvent {
    fn from(event: kad::Event) -> Self {
        LlmP2pEvent::Kademlia(event)
    }
}

impl From<gossipsub::Event> for LlmP2pEvent {
    fn from(event: gossipsub::Event) -> Self {
        LlmP2pEvent::Gossipsub(event)
    }
}

/// The main network behaviour struct combining all protocols.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "LlmP2pEvent")]
pub struct LlmP2pBehaviour {
    /// Kademlia DHT for peer discovery.
    pub kademlia: kad::Behaviour<MemoryStore>,
    
    /// Gossipsub for broadcasting information.
    pub gossipsub: gossipsub::Behaviour,
    
    /// A custom request-response protocol for direct LLM queries.
    pub request_response: request_response::cbor::Behaviour<LlmRequest, LlmResponse>,
}

impl LlmP2pBehaviour {
    /// Creates a new network behaviour with the given identity.
    pub fn new(identity: &crate::identity::Identity) -> Result<Self> {
        let peer_id = identity.peer_id;
        
        // Configure Kademlia
        let mut kad_config = kad::Config::default();
        kad_config.set_query_timeout(Duration::from_secs(60));
        
        let store = MemoryStore::new(peer_id);
        let kademlia = kad::Behaviour::with_config(peer_id, store, kad_config);
        
        // Configure Gossipsub
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(ValidationMode::Strict)
            .build()
            .map_err(|e| crate::error::Error::Network(format!("Failed to build gossipsub config: {}", e)))?;
            
        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(identity.p2p_keypair.clone()),
            gossipsub_config,
        )
        .map_err(|e| crate::error::Error::Network(format!("Failed to create gossipsub behaviour: {}", e)))?;
        
        // Configure request-response
        let protocols = std::iter::once((
            StreamProtocol::new(LLM_PROTOCOL),
            ProtocolSupport::Full,
        ));
        
        let request_response = request_response::cbor::Behaviour::new(
            protocols,
            request_response::Config::default()
                .with_request_timeout(Duration::from_secs(300)),
        );
        
        Ok(Self {
            kademlia,
            gossipsub,
            request_response,
        })
    }
}

/// Helper functions for network operations.
pub mod helpers {
    use super::*;
    use libp2p::{Multiaddr, Swarm};
    
    /// Bootstrap the Kademlia DHT by adding known peers.
    pub fn bootstrap_kademlia(
        swarm: &mut Swarm<LlmP2pBehaviour>,
        bootstrap_peers: Vec<(libp2p::PeerId, Multiaddr)>,
    ) {
        for (peer_id, addr) in bootstrap_peers {
            swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
        }
        
        // Bootstrap the DHT
        if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
            tracing::warn!("Failed to bootstrap Kademlia: {:?}", e);
        }
    }
    
    /// Subscribe to a gossipsub topic.
    pub fn subscribe_topic(
        swarm: &mut Swarm<LlmP2pBehaviour>,
        topic: &str,
    ) -> Result<()> {
        let topic = gossipsub::IdentTopic::new(topic);
        swarm.behaviour_mut().gossipsub.subscribe(&topic)
            .map_err(|e| crate::error::Error::Network(format!("Failed to subscribe to topic: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;
    use crate::protocol::ServiceRole;

    #[tokio::test]
    async fn test_llm_p2p_behaviour_creation() {
        let identity = Identity::generate();
        let behaviour = LlmP2pBehaviour::new(&identity);
        assert!(behaviour.is_ok());
    }

    #[test]
    fn test_service_role_kad_keys() {
        let executor_key = ServiceRole::Executor.to_kad_key();
        let accountant_key = ServiceRole::Accountant.to_kad_key();
        
        assert_eq!(executor_key, b"crowd-models/executor");
        assert_eq!(accountant_key, b"crowd-models/accountant");
        assert_ne!(executor_key, accountant_key);
    }

    #[tokio::test]
    async fn test_behaviour_components() {
        let identity = Identity::generate();
        let _behaviour = LlmP2pBehaviour::new(&identity).unwrap();
        
        // Ensure all components are properly initialized
        // This is a basic structural test - just verify the behaviour was created
        assert!(true); // Basic smoke test
    }

    mod helpers_tests {
        use super::*;
        use libp2p::{SwarmBuilder, Multiaddr};

        #[tokio::test]
        async fn test_subscribe_topic() -> Result<()> {
            let identity = Identity::generate();
            let behaviour = LlmP2pBehaviour::new(&identity).unwrap();
            
            let mut swarm = SwarmBuilder::with_existing_identity(identity.p2p_keypair.clone())
                .with_tokio()
                .with_tcp(
                    libp2p::tcp::Config::default(),
                    libp2p::noise::Config::new,
                    libp2p::yamux::Config::default,
                )
                .map_err(|e| crate::error::Error::Network(format!("Failed to build swarm: {}", e)))?
                .with_behaviour(|_| behaviour)
                .map_err(|e| crate::error::Error::Network(format!("Failed to set behaviour: {}", e)))?
                .build();

            let result = helpers::subscribe_topic(&mut swarm, "test-topic");
            assert!(result.is_ok());
            Ok(())
        }

        #[tokio::test]
        async fn test_bootstrap_kademlia() -> Result<()> {
            let identity = Identity::generate();
            let behaviour = LlmP2pBehaviour::new(&identity).unwrap();
            
            let mut swarm = SwarmBuilder::with_existing_identity(identity.p2p_keypair.clone())
                .with_tokio()
                .with_tcp(
                    libp2p::tcp::Config::default(),
                    libp2p::noise::Config::new,
                    libp2p::yamux::Config::default,
                )
                .map_err(|e| crate::error::Error::Network(format!("Failed to build swarm: {}", e)))?
                .with_behaviour(|_| behaviour)
                .map_err(|e| crate::error::Error::Network(format!("Failed to set behaviour: {}", e)))?
                .build();

            // Test with empty bootstrap peers (should not panic)
            helpers::bootstrap_kademlia(&mut swarm, vec![]);
            
            // Test with some bootstrap peers
            let peer_id = identity.peer_id;
            let addr: Multiaddr = "/ip4/127.0.0.1/tcp/9000".parse().unwrap();
            helpers::bootstrap_kademlia(&mut swarm, vec![(peer_id, addr)]);
            
            Ok(())
        }
    }
}