# Core Library

The `lloom-core` crate provides the foundational functionality for the Lloom P2P network. It implements identity management, networking protocols, message structures, and cryptographic signing.

## Overview

The core library is the foundation upon which all Lloom nodes (clients, executors, and validators) are built. It provides:

- **Identity Management**: Cryptographic key generation and management
- **P2P Networking**: libp2p-based networking stack
- **Message Protocol**: Standardized request/response structures
- **Cryptographic Signing**: EIP-712 compliant message signing
- **Error Handling**: Comprehensive error types

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
lloom-core = "0.1.0"
```

## Core Components

### Identity

The [`Identity`] struct manages cryptographic identities:

```rust
use lloom_core::Identity;

// Generate a new random identity
let identity = Identity::generate();
println!("PeerId: {}", identity.peer_id);
println!("EVM Address: {}", identity.evm_address);

// Load from existing private key
let identity = Identity::from_str(
    "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
)?;

// Save identity to file
identity.save_to_file("~/.lloom/identity")?;

// Load identity from file
let identity = Identity::from_file("~/.lloom/identity")?;
```

### Network Behavior

The [`LloomBehaviour`] implements the P2P networking stack:

```rust
use lloom_core::{Identity, LloomBehaviour, LloomEvent};
use libp2p::{Swarm, SwarmBuilder};

let identity = Identity::generate();
let behaviour = LloomBehaviour::new(&identity)?;

// Create a swarm
let mut swarm = SwarmBuilder::with_existing_identity(identity.keypair.clone())
    .with_tokio()
    .with_tcp(
        Default::default(),
        (libp2p::tls::Config::new, libp2p::noise::Config::new),
        libp2p::yamux::Config::default,
    )?
    .with_behaviour(|_| behaviour)?
    .build();

// Handle events
loop {
    match swarm.next().await {
        Some(SwarmEvent::Behaviour(LloomEvent::RequestReceived { request, channel })) => {
            // Handle incoming request
        }
        // ... handle other events
    }
}
```

### Message Protocol

Core message types for LLM interactions:

```rust
use lloom_core::protocol::{LlmRequest, LlmResponse, RequestMessage};

// Create an LLM request
let request = LlmRequest {
    model: "gpt-3.5-turbo".to_string(),
    prompt: "Explain quantum computing".to_string(),
    system_prompt: Some("You are a helpful assistant".to_string()),
    temperature: Some(0.7),
    max_tokens: Some(500),
    executor_address: "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string(),
    inbound_price: "500000000000000".to_string(),    // 0.0005 ETH per token
    outbound_price: "1000000000000000".to_string(), // 0.001 ETH per token
    nonce: 1,
    deadline: chrono::Utc::now().timestamp() as u64 + 3600, // 1 hour deadline
};

// Create a response
let response = LlmResponse {
    model: "gpt-3.5-turbo".to_string(),
    content: "Quantum computing is...".to_string(),
    prompt_tokens: 15,
    completion_tokens: 85,
    total_tokens: 100,
    client_address: "0x123...".to_string(),
    inbound_price: request.inbound_price.clone(),
    outbound_price: request.outbound_price.clone(),
    timestamp: chrono::Utc::now().timestamp() as u64,
    success: true,
};
```

### Message Signing

EIP-712 compliant message signing:

```rust
use lloom_core::{Identity, protocol::LlmRequest, signing::SignableMessage};

let identity = Identity::generate();
let request = LlmRequest { /* ... */ };

// Sign a request
let signed_request = request.sign_blocking(&identity.wallet)?;

