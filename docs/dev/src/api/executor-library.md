# Executor Library

The `lloom-executor` crate provides the framework for running an executor node that processes LLM requests from the network. It handles request validation, LLM backend integration, and response generation.

## Overview

The executor library provides:
- **LLM Backend Integration**: Support for multiple LLM providers
- **Request Processing**: Validates and executes incoming requests  
- **Resource Management**: Controls concurrent execution and resource usage
- **Pricing Engine**: Dynamic pricing based on demand and costs
- **Monitoring**: Comprehensive metrics and health checks

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
lloom-executor = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

### Basic Executor

Run a simple executor:

```rust
use lloom_executor::{Executor, ExecutorConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create executor with default config
    let config = ExecutorConfig::default();
    let executor = Executor::new(config).await?;
    
    // Start processing requests
    executor.run().await?;
    
    Ok(())
}
```

### Custom Configuration

Configure executor behavior:

```rust
use lloom_executor::{Executor, ExecutorConfig, LlmBackend};
use std::time::Duration;

let config = ExecutorConfig {
    // Network settings
    listen_address: "/ip4/0.0.0.0/tcp/4001".parse()?,
    bootstrap_peers: vec![
        "/ip4/bootstrap.lloom.network/tcp/4001/p2p/12D3KooW...".parse()?
    ],
    
    // Identity
    identity_path: Some("~/.lloom/executor_identity".into()),
    ethereum_key_path: Some("~/.lloom/executor_eth_key".into()),
    
    // LLM backend
    llm_backend: LlmBackend::LMStudio {
        base_url: "http://localhost:1234/v1".to_string(),
        api_key: None,
    },
    
    // Request handling
    max_concurrent_requests: 10,
    request_timeout: Duration::from_secs(300),
    max_tokens_per_request: 4096,
    
    // Pricing
    base_inbound_price: "500000000000000".to_string(),   // 0.0005 ETH/token
    base_outbound_price: "1000000000000000".to_string(), // 0.001 ETH/token
    
    // Resource limits
    max_memory_gb: 32,
    max_gpu_memory_gb: 24,
};

let executor = Executor::new(config).await?;
```

## LLM Backend Integration

### Supported Backends

Configure different LLM providers:

```rust
use lloom_executor::{LlmBackend, ModelConfig};

// LMStudio (local)
let lmstudio_backend = LlmBackend::LMStudio {
    base_url: "http://localhost:1234/v1".to_string(),
    api_key: None,
};

// OpenAI
let openai_backend = LlmBackend::OpenAI {
    api_key: std::env::var("OPENAI_API_KEY")?,
    organization: None,
    base_url: None, // Use default
};

// Custom backend
let custom_backend = LlmBackend::Custom {
    endpoint: "https://my-llm.example.com/v1".to_string(),
    headers: vec![
        ("Authorization", "Bearer token123"),
        ("X-Custom-Header", "value"),
    ],
    request_transformer: Some(Box::new(|req| {
        // Transform request format
        serde_json::json!({
            "prompt": req.prompt,
            "max_length": req.max_tokens,
        })
    })),
};
```

### Model Configuration

Configure available models:

```rust
use lloom_executor::{Executor, ModelConfig, ModelCapability};

let executor = Executor::new(config).await?;

// Add model configuration
executor.add_model(ModelConfig {
    name: "llama-2-13b-chat".to_string(),
    aliases: vec!["llama-2-13b", "llama2-chat"],
    context_length: 4096,
    max_batch_size: 4,
    capabilities: vec![
        ModelCapability::Chat,
        ModelCapability::Completion,
    ],
    
    // Pricing for this specific model
    inbound_price: Some("600000000000000".to_string()),
    outbound_price: Some("1200000000000000".to_string()),
    
    // Resource requirements
    min_gpu_memory_gb: 16,
    optimal_batch_size: 2,
    
    // Performance hints
    tokens_per_second: Some(50.0),
    first_token_latency_ms: Some(500),
})?;
```

### Custom LLM Client

Implement custom LLM backend:

