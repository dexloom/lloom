# Custom Executor Backend

This guide demonstrates how to implement custom LLM backends for the Lloom executor, allowing you to integrate any LLM service or model.

## Basic Custom Backend

### Implementing the LLM Client Trait

Create a custom backend by implementing the `LlmClient` trait:

```rust
use lloom_executor::{LlmClient, LlmRequest, LlmResponse, ModelInfo};
use async_trait::async_trait;
use anyhow::Result;

pub struct MyCustomLlm {
    api_endpoint: String,
    api_key: String,
    client: reqwest::Client,
}

#[async_trait]
impl LlmClient for MyCustomLlm {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        // Prepare API request
        let api_request = serde_json::json!({
            "model": request.model,
            "messages": [
                {
                    "role": "system",
                    "content": request.system_prompt.unwrap_or_default()
                },
                {
                    "role": "user",
                    "content": request.prompt
                }
            ],
            "max_tokens": request.max_tokens.unwrap_or(1000),
            "temperature": request.temperature.unwrap_or(0.7),
        });
        
        // Make API call
        let response = self.client
            .post(&format!("{}/completions", self.api_endpoint))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&api_request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("API error: {}", error_text));
        }
        
        // Parse response
        let api_response: ApiResponse = response.json().await?;
        
        Ok(LlmResponse {
            content: api_response.choices[0].message.content.clone(),
            model: request.model,
            prompt_tokens: api_response.usage.prompt_tokens,
            completion_tokens: api_response.usage.completion_tokens,
            total_tokens: api_response.usage.total_tokens,
        })
    }
    
    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let response = self.client
            .get(&format!("{}/models", self.api_endpoint))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;
        
        let models: ModelsResponse = response.json().await?;
        
        Ok(models.data.into_iter().map(|m| ModelInfo {
            id: m.id,
            context_length: m.context_length.unwrap_or(2048),
        }).collect())
    }
    
    async fn health_check(&self) -> Result<bool> {
        let response = self.client
            .get(&format!("{}/health", self.api_endpoint))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;
        
        Ok(response.is_ok())
    }
}

// Response structures
#[derive(serde::Deserialize)]
struct ApiResponse {
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(serde::Deserialize)]
struct Choice {
    message: Message,
}

#[derive(serde::Deserialize)]
struct Message {
    content: String,
}

#[derive(serde::Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(serde::Deserialize)]
struct ModelsResponse {
    data: Vec<Model>,
}

#[derive(serde::Deserialize)]
struct Model {
    id: String,
    context_length: Option<u32>,
}
```

### Using the Custom Backend

```rust
use lloom_executor::{Executor, ExecutorConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Create custom LLM client
    let custom_llm = MyCustomLlm {
        api_endpoint: "https://api.mycustomllm.com/v1".to_string(),
        api_key: std::env::var("CUSTOM_LLM_API_KEY")?,
        client: reqwest::Client::new(),
    };
    
    // Create executor with custom backend
    let mut config = ExecutorConfig::default();
    let mut executor = Executor::new(config).await?;
    executor.set_llm_client(Box::new(custom_llm))?;
    
    // Run executor
    executor.run().await?;
    
    Ok(())
}
```

## Advanced Custom Backend

### With Streaming Support

Implement streaming for real-time responses:

```rust
use futures::Stream;
use tokio::sync::mpsc;

pub struct StreamingLlm {
    // ... fields
}

impl StreamingLlm {
    pub async fn complete_stream(
        &self,
        request: LlmRequest,
    ) -> Result<impl Stream<Item = Result<String>>> {
        let (tx, rx) = mpsc::channel(100);
        
        // Spawn task to handle streaming
        let api_endpoint = self.api_endpoint.clone();
        let api_key = self.api_key.clone();
        
        tokio::spawn(async move {
            if let Err(e) = stream_response(api_endpoint, api_key, request, tx).await {
                eprintln!("Stream error: {}", e);
            }
        });
        
        Ok(tokio_stream::wrappers::ReceiverStream::new(rx))
    }
}

async fn stream_response(
    endpoint: String,
    api_key: String,
    request: LlmRequest,
    tx: mpsc::Sender<Result<String>>,
) -> Result<()> {
    let client = reqwest::Client::new();
    
    let mut response = client
        .post(&format!("{}/completions/stream", endpoint))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": request.model,
            "messages": [{
                "role": "user",
                "content": request.prompt
            }],
            "stream": true,
        }))
        .send()
        .await?;
    
    let mut buffer = String::new();
    
    while let Some(chunk) = response.chunk().await? {
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        
        // Parse SSE events
        while let Some(line_end) = buffer.find('\n') {
            let line = buffer[..line_end].trim();
            buffer = buffer[line_end + 1..].to_string();
            
            if line.starts_with("data: ") {
                let data = &line[6..];
                if data != "[DONE]" {
                    if let Ok(json) = serde_json::from_str::<StreamChunk>(data) {
                        if let Some(content) = json.choices[0].delta.content.as_ref() {
                            tx.send(Ok(content.clone())).await?;
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}

#[derive(serde::Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(serde::Deserialize)]
struct StreamChoice {
    delta: Delta,
}

#[derive(serde::Deserialize)]
struct Delta {
    content: Option<String>,
}
```

