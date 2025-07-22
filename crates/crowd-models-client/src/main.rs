//! Crowd Models Client
//! 
//! A CLI tool for interacting with the Crowd Models P2P network to request LLM services.

use anyhow::Result;
use clap::Parser;
use tracing::info;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Private key (hex encoded) for identity
    #[arg(long, env = "CROWD_MODELS_PRIVATE_KEY")]
    private_key: Option<String>,
    
    /// Bootstrap nodes to connect to
    #[arg(long, value_delimiter = ',')]
    bootstrap_nodes: Vec<String>,
    
    /// Model to use for the request
    #[arg(long, default_value = "gpt-3.5-turbo")]
    model: String,
    
    /// Prompt to send to the model
    #[arg(long)]
    prompt: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    
    info!("Starting Crowd Models Client...");
    info!("Model: {}", args.model);
    info!("Prompt: {}", args.prompt);
    
    // TODO: Implement client logic
    println!("Client implementation coming soon!");
    
    Ok(())
}