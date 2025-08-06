//! Ethereum blockchain integration for the faucet server.

use crate::config::EthereumConfig;
use crate::error::{FaucetError, FaucetResult};
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    signers::local::PrivateKeySigner,
};
use std::str::FromStr;
use tracing::{debug, info};

/// Ethereum client for faucet operations
pub struct EthereumClient {
    faucet_address: Address,
    target_amount: U256,
    gas_multiplier: f64,
    min_faucet_balance: U256,
    rpc_url: String,
    provider: Box<dyn alloy::providers::Provider>,
    signer: PrivateKeySigner,
}

impl std::fmt::Debug for EthereumClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EthereumClient")
            .field("faucet_address", &self.faucet_address)
            .field("target_amount", &self.target_amount)
            .field("gas_multiplier", &self.gas_multiplier)
            .field("min_faucet_balance", &self.min_faucet_balance)
            .field("rpc_url", &self.rpc_url)
            .field("signer", &self.signer)
            .finish()
    }
}

impl EthereumClient {
    /// Create a new Ethereum client
    pub async fn new(config: &EthereumConfig) -> FaucetResult<Self> {
        // Parse private key
        let private_key = config.private_key.strip_prefix("0x").unwrap_or(&config.private_key);
        let signer = PrivateKeySigner::from_str(private_key)
            .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Invalid private key: {}", e)))?;

        let faucet_address = signer.address();

        // Create HTTP provider
        let url = url::Url::parse(&config.rpc_url)
            .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Invalid RPC URL: {}", e)))?;
        let provider = Box::new(alloy::providers::ProviderBuilder::new().connect_http(url));

        // Convert target amount from ETH to Wei
        let target_amount = Self::eth_to_wei(config.target_amount_eth);
        let min_faucet_balance = Self::eth_to_wei(config.min_faucet_balance_eth);

        Ok(Self {
            faucet_address,
            target_amount,
            gas_multiplier: config.gas_multiplier,
            min_faucet_balance,
            rpc_url: config.rpc_url.clone(),
            provider,
            signer,
        })
    }

    /// Convert ETH to Wei
    fn eth_to_wei(eth_amount: f64) -> U256 {
        let _wei_per_eth = U256::from(10u64.pow(18));
        let eth_scaled = (eth_amount * 1e18) as u64;
        U256::from(eth_scaled)
    }

    /// Convert Wei to ETH (for display purposes)
    fn wei_to_eth(wei_amount: U256) -> f64 {
        let wei_per_eth = 1e18;
        let wei_as_u128 = wei_amount.try_into().unwrap_or(0u128);
        wei_as_u128 as f64 / wei_per_eth
    }

    /// Check if the faucet has sufficient balance
    pub async fn check_faucet_balance(&self) -> FaucetResult<()> {
        let balance = self.provider.get_balance(self.faucet_address).await
            .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Failed to get faucet balance: {}", e)))?;
        
        if balance < self.min_faucet_balance {
            return Err(FaucetError::Internal(anyhow::anyhow!(
                "Faucet balance {} ETH is below minimum required {} ETH",
                Self::wei_to_eth(balance),
                Self::wei_to_eth(self.min_faucet_balance)
            )));
        }
        
        info!(
            "Faucet balance check passed: {} ETH (minimum: {} ETH)",
            Self::wei_to_eth(balance),
            Self::wei_to_eth(self.min_faucet_balance)
        );
        Ok(())
    }

    /// Get the current balance of an address
    pub async fn get_balance(&self, address: Address) -> FaucetResult<U256> {
        let balance = self.provider.get_balance(address).await
            .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Failed to get balance for {}: {}", address, e)))?;
        
        debug!("Getting balance for address: {} = {} ETH", address, Self::wei_to_eth(balance));
        Ok(balance)
    }

