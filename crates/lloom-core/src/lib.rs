//! # Lloom Core
//!
//! This crate provides the core functionality for the Lloom P2P network,
//! including identity management, networking protocols, and shared data structures.

pub mod eip712;
pub mod identity;
pub mod network;
pub mod protocol;
pub mod signing;
pub mod error;

pub use eip712::*;
pub use identity::Identity;
pub use network::{LloomBehaviour, LloomEvent};
pub use protocol::{LlmRequest, LlmResponse, UsageRecord, RequestMessage, ResponseMessage, SignedLlmRequest, SignedLlmResponse};
pub use signing::{SignedMessage, SignableMessage, VerificationConfig, sign_message_blocking, verify_signed_message, verify_signed_message_basic, verify_signed_message_permissive};
pub use error::{Error, Result};

// Re-export commonly used types
pub use libp2p::{PeerId, Multiaddr};
pub use alloy::primitives::Address;