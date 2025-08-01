# Client Library

The `lloom-client` crate provides a high-level API for interacting with the Lloom network as a client. It handles network discovery, request routing, and response verification.

## Overview

The client library provides:
- **Simple API**: Easy-to-use interface for LLM requests
- **Automatic Discovery**: Finds suitable executors automatically
- **Request Management**: Handles signing, routing, and retries
- **Response Verification**: Validates executor responses
- **Connection Pooling**: Efficient network resource usage

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
lloom-client = "0.1.0"

# For async runtime
tokio = { version = "1", features = ["full"] }
```

## Quick Start

### Basic Client Usage

Create and use a client:

```rust
use lloom_client::{Client, ClientConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client with default config
    let client = Client::new(ClientConfig::default()).await?;
    
    // Make a request
    let response = client
        .complete("Explain quantum computing in simple terms")
        .await?;
    
    println!("Response: {}", response.content);
    println!("Tokens used: {}", response.total_tokens);
    
    Ok(())
}
```

### Advanced Configuration

Configure client behavior:

```rust
use lloom_client::{Client, ClientConfig, ExecutorSelection};
use std::time::Duration;

let config = ClientConfig {
    // Network settings
    listen_address: "/ip4/0.0.0.0/tcp/0".parse()?,
    bootstrap_peers: vec![
        "/ip4/bootstrap.lloom.network/tcp/4001/p2p/12D3KooW...".parse()?
    ],
    
    // Identity
    identity_path: Some("~/.lloom/client_identity".into()),
    
    // Request defaults
    default_model: Some("gpt-3.5-turbo".to_string()),
    default_max_tokens: 1000,
    default_temperature: 0.7,
    
    // Timeouts
    request_timeout: Duration::from_secs(120),
    discovery_timeout: Duration::from_secs(30),
    
    // Executor selection
    executor_selection: ExecutorSelection::BestPrice,
    max_price_per_token: Some("1000000000000000".to_string()), // 0.001 ETH
    
    // Performance
    max_concurrent_requests: 10,
    enable_request_cache: true,
    cache_ttl: Duration::from_secs(3600),
};

let client = Client::new(config).await?;
```

## Making Requests

### Simple Completion

Basic text completion:

```rust
use lloom_client::Client;

let client = Client::new_default().await?;

// Simple completion
let response = client.complete("Write a haiku about Rust").await?;
println!("{}", response.content);

// With options
let response = client
    .complete_with_options(
        "Explain the theory of relativity",
        lloom_client::CompletionOptions {
            model: Some("gpt-4".to_string()),
            max_tokens: Some(500),
            temperature: Some(0.3),
            system_prompt: Some("You are a physics professor".to_string()),
        }
    )
    .await?;
```

### Request Builder

Build complex requests:

```rust
use lloom_client::{Client, RequestBuilder};

let client = Client::new_default().await?;

let response = client
    .request()
    .model("gpt-3.5-turbo")
    .prompt("Translate to French: Hello, world!")
    .system_prompt("You are a professional translator")
    .temperature(0.1)
    .max_tokens(100)
    .max_price("0.001") // Maximum 0.001 ETH total
    .preferred_executor(Some(executor_peer_id))
    .deadline_minutes(5)
    .execute()
    .await?;
```

### Streaming Responses

Get responses as they're generated:

```rust
use lloom_client::{Client, StreamOptions};
use futures::StreamExt;

let client = Client::new_default().await?;

let mut stream = client
    .complete_stream(
        "Write a long story about space exploration",
        StreamOptions {
            model: Some("gpt-4".to_string()),
            chunk_size: Some(10), // Tokens per chunk
        }
    )
    .await?;

while let Some(chunk) = stream.next().await {
    match chunk {
        Ok(text) => print!("{}", text),
        Err(e) => eprintln!("Stream error: {}", e),
    }
}
```

## Executor Discovery

### Automatic Discovery

The client automatically discovers executors:

```rust
use lloom_client::Client;

let client = Client::new_default().await?;

// List available executors
let executors = client.discover_executors().await?;
for (peer_id, info) in executors {
    println!("Executor: {}", peer_id);
    println!("  Models: {:?}", info.models);
    println!("  Capacity: {}", info.capacity);
    println!("  Pricing: {} ETH/token", info.base_price);
}

