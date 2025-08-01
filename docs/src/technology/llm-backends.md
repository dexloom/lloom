# LLM Backend Integration

Lloom supports multiple LLM backend providers through a flexible, pluggable architecture. This allows executors to serve requests using various models and providers while maintaining a consistent interface for clients.

## Supported Backends

### LMStudio

LMStudio provides a local, privacy-focused solution for running open-source language models.

**Features:**
- Local model hosting with no external API calls
- Support for GGUF, GGML, and other quantized formats
- Built-in model management and downloading
- OpenAI-compatible API endpoint

**Configuration:**
```toml
[llm_client]
backend = "lmstudio"
base_url = "http://localhost:1234/v1"
api_key = "not-needed"  # LMStudio doesn't require authentication
timeout_secs = 300      # Longer timeout for local inference
```

**Model Discovery:**
LMStudio exposes a models endpoint that Lloom can query:
```rust
// Automatic model discovery
let models = llm_client.list_models().await?;
for model in models {
    println!("Available model: {}", model.id);
}
```

### OpenAI

Integration with OpenAI's API for access to GPT models.

**Features:**
- Access to latest GPT models
- High-quality responses
- Reliable infrastructure
- Function calling support (planned)

**Configuration:**
```toml
[llm_client]
backend = "openai"
base_url = "https://api.openai.com/v1"
api_key = "${OPENAI_API_KEY}"  # Set via environment variable
timeout_secs = 60
```

### Adding Custom Backends

The LLM client trait makes it easy to add new backends:

```rust
#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse>;
    async fn list_models(&self) -> Result<Vec<Model>>;
    async fn health_check(&self) -> Result<bool>;
}
```

## Request Processing

### Request Structure

All backends receive standardized requests:

```rust
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stream: bool,
}

pub struct Message {
    pub role: Role,
    pub content: String,
}

pub enum Role {
    System,
    User,
    Assistant,
}
```

### Response Handling

Responses are normalized across backends:

```rust
pub struct LlmResponse {
    pub model: String,
    pub content: String,
    pub usage: TokenUsage,
    pub finish_reason: FinishReason,
}

pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

## Token Counting

Accurate token counting is critical for billing and resource management.

### TikToken Integration

For OpenAI models, we use the official tiktoken library:

```rust
use tiktoken_rs::{get_bpe_from_model, get_completion_max_tokens};

pub fn count_tokens(model: &str, text: &str) -> Result<usize> {
    let bpe = get_bpe_from_model(model)?;
    Ok(bpe.encode_ordinary(text).len())
}
```

### Model-Specific Counting

Different models use different tokenizers:

| Model Family | Tokenizer | Avg Tokens/Word |
|-------------|-----------|-----------------|
| GPT-4       | cl100k_base | ~1.3 |
| GPT-3.5     | cl100k_base | ~1.3 |
| Llama 2     | SentencePiece | ~1.5 |
| Mistral     | SentencePiece | ~1.4 |

### Estimation Fallback

For models without precise tokenizers:

```rust
pub fn estimate_tokens(text: &str) -> u32 {
    let word_count = text.split_whitespace().count();
    let char_count = text.chars().count();
    
    // Heuristic: average of word-based and character-based estimates
    let word_estimate = (word_count as f32 * 1.3) as u32;
    let char_estimate = (char_count as f32 / 4.0) as u32;
    
    (word_estimate + char_estimate) / 2
}
```

## Model Selection

### Dynamic Model Discovery

Executors can advertise available models:

```rust
impl Executor {
    pub async fn advertise_models(&mut self) -> Result<()> {
        let models = self.llm_client.list_models().await?;
        
        for model in models {
            self.available_models.insert(
                model.id.clone(),
                ModelInfo {
                    name: model.id,
                    context_length: model.context_length,
                    pricing: self.config.get_model_pricing(&model.id),
                }
            );
        }
        
        // Broadcast updated model list to network
        self.broadcast_model_availability().await?;
        Ok(())
    }
}
```

### Model Routing

Clients can request specific models or categories:

```rust
pub enum ModelRequest {
    Specific(String),           // "gpt-4", "llama-2-70b"
    Category(ModelCategory),    // Fast, Balanced, Quality
    Cheapest,                   // Lowest cost per token
    Fastest,                    // Lowest latency
}
```

## Performance Optimization

### Request Batching

For backends that support it:

```rust
pub struct BatchedRequest {
    pub requests: Vec<LlmRequest>,
    pub batch_size: usize,
}

impl LlmBackend for BatchedBackend {
    async fn complete_batch(&self, batch: BatchedRequest) -> Result<Vec<LlmResponse>> {
        // Process multiple requests in single API call
    }
}
```

### Caching

Response caching for identical requests:

```rust
pub struct CachedBackend<B: LlmBackend> {
    backend: B,
    cache: Arc<RwLock<LruCache<RequestHash, LlmResponse>>>,
}

impl<B: LlmBackend> CachedBackend<B> {
    pub async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        let hash = hash_request(&request);
        
        // Check cache first
        if let Some(cached) = self.cache.read().await.get(&hash) {
            return Ok(cached.clone());
        }
        