```rust
use lloom_executor::{LlmClient, LlmRequest, LlmResponse};
use async_trait::async_trait;

struct MyCustomLlm {
    client: reqwest::Client,
    endpoint: String,
}

#[async_trait]
impl LlmClient for MyCustomLlm {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        // Send request to your LLM
        let response = self.client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await?;
        
        // Parse response
        let result: MyApiResponse = response.json().await?;
        
        Ok(LlmResponse {
            content: result.generated_text,
            model: request.model,
            prompt_tokens: result.usage.prompt_tokens,
            completion_tokens: result.usage.completion_tokens,
            total_tokens: result.usage.total_tokens,
        })
    }
    
    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        // Return available models
        Ok(vec![
            ModelInfo {
                id: "my-model-v1".to_string(),
                context_length: 2048,
            }
        ])
    }
    
    async fn health_check(&self) -> Result<bool> {
        // Check if backend is healthy
        Ok(self.client.get(&self.endpoint).send().await?.status().is_success())
    }
}

// Use custom client
let custom_client = MyCustomLlm {
    client: reqwest::Client::new(),
    endpoint: "https://api.example.com/llm".to_string(),
};

executor.set_llm_client(Box::new(custom_client))?;
```

## Request Processing

### Request Handler

Customize request processing:

```rust
use lloom_executor::{Executor, RequestHandler, RequestContext};

struct CustomRequestHandler;

#[async_trait]
impl RequestHandler for CustomRequestHandler {
    async fn validate_request(
        &self,
        ctx: &RequestContext,
        request: &SignedLlmRequest,
    ) -> Result<()> {
        // Custom validation logic
        if request.payload.prompt.len() > 10000 {
            return Err("Prompt too long".into());
        }
        
        // Check if client is authorized
        if !is_authorized(&request.signer) {
            return Err("Unauthorized client".into());
        }
        
        Ok(())
    }
    
    async fn before_execution(
        &self,
        ctx: &mut RequestContext,
        request: &SignedLlmRequest,
    ) -> Result<()> {
        // Pre-processing
        ctx.set("start_time", Instant::now());
        log_request(&request);
        Ok(())
    }
    
    async fn after_execution(
        &self,
        ctx: &RequestContext,
        request: &SignedLlmRequest,
        response: &mut LlmResponse,
    ) -> Result<()> {
        // Post-processing
        let duration = ctx.get::<Instant>("start_time")?.elapsed();
        response.metadata.insert("processing_time_ms", duration.as_millis());
        Ok(())
    }
}

executor.set_request_handler(Box::new(CustomRequestHandler))?;
```

### Request Queue Management

Configure request queuing:

```rust
use lloom_executor::{QueueConfig, PriorityStrategy};

let queue_config = QueueConfig {
    max_queue_size: 1000,
    priority_strategy: PriorityStrategy::PriceWeighted {
        price_weight: 0.7,
        age_weight: 0.3,
    },
    timeout_ms: 60000,
    
    // Separate queues by priority
    priority_queues: vec![
        ("high", 100),    // High priority: 100 slots
        ("normal", 800),  // Normal priority: 800 slots  
        ("low", 100),     // Low priority: 100 slots
    ],
};

executor.set_queue_config(queue_config)?;
```

### Batch Processing

Enable request batching:

```rust
use lloom_executor::{BatchConfig, BatchStrategy};

let batch_config = BatchConfig {
    enabled: true,
    max_batch_size: 8,
    batch_timeout_ms: 100,
    
    strategy: BatchStrategy::Adaptive {
        min_batch_size: 2,
        target_latency_ms: 1000,
        max_wait_ms: 500,
    },
    
    // Group by model for efficiency
    group_by: vec!["model", "temperature"],
};

executor.set_batch_config(batch_config)?;
```

## Resource Management

### GPU Management

Configure GPU usage:

```rust
use lloom_executor::{GpuConfig, GpuAllocationStrategy};

let gpu_config = GpuConfig {
    devices: vec![0, 1], // Use GPU 0 and 1
    
    allocation_strategy: GpuAllocationStrategy::ModelAffinity {
        affinities: vec![
            ("llama-2-70b", vec![0, 1]), // Large model uses both
            ("llama-2-13b", vec![0]),    // Smaller model on GPU 0
            ("llama-2-7b", vec![1]),     // Smallest on GPU 1
        ],
    },
    
    memory_limit_per_device_gb: 22, // Leave some headroom
    enable_memory_growth: true,
    cuda_visible_devices: Some("0,1".to_string()),
};

executor.set_gpu_config(gpu_config)?;
```

### Memory Management

Control memory usage:

```rust
use lloom_executor::{MemoryConfig, MemoryStrategy};

let memory_config = MemoryConfig {
    max_total_memory_gb: 32,
    max_model_cache_gb: 24,
    
    strategy: MemoryStrategy::Adaptive {
        min_free_memory_gb: 4,
        eviction_policy: EvictionPolicy::LeastRecentlyUsed,
        preload_models: vec!["llama-2-13b-chat"],
    },
    
    enable_swap: false,
    oom_handler: Some(Box::new(|| {
        // Custom OOM handling
        eprintln!("Out of memory! Clearing cache...");
        clear_model_cache();
    })),
};

executor.set_memory_config(memory_config)?;
```