// Find executors for specific model
let model_executors = client.find_model_providers("llama-2-70b").await?;
println!("Found {} executors offering llama-2-70b", model_executors.len());
```

### Manual Executor Selection

Choose specific executors:

```rust
use lloom_client::{Client, ExecutorSelector};

let client = Client::new_default().await?;

// Use specific executor
let response = client
    .request()
    .prompt("Hello")
    .executor(executor_peer_id)
    .execute()
    .await?;

// Use custom selector
let selector = ExecutorSelector::new()
    .require_model("gpt-4")
    .max_latency_ms(100)
    .min_reliability(0.95)
    .prefer_location("us-east-1");

let response = client
    .request()
    .prompt("Hello")
    .executor_selector(selector)
    .execute()
    .await?;
```

## Advanced Features

### Batch Requests

Send multiple requests efficiently:

```rust
use lloom_client::{Client, BatchRequest};

let client = Client::new_default().await?;

let batch = BatchRequest::new()
    .add("Translate to Spanish: Hello", None)
    .add("Translate to French: Hello", Some("gpt-4"))
    .add("Translate to German: Hello", None);

let responses = client.complete_batch(batch).await?;
for (i, response) in responses.into_iter().enumerate() {
    println!("Response {}: {}", i, response?.content);
}
```

### Request Caching

Enable response caching:

```rust
use lloom_client::{Client, CacheConfig};

let mut config = ClientConfig::default();
config.cache = Some(CacheConfig {
    enabled: true,
    max_size_mb: 100,
    ttl_seconds: 3600,
    cache_key_fields: vec!["prompt", "model", "temperature"],
});

let client = Client::new(config).await?;

// First request hits network
let response1 = client.complete("What is 2+2?").await?;

// Second identical request uses cache
let response2 = client.complete("What is 2+2?").await?;
assert_eq!(response1.content, response2.content);
```

### Retry Logic

Configure automatic retries:

```rust
use lloom_client::{Client, RetryConfig};

let mut config = ClientConfig::default();
config.retry = RetryConfig {
    max_attempts: 3,
    initial_delay_ms: 1000,
    max_delay_ms: 30000,
    exponential_base: 2.0,
    retry_on: vec![
        RetryCondition::Timeout,
        RetryCondition::NetworkError,
        RetryCondition::ExecutorBusy,
    ],
};

let client = Client::new(config).await?;
```

### Request Lifecycle Hooks

Add custom logic to request processing:

```rust
use lloom_client::{Client, RequestHooks};

let client = Client::new_default().await?;

client.set_hooks(RequestHooks {
    before_send: Some(Box::new(|request| {
        println!("Sending request to model: {}", request.model);
        Ok(())
    })),
    
    after_receive: Some(Box::new(|response| {
        println!("Received {} tokens", response.total_tokens);
        Ok(())
    })),
    
    on_error: Some(Box::new(|error| {
        eprintln!("Request failed: {}", error);
        Ok(())
    })),
});
```

## Cost Management

### Price Tracking

Monitor costs:

```rust
use lloom_client::{Client, CostTracker};

let client = Client::new_default().await?;
let cost_tracker = client.cost_tracker();

// Make some requests
client.complete("Hello").await?;
client.complete("World").await?;

// Check costs
let stats = cost_tracker.get_stats();
println!("Total spent: {} ETH", stats.total_cost_eth);
println!("Average cost per request: {} ETH", stats.avg_cost_per_request);
println!("Total tokens used: {}", stats.total_tokens);
```

### Budget Limits

Set spending limits:

```rust
use lloom_client::{Client, BudgetConfig};

let mut config = ClientConfig::default();
config.budget = Some(BudgetConfig {
    max_cost_per_request_eth: 0.01,
    max_cost_per_hour_eth: 1.0,
    max_cost_per_day_eth: 10.0,
    action_on_limit: BudgetLimitAction::Reject,
});

let client = Client::new(config).await?;

// Request will be rejected if it would exceed budget
match client.complete("Expensive request").await {
    Err(ClientError::BudgetExceeded { limit, would_cost }) => {
        println!("Budget exceeded: limit={}, cost={}", limit, would_cost);
    }
    Ok(response) => println!("Success: {}", response.content),
    Err(e) => eprintln!("Other error: {}", e),
}
```

## Error Handling

### Error Types

Handle different error scenarios:

```rust
use lloom_client::{Client, ClientError};

