//! Protocol definitions for the Crowd Models P2P network.
//! 
//! This module defines the message types and data structures used for
//! communication between nodes in the network.

use serde::{Deserialize, Serialize};
use alloy::primitives::Address;
use crate::signing::{SignedMessage, SignableMessage};

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
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

/// Type aliases for commonly used signed messages
pub type SignedLlmRequest = SignedMessage<LlmRequest>;
pub type SignedLlmResponse = SignedMessage<LlmResponse>;
pub type SignedUsageRecord = SignedMessage<UsageRecord>;

/// Wrapper enum for request messages to support both signed and unsigned variants
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RequestMessage {
    /// Unsigned LLM request (for backwards compatibility)
    LlmRequest(LlmRequest),
    /// Signed LLM request (with cryptographic signature)
    SignedLlmRequest(SignedLlmRequest),
}

/// Wrapper enum for response messages to support both signed and unsigned variants
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ResponseMessage {
    /// Unsigned LLM response (for backwards compatibility)
    LlmResponse(LlmResponse),
    /// Signed LLM response (with cryptographic signature)
    SignedLlmResponse(SignedLlmResponse),
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
        };

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.prompt, "Test prompt");
        assert!(request.system_prompt.is_none());
        assert!(request.temperature.is_none());
        assert!(request.max_tokens.is_none());
    }

    #[test]
    fn test_llm_response_success() {
        let response = LlmResponse {
            content: "Generated content".to_string(),
            token_count: 42,
            model_used: "gpt-3.5-turbo".to_string(),
            error: None,
        };

        assert_eq!(response.content, "Generated content");
        assert_eq!(response.token_count, 42);
        assert_eq!(response.model_used, "gpt-3.5-turbo");
        assert!(response.error.is_none());
    }

    #[test]
    fn test_llm_response_error() {
        let response = LlmResponse {
            content: String::new(),
            token_count: 0,
            model_used: "gpt-4".to_string(),
            error: Some("API rate limit exceeded".to_string()),
        };

        assert!(response.content.is_empty());
        assert_eq!(response.token_count, 0);
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
        let accountant_key = ServiceRole::Accountant.to_kad_key();

        assert_eq!(executor_key, b"crowd-models/executor".to_vec());
        assert_eq!(accountant_key, b"crowd-models/accountant".to_vec());
        assert_ne!(executor_key, accountant_key);
    }

    #[test]
    fn test_service_role_equality() {
        assert_eq!(ServiceRole::Executor, ServiceRole::Executor);
        assert_eq!(ServiceRole::Accountant, ServiceRole::Accountant);
        assert_ne!(ServiceRole::Executor, ServiceRole::Accountant);
    }

    #[test]
    fn test_serialization_llm_request() {
        let request = LlmRequest {
            model: "gpt-3.5-turbo".to_string(),
            prompt: "Test".to_string(),
            system_prompt: Some("System".to_string()),
            temperature: Some(0.5),
            max_tokens: Some(100),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: LlmRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(request.model, deserialized.model);
        assert_eq!(request.prompt, deserialized.prompt);
        assert_eq!(request.system_prompt, deserialized.system_prompt);
        assert_eq!(request.temperature, deserialized.temperature);
        assert_eq!(request.max_tokens, deserialized.max_tokens);
    }

    #[test]
    fn test_serialization_llm_response() {
        let response = LlmResponse {
            content: "Response content".to_string(),
            token_count: 25,
            model_used: "gpt-4".to_string(),
            error: None,
        };

        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: LlmResponse = serde_json::from_str(&serialized).unwrap();

        assert_eq!(response.content, deserialized.content);
        assert_eq!(response.token_count, deserialized.token_count);
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
        assert_eq!(constants::LLM_PROTOCOL, "/crowd-models/llm/1.0.0");
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
            token_count: 15,
            model_used: "gpt-3.5-turbo".to_string(),
            error: Some("Test error".to_string()),
        };

        let cloned = original.clone();
        assert_eq!(original.content, cloned.content);
        assert_eq!(original.token_count, cloned.token_count);
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
            token_count: 10,
            model_used: "gpt-4".to_string(),
            error: None,
        };

        let signed_response = response.sign_blocking(&signer).unwrap();
        
        assert_eq!(signed_response.payload.content, "Test response");
        assert_eq!(signed_response.payload.token_count, 10);
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
        };

        // Test that the type alias works
        let signed_request: SignedLlmRequest = request.sign_blocking(&signer).unwrap();
        assert_eq!(signed_request.payload.prompt, "Type alias test");

        let response = LlmResponse {
            content: "Response content".to_string(),
            token_count: 15,
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