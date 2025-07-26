//! # Lloom Executor Library
//!
//! This library provides reusable components for building LLM service executor nodes
//! in the Lloom P2P network. It includes configuration management, LLM client abstractions,
//! blockchain integration, and request processing utilities.
//!
//! ## Usage
//!
//! ### As a Library
//!
//! Use this crate to build custom executor implementations:
//!
//! ```rust,no_run
//! use lloom_executor::{ExecutorConfig, LlmClient};
//! use lloom_executor::processing::RequestProcessor;
//!
//! // Load configuration
//! let config = ExecutorConfig::from_file("config.toml").unwrap();
//!
//! // Initialize LLM client
//! let llm_client = LlmClient::new(config.llm_backends[0].clone()).unwrap();
//!
//! // Create request processor
//! let processor = RequestProcessor::new(config, vec![("openai".to_string(), llm_client)]);
//! ```
//!
//! ### As a Binary
//!
//! Run the included binary for a complete executor node:
//!
//! ```bash
//! lloom-executor --config config.toml --bootstrap-nodes /ip4/127.0.0.1/tcp/9000
//! ```

pub mod config;
pub mod llm_client;
pub mod blockchain;

/// Request processing and response utilities
pub mod processing {
    use std::collections::HashMap;
    use crate::{config::ExecutorConfig, llm_client::LlmClient};
    use lloom_core::protocol::{LlmRequest, LlmResponse};
    use alloy::primitives::Address;

    /// High-level request processor for handling LLM requests
    pub struct RequestProcessor {
        config: ExecutorConfig,
        llm_clients: HashMap<String, LlmClient>,
    }

    impl RequestProcessor {
        /// Create a new request processor
        pub fn new(config: ExecutorConfig, llm_clients: Vec<(String, LlmClient)>) -> Self {
            let clients_map = llm_clients.into_iter().collect();
            Self {
                config,
                llm_clients: clients_map,
            }
        }

        /// Process an LLM request and return a response
        pub async fn process_request(
            &self,
            request: LlmRequest,
            _verified_signer: Option<Address>,
        ) -> Result<LlmResponse, anyhow::Error> {
            // Find the appropriate backend for this model
            let backend_name = match self.config.find_backend_for_model(&request.model) {
                Some(backend) => backend.name.clone(),
                None => {
                    return Ok(LlmResponse {
                        content: String::new(),
                        inbound_tokens: 0,
                        outbound_tokens: 0,
                        total_cost: "0".to_string(),
                        model_used: request.model.clone(),
                        error: Some(format!("Model {} not supported", request.model)),
                    });
                }
            };

            // Get the LLM client
            let llm_client = match self.llm_clients.get(&backend_name) {
                Some(client) => client,
                None => {
                    return Ok(LlmResponse {
                        content: String::new(),
                        inbound_tokens: 0,
                        outbound_tokens: 0,
                        total_cost: "0".to_string(),
                        model_used: request.model.clone(),
                        error: Some(format!("Backend {} not available", backend_name)),
                    });
                }
            };

            // Execute the LLM request
            match llm_client.lmstudio_chat_completion(
                &request.model,
                &request.prompt,
                request.system_prompt.as_deref(),
                request.temperature,
                request.max_tokens,
            ).await {
                Ok((content, token_count, _stats, _model_info)) => {
                    Ok(LlmResponse {
                        content,
                        inbound_tokens: (token_count / 2) as u64,  // Rough estimate
                        outbound_tokens: (token_count / 2) as u64,
                        total_cost: format!("{}", (token_count as u64) * 1000000000000000u64),
                        model_used: request.model.clone(),
                        error: None,
                    })
                }
                Err(e) => {
                    Ok(LlmResponse {
                        content: String::new(),
                        inbound_tokens: 0,
                        outbound_tokens: 0,
                        total_cost: "0".to_string(),
                        model_used: request.model,
                        error: Some(e.to_string()),
                    })
                }
            }
        }

        /// Get available models from all configured backends
        pub fn get_available_models(&self) -> Vec<String> {
            self.config.get_all_supported_models()
        }
    }
}

/// Utilities for executor operations
pub mod utils {
    use std::collections::HashMap;
    use crate::{config::LlmBackendConfig, llm_client::LlmClient};
    use anyhow::Result;

    /// Initialize LLM clients from configuration
    pub async fn initialize_llm_clients(
        backends: &mut [LlmBackendConfig]
    ) -> Result<HashMap<String, LlmClient>> {
        let mut llm_clients = HashMap::new();
        
        for backend_config in backends {
            match LlmClient::new(backend_config.clone()) {
                Ok(client) => {
                    // For LMStudio backends, try to discover available models
                    if client.is_lmstudio_backend() {
                        if let Ok(discovered_models) = client.discover_lmstudio_models().await {
                            if !discovered_models.is_empty() {
                                // Update the backend config with discovered models if none were specified
                                if backend_config.supported_models.is_empty() ||
                                   backend_config.supported_models == vec!["llama-2-7b-chat", "mistral-7b-instruct", "your-loaded-model"] {
                                    backend_config.supported_models = discovered_models;
                                }
                            }
                        }
                    }
                    
                    llm_clients.insert(backend_config.name.clone(), client);
                }
                Err(_e) => {
                    // Skip failed clients
                    continue;
                }
            }
        }
        
        if llm_clients.is_empty() {
            return Err(anyhow::anyhow!("No LLM clients could be initialized"));
        }
        
        Ok(llm_clients)
    }
}

// Re-export commonly used types for convenience
pub use config::{ExecutorConfig, LlmBackendConfig, BlockchainConfig, NetworkConfig};
pub use llm_client::{LlmClient, ModelInfo};
pub use processing::RequestProcessor;