let client = Client::new_default().await?;

match client.complete("Test prompt").await {
    Ok(response) => println!("Success: {}", response.content),
    
    Err(ClientError::NoExecutorsAvailable { model }) => {
        eprintln!("No executors found for model: {}", model);
    }
    
    Err(ClientError::RequestTimeout { elapsed }) => {
        eprintln!("Request timed out after {:?}", elapsed);
    }
    
    Err(ClientError::InvalidSignature { signer }) => {
        eprintln!("Invalid signature from: {}", signer);
    }
    
    Err(ClientError::NetworkError(e)) => {
        eprintln!("Network error: {}", e);
    }
    
    Err(e) => eprintln!("Unexpected error: {}", e),
}
```

### Error Recovery

Implement fallback strategies:

```rust
use lloom_client::{Client, FallbackStrategy};

let client = Client::new_default().await?;

let response = client
    .request()
    .prompt("Important query")
    .fallback_strategy(FallbackStrategy::Sequential(vec![
        // Try preferred model first
        FallbackOption::Model("gpt-4"),
        // Fall back to cheaper model
        FallbackOption::Model("gpt-3.5-turbo"),
        // Finally try any available model
        FallbackOption::AnyModel,
    ]))
    .execute()
    .await?;
```

## Monitoring

### Metrics

Export client metrics:

```rust
use lloom_client::{Client, MetricsConfig};

let mut config = ClientConfig::default();
config.metrics = Some(MetricsConfig {
    enabled: true,
    endpoint: "0.0.0.0:9091".parse()?,
    push_gateway: Some("http://prometheus:9091".to_string()),
    push_interval: Duration::from_secs(30),
});

let client = Client::new(config).await?;

// Metrics available at http://localhost:9091/metrics
// - lloom_client_requests_total
// - lloom_client_request_duration_seconds
// - lloom_client_tokens_used_total
// - lloom_client_cost_total
```

### Logging

Configure logging:

```rust
use tracing_subscriber;

// Initialize logging
tracing_subscriber::fmt()
    .with_env_filter("lloom_client=debug")
    .init();

// Client will now log detailed information
let client = Client::new_default().await?;
```

## Testing

### Mock Client

Use mock client for testing:

```rust
#[cfg(test)]
mod tests {
    use lloom_client::{MockClient, MockResponse};
    
    #[tokio::test]
    async fn test_my_function() {
        // Create mock client
        let mut mock = MockClient::new();
        
        // Set up expected behavior
        mock.expect_complete()
            .with("test prompt")
            .returning(|_| Ok(MockResponse {
                content: "Mock response".to_string(),
                total_tokens: 10,
                model: "mock-model".to_string(),
            }));
        
        // Use mock in tests
        let result = my_function(&mock).await?;
        assert_eq!(result, "expected value");
    }
}
```

### Integration Testing

Test against local network:

```rust
#[cfg(test)]
mod integration_tests {
    use lloom_client::test_utils::{spawn_test_network, TestExecutor};
    
    #[tokio::test]
    async fn test_full_flow() {
        // Spawn test network
        let network = spawn_test_network(3).await?;
        
        // Add test executor
        let executor = TestExecutor::new()
            .with_model("test-model")
            .with_response_fn(|req| format!("Echo: {}", req.prompt));
        
        network.add_executor(executor).await?;
        
        // Create client connected to test network
        let client = network.create_client().await?;
        
        // Test request
        let response = client.complete("Hello").await?;
        assert_eq!(response.content, "Echo: Hello");
    }
}
```

## Best Practices

1. **Resource Management**
   - Reuse client instances
   - Configure appropriate timeouts
   - Set reasonable concurrent request limits

2. **Error Handling**
   - Always handle network errors
   - Implement retry logic for transient failures
   - Log errors with context

3. **Cost Control**
   - Set budget limits
   - Monitor spending
   - Choose appropriate models for tasks

4. **Performance**
   - Enable caching for repeated queries
   - Use batch requests when possible
   - Configure connection pooling

5. **Security**
   - Store keys securely
   - Verify response signatures
   - Use encrypted connections