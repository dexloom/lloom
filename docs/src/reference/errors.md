# Error Reference

This reference documents all error types, codes, and messages in the Lloom system, along with their causes and solutions.

## Error Categories

### Network Errors (1xxx)

#### Error 1001: Connection Failed
- **Message**: `Failed to connect to peer: {peer_id}`
- **Cause**: Unable to establish connection to a peer
- **Solutions**:
  - Check network connectivity
  - Verify peer is online
  - Check firewall settings
  - Ensure correct bootstrap peers

```rust
match client.connect(peer_id).await {
    Err(NetworkError::ConnectionFailed { peer_id, reason }) => {
        log::error!("Connection to {} failed: {}", peer_id, reason);
        // Retry with exponential backoff
    }
    _ => {}
}
```

#### Error 1002: No Peers Available
- **Message**: `No peers available in the network`
- **Cause**: No peers discovered or connected
- **Solutions**:
  - Add bootstrap peers
  - Enable mDNS for local discovery
  - Check network configuration
  - Wait for peer discovery

#### Error 1003: Request Timeout
- **Message**: `Request timed out after {duration}s`
- **Cause**: No response received within timeout period
- **Solutions**:
  - Increase timeout value
  - Check network latency
  - Verify executor availability
  - Retry request

#### Error 1004: Protocol Mismatch
- **Message**: `Protocol version mismatch: expected {expected}, got {actual}`
- **Cause**: Incompatible protocol versions between peers
- **Solutions**:
  - Update to compatible version
  - Check network ID configuration

### Request Errors (2xxx)

#### Error 2001: Invalid Request
- **Message**: `Invalid request format: {details}`
- **Cause**: Malformed or incomplete request data
- **Solutions**:
  - Check request structure
  - Validate required fields
  - Ensure proper encoding

```rust
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("Invalid model specified: {0}")]
    InvalidModel(String),
    
    #[error("Prompt exceeds maximum length: {actual} > {max}")]
    PromptTooLong { actual: usize, max: usize },
    
    #[error("Missing required field: {0}")]
    MissingField(String),
}
```

#### Error 2002: No Executors Available
- **Message**: `No executors available for model: {model}`
- **Cause**: No executors offering the requested model
- **Solutions**:
  - Try a different model
  - Wait for executors to come online
  - Check model name spelling

#### Error 2003: Request Rejected
- **Message**: `Request rejected by executor: {reason}`
- **Cause**: Executor declined to process request
- **Possible Reasons**:
  - Insufficient capacity
  - Price disagreement
  - Rate limiting
  - Blacklisted client

#### Error 2004: Invalid Signature
- **Message**: `Invalid request signature`
- **Cause**: Request signature verification failed
- **Solutions**:
  - Check identity configuration
  - Ensure clock synchronization
  - Verify signing implementation

### Execution Errors (3xxx)

#### Error 3001: LLM Backend Error
- **Message**: `LLM backend error: {details}`
- **Cause**: Error from underlying LLM service
- **Solutions**:
  - Check API credentials
  - Verify backend health
  - Check rate limits
  - Retry with backoff

#### Error 3002: Resource Exhausted
- **Message**: `Resource exhausted: {resource_type}`
- **Cause**: System resources depleted
- **Resource Types**:
  - Memory
  - GPU memory
  - Queue space
  - File handles
- **Solutions**:
  - Reduce concurrent requests
  - Increase resource limits
  - Add more executors

#### Error 3003: Model Not Found
- **Message**: `Model not found: {model}`
- **Cause**: Requested model not available
- **Solutions**:
  - Check model name
  - Verify model is loaded
  - Use `list-models` command

#### Error 3004: Context Length Exceeded
- **Message**: `Context length exceeded: {tokens} > {max_tokens}`
- **Cause**: Input too long for model
- **Solutions**:
  - Reduce prompt length
  - Use a model with larger context
  - Split into multiple requests

### Validation Errors (4xxx)

