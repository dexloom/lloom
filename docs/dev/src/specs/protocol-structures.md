# Protocol Structures Specification

This document provides the complete specification for all protocol structures used in the Lloom P2P network. These structures define the data formats for network communication, request processing, and accounting.

## Overview

The Lloom protocol defines a comprehensive set of message structures that enable:
- **LLM Service Requests**: Structured requests for language model services
- **Response Handling**: Standardized responses with usage metrics
- **Service Discovery**: Finding and advertising network services
- **Accounting**: Tracking usage and costs
- **Network Control**: Protocol-level control messages

## Core Message Types

### LLM Request Structure

The fundamental request structure for LLM services:

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

#### Field Specifications

- **model**: Must match executor's advertised models exactly
- **temperature**: Encoded as f32, typical range 0.0-1.0
- **max_tokens**: Hard limit on generation, must not exceed model's context
- **prices**: Stored as strings to preserve precision
- **nonce**: Client-specific, monotonically increasing
- **deadline**: Absolute timestamp, not duration

### LLM Response Structure

The response structure containing results and metrics:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// Model actually used (may differ from requested)
    pub model: String,
    
    /// Generated content
    pub content: String,
    
    /// Number of tokens in prompt (including system)
    pub prompt_tokens: u32,
    
    /// Number of tokens generated
    pub completion_tokens: u32,
    
    /// Total token count (prompt + completion)
    pub total_tokens: u32,
    
    /// Client's Ethereum address
    pub client_address: String,
    
    /// Agreed inbound price (must match request)
    pub inbound_price: String,
    
    /// Agreed outbound price (must match request)
    pub outbound_price: String,
    
    /// Response generation timestamp
    pub timestamp: u64,
    
    /// Whether execution succeeded
    pub success: bool,
}
```

#### Response Validation Rules

1. `total_tokens` must equal `prompt_tokens + completion_tokens`
2. Prices must match those in the original request
3. `timestamp` must be after request timestamp but before deadline
4. `model` should match requested model (log warning if different)

### Usage Record Structure

For accounting and billing tracking:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// Type of usage record
    pub record_type: UsageType,
    
    /// Client Ethereum address
    pub client: Address,
    
    /// Executor Ethereum address
    pub executor: Address,
    
    /// Model identifier
    pub model: String,
    
    /// Prompt token count
    pub prompt_tokens: u32,
    
    /// Completion token count
    pub completion_tokens: u32,
    
    /// Total tokens
    pub total_tokens: u32,
    
    /// Inbound token price (wei)
    pub inbound_price: String,
    
    /// Outbound token price (wei)
    pub outbound_price: String,
    
    /// Total cost (wei)
    pub total_cost: String,
    
    /// Record timestamp
    pub timestamp: u64,
    
    /// Hash of original request
    pub request_hash: Option<[u8; 32]>,
    
    /// Hash of response
    pub response_hash: Option<[u8; 32]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UsageType {
    LlmExecution,
    ValidationCheck,
    TestRequest,
}
```

## Message Envelopes

### Request Message Envelope

Wrapper for all request types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestMessage {
    /// LLM completion request
    LlmRequest(SignedMessage<LlmRequest>),
    
    /// Service discovery request
    DiscoverServices {
        service_type: ServiceType,
        requirements: Option<Requirements>,
    },
    
    /// Health check ping
    Ping,
    
    /// Get executor information
    GetInfo,
    
    /// Get supported models
    GetModels,
    
    /// Get current pricing
    GetPricing {
        models: Vec<String>,
    },
}
```

### Response Message Envelope

Wrapper for all response types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseMessage {
    /// LLM completion response
    LlmResponse(SignedMessage<LlmResponse>),
    
    /// Service discovery response
    ServiceList {
        services: Vec<ServiceInfo>,
    },
    
    /// Health check pong
    Pong,
    
    /// Executor information
    ExecutorInfo {
        peer_id: PeerId,
        version: String,
        capabilities: ExecutorCapabilities,
    },
    
    /// Model list
    ModelList {
        models: Vec<ModelInfo>,
    },
    
    /// Pricing information
    PricingInfo {
        prices: HashMap<String, ModelPricing>,
    },
    
    /// Error response
    Error(ErrorInfo),
}
```

## Service Discovery Structures

