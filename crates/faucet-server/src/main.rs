//! Main entry point for the faucet server.

use anyhow::Result;
use clap::{Arg, Command};
use faucet_server::{config::FaucetConfig, http::start_server};
use std::path::Path;
use tracing::{error, info};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    // Parse command line arguments
    let matches = Command::new("faucet-server")
        .version("1.0.0")
        .author("Lloom Team")
        .about("Ethereum Faucet Server - Send ETH to addresses via email verification")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Path to configuration file")
                .default_value("faucet-config.toml"),
        )
        .arg(
            Arg::new("generate-config")
                .long("generate-config")
                .help("Generate a default configuration file and exit")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let config_path = matches.get_one::<String>("config").unwrap();

    // Handle config generation
    if matches.get_flag("generate-config") {
        return generate_config(config_path);
    }

    info!("Starting Lloom Faucet Server v1.0.0");
    info!("Loading configuration from: {}", config_path);

    // Load configuration
    let config = match load_config(config_path) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            error!("Use --generate-config to create a default configuration file");
            std::process::exit(1);
        }
    };

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Configuration validation failed: {}", e);
        std::process::exit(1);
    }

    info!("Configuration loaded and validated successfully");
    info!("Server will bind to: {}:{}", config.http.bind_address, config.http.port);
    info!("Ethereum RPC: {}", config.ethereum.rpc_url);
    info!("SMTP server: {}:{}", config.smtp.server, config.smtp.port);

    // Start the server
    if let Err(e) = start_server(&config).await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

/// Load configuration from file
fn load_config(path: &str) -> Result<FaucetConfig> {
    if !Path::new(path).exists() {
        return Err(anyhow::anyhow!(
            "Configuration file '{}' not found. Use --generate-config to create one.",
            path
        ));
    }

    FaucetConfig::from_file(path).map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))
}

/// Generate a default configuration file
fn generate_config(path: &str) -> Result<()> {
    let config = FaucetConfig::default();
    
    config.save_to_file(path)?;
    
    println!("Generated default configuration file: {}", path);
    println!();
    println!("IMPORTANT: Please edit the configuration file before running the server:");
    println!("1. Set your Ethereum private key (ethereum.private_key)");
    println!("2. Configure your SMTP settings (smtp section)");
    println!("3. Adjust security settings as needed (security section)");
    println!("4. Set the target funding amount (ethereum.target_amount_eth)");
    println!();
    println!("Example usage after configuration:");
    println!("  cargo run --bin faucet-server -- --config {}", path);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_generate_and_load_config() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let temp_path = temp_file.path().to_str().unwrap();
        
        // Generate config
        generate_config(temp_path)?;
        
        // Should be able to load it
        let config = load_config(temp_path)?;
        
        // Should have default values
        assert_eq!(config.http.port, 3030);
        assert_eq!(config.ethereum.target_amount_eth, 1.0);
        
        Ok(())
    }

    #[test]
    fn test_load_nonexistent_config() {
        let result = load_config("nonexistent-file.toml");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_config_validation() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let temp_path = temp_file.path().to_str().unwrap();
        
        // Generate and load default config
        generate_config(temp_path)?;
        let config = load_config(temp_path)?;
        
        // Default config should fail validation (missing real credentials)
        assert!(config.validate().is_err());
        
        Ok(())
    }
}
