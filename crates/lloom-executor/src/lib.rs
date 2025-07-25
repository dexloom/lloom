//! Lloom Executor Library
//!
//! This library provides the core functionality for the executor node,
//! including LLM client management, model discovery, and blockchain integration.

pub mod config;
pub mod llm_client;
pub mod blockchain;

// Re-export commonly used types for convenience
pub use config::{ExecutorConfig, LlmBackendConfig, BlockchainConfig, NetworkConfig};
pub use llm_client::{LlmClient, ModelInfo};