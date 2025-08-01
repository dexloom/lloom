# Message Protocol

The protocol module defines the core message types and data structures used for communication in the Lloom network. It provides a standardized format for LLM requests, responses, and network messages.

## Overview

The Lloom protocol defines:
- **Request/Response Messages**: Structured formats for LLM interactions
- **Service Discovery**: Messages for finding and advertising services
- **Usage Records**: Accounting and billing information
- **Network Control**: Protocol-level control messages

## Core Message Types

### `LlmRequest`

Request for LLM completion:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    /// Model identifier (e.g., "gpt-3.5-turbo", "llama-2-13b")
    pub model: String,
    
    /// User prompt text
    pub prompt: String,
    
    /// Optional system prompt for behavior control
    pub system_prompt: Option<String>,
    
    /// Sampling temperature (0.0 - 2.0)
    pub temperature: Option<f32>,
    
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    
    /// Target executor's Ethereum address
    pub executor_address: String,
    
    /// Price per inbound token (wei)
    pub inbound_price: String,
    
    /// Price per outbound token (wei)
    pub outbound_price: String,
    
    /// Request nonce for replay protection
    pub nonce: u64,
    
    /// Unix timestamp deadline
    pub deadline: u64,
}
```

### `LlmResponse`

Response from LLM execution:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// Model actually used
    pub model: String,
    
    /// Generated content
    pub content: String,
    
    /// Number of tokens in prompt
    pub prompt_tokens: u32,
    
    /// Number of tokens generated
    pub completion_tokens: u32,
    
    /// Total token count
    pub total_tokens: u32,
    
    /// Client's Ethereum address
    pub client_address: String,
    
    /// Agreed inbound price
    pub inbound_price: String,
    
    /// Agreed outbound price
    pub outbound_price: String,
    
    /// Response timestamp
    pub timestamp: u64,
    
    /// Whether execution succeeded
    pub success: bool,
}
```

## Request Creation

### Basic Request

Create a simple LLM request:

```rust
use lloom_core::protocol::LlmRequest;
use chrono::Utc;

let request = LlmRequest {
    model: "gpt-3.5-turbo".to_string(),
    prompt: "Explain quantum computing in simple terms".to_string(),
    system_prompt: None,
    temperature: Some(0.7),
    max_tokens: Some(500),
    executor_address: "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string(),
    inbound_price: "500000000000000".to_string(),    // 0.0005 ETH per token
    outbound_price: "1000000000000000".to_string(), // 0.001 ETH per token
    nonce: 1,
    deadline: (Utc::now().timestamp() + 3600) as u64, // 1 hour from now
};
```

### Advanced Request Options

Create request with all options:

```rust
use lloom_core::protocol::{LlmRequest, RequestBuilder};

let request = RequestBuilder::new("gpt-4")
    .prompt("Write a technical analysis of Rust's ownership system")
    .system_prompt("You are an expert systems programmer")
    .temperature(0.3)  // Lower temperature for technical content
    .max_tokens(2000)
    .executor_address("0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a")
    .pricing(
        "1000000000000000",   // 0.001 ETH per inbound token
        "2000000000000000"    // 0.002 ETH per outbound token
    )
    .deadline_minutes(30)
    .build()?;
```

### Request Validation

Validate request before sending:

```rust
use lloom_core::protocol::{LlmRequest, validate_request};

let request = LlmRequest { /* ... */ };

match validate_request(&request) {
    Ok(()) => println!("Request is valid"),
    Err(e) => println!("Invalid request: {}", e),
}

// Specific validations
assert!(request.temperature.unwrap_or(1.0) >= 0.0);
assert!(request.temperature.unwrap_or(1.0) <= 2.0);
assert!(request.max_tokens.unwrap_or(100) <= 4096);
assert!(request.deadline > Utc::now().timestamp() as u64);
```

## Response Handling

### Creating Responses

Build response as an executor:

