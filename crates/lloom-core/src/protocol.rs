//! Protocol definitions for the Lloom P2P network.
//!
//! This module defines the message types and data structures used for
//! communication between nodes in the network.

use serde::{Deserialize, Serialize};
use alloy::primitives::Address;
use crate::signing::{SignedMessage, SignableMessage};
use std::collections::HashMap;

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
    /// Ethereum address of the executor node
    pub executor_address: String,
    /// Price per inbound token in wei (UINT256 as string)
    pub inbound_price: String,
    /// Price per outbound token in wei (UINT256 as string)
    pub outbound_price: String,
    /// Client nonce for replay protection
    pub nonce: u64,
    /// Unix timestamp deadline for request validity
    pub deadline: u64,
}

/// A response sent from an Executor to a Client.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LlmResponse {
    /// The generated content from the model.
    pub content: String,
    /// Number of tokens in the prompt (input)
    pub inbound_tokens: u64,
    /// Number of tokens in the response (output)
    pub outbound_tokens: u64,
    /// Total cost in wei (UINT256 as string)
    pub total_cost: String,
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
    Validator,
}

impl ServiceRole {
    /// Get the Kademlia key for this service role.
    pub fn to_kad_key(&self) -> Vec<u8> {
        match self {
            ServiceRole::Executor => b"lloom/executor".to_vec(),
            ServiceRole::Validator => b"lloom/validator".to_vec(),
        }
    }
}

/// Protocol constants.
pub mod constants {
    /// The protocol ID for LLM request/response.
    pub const LLM_PROTOCOL: &str = "/lloom/llm/1.0.0";
    
    /// Default timeout for LLM requests (in seconds).
    pub const DEFAULT_REQUEST_TIMEOUT: u64 = 300; // 5 minutes
    
    /// Maximum age for signed messages (in seconds) - 5 minutes for replay protection.
    pub const MAX_MESSAGE_AGE_SECS: u64 = 300;
    
    /// Maximum batch size for blockchain submissions.
    pub const MAX_BATCH_SIZE: usize = 100;
    
    /// Interval for batch submissions (in seconds).
    pub const BATCH_SUBMISSION_INTERVAL: u64 = 300; // 5 minutes
}

// Implement SignableMessage for protocol messages
impl SignableMessage for LlmRequest {}
impl SignableMessage for LlmResponse {}
impl SignableMessage for UsageRecord {}

// Implement SignableMessage for model announcement protocol messages
impl SignableMessage for ModelAnnouncement {}
impl SignableMessage for ModelQuery {}
impl SignableMessage for ModelQueryResponse {}
impl SignableMessage for ModelUpdate {}
impl SignableMessage for AcknowledgmentResponse {}

/// Type aliases for commonly used signed messages
pub type SignedLlmRequest = SignedMessage<LlmRequest>;
pub type SignedLlmResponse = SignedMessage<LlmResponse>;
pub type SignedUsageRecord = SignedMessage<UsageRecord>;

/// Type aliases for model announcement protocol signed messages
pub type SignedModelAnnouncement = SignedMessage<ModelAnnouncement>;
pub type SignedModelQuery = SignedMessage<ModelQuery>;
pub type SignedModelQueryResponse = SignedMessage<ModelQueryResponse>;
pub type SignedModelUpdate = SignedMessage<ModelUpdate>;
pub type SignedAcknowledgmentResponse = SignedMessage<AcknowledgmentResponse>;

/// Wrapper enum for request messages to support both signed and unsigned variants
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RequestMessage {
    /// Unsigned LLM request (for backwards compatibility)
    LlmRequest(LlmRequest),
    /// Signed LLM request (with cryptographic signature)
    SignedLlmRequest(SignedLlmRequest),
    
    // Model announcement protocol messages
    /// Model announcement from executor to validator
    ModelAnnouncement(SignedMessage<ModelAnnouncement>),
    /// Model query from client to validator
    ModelQuery(SignedMessage<ModelQuery>),
    /// Model update from executor to validator
    ModelUpdate(SignedMessage<ModelUpdate>),
}

