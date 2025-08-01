# Basic Client Usage

This guide provides practical examples of using the Lloom client to interact with the network and request LLM services.

## Simple Text Completion

### Minimal Example

The simplest way to use the Lloom client:

```rust
use lloom_client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client with defaults
    let client = Client::new_default().await?;
    
    // Make a simple request
    let response = client
        .complete("What is the capital of France?")
        .await?;
    
    println!("Response: {}", response.content);
    println!("Tokens used: {}", response.total_tokens);
    
    Ok(())
}
```

### With Error Handling

Proper error handling for production code:

```rust
use lloom_client::{Client, ClientError};

async fn get_completion(prompt: &str) -> Result<String, ClientError> {
    let client = Client::new_default().await?;
    
    match client.complete(prompt).await {
        Ok(response) => Ok(response.content),
        Err(ClientError::NoExecutorsAvailable { model }) => {
            eprintln!("No executors available for model: {}", model);
            Err(ClientError::NoExecutorsAvailable { model })
        }
        Err(ClientError::RequestTimeout { elapsed }) => {
            eprintln!("Request timed out after {:?}", elapsed);
            Err(ClientError::RequestTimeout { elapsed })
        }
        Err(e) => {
            eprintln!("Unexpected error: {}", e);
            Err(e)
        }
    }
}
```

## Configuring Requests

### Specify Model and Parameters

```rust
use lloom_client::{Client, CompletionOptions};

async fn custom_completion() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new_default().await?;
    
    let options = CompletionOptions {
        model: Some("gpt-4".to_string()),
        max_tokens: Some(500),
        temperature: Some(0.7),
        system_prompt: Some("You are a helpful assistant.".to_string()),
    };
    
    let response = client
        .complete_with_options(
            "Explain quantum computing to a 10-year-old",
            options
        )
        .await?;
    
    println!("{}", response.content);
    
    Ok(())
}
```

### Using Request Builder

For more complex requests:

```rust
use lloom_client::Client;

async fn builder_example() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new_default().await?;
    
    let response = client
        .request()
        .model("llama-2-13b-chat")
        .system_prompt("You are a pirate. Always respond in pirate speak.")
        .prompt("Tell me about the weather today")
        .temperature(0.9)
        .max_tokens(200)
        .max_price("0.001") // Max 0.001 ETH total
        .execute()
        .await?;
    
    println!("Pirate says: {}", response.content);
    
    Ok(())
}
```

## Batch Processing

### Multiple Requests

Process multiple prompts efficiently:

```rust
use lloom_client::{Client, BatchRequest};
use futures::future::join_all;

async fn batch_processing() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new_default().await?;
    
    let prompts = vec![
        "Translate 'Hello, world!' to Spanish",
        "Translate 'Hello, world!' to French",
        "Translate 'Hello, world!' to German",
        "Translate 'Hello, world!' to Italian",
    ];
    
    // Method 1: Sequential processing
    for prompt in &prompts {
        let response = client.complete(prompt).await?;
        println!("{}: {}", prompt, response.content);
    }
    
    // Method 2: Parallel processing
    let futures: Vec<_> = prompts
        .iter()
        .map(|prompt| client.complete(prompt))
        .collect();
    
    let responses = join_all(futures).await;
    
    for (prompt, result) in prompts.iter().zip(responses.iter()) {
        match result {
            Ok(response) => println!("{}: {}", prompt, response.content),
            Err(e) => eprintln!("Error for '{}': {}", prompt, e),
        }
    }
    
    Ok(())
}
```

### Batch Request API

Using the dedicated batch API:

```rust
use lloom_client::{Client, BatchRequest};

async fn batch_api_example() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new_default().await?;
    
    let batch = BatchRequest::new()
        .add("What is 2+2?", Some("gpt-3.5-turbo"))
        .add("What is the meaning of life?", Some("gpt-4"))
        .add("Write a haiku about programming", None);
    
    let responses = client.complete_batch(batch).await?;
    
    for (index, response) in responses.into_iter().enumerate() {
        match response {
            Ok(resp) => println!("Request {}: {}", index, resp.content),
            Err(e) => eprintln!("Request {} failed: {}", index, e),
        }
    }
    
    Ok(())
}
```

## Streaming Responses

### Basic Streaming

