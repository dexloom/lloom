//! Configuration management for the Executor node.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;

/// Configuration for the Executor node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    /// LLM backend configurations
    pub llm_backends: Vec<LlmBackendConfig>,
    
    /// Blockchain configuration
    pub blockchain: BlockchainConfig,
    
    /// P2P network configuration
    pub network: NetworkConfig,
}

/// Configuration for an LLM backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmBackendConfig {
    /// Name of the backend (e.g., "openai", "anthropic")
    pub name: String,
    
    /// API endpoint URL
    pub endpoint: String,
    
    /// API key (can be overridden by environment variable)
    pub api_key: Option<String>,
    
    /// Supported models on this backend
    pub supported_models: Vec<String>,
    
    /// Rate limit (requests per minute)
    pub rate_limit: Option<u32>,
}

/// Blockchain configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainConfig {
    /// RPC endpoint URL
    pub rpc_url: String,
    
    /// Accounting contract address
    pub contract_address: Option<String>,
    
    /// Gas price multiplier (1.0 = normal, 1.5 = 50% higher)
    pub gas_price_multiplier: f64,
    
    /// Batch submission interval in seconds
    pub batch_interval_secs: u64,
    
    /// Maximum batch size
    pub max_batch_size: usize,
}

/// P2P network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Port to listen on
    pub port: u16,
    
    /// External address (if behind NAT)
    pub external_address: Option<String>,
    
    /// Bootstrap nodes
    pub bootstrap_nodes: Vec<String>,
    
    /// How often to announce ourselves as an executor (seconds)
    pub announce_interval_secs: u64,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            llm_backends: vec![LlmBackendConfig {
                name: "openai".to_string(),
                endpoint: "https://api.openai.com/v1".to_string(),
                api_key: None,
                supported_models: vec![
                    "gpt-3.5-turbo".to_string(),
                    "gpt-4".to_string(),
                    "gpt-4-turbo".to_string(),
                ],
                rate_limit: Some(60),
            }],
            blockchain: BlockchainConfig {
                rpc_url: "https://rpc.sepolia.org".to_string(),
                contract_address: None,
                gas_price_multiplier: 1.2,
                batch_interval_secs: 300, // 5 minutes
                max_batch_size: 100,
            },
            network: NetworkConfig {
                port: 9001,
                external_address: None,
                bootstrap_nodes: vec![],
                announce_interval_secs: 300, // 5 minutes
            },
        }
    }
}

impl ExecutorConfig {
    /// Load configuration from a TOML file
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// Find a backend that supports the given model
    pub fn find_backend_for_model(&self, model: &str) -> Option<&LlmBackendConfig> {
        self.llm_backends.iter()
            .find(|backend| backend.supported_models.contains(&model.to_string()))
    }
    
    /// Get all supported models across all backends
    pub fn get_all_supported_models(&self) -> Vec<String> {
        self.llm_backends.iter()
            .flat_map(|backend| backend.supported_models.clone())
            .collect()
    }
}