#### Error 4001: Validation Failed
- **Message**: `Response validation failed: {reason}`
- **Cause**: Response doesn't meet validation criteria
- **Common Reasons**:
  - Token count mismatch
  - Content hash mismatch
  - Timing violation
  - Model mismatch

#### Error 4002: Insufficient Validators
- **Message**: `Insufficient validators: {available} < {required}`
- **Cause**: Not enough validators for consensus
- **Solutions**:
  - Wait for more validators
  - Reduce consensus requirements
  - Use different network

#### Error 4003: Consensus Failed
- **Message**: `Validator consensus failed: {agreement}% < {threshold}%`
- **Cause**: Validators disagree on response validity
- **Solutions**:
  - Investigate discrepancy
  - Retry request
  - Report suspicious behavior

### Blockchain Errors (5xxx)

#### Error 5001: Transaction Failed
- **Message**: `Blockchain transaction failed: {reason}`
- **Cause**: Smart contract interaction failed
- **Common Reasons**:
  - Insufficient gas
  - Nonce too low
  - Contract reverted
  - Network congestion

```rust
match contract.submit_request(commitment).await {
    Err(BlockchainError::InsufficientGas { required, available }) => {
        log::error!("Need {} gas, have {}", required, available);
    }
    Err(BlockchainError::ContractReverted { reason }) => {
        log::error!("Contract reverted: {}", reason);
    }
    _ => {}
}
```

#### Error 5002: Signature Verification Failed
- **Message**: `On-chain signature verification failed`
- **Cause**: EIP-712 signature invalid
- **Solutions**:
  - Check domain separator
  - Verify signing implementation
  - Ensure correct chain ID

#### Error 5003: Payment Failed
- **Message**: `Payment settlement failed: {reason}`
- **Cause**: Unable to settle payment
- **Solutions**:
  - Check account balance
  - Verify payment amount
  - Ensure contract approval

### Configuration Errors (6xxx)

#### Error 6001: Invalid Configuration
- **Message**: `Invalid configuration: {field} - {reason}`
- **Cause**: Configuration validation failed
- **Solutions**:
  - Check configuration syntax
  - Verify required fields
  - Use example configurations

#### Error 6002: Missing Configuration
- **Message**: `Required configuration missing: {field}`
- **Cause**: Required configuration not provided
- **Solutions**:
  - Add missing field
  - Check environment variables
  - Use default configuration

#### Error 6003: Configuration Conflict
- **Message**: `Configuration conflict: {details}`
- **Cause**: Incompatible configuration options
- **Example**: Enabling both OpenAI and LMStudio backends

### System Errors (7xxx)

#### Error 7001: Storage Error
- **Message**: `Storage operation failed: {operation} - {reason}`
- **Cause**: Database or filesystem error
- **Solutions**:
  - Check disk space
  - Verify permissions
  - Check database integrity

#### Error 7002: Identity Error
- **Message**: `Identity operation failed: {reason}`
- **Cause**: Problem with identity management
- **Solutions**:
  - Check identity file
  - Generate new identity
  - Verify file permissions

#### Error 7003: Initialization Failed
- **Message**: `Component initialization failed: {component} - {reason}`
- **Cause**: Unable to start component
- **Solutions**:
  - Check dependencies
  - Verify configuration
  - Check system resources

## Error Handling Best Practices

