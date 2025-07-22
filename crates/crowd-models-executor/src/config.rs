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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_default_executor_config() {
        let config = ExecutorConfig::default();
        
        assert_eq!(config.llm_backends.len(), 1);
        assert_eq!(config.llm_backends[0].name, "openai");
        assert_eq!(config.llm_backends[0].endpoint, "https://api.openai.com/v1");
        assert_eq!(config.llm_backends[0].supported_models.len(), 3);
        
        assert_eq!(config.blockchain.rpc_url, "https://rpc.sepolia.org");
        assert_eq!(config.blockchain.gas_price_multiplier, 1.2);
        assert_eq!(config.blockchain.batch_interval_secs, 300);
        assert_eq!(config.blockchain.max_batch_size, 100);
        
        assert_eq!(config.network.port, 9001);
        assert_eq!(config.network.announce_interval_secs, 300);
        assert!(config.network.bootstrap_nodes.is_empty());
    }

    #[test]
    fn test_llm_backend_config() {
        let backend = LlmBackendConfig {
            name: "test-backend".to_string(),
            endpoint: "https://api.test.com/v1".to_string(),
            api_key: Some("test-key".to_string()),
            supported_models: vec!["model1".to_string(), "model2".to_string()],
            rate_limit: Some(100),
        };

        assert_eq!(backend.name, "test-backend");
        assert_eq!(backend.endpoint, "https://api.test.com/v1");
        assert_eq!(backend.api_key, Some("test-key".to_string()));
        assert_eq!(backend.supported_models.len(), 2);
        assert_eq!(backend.rate_limit, Some(100));
    }

    #[test]
    fn test_blockchain_config() {
        let blockchain_config = BlockchainConfig {
            rpc_url: "https://mainnet.infura.io/v3/key".to_string(),
            contract_address: Some("0x123...".to_string()),
            gas_price_multiplier: 1.5,
            batch_interval_secs: 600,
            max_batch_size: 50,
        };

        assert_eq!(blockchain_config.rpc_url, "https://mainnet.infura.io/v3/key");
        assert_eq!(blockchain_config.contract_address, Some("0x123...".to_string()));
        assert_eq!(blockchain_config.gas_price_multiplier, 1.5);
        assert_eq!(blockchain_config.batch_interval_secs, 600);
        assert_eq!(blockchain_config.max_batch_size, 50);
    }

    #[test]
    fn test_network_config() {
        let network_config = NetworkConfig {
            port: 8080,
            external_address: Some("/ip4/1.2.3.4/tcp/8080".to_string()),
            bootstrap_nodes: vec!["/ip4/5.6.7.8/tcp/9000".to_string()],
            announce_interval_secs: 120,
        };

        assert_eq!(network_config.port, 8080);
        assert_eq!(network_config.external_address, Some("/ip4/1.2.3.4/tcp/8080".to_string()));
        assert_eq!(network_config.bootstrap_nodes.len(), 1);
        assert_eq!(network_config.announce_interval_secs, 120);
    }

    #[test]
    fn test_find_backend_for_model() {
        let config = ExecutorConfig::default();
        
        // Should find the OpenAI backend for supported models
        let backend = config.find_backend_for_model("gpt-3.5-turbo");
        assert!(backend.is_some());
        assert_eq!(backend.unwrap().name, "openai");
        
        let backend = config.find_backend_for_model("gpt-4");
        assert!(backend.is_some());
        assert_eq!(backend.unwrap().name, "openai");
        
        // Should not find backend for unsupported model
        let backend = config.find_backend_for_model("unsupported-model");
        assert!(backend.is_none());
    }

    #[test]
    fn test_get_all_supported_models() {
        let config = ExecutorConfig::default();
        let models = config.get_all_supported_models();
        
        assert!(models.contains(&"gpt-3.5-turbo".to_string()));
        assert!(models.contains(&"gpt-4".to_string()));
        assert!(models.contains(&"gpt-4-turbo".to_string()));
        assert_eq!(models.len(), 3);
    }

    #[test]
    fn test_multiple_backends() {
        let config = ExecutorConfig {
            llm_backends: vec![
                LlmBackendConfig {
                    name: "openai".to_string(),
                    endpoint: "https://api.openai.com/v1".to_string(),
                    api_key: None,
                    supported_models: vec!["gpt-3.5-turbo".to_string()],
                    rate_limit: Some(60),
                },
                LlmBackendConfig {
                    name: "anthropic".to_string(),
                    endpoint: "https://api.anthropic.com/v1".to_string(),
                    api_key: None,
                    supported_models: vec!["claude-3".to_string()],
                    rate_limit: Some(50),
                },
            ],
            blockchain: BlockchainConfig {
                rpc_url: "https://rpc.sepolia.org".to_string(),
                contract_address: None,
                gas_price_multiplier: 1.2,
                batch_interval_secs: 300,
                max_batch_size: 100,
            },
            network: NetworkConfig {
                port: 9001,
                external_address: None,
                bootstrap_nodes: vec![],
                announce_interval_secs: 300,
            },
        };

        // Should find OpenAI backend for GPT models
        let backend = config.find_backend_for_model("gpt-3.5-turbo");
        assert!(backend.is_some());
        assert_eq!(backend.unwrap().name, "openai");
        
        // Should find Anthropic backend for Claude models
        let backend = config.find_backend_for_model("claude-3");
        assert!(backend.is_some());
        assert_eq!(backend.unwrap().name, "anthropic");
        
        // Should get all supported models from all backends
        let models = config.get_all_supported_models();
        assert_eq!(models.len(), 2);
        assert!(models.contains(&"gpt-3.5-turbo".to_string()));
        assert!(models.contains(&"claude-3".to_string()));
    }

    #[test]
    fn test_config_from_file() -> Result<(), Box<dyn std::error::Error>> {
        let toml_content = r#"
[network]
port = 8080
external_address = "/ip4/1.2.3.4/tcp/8080"
bootstrap_nodes = ["/ip4/5.6.7.8/tcp/9000"]
announce_interval_secs = 120

[blockchain]
rpc_url = "https://mainnet.infura.io/v3/key"
contract_address = "0x123456"
gas_price_multiplier = 1.5
batch_interval_secs = 600
max_batch_size = 50

[[llm_backends]]
name = "openai"
endpoint = "https://api.openai.com/v1"
api_key = "test-key"
supported_models = ["gpt-3.5-turbo", "gpt-4"]
rate_limit = 60

[[llm_backends]]
name = "anthropic"
endpoint = "https://api.anthropic.com/v1"
supported_models = ["claude-3"]
rate_limit = 50
"#;

        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "{}", toml_content)?;
        
        let config = ExecutorConfig::from_file(temp_file.path().to_str().unwrap())?;
        
        assert_eq!(config.network.port, 8080);
        assert_eq!(config.network.external_address, Some("/ip4/1.2.3.4/tcp/8080".to_string()));
        assert_eq!(config.network.bootstrap_nodes.len(), 1);
        
        assert_eq!(config.blockchain.rpc_url, "https://mainnet.infura.io/v3/key");
        assert_eq!(config.blockchain.contract_address, Some("0x123456".to_string()));
        assert_eq!(config.blockchain.gas_price_multiplier, 1.5);
        
        assert_eq!(config.llm_backends.len(), 2);
        assert_eq!(config.llm_backends[0].name, "openai");
        assert_eq!(config.llm_backends[0].api_key, Some("test-key".to_string()));
        assert_eq!(config.llm_backends[1].name, "anthropic");
        assert!(config.llm_backends[1].api_key.is_none());
        
        Ok(())
    }

    #[test]
    fn test_config_serialization() {
        let config = ExecutorConfig::default();
        
        // Test that config can be serialized and deserialized
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: ExecutorConfig = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(config.llm_backends[0].name, deserialized.llm_backends[0].name);
        assert_eq!(config.blockchain.rpc_url, deserialized.blockchain.rpc_url);
        assert_eq!(config.network.port, deserialized.network.port);
    }

    #[test]
    fn test_config_clone() {
        let config = ExecutorConfig::default();
        let cloned = config.clone();
        
        assert_eq!(config.llm_backends[0].name, cloned.llm_backends[0].name);
        assert_eq!(config.blockchain.rpc_url, cloned.blockchain.rpc_url);
        assert_eq!(config.network.port, cloned.network.port);
    }

    #[test]
    fn test_config_debug() {
        let config = ExecutorConfig::default();
        let debug_str = format!("{:?}", config);
        
        assert!(debug_str.contains("ExecutorConfig"));
        assert!(debug_str.contains("llm_backends"));
        assert!(debug_str.contains("blockchain"));
        assert!(debug_str.contains("network"));
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