// Verify signature
use lloom_core::signing::verify_signed_message;
let is_valid = verify_signed_message(&signed_request)?;
```

## API Reference

### Identity Management

#### `Identity`

Represents a cryptographic identity with both P2P and Ethereum components.

```rust
pub struct Identity {
    pub keypair: Keypair,           // libp2p keypair
    pub peer_id: PeerId,           // Derived peer ID
    pub wallet: PrivateKeySigner,  // Ethereum wallet
    pub evm_address: Address,      // Ethereum address
}
```

**Methods:**

- `generate() -> Self` - Generate new random identity
- `from_private_key(key: &[u8]) -> Result<Self>` - Create from private key bytes
- `from_str(hex: &str) -> Result<Self>` - Create from hex string
- `from_file(path: impl AsRef<Path>) -> Result<Self>` - Load from file
- `save_to_file(&self, path: impl AsRef<Path>) -> Result<()>` - Save to file

### Network Protocol

#### `LloomBehaviour`

P2P network behavior combining multiple protocols:

```rust
pub struct LloomBehaviour {
    pub kad: Kademlia<MemoryStore>,
    pub gossipsub: Gossipsub,
    pub request_response: RequestResponse<LloomCodec>,
    pub mdns: Mdns,
}
```

**Methods:**

- `new(identity: &Identity) -> Result<Self>` - Create new behavior
- `add_bootstrap_peer(&mut self, peer: &str) -> Result<()>` - Add bootstrap peer
- `advertise_executor_service(&mut self)` - Advertise as executor
- `discover_executors(&mut self)` - Discover available executors
- `send_request(&mut self, peer: &PeerId, request: RequestMessage) -> RequestId` - Send request

#### `LloomEvent`

Network events emitted by the behavior:

```rust
pub enum LloomEvent {
    // Gossipsub events
    MessageReceived { message: Message },
    
    // Kademlia events
    RoutingUpdated,
    StoreSuccess { key: record::Key },
    
    // Request-Response events
    RequestReceived { 
        request: RequestMessage, 
        channel: ResponseChannel<ResponseMessage> 
    },
    ResponseReceived { 
        request_id: RequestId, 
        response: ResponseMessage 
    },
    
    // mDNS events
    PeerDiscovered(PeerId),
}
```

### Message Types

#### `LlmRequest`

Request for LLM completion:

```rust
pub struct LlmRequest {
    pub model: String,              // Model identifier
    pub prompt: String,             // User prompt
    pub system_prompt: Option<String>, // System prompt
    pub temperature: Option<f32>,   // Sampling temperature
    pub max_tokens: Option<u32>,    // Max tokens to generate
    pub executor_address: String,   // Target executor address
    pub inbound_price: String,      // Wei per inbound token
    pub outbound_price: String,     // Wei per outbound token
    pub nonce: u64,                 // Request nonce
    pub deadline: u64,              // Unix timestamp deadline
}
```

#### `LlmResponse`

Response from LLM execution:

```rust
pub struct LlmResponse {
    pub model: String,              // Model used
    pub content: String,            // Generated content
    pub prompt_tokens: u32,         // Input token count
    pub completion_tokens: u32,     // Output token count
    pub total_tokens: u32,          // Total tokens
    pub client_address: String,     // Client address
    pub inbound_price: String,      // Agreed inbound price
    pub outbound_price: String,     // Agreed outbound price
    pub timestamp: u64,             // Response timestamp
    pub success: bool,              // Execution success
}
```

### Signing and Verification

#### `SignableMessage` Trait

Trait for messages that can be signed:

```rust
pub trait SignableMessage: Serialize {
    fn sign_blocking(&self, signer: &PrivateKeySigner) -> Result<SignedMessage<Self>>;
    fn sign(&self, signer: &PrivateKeySigner) -> impl Future<Output = Result<SignedMessage<Self>>>;
}
```

#### `SignedMessage<T>`

Wrapper for signed messages:

```rust
pub struct SignedMessage<T: Serialize> {
    pub payload: T,             // Original message
    pub signer: Address,        // Signer's address
    pub signature: Bytes,       // ECDSA signature
    pub timestamp: u64,         // Signing timestamp
    pub nonce: Option<u64>,     // Optional nonce
}
```

#### Verification Functions

```rust
// Basic verification (signature only)
pub fn verify_signed_message_basic<T>(message: &SignedMessage<T>) -> Result<bool>
where T: Serialize

