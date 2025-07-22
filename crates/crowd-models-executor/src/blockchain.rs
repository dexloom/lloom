//! Blockchain integration for submitting usage records to the Ethereum smart contract.

use alloy::{
    contract::ContractInstance,
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    sol,
    transports::http::{Client, Http},
};
use anyhow::{Result, anyhow};
use crate::config::BlockchainConfig;
use crowd_models_core::{identity::Identity, protocol::UsageRecord};
use std::sync::Arc;
use tracing::{info, warn, error};

// Generate the contract interface using the sol! macro
sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    AccountingContract,
    "contracts/Accounting.sol"
);

/// Blockchain client for interacting with the Accounting smart contract
pub struct BlockchainClient {
    provider: Arc<dyn Provider<Http<Client>>>,
    contract: Option<ContractInstance<Http<Client>, Arc<dyn Provider<Http<Client>>>>>,
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
            .with_recommended_fillers()
            .wallet(identity.wallet.clone())
            .on_http(config.rpc_url.parse()?);
        
        let provider = Arc::new(provider);
        
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
        contract: &ContractInstance<Http<Client>, Arc<dyn Provider<Http<Client>>>>,
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
        contract: &ContractInstance<Http<Client>, Arc<dyn Provider<Http<Client>>>>,
        record: &UsageRecord,
    ) -> Result<String> {
        let call = contract.recordUsage(
            record.client_address,
            record.model.clone(),
            U256::from(record.token_count),
        );
        
        // Estimate gas and adjust gas price
        let mut call = call.gas_price(
            (self.provider.get_gas_price().await? * U256::from((self.config.gas_price_multiplier * 100.0) as u64)) / U256::from(100)
        );
        
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
    pub async fn get_executor_stats(&self) -> Result<u64> {
        let contract = self.contract.as_ref()
            .ok_or_else(|| anyhow!("Contract address not set"))?;
        
        let stats = contract.getExecutorStats(self.identity.evm_address).call().await?;
        Ok(stats.totalTokens.to::<u64>())
    }
    
    /// Check if the blockchain connection is working
    pub async fn health_check(&self) -> Result<()> {
        // Try to get the latest block number
        let block_number = self.provider.get_block_number().await?;
        info!("Blockchain health check passed: latest block {}", block_number);
        
        // If contract is set, try to call a view function
        if let Some(contract) = &self.contract {
            let network_stats = contract.getNetworkStats().call().await?;
            info!("Contract health check passed: total network tokens {}", 
                  network_stats.totalTokens);
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
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
}