### With Token Counting

Implement accurate token counting:

```rust
use tiktoken_rs::CoreBPE;
use std::sync::Arc;

pub struct TokenAwareLlm {
    backend: Box<dyn LlmClient>,
    tokenizers: Arc<TokenizerCache>,
}

struct TokenizerCache {
    tokenizers: std::sync::RwLock<HashMap<String, CoreBPE>>,
}

impl TokenizerCache {
    fn get_or_create(&self, model: &str) -> Result<CoreBPE> {
        // Check cache
        if let Some(tokenizer) = self.tokenizers.read().unwrap().get(model) {
            return Ok(tokenizer.clone());
        }
        
        // Create new tokenizer
        let tokenizer = match model {
            m if m.starts_with("gpt-3.5") => tiktoken_rs::cl100k_base()?,
            m if m.starts_with("gpt-4") => tiktoken_rs::cl100k_base()?,
            m if m.contains("llama") => {
                // Use sentencepiece for Llama models
                create_llama_tokenizer(m)?
            }
            _ => {
                // Fallback to estimation
                return Err(anyhow::anyhow!("No tokenizer for model: {}", model));
            }
        };
        
        // Cache it
        self.tokenizers.write().unwrap().insert(model.to_string(), tokenizer.clone());
        
        Ok(tokenizer)
    }
}

#[async_trait]
impl LlmClient for TokenAwareLlm {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        // Count input tokens accurately
        let tokenizer = self.tokenizers.get_or_create(&request.model)?;
        
        let system_tokens = request.system_prompt
            .as_ref()
            .map(|p| tokenizer.encode_ordinary(p).len())
            .unwrap_or(0);
        
        let prompt_tokens = tokenizer.encode_ordinary(&request.prompt).len();
        let total_input_tokens = system_tokens + prompt_tokens;
        
        // Make request
        let mut response = self.backend.complete(request).await?;
        
        // Verify token counts
        if response.prompt_tokens != total_input_tokens as u32 {
            tracing::warn!(
                "Token count mismatch: reported {}, calculated {}",
                response.prompt_tokens,
                total_input_tokens
            );
            response.prompt_tokens = total_input_tokens as u32;
        }
        
        // Count output tokens
        let output_tokens = tokenizer.encode_ordinary(&response.content).len();
        if response.completion_tokens != output_tokens as u32 {
            response.completion_tokens = output_tokens as u32;
        }
        
        response.total_tokens = response.prompt_tokens + response.completion_tokens;
        
        Ok(response)
    }
    
    // ... other methods
}
```

## Local Model Integration

### Llama.cpp Backend

Integrate local models using llama.cpp:

```rust
use std::process::Stdio;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};

pub struct LlamaCppBackend {
    model_path: String,
    context_size: u32,
    n_gpu_layers: u32,
}

#[async_trait]
impl LlmClient for LlamaCppBackend {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        let prompt = format!(
            "[INST] <<SYS>>\n{}\n<</SYS>>\n\n{} [/INST]",
            request.system_prompt.unwrap_or_default(),
            request.prompt
        );
        
        let mut child = Command::new("llama.cpp/main")
            .args(&[
                "-m", &self.model_path,
                "-p", &prompt,
                "-n", &request.max_tokens.unwrap_or(512).to_string(),
                "-c", &self.context_size.to_string(),
                "-ngl", &self.n_gpu_layers.to_string(),
                "--temp", &request.temperature.unwrap_or(0.7).to_string(),
                "-t", "4", // threads
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        
        let stdout = child.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        
        let mut response_text = String::new();
        while let Some(line) = lines.next_line().await? {
            response_text.push_str(&line);
            response_text.push('\n');
        }
        
        // Wait for process to complete
        let status = child.wait().await?;
        if !status.success() {
            return Err(anyhow::anyhow!("Llama.cpp failed with status: {}", status));
        }
        
        // Estimate tokens (llama.cpp doesn't always report exact counts)
        let prompt_tokens = (prompt.len() / 4) as u32; // Rough estimate
        let completion_tokens = (response_text.len() / 4) as u32;
        
        Ok(LlmResponse {
            content: response_text.trim().to_string(),
            model: request.model,
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        })
    }
    
    // ... other methods
}
```