### Service Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceType {
    Executor,
    Validator,
    Bootstrap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Service provider's peer ID
    pub peer_id: PeerId,
    
    /// Service type
    pub service_type: ServiceType,
    
    /// Service-specific metadata
    pub metadata: ServiceMetadata,
    
    /// Last update timestamp
    pub last_seen: u64,
    
    /// Service addresses
    pub addresses: Vec<Multiaddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceMetadata {
    Executor {
        models: Vec<String>,
        capacity: u32,
        average_latency_ms: Option<u32>,
    },
    Validator {
        stake: String,
        validation_rate: f32,
    },
    Bootstrap {
        network_size: u32,
    },
}
```

### Model Information

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier
    pub id: String,
    
    /// Human-readable name
    pub name: String,
    
    /// Model context length
    pub context_length: u32,
    
    /// Supported capabilities
    pub capabilities: Vec<ModelCapability>,
    
    /// Performance characteristics
    pub performance: ModelPerformance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelCapability {
    Chat,
    Completion,
    Embedding,
    CodeGeneration,
    FunctionCalling,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPerformance {
    /// Average tokens per second
    pub tokens_per_second: Option<f32>,
    
    /// First token latency (ms)
    pub first_token_latency_ms: Option<u32>,
    
    /// Memory requirement (GB)
    pub memory_requirement_gb: f32,
}
```

### Pricing Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Model identifier
    pub model: String,
    
    /// Base price per inbound token (wei)
    pub inbound_price: String,
    
    /// Base price per outbound token (wei)
    pub outbound_price: String,
    
    /// Price multipliers
    pub multipliers: PriceMultipliers,
    
    /// Minimum request cost (wei)
    pub minimum_cost: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceMultipliers {
    /// Peak hours multiplier
    pub peak_hours: f32,
    
    /// High demand multiplier
    pub high_demand: f32,
    
    /// Priority request multiplier
    pub priority: f32,
    
    /// Batch discount
    pub batch_discount: f32,
}
```

## Network Control Messages

### Health Check

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// Check identifier
    pub id: Uuid,
    
    /// Timestamp
    pub timestamp: u64,
    
    /// Optional echo data
    pub echo_data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall health
    pub healthy: bool,
    
    /// Current load (0.0 - 1.0)
    pub load: f32,
    
    /// Available capacity
    pub available_capacity: u32,
    
    /// Active requests
    pub active_requests: u32,
    
    /// Queue size
    pub queue_size: u32,
    
    /// Component statuses
    pub components: HashMap<String, ComponentStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub name: String,
    pub healthy: bool,
    pub message: Option<String>,
}
```