### CPU Management

Configure CPU usage:

```rust
use lloom_executor::{CpuConfig, ThreadPoolConfig};

let cpu_config = CpuConfig {
    max_threads: num_cpus::get(),
    
    thread_pools: vec![
        ThreadPoolConfig {
            name: "inference".to_string(),
            size: 8,
            priority: ThreadPriority::High,
        },
        ThreadPoolConfig {
            name: "preprocessing".to_string(),
            size: 4,
            priority: ThreadPriority::Normal,
        },
    ],
    
    cpu_affinity: Some(vec![0, 1, 2, 3]), // Pin to first 4 cores
    nice_level: Some(10), // Lower priority
};

executor.set_cpu_config(cpu_config)?;
```

## Pricing Engine

### Dynamic Pricing

Implement dynamic pricing:

```rust
use lloom_executor::{PricingEngine, PricingStrategy, MarketConditions};

struct DynamicPricingEngine;

#[async_trait]
impl PricingEngine for DynamicPricingEngine {
    async fn calculate_price(
        &self,
        model: &str,
        market: &MarketConditions,
    ) -> Result<(String, String)> { // (inbound_price, outbound_price)
        // Base prices
        let mut inbound = 0.0005; // ETH per token
        let mut outbound = 0.001;
        
        // Adjust based on demand
        if market.current_queue_size > market.target_queue_size {
            let multiplier = 1.0 + (market.utilization_rate - 0.8) * 2.0;
            inbound *= multiplier;
            outbound *= multiplier;
        }
        
        // Model-specific adjustments
        match model {
            "gpt-4" => {
                inbound *= 2.0;
                outbound *= 2.0;
            },
            "llama-2-70b" => {
                inbound *= 1.5;
                outbound *= 1.5;
            },
            _ => {}
        }
        
        // Convert to wei
        Ok((
            format!("{}", (inbound * 1e18) as u128),
            format!("{}", (outbound * 1e18) as u128),
        ))
    }
}

executor.set_pricing_engine(Box::new(DynamicPricingEngine))?;
```

### Price Updates

Broadcast price changes:

```rust
use lloom_executor::{PriceUpdateStrategy, PriceAnnouncement};

executor.set_price_update_strategy(PriceUpdateStrategy {
    update_interval: Duration::from_secs(300), // Every 5 minutes
    
    min_change_percent: 5.0, // Only update if >5% change
    
    announcement_delay: Duration::from_secs(60), // Give clients time
    
    on_price_change: Some(Box::new(|old_price, new_price| {
        println!("Price changed from {} to {}", old_price, new_price);
    })),
});
```

## Monitoring

### Health Checks

Implement health monitoring:

```rust
use lloom_executor::{HealthChecker, HealthStatus};

executor.set_health_checker(Box::new(|executor| {
    let mut status = HealthStatus::default();
    
    // Check LLM backend
    status.llm_backend_healthy = executor.llm_client().health_check().await?;
    
    // Check resource usage
    status.memory_usage_percent = get_memory_usage();
    status.gpu_utilization_percent = get_gpu_utilization();
    
    // Check queue
    status.queue_size = executor.queue_size();
    status.active_requests = executor.active_requests();
    
    // Overall health
    status.healthy = status.llm_backend_healthy 
        && status.memory_usage_percent < 90.0
        && status.queue_size < 1000;
    
    Ok(status)
}));

// Expose health endpoint
executor.enable_health_endpoint("0.0.0.0:8080")?;
```

### Metrics Export

Export Prometheus metrics:

```rust
use lloom_executor::{MetricsConfig, MetricLabels};

let metrics_config = MetricsConfig {
    enabled: true,
    endpoint: "0.0.0.0:9092".parse()?,
    
    // Custom labels for all metrics
    global_labels: vec![
        ("region", "us-east-1"),
        ("instance", "executor-1"),
    ],
    
    // Metric-specific configuration
    histograms: vec![
        ("request_duration", vec![0.1, 0.5, 1.0, 2.5, 5.0, 10.0]),
        ("token_latency", vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0]),
    ],
    
    // Push to Prometheus Pushgateway
    push_gateway: Some("http://prometheus-pushgateway:9091".to_string()),
    push_interval: Duration::from_secs(30),
};

executor.set_metrics_config(metrics_config)?;
```

### Custom Metrics

Add custom metrics:

