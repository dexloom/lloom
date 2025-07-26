//! # Lloom Core
//!
//! This crate provides the foundational functionality for the Lloom P2P network,
//! a decentralized system for requesting and providing Large Language Model (LLM) services.
//!
//! ## Architecture Overview
//!
//! The Lloom network consists of three types of nodes:
//!
//! - **Clients**: Request LLM services from the network
//! - **Executors**: Provide LLM services and execute requests
//! - **Validators**: Serve as bootstrap nodes and maintain network integrity
//!
//! ## Core Components
//!
//! ### Identity Management ([`identity`])
//!
//! Provides cryptographic identity functionality including:
//! - ECC key pair generation and management
//! - P2P network identity (PeerId) derivation
//! - Ethereum-compatible address generation
//! - Consistent identity across network protocols
//!
//! ### Network Protocol ([`network`])
//!
//! Implements the P2P networking layer using libp2p:
//! - Kademlia DHT for service discovery
//! - Gossipsub for network announcements
//! - Request-Response protocol for LLM interactions
//! - Bootstrap and peer management
//!
//! ### Message Protocol ([`protocol`])
//!
//! Defines the core message types and data structures:
//! - [`LlmRequest`] and [`LlmResponse`] for service interactions
//! - [`UsageRecord`] for blockchain accounting
//! - Service role definitions and discovery keys
//! - Message routing and validation
//!
//! ### Cryptographic Signing ([`signing`])
//!
//! Provides EIP-712 compliant message signing and verification:
//! - Tamper-proof message authentication
//! - Replay attack protection with timestamps
//! - Flexible verification policies
//! - Integration with Ethereum ecosystem
//!
//! ### EIP-712 Implementation ([`eip712`])
//!
//! Ethereum Improvement Proposal 712 implementation for structured data signing:
//! - Type-safe message encoding
//! - Domain separation for security
//! - Compatible with Ethereum wallets and tools
//!
//! ## Usage Examples
//!
//! ### Basic Identity Creation
//!
//! ```rust
//! use lloom_core::Identity;
//!
//! // Generate a new random identity
//! let identity = Identity::generate();
//! println!("PeerId: {}", identity.peer_id);
//! println!("EVM Address: {}", identity.evm_address);
//!
//! // Load identity from private key
//! let identity = Identity::from_str("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80")?;
//! # Ok::<(), lloom_core::Error>(())
//! ```
//!
//! ### Network Behavior Setup
//!
//! ```rust,no_run
//! use lloom_core::{Identity, LloomBehaviour};
//!
//! let identity = Identity::generate();
//! let behaviour = LloomBehaviour::new(&identity)?;
//! # Ok::<(), lloom_core::Error>(())
//! ```
//!
//! ### Message Signing
//!
//! ```rust
//! use lloom_core::{Identity, protocol::LlmRequest, signing::SignableMessage};
//!
//! let identity = Identity::generate();
//! let request = LlmRequest {
//!     model: "gpt-3.5-turbo".to_string(),
//!     prompt: "Hello, world!".to_string(),
//!     system_prompt: None,
//!     temperature: Some(0.7),
//!     max_tokens: Some(100),
//!     executor_address: "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string(),
//!     inbound_price: "500000000000000".to_string(),
//!     outbound_price: "1000000000000000".to_string(),
//!     nonce: 1,
//!     deadline: 1234567890,
//! };
//!
//! let signed_request = request.sign_blocking(&identity.wallet)?;
//! # Ok::<(), lloom_core::Error>(())
//! ```

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