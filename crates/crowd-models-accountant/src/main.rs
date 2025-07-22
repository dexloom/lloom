//! Crowd Models Accountant
//! 
//! A stable supernode that serves as a network anchor for peer discovery.

use anyhow::Result;
use clap::Parser;
use tracing::info;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Private key (hex encoded) for identity
    #[arg(long, env = "CROWD_MODELS_PRIVATE_KEY")]
    private_key: Option<String>,
    
    /// Address to listen on
    #[arg(long, default_value = "/ip4/0.0.0.0/tcp/4001")]
    listen_address: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    
    info!("Starting Crowd Models Accountant...");
    info!("Listen address: {}", args.listen_address);
    
    // TODO: Implement accountant logic
    println!("Accountant implementation coming soon!");
    
    Ok(())
}