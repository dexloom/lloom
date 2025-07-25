//! Example demonstrating LMStudio integration
//! 
//! This example shows how to configure and use LMStudio as a backend.
//! To run this example:
//! 1. Start LMStudio and load a model
//! 2. Enable the local server (default: localhost:1234)
//! 3. Run: cargo run --example test_lmstudio

// Import from the executor library
use lloom_executor::{LlmBackendConfig, LlmClient};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🚀 Testing LMStudio integration...");
    
    // Create LMStudio backend configuration
    let lmstudio_config = LlmBackendConfig {
        name: "lmstudio".to_string(),
        endpoint: "http://localhost:1234/api/v0".to_string(),
        api_key: None, // No API key needed for local LMStudio
        supported_models: vec![], // Will be auto-discovered
        rate_limit: Some(100),
    };
    
    // Create client
    let client = LlmClient::new(lmstudio_config)?;
    
    // Test if this is detected as LMStudio backend
    if client.is_lmstudio_backend() {
        println!("✅ Detected as LMStudio backend");
        
        // Try to discover models
        match client.discover_lmstudio_models().await {
            Ok(models) => {
                if models.is_empty() {
                    println!("⚠️  No models found. Make sure LMStudio is running with a loaded model.");
                } else {
                    println!("🎯 Discovered models: {:?}", models);
                    
                    // Test chat completion with the first model
                    if let Some(model) = models.first() {
                        println!("🧠 Testing chat completion with model: {}", model);
                        
                        match client.lmstudio_chat_completion(
                            model,
                            "Hello! Can you tell me about yourself?",
                            Some("You are a helpful AI assistant."),
                            Some(0.7),
                            Some(100),
                        ).await {
                            Ok((content, tokens, stats, model_info)) => {
                                println!("✅ Chat completion successful!");
                                println!("📝 Response: {}", content);
                                println!("🔢 Tokens used: {}", tokens);
                                
                                if let Some(stats) = stats {
                                    println!("⚡ Performance metrics:");
                                    if let Some(tps) = stats.tokens_per_second {
                                        println!("   - Tokens/sec: {:.2}", tps);
                                    }
                                    if let Some(ttft) = stats.time_to_first_token {
                                        println!("   - Time to first token: {:.3}s", ttft);
                                    }
                                }
                                
                                if let Some(model_info) = model_info {
                                    println!("🏗️  Model info:");
                                    if let Some(arch) = model_info.architecture {
                                        println!("   - Architecture: {}", arch);
                                    }
                                    if let Some(size) = model_info.size {
                                        println!("   - Size: {}", size);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("❌ Chat completion failed: {}", e);
                                println!("💡 Make sure LMStudio is running and has a model loaded");
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("❌ Model discovery failed: {}", e);
                println!("💡 Make sure LMStudio is running at http://localhost:1234");
            }
        }
    } else {
        println!("❌ Not detected as LMStudio backend");
    }
    
    Ok(())
}