// Standard verification (with timestamp check)
pub fn verify_signed_message<T>(message: &SignedMessage<T>) -> Result<bool>
where T: Serialize

// Permissive verification (larger time window)
pub fn verify_signed_message_permissive<T>(message: &SignedMessage<T>) -> Result<bool>
where T: Serialize

// Custom verification
pub fn verify_with_config<T>(
    message: &SignedMessage<T>, 
    config: &VerificationConfig
) -> Result<bool>
where T: Serialize
```

### Error Handling

#### `Error` Enum

Comprehensive error type:

```rust
pub enum Error {
    // I/O errors
    Io(std::io::Error),
    
    // Serialization errors
    Serialization(String),
    SerdeJson(serde_json::Error),
    
    // Cryptographic errors
    InvalidSignature,
    SignatureError(String),
    InvalidAddress(String),
    
    // Network errors
    Network(String),
    Libp2p(libp2p::core::transport::TransportError),
    
    // Protocol errors
    InvalidMessage(String),
    Timeout,
    
    // Generic errors
    Other(String),
}
```

## Usage Examples

### Building a Client

```rust
use lloom_core::{Identity, LloomBehaviour, protocol::*, signing::*};

async fn run_client() -> Result<()> {
    // Setup identity
    let identity = Identity::generate();
    
    // Create network behavior
    let behaviour = LloomBehaviour::new(&identity)?;
    
    // Create and sign request
    let request = LlmRequest {
        model: "gpt-3.5-turbo".to_string(),
        prompt: "Hello, world!".to_string(),
        // ... other fields
    };
    
    let signed_request = request.sign(&identity.wallet).await?;
    
    // Send via network
    behaviour.send_request(&executor_peer_id, RequestMessage::LlmRequest(signed_request));
    
    Ok(())
}
```

### Building an Executor

```rust
use lloom_core::{Identity, LloomBehaviour, LloomEvent, protocol::*, signing::*};

async fn run_executor(behaviour: &mut LloomBehaviour) -> Result<()> {
    // Advertise executor service
    behaviour.advertise_executor_service();
    
    // Handle incoming requests
    match event {
        LloomEvent::RequestReceived { request, channel } => {
            if let RequestMessage::LlmRequest(signed_req) = request {
                // Verify signature
                if verify_signed_message(&signed_req)? {
                    // Process request
                    let response = process_llm_request(&signed_req.payload).await?;
                    
                    // Sign response
                    let signed_response = response.sign(&identity.wallet).await?;
                    
                    // Send back
                    behaviour.send_response(channel, ResponseMessage::LlmResponse(signed_response));
                }
            }
        }
        _ => {}
    }
    
    Ok(())
}
```

### Custom Network Configuration

```rust
use lloom_core::{Identity, network::create_custom_behaviour};

let config = NetworkConfig {
    kad_protocol: "/lloom/kad/1.0.0".to_string(),
    enable_mdns: true,
    gossipsub_config: GossipsubConfig {
        heartbeat_interval: Duration::from_secs(10),
        // ... other settings
    },
};

let behaviour = create_custom_behaviour(&identity, config)?;
```

## Best Practices

1. **Identity Management**
   - Store private keys securely
   - Use hardware security modules in production
   - Implement key rotation policies

2. **Network Operations**
   - Always verify signatures before processing
   - Implement request timeouts
   - Use connection pooling for efficiency

3. **Error Handling**
   - Use the `Result` type consistently
   - Log errors with context
   - Implement retry logic for transient failures

4. **Performance**
   - Reuse identities and connections
   - Batch operations when possible
   - Monitor resource usage

5. **Security**
   - Validate all inputs
   - Use time-based nonces
   - Implement rate limiting