```rust
use lloom_executor::metrics::{register_counter, register_histogram};

// Register custom metrics
let cache_hits = register_counter!(
    "executor_cache_hits_total",
    "Number of cache hits"
);

let model_load_time = register_histogram!(
    "executor_model_load_seconds",
    "Time to load model",
    vec![1.0, 5.0, 10.0, 30.0, 60.0]
);

// Use in code
cache_hits.increment();
model_load_time.observe(load_duration.as_secs_f64());
```

## Advanced Features

### Request Filtering

Filter incoming requests:

```rust
use lloom_executor::{RequestFilter, FilterAction};

executor.add_request_filter(Box::new(|request| {
    // Block specific clients
    if BLOCKED_CLIENTS.contains(&request.signer) {
        return FilterAction::Reject("Client is blocked");
    }
    
    // Rate limit by client
    if get_client_request_rate(&request.signer) > 100 {
        return FilterAction::Reject("Rate limit exceeded");
    }
    
    // Require minimum price
    let min_price = "1000000000000000"; // 0.001 ETH
    if request.payload.outbound_price < min_price {
        return FilterAction::Reject("Price too low");
    }
    
    FilterAction::Accept
}));
```

### Response Transformation

Transform responses before sending:

```rust
use lloom_executor::{ResponseTransformer, TransformContext};

executor.set_response_transformer(Box::new(|ctx, response| {
    // Add metadata
    response.metadata.insert("executor_version", env!("CARGO_PKG_VERSION"));
    response.metadata.insert("processing_node", get_node_id());
    
    // Content filtering
    if contains_inappropriate_content(&response.content) {
        response.content = "[Content filtered]".to_string();
        response.metadata.insert("filtered", "true");
    }
    
    // Compression for large responses
    if response.content.len() > 10000 {
        response.content = compress(&response.content)?;
        response.metadata.insert("compressed", "true");
    }
    
    Ok(())
}));
```

### Plugin System

Extend executor with plugins:

```rust
use lloom_executor::{Plugin, PluginContext};

struct LoggingPlugin;

#[async_trait]
impl Plugin for LoggingPlugin {
    fn name(&self) -> &str {
        "logging"
    }
    
    async fn on_request(&self, ctx: &PluginContext, request: &SignedLlmRequest) -> Result<()> {
        info!("Request from {} for model {}", request.signer, request.payload.model);
        Ok(())
    }
    
    async fn on_response(&self, ctx: &PluginContext, response: &LlmResponse) -> Result<()> {
        info!("Generated {} tokens", response.total_tokens);
        Ok(())
    }
}

executor.register_plugin(Box::new(LoggingPlugin))?;
```

## Testing

### Mock Executor

Test executor behavior:

```rust
#[cfg(test)]
mod tests {
    use lloom_executor::test_utils::{MockExecutor, MockLlmClient};
    
    #[tokio::test]
    async fn test_request_processing() {
        // Create mock executor
        let mut mock_executor = MockExecutor::new();
        
        // Set up mock LLM client
        let mock_llm = MockLlmClient::new()
            .with_response("Test response")
            .with_token_count(10, 20);
        
        mock_executor.set_llm_client(Box::new(mock_llm));
        
        // Test request processing
        let request = create_test_request();
        let response = mock_executor.process_request(request).await?;
        
        assert_eq!(response.content, "Test response");
        assert_eq!(response.total_tokens, 30);
    }
}
```

### Integration Testing

Test full executor:

```rust
#[cfg(test)]
mod integration_tests {
    use lloom_executor::test_utils::spawn_test_executor;
    
    #[tokio::test]
    async fn test_executor_lifecycle() {
        // Spawn test executor
        let executor = spawn_test_executor().await?;
        
        // Verify health
        let health = executor.health_status().await?;
        assert!(health.healthy);
        
        // Send test request
        let response = executor.test_request("Hello").await?;
        assert!(!response.content.is_empty());
        
        // Check metrics
        let metrics = executor.get_metrics();
        assert_eq!(metrics.total_requests, 1);
    }
}
```

## Best Practices

1. **Resource Planning**
   - Monitor resource usage continuously
   - Set conservative limits initially
   - Plan for peak load scenarios

2. **Error Handling**
   - Implement comprehensive error handling
   - Log errors with full context
   - Fail gracefully under load

3. **Performance**
   - Enable request batching when possible
   - Cache model responses appropriately
   - Optimize model loading times

4. **Security**
   - Validate all incoming requests
   - Implement rate limiting
   - Monitor for abuse patterns

5. **Monitoring**
   - Export detailed metrics
   - Set up alerting for anomalies
   - Track performance trends