/// Wrapper enum for response messages to support both signed and unsigned variants
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ResponseMessage {
    /// Unsigned LLM response (for backwards compatibility)
    LlmResponse(LlmResponse),
    /// Signed LLM response (with cryptographic signature)
    SignedLlmResponse(SignedLlmResponse),
    
    // Model announcement protocol responses
    /// Response to model query
    ModelQueryResponse(SignedMessage<ModelQueryResponse>),
    /// Acknowledgment response for announcements and updates
    AcknowledgmentResponse(SignedMessage<AcknowledgmentResponse>),
}

// ============================================================================
// Model Announcement Protocol Messages
// ============================================================================

/// Announcement message sent by executors to validators
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModelAnnouncement {
    /// Executor's peer ID
    pub executor_peer_id: String, // Using String for libp2p::PeerId compatibility
    
    /// Executor's EVM address for on-chain verification
    pub executor_address: Address,
    
    /// List of supported models with their capabilities
    pub models: Vec<ModelDescriptor>,
    
    /// Announcement type (initial/update/removal)
    pub announcement_type: AnnouncementType,
    
    /// Unix timestamp of the announcement
    pub timestamp: u64,
    
    /// Nonce for replay protection
    pub nonce: u64,
    
    /// Protocol version for compatibility
    pub protocol_version: u8,
}

/// Individual model descriptor with capabilities
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelDescriptor {
    /// Model identifier (e.g., "gpt-4", "llama-2-7b")
    pub model_id: String,
    
    /// Backend type (e.g., "openai", "lmstudio", "custom")
    pub backend_type: String,
    
    /// Model capabilities and metadata
    pub capabilities: ModelCapabilities,
    
    /// Current availability status
    pub is_available: bool,
    
    /// Pricing information (optional)
    pub pricing: Option<ModelPricing>,
}

/// Model capabilities and metadata
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelCapabilities {
    /// Maximum context length in tokens
    pub max_context_length: u32,
    
    /// Supported features
    pub features: Vec<String>, // e.g., ["chat", "completion", "embeddings"]
    
    /// Model architecture (optional)
    pub architecture: Option<String>,
    
    /// Model size/parameters (optional)
    pub model_size: Option<String>,
    
    /// Performance metrics (optional)
    pub performance: Option<PerformanceMetrics>,
    
    /// Additional metadata as key-value pairs
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Performance metrics for a model
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PerformanceMetrics {
    /// Average tokens per second
    pub avg_tokens_per_second: Option<f64>,
    
    /// Average time to first token (in seconds)
    pub avg_time_to_first_token: Option<f64>,
    
    /// Success rate (0.0 to 1.0)
    pub success_rate: Option<f64>,
    
    /// Average latency in milliseconds
    pub avg_latency_ms: Option<u64>,
}

/// Pricing information for a model
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelPricing {
    /// Price per input token in wei (as string for large numbers)
    pub input_token_price: String,
    
    /// Price per output token in wei
    pub output_token_price: String,
    
    /// Minimum request fee (if any)
    pub minimum_fee: Option<String>,
}

/// Type of announcement
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AnnouncementType {
    /// Initial announcement when executor connects
    Initial,
    /// Update to existing model list
    Update,
    /// Graceful removal before disconnect
    Removal,
    /// Heartbeat to maintain presence
    Heartbeat,
}

/// Query from client to validator for model information
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModelQuery {
    /// Type of query
    pub query_type: ModelQueryType,
    
    /// Optional filters for the query
    pub filters: Option<QueryFilters>,
    
    /// Maximum number of results (for pagination)
    pub limit: Option<u32>,
    
    /// Offset for pagination
    pub offset: Option<u32>,
    
    /// Query ID for response correlation
    pub query_id: String,
    
    /// Timestamp of the query
    pub timestamp: u64,
}

/// Type of model query
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ModelQueryType {
    /// List all available models
    ListAllModels,
    /// Find executors for a specific model
    FindModel(String),
    /// Get detailed info about specific executors
    ExecutorInfo(Vec<String>), // Vec<libp2p::PeerId> as Strings
    /// Search models by capabilities
    SearchByCapabilities,
    /// Get statistics about model availability
    GetStatistics,
}

/// Filters for model queries
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryFilters {
    /// Filter by backend type
    pub backend_type: Option<String>,
    
    /// Minimum required context length
    pub min_context_length: Option<u32>,
    
    /// Required features
    pub required_features: Option<Vec<String>>,
    
    /// Maximum price per token (in wei)
    pub max_price: Option<String>,
    
    /// Only show available models
    pub only_available: bool,
    
    /// Minimum success rate
    pub min_success_rate: Option<f64>,
}

/// Response from validator to client's model query
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModelQueryResponse {
    /// Query ID this responds to
    pub query_id: String,
    
    /// Query results
    pub result: QueryResult,
    
    /// Total count (for pagination)
    pub total_count: Option<u32>,
    
    /// Response timestamp
    pub timestamp: u64,
    
    /// Validator's peer ID
    pub validator_peer_id: String, // libp2p::PeerId as String
}

/// Query result variants
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum QueryResult {
    /// List of models with their executors
    ModelList(Vec<ModelEntry>),
    /// List of executors for a specific model
    ExecutorList(Vec<ExecutorEntry>),
    /// Detailed executor information
    ExecutorDetails(Vec<ExecutorDetail>),
    /// Network statistics
    Statistics(NetworkStatistics),
    /// Error response
    Error(QueryError),
}

/// Model entry in query response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModelEntry {
    /// Model identifier
    pub model_id: String,
    
    /// Number of executors supporting this model
    pub executor_count: u32,
    
    /// List of executor peer IDs
    pub executors: Vec<String>, // Vec<libp2p::PeerId> as Strings
    
    /// Aggregated capabilities (best available)
    pub capabilities: ModelCapabilities,
    
    /// Average pricing across executors
    pub avg_pricing: Option<ModelPricing>,
}