```rust
use lloom_core::protocol::{LlmResponse, ResponseBuilder};

let response = ResponseBuilder::new()
    .model("gpt-3.5-turbo")
    .content("Quantum computing uses quantum mechanics principles...")
    .token_usage(15, 127, 142)  // prompt, completion, total
    .client_address("0x5678...")
    .pricing(
        request.inbound_price.clone(),
        request.outbound_price.clone()
    )
    .success(true)
    .build();
```

### Response Validation

Verify response integrity:

```rust
use lloom_core::protocol::{validate_response, verify_response_matches_request};

// Basic validation
validate_response(&response)?;

// Verify response matches request
verify_response_matches_request(&request, &response)?;

// Check token counts
assert_eq!(
    response.total_tokens, 
    response.prompt_tokens + response.completion_tokens
);
```

## Message Envelopes

### Request/Response Messages

Wrapper types for network transmission:

```rust
use lloom_core::protocol::{RequestMessage, ResponseMessage};

// Wrap LLM request
let request_msg = RequestMessage::LlmRequest(signed_request);

// Other request types
let ping = RequestMessage::Ping;
let discover = RequestMessage::DiscoverServices {
    service_type: ServiceType::Executor,
    requirements: Some(Requirements {
        models: vec!["gpt-4".to_string()],
        min_capacity: 5,
    }),
};

// Response types
let response_msg = ResponseMessage::LlmResponse(signed_response);
let pong = ResponseMessage::Pong;
let error = ResponseMessage::Error("Invalid request".to_string());
```

### Signed Messages

All LLM requests and responses must be signed:

```rust
use lloom_core::protocol::{SignedLlmRequest, SignedLlmResponse};
use lloom_core::signing::SignableMessage;

// Sign request
let signed_request: SignedLlmRequest = request.sign(&identity.wallet).await?;

// Access signed components
println!("Signer: {}", signed_request.signer);
println!("Signature: 0x{}", hex::encode(&signed_request.signature));
println!("Timestamp: {}", signed_request.timestamp);

// Verify signature
use lloom_core::signing::verify_signed_message;
assert!(verify_signed_message(&signed_request)?);
```

## Usage Records

### Creating Usage Records

Track resource usage for billing:

```rust
use lloom_core::protocol::{UsageRecord, UsageType};

let usage = UsageRecord {
    record_type: UsageType::LlmExecution,
    client: client_address,
    executor: executor_address,
    model: "gpt-3.5-turbo".to_string(),
    prompt_tokens: 50,
    completion_tokens: 200,
    total_tokens: 250,
    inbound_price: "500000000000000".to_string(),
    outbound_price: "1000000000000000".to_string(),
    total_cost: "150000000000000000".to_string(), // 0.15 ETH
    timestamp: Utc::now().timestamp() as u64,
    request_hash: Some(request_hash),
    response_hash: Some(response_hash),
};
```

### Calculating Costs

Compute costs from usage:

```rust
use lloom_core::protocol::{calculate_cost, parse_wei};

let inbound_cost = calculate_cost(
    usage.prompt_tokens,
    &usage.inbound_price
)?;

let outbound_cost = calculate_cost(
    usage.completion_tokens,
    &usage.outbound_price
)?;

let total_cost = inbound_cost + outbound_cost;
assert_eq!(total_cost.to_string(), usage.total_cost);

// Convert to ETH for display
let eth_cost = parse_wei(&total_cost.to_string())?;
println!("Total cost: {} ETH", eth_cost);
```

## Service Discovery Protocol

### Service Types

Define available services:

```rust
use lloom_core::protocol::{ServiceType, ServiceInfo};

// Executor service
let executor_service = ServiceType::Executor;
let executor_info = ServiceInfo::Executor {
    peer_id: peer_id.clone(),
    models: vec![
        ModelInfo {
            name: "gpt-3.5-turbo".to_string(),
            context_length: 4096,
            capabilities: vec!["chat", "completion"],
        },
        ModelInfo {
            name: "llama-2-13b".to_string(),
            context_length: 4096,
            capabilities: vec!["chat", "instruct"],
        },
    ],
    capacity: 10,  // Concurrent requests
    pricing: PricingInfo {
        base_inbound_price: "500000000000000".to_string(),
        base_outbound_price: "1000000000000000".to_string(),
        surge_multiplier: 1.5,
    },
};

// Validator service
let validator_service = ServiceType::Validator;
let validator_info = ServiceInfo::Validator {
    peer_id: peer_id.clone(),
    stake_amount: "1000000000000000000".to_string(), // 1 ETH
    validation_rate: 0.1, // Validates 10% of transactions
    specializations: vec!["quality", "compliance"],
};
```