### Rate Limiting

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitInfo {
    /// Requests remaining in current window
    pub requests_remaining: u32,
    
    /// Window reset timestamp
    pub reset_at: u64,
    
    /// Window duration (seconds)
    pub window_seconds: u32,
    
    /// Total limit
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitExceeded {
    /// Retry after (seconds)
    pub retry_after: u32,
    
    /// Current limit
    pub limit: u32,
    
    /// Window type
    pub window: String,
    
    /// Optional message
    pub message: Option<String>,
}
```

## Error Structures

### Error Information

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Error code
    pub code: ErrorCode,
    
    /// Human-readable message
    pub message: String,
    
    /// Detailed description
    pub details: Option<String>,
    
    /// Error context
    pub context: HashMap<String, String>,
    
    /// Timestamp
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u32)]
pub enum ErrorCode {
    // Client errors (400-499)
    InvalidRequest = 400,
    Unauthorized = 401,
    PaymentRequired = 402,
    Forbidden = 403,
    NotFound = 404,
    Timeout = 408,
    TooManyRequests = 429,
    
    // Server errors (500-599)
    InternalError = 500,
    NotImplemented = 501,
    ServiceUnavailable = 503,
    InsufficientCapacity = 507,
    
    // Custom errors (1000+)
    InvalidSignature = 1001,
    InvalidNonce = 1002,
    DeadlineExceeded = 1003,
    ModelNotAvailable = 1004,
    InsufficientBalance = 1005,
}
```

## Signed Message Wrappers

### Generic Signed Message

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedMessage<T: Serialize> {
    /// Message payload
    pub payload: T,
    
    /// Signer's Ethereum address
    pub signer: Address,
    
    /// ECDSA signature
    pub signature: Bytes,
    
    /// Signing timestamp
    pub timestamp: u64,
    
    /// Optional nonce
    pub nonce: Option<u64>,
}
```

### Type Aliases

```rust
pub type SignedLlmRequest = SignedMessage<LlmRequest>;
pub type SignedLlmResponse = SignedMessage<LlmResponse>;
pub type SignedUsageRecord = SignedMessage<UsageRecord>;
```

## Protocol Versioning

### Version Information

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolVersion {
    /// Major version (breaking changes)
    pub major: u16,
    
    /// Minor version (new features)
    pub minor: u16,
    
    /// Patch version (bug fixes)
    pub patch: u16,
}

impl ProtocolVersion {
    pub const CURRENT: Self = Self {
        major: 1,
        minor: 0,
        patch: 0,
    };
    
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.major == other.major && self.minor >= other.minor
    }
}
```

### Version Negotiation

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionNegotiation {
    /// Supported versions
    pub supported: Vec<ProtocolVersion>,
    
    /// Preferred version
    pub preferred: ProtocolVersion,
    
    /// Minimum acceptable version
    pub minimum: ProtocolVersion,
}
```

## Binary Encoding

### Serialization Format

All structures use the following encoding:

1. **Network Transport**: CBOR (Concise Binary Object Representation)
2. **Storage**: MessagePack or Protocol Buffers
3. **Human-Readable**: JSON with sorted keys
4. **Hashing**: Canonical JSON with deterministic field ordering

### Encoding Examples

```rust
// Network encoding
pub fn encode_for_network<T: Serialize>(msg: &T) -> Result<Vec<u8>> {
    serde_cbor::to_vec(msg).map_err(Into::into)
}

// Storage encoding
pub fn encode_for_storage<T: Serialize>(msg: &T) -> Result<Vec<u8>> {
    rmp_serde::to_vec(msg).map_err(Into::into)
}

// Canonical encoding for hashing
pub fn encode_canonical<T: Serialize>(msg: &T) -> Result<Vec<u8>> {
    let json = serde_json::to_value(msg)?;
    let canonical = canonicalize_json(&json)?;
    Ok(serde_json::to_vec(&canonical)?)
}
```

## Validation Rules

### Request Validation

```rust
pub fn validate_request(req: &LlmRequest) -> Result<()> {
    // Model validation
    if req.model.is_empty() {
        return Err("Model cannot be empty");
    }
    
    // Temperature validation
    if let Some(temp) = req.temperature {
        if temp < 0.0 || temp > 2.0 {
            return Err("Temperature must be between 0.0 and 2.0");
        }
    }
    
    // Token limit validation
    if let Some(max) = req.max_tokens {
        if max == 0 || max > 100_000 {
            return Err("Invalid max_tokens");
        }
    }
    
    // Price validation
    validate_price(&req.inbound_price)?;
    validate_price(&req.outbound_price)?;
    
    // Deadline validation
    if req.deadline <= current_timestamp() {
        return Err("Deadline has already passed");
    }
    
    Ok(())
}
```

### Response Validation

```rust
pub fn validate_response(res: &LlmResponse) -> Result<()> {
    // Token count validation
    if res.total_tokens != res.prompt_tokens + res.completion_tokens {
        return Err("Token count mismatch");
    }
    
    // Content validation
    if res.success && res.content.is_empty() {
        return Err("Successful response cannot have empty content");
    }
    
    // Price validation
    validate_price(&res.inbound_price)?;
    validate_price(&res.outbound_price)?;
    
    Ok(())
}
```

## Extension Mechanisms

### Custom Fields

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionField {
    /// Extension namespace
    pub namespace: String,
    
    /// Field name
    pub name: String,
    
    /// Field value (JSON)
    pub value: serde_json::Value,
}

pub trait Extensible {
    fn add_extension(&mut self, field: ExtensionField);
    fn get_extension(&self, namespace: &str, name: &str) -> Option<&serde_json::Value>;
}
```

### Future Compatibility

All structures include versioning and extension support:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FutureCompatible<T> {
    /// Protocol version
    pub version: ProtocolVersion,
    
    /// Main payload
    pub data: T,
    
    /// Extension fields
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<ExtensionField>,
    
    /// Unknown fields (preserved for forward compatibility)
    #[serde(flatten)]
    pub unknown: HashMap<String, serde_json::Value>,
}
```