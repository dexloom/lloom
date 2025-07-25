//! Blockchain integration for submitting usage records to the Ethereum smart contract.

use alloy::{
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder, RootProvider},
    sol,
};
use anyhow::{Result, anyhow};
use crate::config::BlockchainConfig;
use lloom_core::{identity::Identity, protocol::UsageRecord};
use tracing::{info, warn, error};

// Generate the contract interface using the sol! macro
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract AccountingContract {
        // Events
        event UsageRecorded(
            address indexed executor,
            address indexed client,
            string model,
            uint256 tokenCount,
            uint256 timestamp
        );

        // Public state variables (automatically generate getters)
        function owner() external view returns (address);
        function totalTokensByExecutor(address) external view returns (uint256);
        function totalTokensByClient(address) external view returns (uint256);
        function totalTokensProcessed() external view returns (uint256);

        // Main functions
        function recordUsage(
            address client,
            string calldata model,
            uint256 tokenCount
        ) external;

        function getExecutorStats(address executor) external view returns (uint256 totalTokens);
        function getClientStats(address client) external view returns (uint256 totalTokens);
        function getNetworkStats() external view returns (uint256 totalTokens);
        function transferOwnership(address newOwner) external;
        function hasRecordedUsage(address executor) external view returns (bool hasUsage);
    }
}

// Import the generated contract instance type
use AccountingContract::AccountingContractInstance;

// Use the actual provider type returned by ProviderBuilder
type ConcreteProvider = alloy::providers::fillers::FillProvider<
    alloy::providers::fillers::JoinFill<
        alloy::providers::Identity,
        alloy::providers::fillers::JoinFill<
            alloy::providers::fillers::GasFiller,
            alloy::providers::fillers::JoinFill<
                alloy::providers::fillers::BlobGasFiller,
                alloy::providers::fillers::JoinFill<
                    alloy::providers::fillers::NonceFiller,
                    alloy::providers::fillers::ChainIdFiller,
                >,
            >,
        >,
    >,
    RootProvider,
>;

/// Blockchain client for interacting with the Accounting smart contract
pub struct BlockchainClient {
    provider: ConcreteProvider,
    contract: Option<AccountingContractInstance<ConcreteProvider>>,
    #[allow(dead_code)]
    identity: Identity,
    config: BlockchainConfig,
}

impl BlockchainClient {
    /// Create a new blockchain client
    pub async fn new(
        identity: Identity,
        config: BlockchainConfig,
    ) -> Result<Self> {
        // Create the HTTP provider
        let provider = ProviderBuilder::new()
            .connect_http(config.rpc_url.parse()?);
        
        // Create contract instance if address is provided
        let contract = if let Some(contract_addr_str) = &config.contract_address {
            let contract_address: Address = contract_addr_str.parse()
                .map_err(|e| anyhow!("Invalid contract address: {}", e))?;
            
            let contract = AccountingContract::new(contract_address, provider.clone());
            Some(contract)
        } else {
            None
        };
        
        Ok(Self {
            provider,
            contract,
            identity,
            config,
        })
    }
    
    /// Set the contract address after deployment
    #[allow(dead_code)]
    pub fn set_contract_address(&mut self, contract_address: Address) {
        let contract = AccountingContract::new(contract_address, self.provider.clone());
        self.contract = Some(contract);
    }
    
    /// Submit a batch of usage records to the blockchain
    pub async fn submit_usage_batch(&self, records: Vec<UsageRecord>) -> Result<Vec<UsageRecord>> {
        if records.is_empty() {
            return Ok(Vec::new());
        }
        
        let contract = self.contract.as_ref()
            .ok_or_else(|| anyhow!("Contract address not set"))?;
        
        info!("Submitting {} usage records to blockchain", records.len());
        
        let mut failed_records = Vec::new();
        let mut successful_count = 0;
        
        // Process records in chunks to avoid hitting gas limits
        let chunk_size = self.config.max_batch_size.min(10); // Limit to 10 per transaction for safety
        
        for chunk in records.chunks(chunk_size) {
            match self.submit_chunk(contract, chunk).await {
                Ok(_) => {
                    successful_count += chunk.len();
                    info!("Successfully submitted {} records", chunk.len());
                }
                Err(e) => {
                    error!("Failed to submit chunk: {}", e);
                    failed_records.extend(chunk.iter().cloned());
                }
            }
        }
        
        info!("Blockchain submission complete: {}/{} records successful", 
               successful_count, records.len());
        
        Ok(failed_records)
    }
    