### Discovery Messages

Query for services:

```rust
use lloom_core::protocol::{DiscoveryRequest, DiscoveryResponse};

// Create discovery request
let request = DiscoveryRequest {
    service_type: ServiceType::Executor,
    filters: Some(DiscoveryFilters {
        required_models: vec!["gpt-4".to_string()],
        max_price: Some("2000000000000000".to_string()),
        min_capacity: Some(5),
        location: Some("us-east-1".to_string()),
    }),
    max_results: 10,
};

// Handle discovery response
let response = DiscoveryResponse {
    services: vec![
        ServiceInfo::Executor { /* ... */ },
        ServiceInfo::Executor { /* ... */ },
    ],
    total_available: 15,
    timestamp: Utc::now().timestamp() as u64,
};
```

## Network Control Messages

### Health Checks

Monitor peer health:

```rust
use lloom_core::protocol::{HealthCheck, HealthStatus};

// Send health check
let check = HealthCheck {
    timestamp: Utc::now().timestamp() as u64,
    request_id: generate_request_id(),
};

// Respond with status
let status = HealthStatus {
    healthy: true,
    load: 0.65, // 65% capacity used
    version: env!("CARGO_PKG_VERSION").to_string(),
    uptime_seconds: 86400,
    active_requests: 7,
    available_models: vec!["gpt-3.5-turbo", "llama-2-13b"],
};
```

### Rate Limiting

Implement rate limit messages:

```rust
use lloom_core::protocol::{RateLimitStatus, RateLimitExceeded};

// Check rate limit
let status = RateLimitStatus {
    requests_remaining: 45,
    reset_time: Utc::now().timestamp() as u64 + 3600,
    limit: 100,
    window_seconds: 3600,
};

// Rate limit exceeded response
let exceeded = RateLimitExceeded {
    retry_after: 300, // Retry after 5 minutes
    limit: 100,
    window: "1h".to_string(),
    message: "Rate limit exceeded, please retry later".to_string(),
};
```

## Protocol Versioning

### Version Negotiation

Handle protocol version compatibility:

```rust
use lloom_core::protocol::{ProtocolVersion, VersionNegotiation};

// Current protocol version
const CURRENT_VERSION: ProtocolVersion = ProtocolVersion {
    major: 1,
    minor: 0,
    patch: 0,
};

// Negotiate version
let negotiation = VersionNegotiation {
    supported_versions: vec![
        ProtocolVersion { major: 1, minor: 0, patch: 0 },
        ProtocolVersion { major: 1, minor: 1, patch: 0 },
    ],
    preferred_version: CURRENT_VERSION,
};

// Check compatibility
fn is_compatible(client_version: &ProtocolVersion, server_version: &ProtocolVersion) -> bool {
    // Major version must match
    client_version.major == server_version.major &&
    // Server minor version must be >= client
    server_version.minor >= client_version.minor
}
```

### Migration Support

Handle protocol updates:

```rust
use lloom_core::protocol::{migrate_request, ProtocolMigration};

// Migrate old format to new
let old_request = LlmRequestV1 { /* old fields */ };
let new_request = migrate_request(old_request, &CURRENT_VERSION)?;

// Register migration handlers
let migrations = ProtocolMigration::new()
    .register(
        ProtocolVersion { major: 1, minor: 0, patch: 0 },
        ProtocolVersion { major: 1, minor: 1, patch: 0 },
        |old: LlmRequestV1_0| -> Result<LlmRequestV1_1> {
            // Migration logic
            Ok(LlmRequestV1_1 {
                // Map fields
            })
        }
    );
```

## Serialization

### Binary Encoding

