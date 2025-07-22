//! # Crowd Models Core
//! 
//! This crate provides the core functionality for the Crowd Models P2P network,
//! including identity management, networking protocols, and shared data structures.

pub mod identity;
pub mod network;
pub mod protocol;
pub mod error;

pub use identity::Identity;
pub use network::{LlmP2pBehaviour, LlmP2pEvent};
pub use protocol::{LlmRequest, LlmResponse, UsageRecord};
pub use error::{Error, Result};

// Re-export commonly used types
pub use libp2p::{PeerId, Multiaddr};
pub use alloy::primitives::Address;