### ONNX Runtime Backend

Use ONNX models for cross-platform compatibility:

```rust
use ort::{Environment, SessionBuilder, Value};
use ndarray::Array;

pub struct OnnxBackend {
    environment: Arc<Environment>,
    session: Arc<ort::Session>,
    tokenizer: Arc<Tokenizer>,
}

impl OnnxBackend {
    pub fn new(model_path: &str) -> Result<Self> {
        let environment = Environment::builder()
            .with_name("lloom")
            .build()?
            .into_arc();
        
        let session = SessionBuilder::new(&environment)?
            .with_model_from_file(model_path)?
            .into_arc();
        
        let tokenizer = Arc::new(Tokenizer::from_file("tokenizer.json")?);
        
        Ok(Self {
            environment,
            session,
            tokenizer,
        })
    }
}

#[async_trait]
impl LlmClient for OnnxBackend {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        // Tokenize input
        let encoding = self.tokenizer.encode(&request.prompt, false)?;
        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();
        
        // Prepare inputs
        let input_ids_array = Array::from_vec(input_ids.to_vec())
            .into_shape((1, input_ids.len()))?;
        let attention_mask_array = Array::from_vec(attention_mask.to_vec())
            .into_shape((1, attention_mask.len()))?;
        
        // Run inference
        let outputs = self.session.run(vec![
            Value::from_array(self.session.allocator(), &input_ids_array)?,
            Value::from_array(self.session.allocator(), &attention_mask_array)?,
        ])?;
        
        // Decode output
        let output_ids = outputs[0].try_extract::<f32>()?.view().to_vec();
        let output_tokens = decode_output_ids(&output_ids, &self.tokenizer)?;
        
        Ok(LlmResponse {
            content: output_tokens,
            model: request.model,
            prompt_tokens: input_ids.len() as u32,
            completion_tokens: output_ids.len() as u32,
            total_tokens: (input_ids.len() + output_ids.len()) as u32,
        })
    }
    
    // ... other methods
}
```

## Multi-Backend Router

### Load Balancing Across Backends

Route requests to multiple backends:

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct MultiBackendRouter {
    backends: Vec<(String, Box<dyn LlmClient + Send + Sync>)>,
    strategy: RoutingStrategy,
    current_index: AtomicUsize,
}

pub enum RoutingStrategy {
    RoundRobin,
    LeastConnections,
    ModelAffinity,
    CostOptimized,
}

#[async_trait]
impl LlmClient for MultiBackendRouter {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        let backend = match self.strategy {
            RoutingStrategy::RoundRobin => {
                let index = self.current_index.fetch_add(1, Ordering::Relaxed) % self.backends.len();
                &self.backends[index].1
            }
            RoutingStrategy::ModelAffinity => {
                // Route based on model
                self.backends.iter()
                    .find(|(name, backend)| {
                        // Check if backend supports the model
                        match backend.list_models().await {
                            Ok(models) => models.iter().any(|m| m.id == request.model),
                            Err(_) => false,
                        }
                    })
                    .map(|(_, backend)| backend)
                    .unwrap_or(&self.backends[0].1)
            }
            // ... other strategies
        };
        
        backend.complete(request).await
    }
    
    // ... other methods
}
```

### Fallback Handling

Implement fallback for reliability:

```rust
pub struct FallbackBackend {
    primary: Box<dyn LlmClient + Send + Sync>,
    fallbacks: Vec<Box<dyn LlmClient + Send + Sync>>,
}

#[async_trait]
impl LlmClient for FallbackBackend {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        // Try primary first
        match self.primary.complete(request.clone()).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                tracing::warn!("Primary backend failed: {}", e);
            }
        }
        
        // Try fallbacks in order
        for (index, fallback) in self.fallbacks.iter().enumerate() {
            tracing::info!("Trying fallback backend {}", index + 1);
            
            match fallback.complete(request.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    tracing::warn!("Fallback {} failed: {}", index + 1, e);
                }
            }
        }
        
        Err(anyhow::anyhow!("All backends failed"))
    }
    
    // ... other methods
}
```

## Testing Custom Backends

### Mock Backend for Testing

```rust
use std::sync::Mutex;

pub struct MockLlmBackend {
    responses: Arc<Mutex<VecDeque<LlmResponse>>>,
    should_fail: Arc<Mutex<bool>>,
}

impl MockLlmBackend {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::new())),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }
    
    pub fn add_response(&self, response: LlmResponse) {
        self.responses.lock().unwrap().push_back(response);
    }
    
    pub fn set_failure(&self, should_fail: bool) {
        *self.should_fail.lock().unwrap() = should_fail;
    }
}