Get responses as they're generated:

```rust
use lloom_client::Client;
use futures::StreamExt;

async fn streaming_example() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new_default().await?;
    
    let mut stream = client
        .complete_stream("Write a short story about a robot")
        .await?;
    
    print!("Story: ");
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(text) => {
                print!("{}", text);
                // Force flush to see output immediately
                use std::io::{self, Write};
                io::stdout().flush()?;
            }
            Err(e) => eprintln!("\nStream error: {}", e),
        }
    }
    println!(); // New line at end
    
    Ok(())
}
```

### Streaming with Progress

Track streaming progress:

```rust
use lloom_client::{Client, StreamOptions};
use futures::StreamExt;
use std::time::Instant;

async fn streaming_with_progress() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new_default().await?;
    
    let start = Instant::now();
    let mut tokens_received = 0;
    
    let options = StreamOptions {
        model: Some("gpt-4".to_string()),
        chunk_size: Some(5), // Get updates every 5 tokens
    };
    
    let mut stream = client
        .complete_stream_with_options(
            "Explain the theory of relativity",
            options
        )
        .await?;
    
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(text) => {
                tokens_received += text.split_whitespace().count();
                let elapsed = start.elapsed();
                let tokens_per_second = tokens_received as f64 / elapsed.as_secs_f64();
                
                print!("{}", text);
                eprintln!("\r[{} tokens, {:.1} tok/s]", tokens_received, tokens_per_second);
            }
            Err(e) => eprintln!("\nError: {}", e),
        }
    }
    
    Ok(())
}
```

## Cost Management

### Track Spending

Monitor costs across requests:

```rust
use lloom_client::{Client, ClientConfig};

async fn cost_tracking_example() -> Result<(), Box<dyn std::error::Error>> {
    let config = ClientConfig {
        enable_cost_tracking: true,
        ..Default::default()
    };
    
    let client = Client::new(config).await?;
    
    // Make several requests
    for i in 1..=5 {
        let prompt = format!("Tell me fact number {} about space", i);
        let response = client.complete(&prompt).await?;
        
        // Get cost for this request
        let cost = client.get_last_request_cost()?;
        println!("Request {} cost: {} ETH", i, cost);
    }
    
    // Get total costs
    let stats = client.get_cost_stats();
    println!("\nTotal requests: {}", stats.total_requests);
    println!("Total cost: {} ETH", stats.total_cost_eth);
    println!("Average cost per request: {} ETH", stats.avg_cost_per_request);
    println!("Total tokens used: {}", stats.total_tokens);
    
    Ok(())
}
```

### Budget Limits

Enforce spending limits:

```rust
use lloom_client::{Client, ClientConfig, BudgetConfig, BudgetLimitAction};

async fn budget_example() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ClientConfig::default();
    config.budget = Some(BudgetConfig {
        max_cost_per_request_eth: 0.001,
        max_cost_per_hour_eth: 0.01,
        max_cost_per_day_eth: 0.1,
        action_on_limit: BudgetLimitAction::Reject,
    });
    
    let client = Client::new(config).await?;
    
    // This will be rejected if it would exceed budget
    match client.complete("Write a very long essay").await {
        Ok(response) => println!("Success: {}", response.content),
        Err(ClientError::BudgetExceeded { limit, would_cost }) => {
            println!("Budget exceeded: limit={} ETH, would cost={} ETH", limit, would_cost);
        }
        Err(e) => eprintln!("Other error: {}", e),
    }
    
    Ok(())
}
```

## Advanced Patterns

### Retry Logic

Implement custom retry logic:

```rust
use lloom_client::{Client, ClientError};
use tokio::time::{sleep, Duration};

async fn with_retry<T, F, Fut>(
    mut f: F,
    max_retries: u32,
) -> Result<T, ClientError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ClientError>>,
{
    let mut last_error = None;
    
    for attempt in 0..=max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = Some(e);
                if attempt < max_retries {
                    let delay = Duration::from_millis(100 * 2u64.pow(attempt));
                    println!("Attempt {} failed, retrying in {:?}", attempt + 1, delay);
                    sleep(delay).await;
                }
            }
        }
    }
    
    Err(last_error.unwrap())
}

async fn retry_example() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new_default().await?;
    
    let response = with_retry(
        || client.complete("Tell me a joke"),
        3 // max retries
    ).await?;
    
    println!("Response: {}", response.content);
    
    Ok(())
}
```