Efficient binary serialization:

```rust
use lloom_core::protocol::{encode_message, decode_message};

// Encode to bytes
let bytes = encode_message(&request)?;
println!("Encoded size: {} bytes", bytes.len());

// Decode from bytes
let decoded: LlmRequest = decode_message(&bytes)?;
assert_eq!(request, decoded);
```

### JSON Encoding

Human-readable JSON format:

```rust
use serde_json;

// Serialize to JSON
let json = serde_json::to_string_pretty(&request)?;
println!("Request JSON:\n{}", json);

// Parse from JSON
let parsed: LlmRequest = serde_json::from_str(&json)?;
```

### Custom Serialization

Implement custom formats:

```rust
use lloom_core::protocol::{ProtocolCodec, CodecType};

struct CompressedCodec;

impl ProtocolCodec for CompressedCodec {
    fn encode<T: Serialize>(&self, msg: &T) -> Result<Vec<u8>> {
        let json = serde_json::to_vec(msg)?;
        Ok(compress(&json)?)
    }
    
    fn decode<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T> {
        let json = decompress(data)?;
        Ok(serde_json::from_slice(&json)?)
    }
}
```

## Protocol Extensions

### Custom Fields

Add extension fields:

```rust
use lloom_core::protocol::{Extensions, ExtensionField};

let mut request = LlmRequest { /* ... */ };

// Add custom extensions
let mut extensions = Extensions::new();
extensions.add("priority", ExtensionField::String("high".to_string()));
extensions.add("session_id", ExtensionField::String(session_id));
extensions.add("retry_count", ExtensionField::Number(2));

// Attach to request
request.with_extensions(extensions);
```

### Plugin System

Extend protocol with plugins:

```rust
use lloom_core::protocol::{ProtocolPlugin, PluginContext};

struct LoggingPlugin;

impl ProtocolPlugin for LoggingPlugin {
    fn on_request(&self, ctx: &mut PluginContext, req: &LlmRequest) -> Result<()> {
        info!("Processing request for model: {}", req.model);
        ctx.set("request_start", Utc::now());
        Ok(())
    }
    
    fn on_response(&self, ctx: &mut PluginContext, res: &LlmResponse) -> Result<()> {
        if let Some(start) = ctx.get::<DateTime<Utc>>("request_start") {
            let duration = Utc::now() - start;
            info!("Request completed in {}ms", duration.num_milliseconds());
        }
        Ok(())
    }
}
```

## Testing Utilities

### Mock Messages

Generate test messages:

```rust
#[cfg(test)]
mod tests {
    use lloom_core::protocol::test_utils::*;
    
    #[test]
    fn test_request_handling() {
        // Generate mock request
        let request = mock_llm_request("gpt-3.5-turbo");
        assert_eq!(request.model, "gpt-3.5-turbo");
        
        // Generate mock response
        let response = mock_llm_response(&request, "Test response content");
        assert_eq!(response.model, request.model);
    }
}
```

### Protocol Fuzzing

Test protocol robustness:

```rust
#[cfg(test)]
use lloom_core::protocol::fuzzing::{fuzz_request, FuzzConfig};

#[test]
fn fuzz_protocol_parsing() {
    let config = FuzzConfig {
        iterations: 1000,
        seed: Some(42),
        max_size: 10 * 1024, // 10KB max
    };
    
    fuzz_request(config, |data| {
        // Should not panic on any input
        let _ = decode_message::<LlmRequest>(data);
    });
}
```

## Best Practices

1. **Always Validate**
   - Validate all incoming messages
   - Check required fields
   - Verify reasonable values

2. **Use Builders**
   - Use builder pattern for complex messages
   - Ensures all required fields are set
   - Provides sensible defaults

3. **Version Carefully**
   - Include version in all messages
   - Support at least one previous version
   - Document breaking changes

4. **Sign Important Messages**
   - All LLM requests/responses must be signed
   - Include timestamp to prevent replay
   - Verify signatures before processing

5. **Handle Errors Gracefully**
   - Return specific error messages
   - Include debugging information
   - Don't expose sensitive data