/// Executor entry in query response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecutorEntry {
    /// Executor's peer ID
    pub peer_id: String, // libp2p::PeerId as String
    
    /// Executor's EVM address
    pub evm_address: Address,
    
    /// Connection status
    pub is_connected: bool,
    
    /// Last seen timestamp
    pub last_seen: u64,
    
    /// Reliability score (0.0 to 1.0)
    pub reliability_score: Option<f64>,
}

/// Detailed executor information
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecutorDetail {
    /// Basic executor info
    pub executor: ExecutorEntry,
    
    /// All models supported by this executor
    pub models: Vec<ModelDescriptor>,
    
    /// Performance statistics
    pub stats: Option<ExecutorStatistics>,
}

/// Error response for queries
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryError {
    /// Error code
    pub code: u32,
    
    /// Error message
    pub message: String,
    
    /// Additional details
    pub details: Option<String>,
}

/// Network statistics
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct NetworkStatistics {
    /// Total number of executors
    pub total_executors: u32,
    
    /// Total number of unique models
    pub total_models: u32,
    
    /// Currently connected executors
    pub connected_executors: u32,
    
    /// Total requests processed
    pub total_requests: u64,
    
    /// Network uptime in seconds
    pub uptime: u64,
    
    /// Last reset timestamp
    pub last_reset: u64,
}

/// Executor performance statistics
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ExecutorStatistics {
    /// Total requests handled
    pub total_requests: u64,
    
    /// Successful requests
    pub successful_requests: u64,
    
    /// Failed requests
    pub failed_requests: u64,
    
    /// Average response time in ms
    pub avg_response_time: u64,
    
    /// Total tokens processed
    pub total_tokens: u64,
    
    /// Last updated timestamp
    pub last_updated: u64,
}

/// Update message for model changes
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModelUpdate {
    /// Executor's peer ID
    pub executor_peer_id: String, // libp2p::PeerId as String
    
    /// Type of update
    pub update_type: UpdateType,
    
    /// Updated model information
    pub updates: Vec<ModelUpdateEntry>,
    
    /// Timestamp of the update
    pub timestamp: u64,
    
    /// Update sequence number for ordering
    pub sequence: u64,
}