        // Process and cache
        let response = self.backend.complete(request).await?;
        self.cache.write().await.put(hash, response.clone());
        Ok(response)
    }
}
```

### Streaming Responses

For real-time applications:

```rust
pub async fn stream_completion(
    &self,
    request: LlmRequest,
) -> Result<impl Stream<Item = Result<StreamChunk>>> {
    let stream = self.backend.stream_complete(request).await?;
    
    Ok(stream.map(|chunk| {
        // Process each chunk
        chunk.map(|c| StreamChunk {
            content: c.delta,
            finish_reason: c.finish_reason,
        })
    }))
}
```

## Error Handling

### Retry Logic

Automatic retry with exponential backoff:

```rust
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub exponential_base: f32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            exponential_base: 2.0,
        }
    }
}
```

### Fallback Strategies

When primary backend fails:

```rust
pub enum FallbackStrategy {
    Secondary(Box<dyn LlmBackend>),    // Use backup backend
    Degrade(DegradedMode),             // Reduce quality/features
    Queue(QueueConfig),                // Queue for later processing
    Reject(ErrorResponse),             // Return error to client
}
```

## Monitoring and Metrics

### Backend Health Checks

Regular health monitoring:

```rust
pub async fn monitor_backend_health(backend: &dyn LlmBackend) {
    loop {
        match backend.health_check().await {
            Ok(healthy) => {
                metrics::gauge!("llm_backend_health", if healthy { 1.0 } else { 0.0 });
            }
            Err(e) => {
                metrics::increment_counter!("llm_backend_errors");
                tracing::error!("Backend health check failed: {}", e);
            }
        }
        
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
```

### Performance Metrics

Track backend performance:

```rust
pub struct BackendMetrics {
    pub request_count: Counter,
    pub request_duration: Histogram,
    pub token_usage: Histogram,
    pub error_rate: Gauge,
}

impl BackendMetrics {
    pub fn record_request(&self, duration: Duration, tokens: u32, success: bool) {
        self.request_count.increment(1);
        self.request_duration.record(duration.as_secs_f64());
        self.token_usage.record(tokens as f64);
        
        if !success {
            self.error_rate.increment(1.0);
        }
    }
}
```

## Security Considerations

### API Key Management

Secure storage and rotation:

```rust
pub struct SecureApiKeyStore {
    keys: Arc<RwLock<HashMap<String, EncryptedKey>>>,
    rotation_interval: Duration,
}

impl SecureApiKeyStore {
    pub async fn get_key(&self, backend: &str) -> Result<String> {
        let keys = self.keys.read().await;
        let encrypted = keys.get(backend)
            .ok_or_else(|| anyhow!("No API key for backend: {}", backend))?;
        
        // Decrypt key using system keyring or HSM
        decrypt_api_key(encrypted)
    }
}
```

### Request Sanitization

Clean potentially harmful content:

```rust
pub fn sanitize_request(request: &mut LlmRequest) -> Result<()> {
    // Remove potential prompt injections
    for message in &mut request.messages {
        message.content = sanitize_content(&message.content)?;
    }
    
    // Validate model name
    if !is_valid_model_name(&request.model) {
        return Err(anyhow!("Invalid model name"));
    }
    
    // Enforce reasonable limits
    if let Some(max_tokens) = request.max_tokens {
        request.max_tokens = Some(max_tokens.min(MAX_ALLOWED_TOKENS));
    }
    
    Ok(())
}
```

## Configuration Examples

### Multi-Backend Setup

```toml
[[executors.backends]]
name = "primary"
type = "openai"
base_url = "https://api.openai.com/v1"
api_key = "${OPENAI_API_KEY}"
models = ["gpt-4", "gpt-3.5-turbo"]

[[executors.backends]]
name = "local"
type = "lmstudio"
base_url = "http://localhost:1234/v1"
models = ["llama-2-13b", "mistral-7b"]

[[executors.backends]]
name = "fallback"
type = "openai"
base_url = "https://api.openai.com/v1"
api_key = "${OPENAI_API_KEY_BACKUP}"
models = ["gpt-3.5-turbo"]
```

### Load Balancing

```toml
[executors.load_balancing]
strategy = "round_robin"  # or "least_connections", "weighted"
health_check_interval_secs = 30
failure_threshold = 3

[[executors.load_balancing.weights]]
backend = "primary"
weight = 70

[[executors.load_balancing.weights]]
backend = "local"
weight = 30
```

## Troubleshooting

### Common Issues

1. **Timeout Errors**
   - Increase `timeout_secs` in configuration
   - Check network connectivity
   - Verify backend is not overloaded

2. **Authentication Failures**
   - Verify API keys are correctly set
   - Check key permissions and quotas
   - Ensure proper environment variable expansion

3. **Model Not Found**
   - Run model discovery to list available models
   - Check model name spelling
   - Verify model is loaded in backend

4. **Token Counting Mismatch**
   - Ensure correct tokenizer for model
   - Account for special tokens
   - Verify encoding matches backend

### Debug Logging

Enable detailed backend logging:

```toml
[logging]
level = "debug"
targets = ["lloom_executor::llm_client"]
```

## Future Enhancements

### Planned Features

1. **Function Calling Support**
   - OpenAI function calling
   - Tool use for supported models

2. **Fine-tuned Model Management**
   - Upload and serve custom models
   - Version control for models

3. **Advanced Routing**
   - Cost-based routing
   - Quality-based routing
   - Latency-optimized routing

4. **Model Quantization**
   - Automatic quantization for efficiency
   - Dynamic bit-width selection

5. **Federated Learning**
   - Contribute to model improvement
   - Privacy-preserving training