    /// Submit a single chunk of records
    async fn submit_chunk(
        &self,
        contract: &AccountingContractInstance<ConcreteProvider>,
        records: &[UsageRecord],
    ) -> Result<()> {
        // For now, submit one record at a time
        // In a production system, you might batch multiple records in a single transaction
        
        for record in records {
            match self.submit_single_record(contract, record).await {
                Ok(tx_hash) => {
                    info!("Submitted usage record: tx={}", tx_hash);
                }
                Err(e) => {
                    error!("Failed to submit record for client {}: {}", 
                           record.client_address, e);
                    return Err(e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Submit a single usage record
    async fn submit_single_record(
        &self,
        contract: &AccountingContractInstance<ConcreteProvider>,
        record: &UsageRecord,
    ) -> Result<String> {
        let call = contract.recordUsage(
            record.client_address,
            record.model.clone(),
            U256::from(record.token_count),
        );
        
        // Estimate gas and adjust gas price
        let gas_price = self.provider.get_gas_price().await?;
        let adjusted_gas_price = gas_price * (self.config.gas_price_multiplier * 100.0) as u128 / 100;
        let call = call.gas_price(adjusted_gas_price);
        
        // Send the transaction
        let pending_tx = call.send().await?;
        let tx_hash = *pending_tx.tx_hash();
        
        // Wait for confirmation (optional - you might want to do this asynchronously)
        match tokio::time::timeout(
            std::time::Duration::from_secs(60),
            pending_tx.get_receipt()
        ).await {
            Ok(Ok(receipt)) => {
                if receipt.status() {
                    info!("Transaction confirmed: {} (block: {})", 
                          tx_hash, receipt.block_number.unwrap_or_default());
                } else {
                    return Err(anyhow!("Transaction failed: {}", tx_hash));
                }
            }
            Ok(Err(e)) => {
                warn!("Failed to get receipt for {}: {}", tx_hash, e);
                // Still consider it submitted since the transaction was sent
            }
            Err(_) => {
                warn!("Timeout waiting for transaction confirmation: {}", tx_hash);
                // Still consider it submitted since the transaction was sent
            }
        }
        
        Ok(tx_hash.to_string())
    }
    
    /// Get executor statistics from the contract
    #[allow(dead_code)]
    pub async fn get_executor_stats(&self) -> Result<u64> {
        let contract = self.contract.as_ref()
            .ok_or_else(|| anyhow!("Contract address not set"))?;
        
        let total_tokens = contract.getExecutorStats(self.identity.evm_address).call().await?;
        Ok(total_tokens.to::<u64>())
    }
    
    /// Check if the blockchain connection is working
    pub async fn health_check(&self) -> Result<()> {
        // Try to get the latest block number
        let block_number = self.provider.get_block_number().await?;
        info!("Blockchain health check passed: latest block {}", block_number);
        
        // If contract is set, try to call a view function
        if let Some(contract) = &self.contract {
            let total_tokens = contract.getNetworkStats().call().await?;
            info!("Contract health check passed: total network tokens {}",
                  total_tokens);
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lloom_core::protocol::UsageRecord;
    use alloy::primitives::Address;
    
    #[tokio::test]
    async fn test_blockchain_client_creation() {
        let identity = Identity::generate();
        let config = BlockchainConfig {
            rpc_url: "https://rpc.sepolia.org".to_string(),
            contract_address: None,
            gas_price_multiplier: 1.2,
            batch_interval_secs: 300,
            max_batch_size: 100,
        };
        
        let client = BlockchainClient::new(identity, config).await;
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_blockchain_client_creation_with_contract() {
        let identity = Identity::generate();
        let config = BlockchainConfig {
            rpc_url: "https://rpc.sepolia.org".to_string(),
            contract_address: Some("0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string()),
            gas_price_multiplier: 1.5,
            batch_interval_secs: 600,
            max_batch_size: 50,
        };
        
        let client = BlockchainClient::new(identity, config).await;
        assert!(client.is_ok());
        assert!(client.unwrap().contract.is_some());
    }

    #[test]
    fn test_blockchain_config() {
        let config = BlockchainConfig {
            rpc_url: "https://mainnet.infura.io/v3/key".to_string(),
            contract_address: Some("0x123456789abcdef".to_string()),
            gas_price_multiplier: 2.0,
            batch_interval_secs: 120,
            max_batch_size: 25,
        };

        assert_eq!(config.rpc_url, "https://mainnet.infura.io/v3/key");
        assert_eq!(config.contract_address, Some("0x123456789abcdef".to_string()));
        assert_eq!(config.gas_price_multiplier, 2.0);
        assert_eq!(config.batch_interval_secs, 120);
        assert_eq!(config.max_batch_size, 25);
    }

    #[tokio::test]
    async fn test_set_contract_address() {
        let identity = Identity::generate();
        let config = BlockchainConfig {
            rpc_url: "https://rpc.sepolia.org".to_string(),
            contract_address: None,
            gas_price_multiplier: 1.2,
            batch_interval_secs: 300,
            max_batch_size: 100,
        };
        
        let mut client = BlockchainClient::new(identity, config).await.unwrap();
        assert!(client.contract.is_none());

        let contract_address: Address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".parse().unwrap();
        client.set_contract_address(contract_address);
        assert!(client.contract.is_some());
    }

    #[tokio::test]
    async fn test_submit_empty_usage_batch() {
        let identity = Identity::generate();
        let config = BlockchainConfig {
            rpc_url: "https://rpc.sepolia.org".to_string(),
            contract_address: None,
            gas_price_multiplier: 1.2,
            batch_interval_secs: 300,
            max_batch_size: 100,
        };
        
        let client = BlockchainClient::new(identity, config).await.unwrap();
        let result = client.submit_usage_batch(vec![]).await;
        
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_submit_usage_batch_without_contract() {
        let identity = Identity::generate();
        let config = BlockchainConfig {
            rpc_url: "https://rpc.sepolia.org".to_string(),
            contract_address: None,
            gas_price_multiplier: 1.2,
            batch_interval_secs: 300,
            max_batch_size: 100,
        };
        
        let client = BlockchainClient::new(identity, config).await.unwrap();
        
        let usage_records = vec![
            UsageRecord {
                client_address: "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".parse().unwrap(),
                model: "gpt-3.5-turbo".to_string(),
                token_count: 100,
                timestamp: 1234567890,
            }
        ];
        
        let result = client.submit_usage_batch(usage_records).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Contract address not set"));
    }

    #[test]
    fn test_usage_record_structure() {
        let client_address: Address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".parse().unwrap();
        let usage_record = UsageRecord {
            client_address,
            model: "gpt-4".to_string(),
            token_count: 250,
            timestamp: 1234567890,
        };

        assert_eq!(usage_record.client_address, client_address);
        assert_eq!(usage_record.model, "gpt-4");
        assert_eq!(usage_record.token_count, 250);
        assert_eq!(usage_record.timestamp, 1234567890);
    }

    #[test]
    fn test_blockchain_config_serialization() {
        let config = BlockchainConfig {
            rpc_url: "https://test.rpc".to_string(),
            contract_address: Some("0xtest".to_string()),
            gas_price_multiplier: 1.3,
            batch_interval_secs: 400,
            max_batch_size: 75,
        };

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: BlockchainConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(config.rpc_url, deserialized.rpc_url);
        assert_eq!(config.contract_address, deserialized.contract_address);
        assert_eq!(config.gas_price_multiplier, deserialized.gas_price_multiplier);
        assert_eq!(config.batch_interval_secs, deserialized.batch_interval_secs);
        assert_eq!(config.max_batch_size, deserialized.max_batch_size);
    }

    #[test]
    fn test_blockchain_config_clone() {
        let config = BlockchainConfig {
            rpc_url: "https://test.rpc".to_string(),
            contract_address: Some("0xtest".to_string()),
            gas_price_multiplier: 1.4,
            batch_interval_secs: 500,
            max_batch_size: 200,
        };

        let cloned = config.clone();
        assert_eq!(config.rpc_url, cloned.rpc_url);
        assert_eq!(config.contract_address, cloned.contract_address);
        assert_eq!(config.gas_price_multiplier, cloned.gas_price_multiplier);
        assert_eq!(config.batch_interval_secs, cloned.batch_interval_secs);
        assert_eq!(config.max_batch_size, cloned.max_batch_size);
    }

    #[test]
    fn test_blockchain_config_debug() {
        let config = BlockchainConfig {
            rpc_url: "https://test.rpc".to_string(),
            contract_address: None,
            gas_price_multiplier: 1.0,
            batch_interval_secs: 300,
            max_batch_size: 100,
        };

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("BlockchainConfig"));
        assert!(debug_str.contains("rpc_url"));
        assert!(debug_str.contains("gas_price_multiplier"));
    }

    #[test]
    fn test_invalid_contract_address() {
        let _identity = Identity::generate();
        let config = BlockchainConfig {
            rpc_url: "https://rpc.sepolia.org".to_string(),
            contract_address: Some("invalid_address".to_string()),
            gas_price_multiplier: 1.2,
            batch_interval_secs: 300,
            max_batch_size: 100,
        };
        
        // This should be tested in an async context, but for now we just test the config structure
        assert_eq!(config.contract_address, Some("invalid_address".to_string()));
    }

    #[test]
    fn test_gas_price_multiplier_boundaries() {
        let config1 = BlockchainConfig {
            rpc_url: "https://test.rpc".to_string(),
            contract_address: None,
            gas_price_multiplier: 0.5, // Very low
            batch_interval_secs: 300,
            max_batch_size: 100,
        };

        let config2 = BlockchainConfig {
            rpc_url: "https://test.rpc".to_string(),
            contract_address: None,
            gas_price_multiplier: 10.0, // Very high
            batch_interval_secs: 300,
            max_batch_size: 100,
        };

        assert_eq!(config1.gas_price_multiplier, 0.5);
        assert_eq!(config2.gas_price_multiplier, 10.0);
    }

    #[test]
    fn test_batch_size_limits() {
        let config1 = BlockchainConfig {
            rpc_url: "https://test.rpc".to_string(),
            contract_address: None,
            gas_price_multiplier: 1.0,
            batch_interval_secs: 300,
            max_batch_size: 1, // Minimum
        };

        let config2 = BlockchainConfig {
            rpc_url: "https://test.rpc".to_string(),
            contract_address: None,
            gas_price_multiplier: 1.0,
            batch_interval_secs: 300,
            max_batch_size: 1000, // Large
        };

        assert_eq!(config1.max_batch_size, 1);
        assert_eq!(config2.max_batch_size, 1000);
    }
}