#[async_trait]
impl LlmClient for MockLlmBackend {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        if *self.should_fail.lock().unwrap() {
            return Err(anyhow::anyhow!("Mock failure"));
        }
        
        if let Some(response) = self.responses.lock().unwrap().pop_front() {
            Ok(response)
        } else {
            // Generate default response
            Ok(LlmResponse {
                content: format!("Mock response to: {}", request.prompt),
                model: request.model,
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            })
        }
    }
    
    // ... other methods
}
```

### Integration Test

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_custom_backend() {
        // Create mock backend
        let mock = MockLlmBackend::new();
        mock.add_response(LlmResponse {
            content: "Test response".to_string(),
            model: "test-model".to_string(),
            prompt_tokens: 5,
            completion_tokens: 10,
            total_tokens: 15,
        });
        
        // Create executor with mock
        let mut executor = create_test_executor().await;
        executor.set_llm_client(Box::new(mock)).unwrap();
        
        // Test request
        let request = LlmRequest {
            model: "test-model".to_string(),
            prompt: "Test prompt".to_string(),
            // ... other fields
        };
        
        let response = executor.process_request(request).await.unwrap();
        assert_eq!(response.content, "Test response");
        assert_eq!(response.total_tokens, 15);
    }
}
```

## Performance Considerations

### Connection Pooling

```rust
pub struct PooledBackend {
    pool: Arc<Pool<reqwest::Client>>,
    endpoint: String,
}

impl PooledBackend {
    pub fn new(endpoint: String, pool_size: usize) -> Result<Self> {
        let pool = Pool::builder()
            .max_size(pool_size)
            .build(|| async {
                Ok(reqwest::Client::builder()
                    .timeout(Duration::from_secs(300))
                    .build()?)
            })?;
        
        Ok(Self {
            pool: Arc::new(pool),
            endpoint,
        })
    }
}
```

### Request Batching

```rust
pub struct BatchingBackend {
    inner: Box<dyn LlmClient + Send + Sync>,
    batch_queue: Arc<Mutex<Vec<PendingRequest>>>,
    batch_size: usize,
    batch_timeout: Duration,
}

struct PendingRequest {
    request: LlmRequest,
    response_tx: oneshot::Sender<Result<LlmResponse>>,
}

impl BatchingBackend {
    async fn process_batch(&self) {
        let mut batch = Vec::new();
        
        {
            let mut queue = self.batch_queue.lock().unwrap();
            while batch.len() < self.batch_size && !queue.is_empty() {
                batch.push(queue.remove(0));
            }
        }
        
        if batch.is_empty() {
            return;
        }
        
        // Process batch in parallel
        let futures: Vec<_> = batch.into_iter()
            .map(|pending| {
                let inner = self.inner.clone();
                async move {
                    let result = inner.complete(pending.request).await;
                    let _ = pending.response_tx.send(result);
                }
            })
            .collect();
        
        futures::future::join_all(futures).await;
    }
}
```

## Deployment Example

Complete example of deploying a custom executor:

```rust
use lloom_executor::{Executor, ExecutorConfig};
use clap::Parser;

#[derive(Parser)]
struct Args {
    #[clap(long, env = "BACKEND_TYPE")]
    backend: String,
    
    #[clap(long, env = "MODEL_PATH")]
    model_path: Option<String>,
    
    #[clap(long, env = "API_ENDPOINT")]
    api_endpoint: Option<String>,
    
    #[clap(long, env = "API_KEY")]
    api_key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Create appropriate backend
    let llm_client: Box<dyn LlmClient + Send + Sync> = match args.backend.as_str() {
        "custom-api" => {
            Box::new(MyCustomLlm {
                api_endpoint: args.api_endpoint.expect("API endpoint required"),
                api_key: args.api_key.expect("API key required"),
                client: reqwest::Client::new(),
            })
        }
        "llama-cpp" => {
            Box::new(LlamaCppBackend {
                model_path: args.model_path.expect("Model path required"),
                context_size: 4096,
                n_gpu_layers: 35,
            })
        }
        "onnx" => {
            Box::new(OnnxBackend::new(
                &args.model_path.expect("Model path required")
            )?)
        }
        _ => {
            return Err(anyhow::anyhow!("Unknown backend type: {}", args.backend));
        }
    };
    
    // Create and run executor
    let config = ExecutorConfig::default();
    let mut executor = Executor::new(config).await?;
    executor.set_llm_client(llm_client)?;
    
    tracing::info!("Starting executor with {} backend", args.backend);
    executor.run().await?;
    
    Ok(())
}
```