### Structured Error Types

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LloomError {
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),
    
    #[error("Request error: {0}")]
    Request(#[from] RequestError),
    
    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),
    
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),
    
    #[error("Blockchain error: {0}")]
    Blockchain(#[from] BlockchainError),
    
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("System error: {0}")]
    System(#[from] SystemError),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}
```

### Error Context

```rust
use anyhow::{Context, Result};

async fn process_request(request: Request) -> Result<Response> {
    let executor = find_executor(&request.model)
        .await
        .context("Failed to find suitable executor")?;
    
    let response = executor
        .execute(request)
        .await
        .with_context(|| format!("Execution failed on executor {}", executor.id))?;
    
    validate_response(&response)
        .context("Response validation failed")?;
    
    Ok(response)
}
```

### Error Recovery

```rust
async fn with_retry<T, F, Fut>(
    f: F,
    max_retries: u32,
) -> Result<T, LloomError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, LloomError>>,
{
    let mut last_error = None;
    
    for attempt in 0..=max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if e.is_retryable() => {
                last_error = Some(e);
                let delay = 2u64.pow(attempt) * 100;
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            Err(e) => return Err(e),
        }
    }
    
    Err(last_error.unwrap())
}
```

### Error Reporting

```rust
impl LloomError {
    pub fn error_code(&self) -> u32 {
        match self {
            LloomError::Network(NetworkError::ConnectionFailed { .. }) => 1001,
            LloomError::Network(NetworkError::NoPeers) => 1002,
            LloomError::Request(RequestError::InvalidModel(_)) => 2001,
            // ... other mappings
        }
    }
    
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "error": {
                "code": self.error_code(),
                "message": self.to_string(),
                "type": self.error_type(),
                "retryable": self.is_retryable(),
            }
        })
    }
}
```

## Client Error Handling

### JavaScript/TypeScript

```typescript
class LloomError extends Error {
    constructor(
        public code: number,
        public message: string,
        public retryable: boolean = false
    ) {
        super(message);
    }
}

async function handleRequest(request: Request): Promise<Response> {
    try {
        return await client.complete(request);
    } catch (error) {
        if (error instanceof LloomError) {
            if (error.code === 1003 && error.retryable) {
                // Retry with backoff
                return await retryWithBackoff(() => client.complete(request));
            }
            
            switch (error.code) {
                case 2002:
                    console.error("No executors available, try later");
                    break;
                case 5001:
                    console.error("Blockchain error:", error.message);
                    break;
                default:
                    console.error("Request failed:", error);
            }
        }
        throw error;
    }
}
```

### Python

```python
class LloomError(Exception):
    def __init__(self, code: int, message: str, retryable: bool = False):
        self.code = code
        self.message = message
        self.retryable = retryable
        super().__init__(message)

def handle_lloom_error(func):
    def wrapper(*args, **kwargs):
        try:
            return func(*args, **kwargs)
        except LloomError as e:
            if e.code == 1003 and e.retryable:
                # Implement retry logic
                return retry_with_backoff(func, *args, **kwargs)
            elif e.code == 2002:
                logger.error(f"No executors available: {e.message}")
            else:
                logger.error(f"Lloom error {e.code}: {e.message}")
            raise
    return wrapper
```

## Debugging Errors

### Enable Debug Logging

```bash
# Detailed error information
export RUST_LOG=lloom=debug,libp2p=debug

# Trace specific module
export RUST_LOG=lloom_executor::llm=trace
```

### Error Inspection

```rust
match result {
    Err(e) => {
        // Print full error chain
        eprintln!("Error: {}", e);
        
        let mut source = e.source();
        while let Some(err) = source {
            eprintln!("Caused by: {}", err);
            source = err.source();
        }
        
        // Print backtrace if available
        if let Some(backtrace) = e.backtrace() {
            eprintln!("Backtrace:\n{}", backtrace);
        }
    }
    _ => {}
}
```

## Common Error Scenarios

### Scenario: First-time Setup

```
Error 7002: Identity operation failed: File not found
Error 1002: No peers available in the network
```

**Solution**: Generate identity and add bootstrap peers

### Scenario: Overloaded Network

```
Error 2002: No executors available for model: gpt-4
Error 3002: Resource exhausted: queue space
Error 1003: Request timed out after 300s
```

**Solution**: Retry with backoff, use different model, or increase timeout

### Scenario: Configuration Issues

```
Error 6001: Invalid configuration: listen_addr - invalid multiaddr
Error 6003: Configuration conflict: multiple LLM backends specified
```

**Solution**: Fix configuration file, use examples as reference