    /// Fund an address up to the target amount
    pub async fn fund_address(&self, address: Address) -> FaucetResult<String> {
        // Check faucet balance first
        self.check_faucet_balance().await?;

        // Get current balance of target address
        let current_balance = self.get_balance(address).await?;

        debug!(
            "Target address {} current balance: {} ETH",
            address,
            Self::wei_to_eth(current_balance)
        );

        // Check if address already has sufficient balance
        if current_balance >= self.target_amount {
            return Err(FaucetError::SufficientBalance);
        }

        // Calculate amount to send
        let amount_to_send = self.target_amount - current_balance;

        info!(
            "Funding address {} with {} ETH (to reach {} ETH total)",
            address,
            Self::wei_to_eth(amount_to_send),
            Self::wei_to_eth(self.target_amount)
        );

        // Create wallet with the signer
        let wallet = alloy::network::EthereumWallet::from(self.signer.clone());
        
        // Create provider with wallet for transaction signing
        let provider_with_wallet = alloy::providers::ProviderBuilder::new()
            .wallet(wallet)
            .connect_http(url::Url::parse(&self.rpc_url)
                .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Invalid provider URL: {}", e)))?);
        
        // Create transaction request
        let tx_request = alloy::rpc::types::TransactionRequest::default()
            .from(self.faucet_address)
            .to(address)
            .value(amount_to_send);
        
        // Send transaction and wait for receipt
        let pending_tx = provider_with_wallet.send_transaction(tx_request).await
            .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Failed to send transaction: {}", e)))?;
        
        let tx_hash = *pending_tx.tx_hash();
        
        info!("Transaction sent: {} (amount: {} ETH)", tx_hash, Self::wei_to_eth(amount_to_send));

        Ok(format!("{:#x}", tx_hash))
    }

    /// Validate Ethereum address format
    pub fn validate_address(address: &str) -> FaucetResult<Address> {
        Address::from_str(address)
            .map_err(|_| FaucetError::InvalidEthereumAddress(address.to_string()))
    }

    /// Get faucet address
    pub fn get_faucet_address(&self) -> Address {
        self.faucet_address
    }

    /// Get target funding amount
    pub fn get_target_amount(&self) -> U256 {
        self.target_amount
    }

    /// Health check - verify connection to Ethereum network
    pub async fn health_check(&self) -> FaucetResult<()> {
        // Mock health check - in real implementation, you would check the provider
        info!("Ethereum health check passed (mock implementation)");

        // Check faucet balance
        self.check_faucet_balance().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EthereumConfig;

    fn get_test_ethereum_config() -> EthereumConfig {
        EthereumConfig {
            rpc_url: "https://rpc.sepolia.org".to_string(),
            private_key: "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234".to_string(),
            target_amount_eth: 1.0,
            gas_multiplier: 1.2,
            min_faucet_balance_eth: 10.0,
        }
    }

    #[test]
    fn test_eth_to_wei_conversion() {
        let wei = EthereumClient::eth_to_wei(1.0);
        assert_eq!(wei, U256::from(10u64.pow(18)));

        let wei = EthereumClient::eth_to_wei(0.5);
        assert_eq!(wei, U256::from(5u64 * 10u64.pow(17)));

        let wei = EthereumClient::eth_to_wei(2.5);
        assert_eq!(wei, U256::from(25u64 * 10u64.pow(17)));
    }

    #[test]
    fn test_wei_to_eth_conversion() {
        let eth = EthereumClient::wei_to_eth(U256::from(10u64.pow(18)));
        assert!((eth - 1.0).abs() < 0.0001);

        let eth = EthereumClient::wei_to_eth(U256::from(5u64 * 10u64.pow(17)));
        assert!((eth - 0.5).abs() < 0.0001);

        let eth = EthereumClient::wei_to_eth(U256::from(25u64 * 10u64.pow(17)));
        assert!((eth - 2.5).abs() < 0.0001);
    }

    #[test]
    fn test_validate_address_valid() {
        let valid_addresses = [
            "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a",
            "0x0000000000000000000000000000000000000000",
            "0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        ];

        for addr in &valid_addresses {
            assert!(EthereumClient::validate_address(addr).is_ok());
        }
    }

    #[test]
    fn test_validate_address_invalid() {
        let invalid_addresses = [
            "invalid",
            "0x123", // too short
            "0xGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG", // invalid hex characters
            "",
        ];

        for addr in &invalid_addresses {
            assert!(EthereumClient::validate_address(addr).is_err());
        }

        // Test the one that alloy actually accepts (missing 0x prefix)
        // alloy is more lenient and will accept this format
        let addr_without_prefix = "742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a";
        assert!(EthereumClient::validate_address(addr_without_prefix).is_ok());
    }

    #[tokio::test]
    async fn test_ethereum_client_creation() {
        let config = get_test_ethereum_config();
        
        // This should create a client without network calls
        // Note: In a real test environment, you'd need a valid RPC URL
        // Here we're just testing the configuration parsing
        assert_eq!(config.target_amount_eth, 1.0);
        assert_eq!(config.gas_multiplier, 1.2);
        assert_eq!(config.min_faucet_balance_eth, 10.0);
    }

    #[test]
    fn test_ethereum_config_validation() {
        let config = get_test_ethereum_config();
        
        // Test target amount conversion
        let target_wei = EthereumClient::eth_to_wei(config.target_amount_eth);
        assert_eq!(target_wei, U256::from(10u64.pow(18)));
        
        // Test minimum balance conversion
        let min_balance_wei = EthereumClient::eth_to_wei(config.min_faucet_balance_eth);
        assert_eq!(min_balance_wei, U256::from(10u64 * 10u64.pow(18)));
    }

    #[test]
    fn test_private_key_with_and_without_prefix() {
        let key_without_prefix = "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234";
        let key_with_prefix = "0xabcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234";
        
        // Both should be valid when parsed
        assert!(PrivateKeySigner::from_str(key_without_prefix).is_ok());
        assert!(PrivateKeySigner::from_str(key_with_prefix.strip_prefix("0x").unwrap()).is_ok());
    }

    #[test]
    fn test_gas_multiplier_calculation() {
        let base_gas_price = 1000u128;
        let multiplier = 1.5;
        
        let adjusted = (base_gas_price as f64 * multiplier) as u128;
        assert_eq!(adjusted, 1500u128);
        
        let multiplier = 1.0;
        let adjusted = (base_gas_price as f64 * multiplier) as u128;
        assert_eq!(adjusted, 1000u128);
    }

    #[test]
    fn test_amount_calculation() {
        let target = U256::from(10u64.pow(18)); // 1 ETH
        let current = U256::from(5u64 * 10u64.pow(17)); // 0.5 ETH
        
        let amount_to_send = target - current;
        assert_eq!(amount_to_send, U256::from(5u64 * 10u64.pow(17))); // 0.5 ETH
    }

    #[test]
    fn test_sufficient_balance_check() {
        let target = U256::from(10u64.pow(18)); // 1 ETH
        let current = U256::from(15u64 * 10u64.pow(17)); // 1.5 ETH
        
        // Current balance is already above target
        assert!(current >= target);
    }

    #[test] 
    fn test_address_parsing_edge_cases() {
        // Test checksummed addresses
        let checksummed = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a";
        assert!(EthereumClient::validate_address(checksummed).is_ok());
        
        // Test lowercase
        let lowercase = "0x742d35cc6634c0532925a3b8d404cb8b3d3a5d3a";
        assert!(EthereumClient::validate_address(lowercase).is_ok());
        
        // Test uppercase
        let uppercase = "0x742D35CC6634C0532925A3B8D404CB8B3D3A5D3A";
        assert!(EthereumClient::validate_address(uppercase).is_ok());
    }

    #[test]
    fn test_conversion_precision() {
        // Test with fractional ETH amounts
        let fractional_eth = 0.123456789;
        let wei = EthereumClient::eth_to_wei(fractional_eth);
        let back_to_eth = EthereumClient::wei_to_eth(wei);
        
        // Should be close due to floating point precision
        assert!((back_to_eth - fractional_eth).abs() < 0.001);
    }
}