/// Type of model update
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum UpdateType {
    /// Add new models
    AddModels,
    /// Remove models
    RemoveModels,
    /// Update model capabilities
    UpdateCapabilities,
    /// Update pricing
    UpdatePricing,
    /// Update availability
    UpdateAvailability,
}

/// Individual model update entry
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModelUpdateEntry {
    /// Model identifier
    pub model_id: String,
    
    /// New descriptor (for adds/updates)
    pub descriptor: Option<ModelDescriptor>,
    
    /// Update reason
    pub reason: Option<String>,
}

/// Simple acknowledgment response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AcknowledgmentResponse {
    /// Request ID being acknowledged
    pub request_id: String,
    
    /// Success status
    pub success: bool,
    
    /// Optional message
    pub message: Option<String>,
    
    /// Timestamp
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_llm_request_creation() {
        let request = LlmRequest {
            model: "gpt-3.5-turbo".to_string(),
            prompt: "Hello, world!".to_string(),
            system_prompt: Some("You are a helpful assistant".to_string()),
            temperature: Some(0.7),
            max_tokens: Some(150),
            executor_address: "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string(),
            inbound_price: "500000000000000".to_string(),
            outbound_price: "1000000000000000".to_string(),
            nonce: 1,
            deadline: 1234567890,
        };

        assert_eq!(request.model, "gpt-3.5-turbo");
        assert_eq!(request.prompt, "Hello, world!");
        assert_eq!(request.system_prompt, Some("You are a helpful assistant".to_string()));
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(150));
    }

    #[test]
    fn test_llm_request_minimal() {
        let request = LlmRequest {
            model: "gpt-4".to_string(),
            prompt: "Test prompt".to_string(),
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            executor_address: "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string(),
            inbound_price: "500000000000000".to_string(), // 0.0005 ETH per token
            outbound_price: "1000000000000000".to_string(), // 0.001 ETH per token
            nonce: 2,
            deadline: 1234567891,
        };

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.prompt, "Test prompt");
        assert!(request.system_prompt.is_none());
        assert!(request.temperature.is_none());
        assert!(request.max_tokens.is_none());
        assert_eq!(request.executor_address, "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a");
        assert_eq!(request.inbound_price, "500000000000000");
        assert_eq!(request.outbound_price, "1000000000000000");
        assert_eq!(request.nonce, 2);
        assert_eq!(request.deadline, 1234567891);
    }

    #[test]
    fn test_llm_response_success() {
        let response = LlmResponse {
            content: "Generated content".to_string(),
            inbound_tokens: 20,
            outbound_tokens: 22,
            total_cost: "62000000000000000".to_string(), // 20 * 0.001 + 22 * 0.002 = 0.064 ETH
            model_used: "gpt-3.5-turbo".to_string(),
            error: None,
        };

        assert_eq!(response.content, "Generated content");
        assert_eq!(response.inbound_tokens, 20);
        assert_eq!(response.outbound_tokens, 22);
        assert_eq!(response.total_cost, "62000000000000000");
        assert_eq!(response.model_used, "gpt-3.5-turbo");
        assert!(response.error.is_none());
    }

    #[test]
    fn test_llm_response_error() {
        let response = LlmResponse {
            content: String::new(),
            inbound_tokens: 0,
            outbound_tokens: 0,
            total_cost: "0".to_string(),
            model_used: "gpt-4".to_string(),
            error: Some("API rate limit exceeded".to_string()),
        };

        assert!(response.content.is_empty());
        assert_eq!(response.inbound_tokens, 0);
        assert_eq!(response.outbound_tokens, 0);
        assert_eq!(response.total_cost, "0");
        assert_eq!(response.error, Some("API rate limit exceeded".to_string()));
    }

    #[test]
    fn test_usage_record() {
        let client_address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".parse::<Address>().unwrap();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        let usage_record = UsageRecord {
            client_address,
            model: "gpt-4".to_string(),
            token_count: 100,
            timestamp,
        };

        assert_eq!(usage_record.client_address, client_address);
        assert_eq!(usage_record.model, "gpt-4");
        assert_eq!(usage_record.token_count, 100);
        assert_eq!(usage_record.timestamp, timestamp);
    }

    #[test]
    fn test_executor_info() {
        let evm_address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".parse::<Address>().unwrap();
        let executor_info = ExecutorInfo {
            peer_id: "12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aFdD7zdBUyJ1BDdyWz4".to_string(),
            evm_address,
            supported_models: vec!["gpt-3.5-turbo".to_string(), "gpt-4".to_string()],
            is_available: true,
        };

        assert_eq!(executor_info.peer_id, "12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aFdD7zdBUyJ1BDdyWz4");
        assert_eq!(executor_info.evm_address, evm_address);
        assert_eq!(executor_info.supported_models.len(), 2);
        assert!(executor_info.is_available);
    }

    #[test]
    fn test_service_role_to_kad_key() {
        let executor_key = ServiceRole::Executor.to_kad_key();
        let validator_key = ServiceRole::Validator.to_kad_key();

        assert_eq!(executor_key, b"lloom/executor".to_vec());
        assert_eq!(validator_key, b"lloom/validator".to_vec());
        assert_ne!(executor_key, validator_key);
    }

    #[test]
    fn test_service_role_equality() {
        assert_eq!(ServiceRole::Executor, ServiceRole::Executor);
        assert_eq!(ServiceRole::Validator, ServiceRole::Validator);
        assert_ne!(ServiceRole::Executor, ServiceRole::Validator);
    }

    #[test]
    fn test_serialization_llm_request() {
        let request = LlmRequest {
            model: "gpt-3.5-turbo".to_string(),
            prompt: "Test".to_string(),
            system_prompt: Some("System".to_string()),
            temperature: Some(0.5),
            max_tokens: Some(100),
            executor_address: "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string(),
            inbound_price: "500000000000000".to_string(),
            outbound_price: "1000000000000000".to_string(),
            nonce: 1,
            deadline: 1234567890,
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: LlmRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(request.model, deserialized.model);
        assert_eq!(request.prompt, deserialized.prompt);
        assert_eq!(request.system_prompt, deserialized.system_prompt);
        assert_eq!(request.temperature, deserialized.temperature);
        assert_eq!(request.max_tokens, deserialized.max_tokens);
        assert_eq!(request.executor_address, deserialized.executor_address);
        assert_eq!(request.inbound_price, deserialized.inbound_price);
        assert_eq!(request.outbound_price, deserialized.outbound_price);
        assert_eq!(request.nonce, deserialized.nonce);
        assert_eq!(request.deadline, deserialized.deadline);
    }

    #[test]
    fn test_serialization_llm_response() {
        let response = LlmResponse {
            content: "Response content".to_string(),
            inbound_tokens: 10,
            outbound_tokens: 15,
            total_cost: "25000000000000000".to_string(),
            model_used: "gpt-4".to_string(),
            error: None,
        };

        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: LlmResponse = serde_json::from_str(&serialized).unwrap();

        assert_eq!(response.content, deserialized.content);
        assert_eq!(response.inbound_tokens, deserialized.inbound_tokens);
        assert_eq!(response.outbound_tokens, deserialized.outbound_tokens);
        assert_eq!(response.total_cost, deserialized.total_cost);
        assert_eq!(response.model_used, deserialized.model_used);
        assert_eq!(response.error, deserialized.error);
    }

    #[test]
    fn test_serialization_usage_record() {
        let client_address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".parse::<Address>().unwrap();
        let usage_record = UsageRecord {
            client_address,
            model: "gpt-3.5-turbo".to_string(),
            token_count: 50,
            timestamp: 1234567890,
        };

        let serialized = serde_json::to_string(&usage_record).unwrap();
        let deserialized: UsageRecord = serde_json::from_str(&serialized).unwrap();

        assert_eq!(usage_record.client_address, deserialized.client_address);
        assert_eq!(usage_record.model, deserialized.model);
        assert_eq!(usage_record.token_count, deserialized.token_count);
        assert_eq!(usage_record.timestamp, deserialized.timestamp);
    }

    #[test]
    fn test_serialization_executor_info() {
        let evm_address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".parse::<Address>().unwrap();
        let executor_info = ExecutorInfo {
            peer_id: "test_peer".to_string(),
            evm_address,
            supported_models: vec!["model1".to_string(), "model2".to_string()],
            is_available: false,
        };

        let serialized = serde_json::to_string(&executor_info).unwrap();
        let deserialized: ExecutorInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(executor_info.peer_id, deserialized.peer_id);
        assert_eq!(executor_info.evm_address, deserialized.evm_address);
        assert_eq!(executor_info.supported_models, deserialized.supported_models);
        assert_eq!(executor_info.is_available, deserialized.is_available);
    }

    #[test]
    fn test_protocol_constants() {
        assert_eq!(constants::LLM_PROTOCOL, "/lloom/llm/1.0.0");
        assert_eq!(constants::DEFAULT_REQUEST_TIMEOUT, 300);
        assert_eq!(constants::MAX_BATCH_SIZE, 100);
        assert_eq!(constants::BATCH_SUBMISSION_INTERVAL, 300);
    }

    #[test]
    fn test_llm_request_clone() {
        let original = LlmRequest {
            model: "gpt-4".to_string(),
            prompt: "Clone test".to_string(),
            system_prompt: None,
            temperature: Some(0.8),
            max_tokens: Some(200),
            executor_address: "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string(),
            inbound_price: "500000000000000".to_string(),
            outbound_price: "1000000000000000".to_string(),
            nonce: 1,
            deadline: 1234567890,
        };

        let cloned = original.clone();
        assert_eq!(original.model, cloned.model);
        assert_eq!(original.prompt, cloned.prompt);
        assert_eq!(original.system_prompt, cloned.system_prompt);
        assert_eq!(original.temperature, cloned.temperature);
        assert_eq!(original.max_tokens, cloned.max_tokens);
    }

    #[test]
    fn test_llm_response_clone() {
        let original = LlmResponse {
            content: "Clone test response".to_string(),
            inbound_tokens: 5,
            outbound_tokens: 10,
            total_cost: "15000000000000000".to_string(),
            model_used: "gpt-3.5-turbo".to_string(),
            error: Some("Test error".to_string()),
        };

        let cloned = original.clone();
        assert_eq!(original.content, cloned.content);
        assert_eq!(original.inbound_tokens, cloned.inbound_tokens);
        assert_eq!(original.outbound_tokens, cloned.outbound_tokens);
        assert_eq!(original.total_cost, cloned.total_cost);
        assert_eq!(original.model_used, cloned.model_used);
        assert_eq!(original.error, cloned.error);
    }

    #[tokio::test]
    async fn test_signed_llm_request() {
        use crate::signing::SignableMessage;
        use alloy::signers::local::PrivateKeySigner;

        let signer: PrivateKeySigner = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse()
            .expect("Valid private key");

        let request = LlmRequest {
            model: "gpt-4".to_string(),
            prompt: "Test signing".to_string(),
            system_prompt: None,
            temperature: Some(0.5),
            max_tokens: Some(100),
            executor_address: "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string(),
            inbound_price: "500000000000000".to_string(),
            outbound_price: "1000000000000000".to_string(),
            nonce: 1,
            deadline: 1234567890,
        };

        let signed_request = request.sign_blocking(&signer).unwrap();
        
        assert_eq!(signed_request.payload.model, "gpt-4");
        assert_eq!(signed_request.payload.prompt, "Test signing");
        assert_eq!(signed_request.signer, signer.address());
        assert!(signed_request.signature.len() == 65);
    }

    #[tokio::test]
    async fn test_signed_llm_response() {
        use crate::signing::SignableMessage;
        use alloy::signers::local::PrivateKeySigner;

        let signer: PrivateKeySigner = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse()
            .expect("Valid private key");

        let response = LlmResponse {
            content: "Test response".to_string(),
            inbound_tokens: 5,
            outbound_tokens: 5,
            total_cost: "10000000000000000".to_string(),
            model_used: "gpt-4".to_string(),
            error: None,
        };

        let signed_response = response.sign_blocking(&signer).unwrap();
        
        assert_eq!(signed_response.payload.content, "Test response");
        assert_eq!(signed_response.payload.inbound_tokens, 5);
        assert_eq!(signed_response.payload.outbound_tokens, 5);
        assert_eq!(signed_response.signer, signer.address());
        assert!(signed_response.signature.len() == 65);
    }

    #[tokio::test]
    async fn test_signed_usage_record() {
        use crate::signing::SignableMessage;
        use alloy::signers::local::PrivateKeySigner;

        let signer: PrivateKeySigner = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse()
            .expect("Valid private key");

        let client_address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".parse::<Address>().unwrap();
        let usage_record = UsageRecord {
            client_address,
            model: "gpt-3.5-turbo".to_string(),
            token_count: 50,
            timestamp: 1234567890,
        };

        let signed_usage_record = usage_record.sign_blocking(&signer).unwrap();
        
        assert_eq!(signed_usage_record.payload.client_address, client_address);
        assert_eq!(signed_usage_record.payload.model, "gpt-3.5-turbo");
        assert_eq!(signed_usage_record.payload.token_count, 50);
        assert_eq!(signed_usage_record.signer, signer.address());
        assert!(signed_usage_record.signature.len() == 65);
    }

    #[tokio::test]
    async fn test_signed_message_type_aliases() {
        use crate::signing::SignableMessage;
        use alloy::signers::local::PrivateKeySigner;

        let signer: PrivateKeySigner = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse()
            .expect("Valid private key");

        let request = LlmRequest {
            model: "gpt-4".to_string(),
            prompt: "Type alias test".to_string(),
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            executor_address: "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string(),
            inbound_price: "500000000000000".to_string(),
            outbound_price: "1000000000000000".to_string(),
            nonce: 1,
            deadline: 1234567890,
        };

        // Test that the type alias works
        let signed_request: SignedLlmRequest = request.sign_blocking(&signer).unwrap();
        assert_eq!(signed_request.payload.prompt, "Type alias test");

        let response = LlmResponse {
            content: "Response content".to_string(),
            inbound_tokens: 7,
            outbound_tokens: 8,
            total_cost: "15000000000000000".to_string(),
            model_used: "gpt-4".to_string(),
            error: None,
        };

        let signed_response: SignedLlmResponse = response.sign_blocking(&signer).unwrap();
        assert_eq!(signed_response.payload.content, "Response content");

        let client_address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".parse::<Address>().unwrap();
        let usage_record = UsageRecord {
            client_address,
            model: "gpt-4".to_string(),
            token_count: 20,
            timestamp: 1234567890,
        };

        let signed_usage_record: SignedUsageRecord = usage_record.sign_blocking(&signer).unwrap();
        assert_eq!(signed_usage_record.payload.token_count, 20);
    }

    #[tokio::test]
    async fn test_signed_message_verification() {
        use crate::signing::{SignableMessage, verify_signed_message_basic};
        use alloy::signers::local::PrivateKeySigner;

        let signer: PrivateKeySigner = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse()
            .expect("Valid private key");

        let request = LlmRequest {
            model: "gpt-4".to_string(),
            prompt: "Verification test".to_string(),
            system_prompt: Some("System".to_string()),
            temperature: Some(0.8),
            max_tokens: Some(200),
            executor_address: "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string(),
            inbound_price: "500000000000000".to_string(),
            outbound_price: "1000000000000000".to_string(),
            nonce: 1,
            deadline: 1234567890,
        };

        let signed_request = request.sign_blocking(&signer).unwrap();
        
        // Verify the signature
        assert!(verify_signed_message_basic(&signed_request).is_ok());
        
        // Test that tampering breaks verification
        let mut tampered_request = signed_request.clone();
        tampered_request.payload.prompt = "Tampered prompt".to_string();
        assert!(verify_signed_message_basic(&tampered_request).is_err());
    }
}