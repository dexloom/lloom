use clap::{Parser, Subcommand};
use lloom_core::Identity;
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Parser)]
#[command(name = "lloom-helper")]
#[command(about = "A CLI tool to generate configuration files for lloom components")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate client configuration (TOML file)
    NewClient {
        /// Output file path
        #[arg(short, long, default_value = "client-config.toml")]
        output: String,
    },
    /// Generate executor configuration (TOML file)
    NewExecutor {
        /// Output file path
        #[arg(short, long, default_value = "executor-config.toml")]
        output: String,
    },
    /// Generate validator configuration (TOML file)
    NewValidator {
        /// Output file path
        #[arg(short, long, default_value = "validator-config.toml")]
        output: String,
    },
    /// Request ETH from faucet using configuration file
    RequestEth {
        /// Email address for faucet request
        #[arg(long)]
        email: String,
        /// Path to configuration file containing private key
        #[arg(long)]
        config: String,
    },
}

#[derive(Serialize, Deserialize)]
struct ClientConfig {
    identity: IdentityConfig,
    network: NetworkConfig,
}

#[derive(Serialize, Deserialize)]
struct ValidatorConfig {
    identity: IdentityConfig,
}

#[derive(Serialize, Deserialize)]
struct IdentityConfig {
    private_key: String,
}

#[derive(Serialize, Deserialize)]
struct NetworkConfig {
    bootstrap_nodes: Vec<String>,
}

const BOOTSTRAP_NODE: &str = "/ip4/34.56.189.68/tcp/5001/p2p/12D3KooWK39hN8NkmWFNRfTjJSpYW9aJvJgXVQNzVVuWpV2yCh7H";
const ETH_RPC_URL: &str = "https://sepolia.base.org";
const CONTRACT_ADDRESS: &str = "0x25e8c5878DdaA22d1753a9223f948B61AeAf47E6";
const CHAIN_ID: u64 = 84532;
const FAUCET_URL: &str = "https://faucet.lloom.dev";

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::NewClient { output } => generate_client_config(&output).await,
        Commands::NewExecutor { output } => generate_executor_config(&output).await,
        Commands::NewValidator { output } => generate_validator_config(&output).await,
        Commands::RequestEth { email, config } => request_eth(&email, &config).await,
    }
}

async fn generate_client_config(output: &str) -> Result<()> {
    let identity = Identity::generate();
    let private_key_hex = hex::encode(identity.wallet.to_bytes());
    let eth_address = format!("0x{}", hex::encode(identity.evm_address.0));

    let config = ClientConfig {
        identity: IdentityConfig {
            private_key: private_key_hex.clone(),
        },
        network: NetworkConfig {
            bootstrap_nodes: vec![BOOTSTRAP_NODE.to_string()],
        },
    };

    let toml_content = toml::to_string_pretty(&config)
        .context("Failed to serialize client configuration to TOML")?;

    fs::write(output, toml_content)
        .context("Failed to write client configuration file")?;

    println!("✅ Client configuration generated successfully!");
    println!("📄 File: {}", output);
    println!("🔑 Private Key: {}", private_key_hex);
    println!("📍 Ethereum Address: {}", eth_address);
    println!("\n📋 Next steps:");
    println!("   1. Run: lloom-client --config {}", output);

    Ok(())
}

async fn generate_executor_config(output: &str) -> Result<()> {
    let identity = Identity::generate();
    let private_key_hex = hex::encode(identity.wallet.to_bytes());
    let eth_address = format!("0x{}", hex::encode(identity.evm_address.0));

    let config_content = format!(
        r#"[identity]
private_key = "{}"

[network]
listen_port = 5001
bootstrap_nodes = ["{}"]

[blockchain]
eth_rpc_url = "{}"
contract_address = "{}"
chain_id = {}

[[llm_backends]]
name = "lmstudio"
endpoint = "http://localhost:1234/v1"
model = "qwen2.5-coder-7b-instruct"
provider = "lmstudio"
"#,
        private_key_hex, BOOTSTRAP_NODE, ETH_RPC_URL, CONTRACT_ADDRESS, CHAIN_ID
    );

    fs::write(output, config_content)
        .context("Failed to write executor configuration file")?;

    println!("✅ Executor configuration generated successfully!");
    println!("📄 File: {}", output);
    println!("🔑 Private Key: {}", private_key_hex);
    println!("📍 Ethereum Address: {}", eth_address);
    println!("\n📋 Next steps:");
    println!("   1. Review and adjust the configuration as needed");
    println!("   2. Ensure LM Studio is running on localhost:1234");
    println!("   3. Run: lloom-executor --config {}", output);

    Ok(())
}

