//! Configuration management for the faucet server.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Configuration for the faucet server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaucetConfig {
    /// HTTP server configuration
    pub http: HttpConfig,
    
    /// Ethereum blockchain configuration
    pub ethereum: EthereumConfig,
    
    /// SMTP email configuration
    pub smtp: SmtpConfig,
    
    /// Security and rate limiting configuration
    pub security: SecurityConfig,
}

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Port to bind to
    pub port: u16,
    
    /// Address to bind to
    pub bind_address: String,
}

/// Ethereum blockchain configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumConfig {
    /// RPC endpoint URL
    pub rpc_url: String,
    
    /// Private key for the faucet wallet (hex string without 0x prefix)
    pub private_key: String,
    
    /// Target amount in ETH to fund addresses to
    pub target_amount_eth: f64,
    
    /// Gas price multiplier (1.0 = normal, 1.5 = 50% higher)
    pub gas_multiplier: f64,
    
    /// Minimum balance required in faucet wallet (in ETH)
    pub min_faucet_balance_eth: f64,
}

/// SMTP configuration for sending emails
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    /// SMTP server hostname
    pub server: String,
    
    /// SMTP server port
    pub port: u16,
    
    /// Username for authentication
    pub username: String,
    
    /// Password for authentication
    pub password: String,
    
    /// From email address
    pub from_address: String,
    
    /// Email subject line
    pub subject: String,
    
    /// Base URL for the faucet service (for clickable links)
    pub base_url: String,
}

/// Security and rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Token expiration time in minutes
    pub token_expiry_minutes: u64,
    
    /// Maximum requests per email address per day
    pub max_requests_per_email_per_day: u32,
    
    /// Maximum requests per IP address per hour
    pub max_requests_per_ip_per_hour: u32,
    
    /// Cleanup interval for expired tokens and rate limits in minutes
    pub cleanup_interval_minutes: u64,
}

impl Default for FaucetConfig {
    fn default() -> Self {
        Self {
            http: HttpConfig {
                port: 3030,
                bind_address: "127.0.0.1".to_string(),
            },
            ethereum: EthereumConfig {
                rpc_url: "https://rpc.sepolia.org".to_string(),
                private_key: "your_private_key_here".to_string(),
                target_amount_eth: 1.0,
                gas_multiplier: 1.2,
                min_faucet_balance_eth: 10.0,
            },
            smtp: SmtpConfig {
                server: "smtp.gmail.com".to_string(),
                port: 587,
                username: "your_email@gmail.com".to_string(),
                password: "your_app_password".to_string(),
                from_address: "your_email@gmail.com".to_string(),
                subject: "Your Faucet Token".to_string(),
                base_url: "http://localhost:3030".to_string(),
            },
            security: SecurityConfig {
                token_expiry_minutes: 15,
                max_requests_per_email_per_day: 1,
                max_requests_per_ip_per_hour: 5,
                cleanup_interval_minutes: 30,
            },
        }
    }
}

impl FaucetConfig {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path.as_ref().to_str().unwrap()))
            .add_source(config::Environment::with_prefix("FAUCET"))
            .build()?;
        
        settings.try_deserialize()
    }
    
    /// Save configuration to a TOML file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let toml_string = toml::to_string_pretty(self)?;
        std::fs::write(path, toml_string)?;
        Ok(())
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate ethereum address format (basic check)
        if self.ethereum.private_key == "your_private_key_here" {
            return Err(anyhow::anyhow!("Private key must be configured"));
        }
        
        if self.ethereum.private_key.len() != 64 {
            return Err(anyhow::anyhow!("Private key must be 64 hex characters"));
        }
        
        // Validate target amount
        if self.ethereum.target_amount_eth <= 0.0 {
            return Err(anyhow::anyhow!("Target amount must be positive"));
        }
        
        // Validate email configuration
        if self.smtp.username == "your_email@gmail.com" {
            return Err(anyhow::anyhow!("SMTP configuration must be set"));
        }
        
        // Validate security settings
        if self.security.token_expiry_minutes == 0 {
            return Err(anyhow::anyhow!("Token expiry must be greater than 0"));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FaucetConfig::default();
        
        assert_eq!(config.http.port, 3030);
        assert_eq!(config.http.bind_address, "127.0.0.1");
        assert_eq!(config.ethereum.target_amount_eth, 1.0);
        assert_eq!(config.security.token_expiry_minutes, 15);
    }

    #[test]
    fn test_config_serialization() {
        let config = FaucetConfig::default();
        
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: FaucetConfig = toml::from_str(&serialized).unwrap();
        
        assert_eq!(config.http.port, deserialized.http.port);
        assert_eq!(config.ethereum.target_amount_eth, deserialized.ethereum.target_amount_eth);
    }

    #[test]
    fn test_config_from_file() -> anyhow::Result<()> {
        let toml_content = r#"
[http]
port = 8080
bind_address = "0.0.0.0"

[ethereum]
rpc_url = "https://mainnet.infura.io/v3/key"
private_key = "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234"
target_amount_eth = 0.5
gas_multiplier = 1.5
min_faucet_balance_eth = 5.0

[smtp]
server = "smtp.example.com"
port = 465
username = "test@example.com"
password = "password123"
from_address = "faucet@example.com"
subject = "Test Token"

[security]
token_expiry_minutes = 30
max_requests_per_email_per_day = 3
max_requests_per_ip_per_hour = 10
cleanup_interval_minutes = 60
"#;

        // Create a temporary file with .toml extension
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path().join("test_config.toml");
        std::fs::write(&temp_path, toml_content)?;
        
        let config = FaucetConfig::from_file(&temp_path)?;
        
        assert_eq!(config.http.port, 8080);
        assert_eq!(config.http.bind_address, "0.0.0.0");
        assert_eq!(config.ethereum.target_amount_eth, 0.5);
        assert_eq!(config.security.token_expiry_minutes, 30);
        
        Ok(())
    }

    #[test]
    fn test_config_validation() {
        let mut config = FaucetConfig::default();
        
        // Should fail with default values
        assert!(config.validate().is_err());
        
        // Fix private key
        config.ethereum.private_key = "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234".to_string();
        config.smtp.username = "test@example.com".to_string();
        
        // Should pass now
        assert!(config.validate().is_ok());
        
        // Test invalid private key length
        config.ethereum.private_key = "short".to_string();
        assert!(config.validate().is_err());
        
        // Test invalid target amount
        config.ethereum.private_key = "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234".to_string();
        config.ethereum.target_amount_eth = -1.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_save_and_load_config() -> anyhow::Result<()> {
        let mut config = FaucetConfig::default();
        config.ethereum.private_key = "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234".to_string();
        config.smtp.username = "test@example.com".to_string();
        config.http.port = 8080;
        
        // Create a temporary file with .toml extension
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path().join("test_save_config.toml");
        config.save_to_file(&temp_path)?;
        
        let loaded_config = FaucetConfig::from_file(&temp_path)?;
        
        assert_eq!(config.http.port, loaded_config.http.port);
        assert_eq!(config.ethereum.private_key, loaded_config.ethereum.private_key);
        assert_eq!(config.smtp.username, loaded_config.smtp.username);
        
        Ok(())
    }
}
