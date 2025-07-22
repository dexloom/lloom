//! Protocol definitions for the Crowd Models P2P network.
//! 
//! This module defines the message types and data structures used for
//! communication between nodes in the network.

use serde::{Deserialize, Serialize};
use alloy::primitives::Address;

/// A request sent from a Client to an Executor.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LlmRequest {
    /// The model to use for generation (e.g., "gpt-3.5-turbo", "gpt-4").
    pub model: String,
    /// The prompt to send to the model.
    pub prompt: String,
    /// Optional system prompt for the model.
    pub system_prompt: Option<String>,
    /// Temperature for generation (0.0 to 2.0).
    pub temperature: Option<f32>,
    /// Maximum tokens to generate.
    pub max_tokens: Option<u32>,
}

/// A response sent from an Executor to a Client.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LlmResponse {
    /// The generated content from the model.
    pub content: String,
    /// The total number of tokens used (prompt + completion).
    pub token_count: u32,
    /// The model that was actually used.
    pub model_used: String,
    /// Optional error message if the request failed.
    pub error: Option<String>,
}

/// A usage record that tracks work done by an Executor.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UsageRecord {
    /// The Ethereum address of the client who made the request.
    pub client_address: Address,
    /// The model that was used.
    pub model: String,
    /// The total number of tokens processed.
    pub token_count: u32,
    /// Timestamp of when the work was completed.
    pub timestamp: u64,
}

/// Information about an Executor's capabilities.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecutorInfo {
    /// The peer ID of the executor.
    pub peer_id: String,
    /// The Ethereum address of the executor.
    pub evm_address: Address,
    /// List of models supported by this executor.
    pub supported_models: Vec<String>,
    /// Whether the executor is currently accepting requests.
    pub is_available: bool,
}

/// A role identifier for service discovery.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServiceRole {
    Executor,
    Accountant,
}

impl ServiceRole {
    /// Get the Kademlia key for this service role.
    pub fn to_kad_key(&self) -> Vec<u8> {
        match self {
            ServiceRole::Executor => b"crowd-models/executor".to_vec(),
            ServiceRole::Accountant => b"crowd-models/accountant".to_vec(),
        }
    }
}

/// Protocol constants.
pub mod constants {
    /// The protocol ID for LLM request/response.
    pub const LLM_PROTOCOL: &str = "/crowd-models/llm/1.0.0";
    
    /// Default timeout for LLM requests (in seconds).
    pub const DEFAULT_REQUEST_TIMEOUT: u64 = 300; // 5 minutes
    
    /// Maximum batch size for blockchain submissions.
    pub const MAX_BATCH_SIZE: usize = 100;
    
    /// Interval for batch submissions (in seconds).
    pub const BATCH_SUBMISSION_INTERVAL: u64 = 300; // 5 minutes
}