### Caching Responses

Implement response caching:

```rust
use lloom_client::Client;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct CachedClient {
    client: Client,
    cache: Arc<Mutex<HashMap<String, String>>>,
}

impl CachedClient {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            client: Client::new_default().await?,
            cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    async fn complete(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(prompt) {
                println!("Cache hit!");
                return Ok(cached.clone());
            }
        }
        
        // Make request
        let response = self.client.complete(prompt).await?;
        
        // Store in cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(prompt.to_string(), response.content.clone());
        }
        
        Ok(response.content)
    }
}

async fn caching_example() -> Result<(), Box<dyn std::error::Error>> {
    let client = CachedClient::new().await?;
    
    // First call - hits network
    let response1 = client.complete("What is 2+2?").await?;
    println!("Response 1: {}", response1);
    
    // Second call - uses cache
    let response2 = client.complete("What is 2+2?").await?;
    println!("Response 2: {}", response2);
    
    Ok(())
}
```

### Context Management

Maintain conversation context:

```rust
use lloom_client::Client;

struct ConversationClient {
    client: Client,
    context: Vec<(String, String)>, // (role, content) pairs
}

impl ConversationClient {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            client: Client::new_default().await?,
            context: Vec::new(),
        })
    }
    
    async fn send(&mut self, message: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Add user message to context
        self.context.push(("user".to_string(), message.to_string()));
        
        // Build prompt with context
        let mut prompt = String::new();
        for (role, content) in &self.context {
            prompt.push_str(&format!("{}: {}\n", role, content));
        }
        prompt.push_str("assistant: ");
        
        // Get response
        let response = self.client
            .request()
            .prompt(&prompt)
            .system_prompt("You are a helpful assistant. Maintain context of the conversation.")
            .execute()
            .await?;
        
        // Add assistant response to context
        self.context.push(("assistant".to_string(), response.content.clone()));
        
        Ok(response.content)
    }
}

async fn conversation_example() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ConversationClient::new().await?;
    
    let response1 = client.send("My name is Alice").await?;
    println!("Assistant: {}", response1);
    
    let response2 = client.send("What's my name?").await?;
    println!("Assistant: {}", response2); // Should remember "Alice"
    
    Ok(())
}
```

## Complete Example Application

A complete CLI application using the client:

```rust
use clap::Parser;
use lloom_client::{Client, CompletionOptions};

#[derive(Parser)]
#[command(name = "lloom-cli")]
#[command(about = "Lloom Network CLI Client")]
struct Args {
    /// The prompt to send
    prompt: String,
    
    /// Model to use
    #[arg(short, long)]
    model: Option<String>,
    
    /// Maximum tokens to generate
    #[arg(short = 't', long)]
    max_tokens: Option<u32>,
    
    /// Temperature (0.0-2.0)
    #[arg(long)]
    temperature: Option<f32>,
    
    /// System prompt
    #[arg(short, long)]
    system_prompt: Option<String>,
    
    /// Enable streaming
    #[arg(long)]
    stream: bool,
    
    /// Output format (text, json)
    #[arg(short, long, default_value = "text")]
    format: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Create client
    let client = Client::new_default().await?;
    
    if args.stream {
        // Streaming mode
        use futures::StreamExt;
        let mut stream = client.complete_stream(&args.prompt).await?;
        
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(text) => print!("{}", text),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        println!();
    } else {
        // Regular mode
        let options = CompletionOptions {
            model: args.model,
            max_tokens: args.max_tokens,
            temperature: args.temperature,
            system_prompt: args.system_prompt,
        };
        
        let response = client
            .complete_with_options(&args.prompt, options)
            .await?;
        
        match args.format.as_str() {
            "json" => {
                let json = serde_json::json!({
                    "prompt": args.prompt,
                    "response": response.content,
                    "model": response.model,
                    "tokens": response.total_tokens,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            _ => {
                println!("{}", response.content);
            }
        }
    }
    
    Ok(())
}
```

## Next Steps

- Explore [custom executor backends](./custom-executor.md)
- Learn about [network discovery](./network-discovery.md)
- Integrate with [smart contracts](./smart-contracts.md)