async fn generate_validator_config(output: &str) -> Result<()> {
    let identity = Identity::generate();
    let private_key_hex = hex::encode(identity.wallet.to_bytes());
    let eth_address = format!("0x{}", hex::encode(identity.evm_address.0));

    let config = ValidatorConfig {
        identity: IdentityConfig {
            private_key: private_key_hex.clone(),
        },
    };

    let toml_content = toml::to_string_pretty(&config)
        .context("Failed to serialize validator configuration to TOML")?;

    fs::write(output, toml_content)
        .context("Failed to write validator configuration file")?;

    println!("✅ Validator configuration generated successfully!");
    println!("📄 File: {}", output);
    println!("🔑 Private Key: {}", private_key_hex);
    println!("📍 Ethereum Address: {}", eth_address);
    println!("\n📋 Next steps:");
    println!("   1. Keep the validator configuration file secure");
    println!("   2. Run: lloom-validator --config {}", output);

    Ok(())
}

async fn request_eth(email: &str, config_path: &str) -> Result<()> {
    println!("🔍 Reading configuration from: {}", config_path);
    
    let private_key_hex = extract_private_key_from_config(config_path)
        .context("Failed to extract private key from configuration")?;
    
    let identity = Identity::from_str(&private_key_hex)
        .context("Failed to create identity from private key")?;
    
    let eth_address = format!("0x{}", hex::encode(identity.evm_address.0));
    
    println!("📍 Ethereum Address: {}", eth_address);
    println!("📧 Email: {}", email);
    println!("🌐 Requesting ETH from faucet...");

    let client = reqwest::Client::new();
    
    // First, request tokens
    let request_payload = serde_json::json!({
        "email": email,
        "ethereum_address": eth_address
    });
    
    let request_response = client
        .post(&format!("{}/request", FAUCET_URL))
        .json(&request_payload)
        .send()
        .await
        .context("Failed to send faucet request")?;
    
    if !request_response.status().is_success() {
        let error_text = request_response.text().await.unwrap_or_default();
        anyhow::bail!("Faucet request failed: {}", error_text);
    }
    
    let request_result: serde_json::Value = request_response.json().await
        .context("Failed to parse faucet request response")?;
    
    println!("✅ Faucet request successful!");
    
    // Extract token from response
    let token = request_result["token"]
        .as_str()
        .context("No token found in faucet response")?;
    
    println!("🎫 Redemption token: {}", token);
    println!("💰 Redeeming tokens...");
    
    // Redeem the tokens
    let redeem_payload = serde_json::json!({
        "token": token
    });
    
    let redeem_response = client
        .post(&format!("{}/redeem", FAUCET_URL))
        .json(&redeem_payload)
        .send()
        .await
        .context("Failed to redeem tokens")?;
    
    if !redeem_response.status().is_success() {
        let error_text = redeem_response.text().await.unwrap_or_default();
        anyhow::bail!("Token redemption failed: {}", error_text);
    }
    
    let redeem_result: serde_json::Value = redeem_response.json().await
        .context("Failed to parse redemption response")?;
    
    println!("🎉 Tokens successfully redeemed!");
    
    if let Some(tx_hash) = redeem_result["transaction_hash"].as_str() {
        println!("📜 Transaction hash: {}", tx_hash);
        println!("🔍 View on explorer: https://sepolia.basescan.org/tx/{}", tx_hash);
    }
    
    println!("💳 Your address {} should now have test ETH!", eth_address);

    Ok(())
}

fn extract_private_key_from_config(config_path: &str) -> Result<String> {
    let content = fs::read_to_string(config_path)
        .context("Failed to read configuration file")?;
    
    // All configurations are now in TOML format
    let config: toml::Value = toml::from_str(&content)
        .context("Failed to parse TOML configuration")?;
    
    config["identity"]["private_key"]
        .as_str()
        .map(|s| s.to_string())
        .context("No private_key found in TOML configuration")
}