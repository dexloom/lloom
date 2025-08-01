# Message Signing Design

This document provides the technical specification for message-level cryptographic signing in the Lloom P2P network. The design ensures non-repudiation, creates audit trails, and provides security for all LLM request-response interactions.

## Architecture Overview

The signing system implements a comprehensive message authentication framework:

### Core Components

1. **Signature Schema**: Standardized wrapper for signed messages
2. **Signing Process**: Deterministic message serialization and signing
3. **Verification Process**: Multi-level verification strategies
4. **Integration Points**: Seamless integration with client and executor nodes

### Design Principles

- **Non-repudiation**: Cryptographically prove message origin
- **Integrity**: Detect any message tampering
- **Replay Protection**: Prevent message replay attacks
- **Compatibility**: Work with existing Ethereum tooling
- **Performance**: Minimal overhead for signing/verification

## Signature Schema

### SignedMessage Structure

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignedMessage<T: Serialize> {
    /// The actual message payload
    pub payload: T,
    
    /// The signer's Ethereum address
    pub signer: Address,
    
    /// Signature of the serialized payload
    pub signature: Bytes,
    
    /// Timestamp when the message was signed
    pub timestamp: u64,
    
    /// Optional nonce to prevent replay attacks
    pub nonce: Option<u64>,
}
```

### Design Rationale

- **Generic Payload**: Supports any serializable message type
- **Ethereum Address**: Compatible with existing identity infrastructure
- **Timestamp**: Enables time-based validation and ordering
- **Optional Nonce**: Additional replay protection when needed

## Signing Process

### 1. Message Preparation

```rust
pub fn prepare_message<T: Serialize>(message: &T) -> Result<Vec<u8>> {
    // Serialize to canonical JSON
    let json = serde_json::to_string(message)?;
    
    // Sort keys for deterministic output
    let canonical: Value = serde_json::from_str(&json)?;
    let canonical_json = to_canonical_json(&canonical)?;
    
    Ok(canonical_json.into_bytes())
}
```

### 2. Hash Generation

```rust
pub fn create_signing_hash(message_bytes: &[u8], timestamp: u64) -> [u8; 32] {
    let mut data = Vec::new();
    
    // Ethereum signed message prefix
    data.extend_from_slice(b"\x19Ethereum Signed Message:\n");
    data.extend_from_slice(&message_bytes.len().to_string().as_bytes());
    data.extend_from_slice(message_bytes);
    data.extend_from_slice(&timestamp.to_le_bytes());
    
    keccak256(&data)
}
```

### 3. Signature Creation

```rust
pub async fn sign_message<T: Serialize>(
    message: &T,
    signer: &PrivateKeySigner,
) -> Result<SignedMessage<T>> {
    let timestamp = Utc::now().timestamp() as u64;
    let message_bytes = prepare_message(message)?;
    let hash = create_signing_hash(&message_bytes, timestamp);
    
    // Sign with Ethereum wallet
    let signature = signer.sign_hash(&hash).await?;
    
    Ok(SignedMessage {
        payload: message.clone(),
        signer: signer.address(),
        signature: signature.to_bytes(),
        timestamp,
        nonce: None,
    })
}
```

## Verification Process

### 1. Signature Recovery

```rust
pub fn recover_signer<T: Serialize>(
    signed_message: &SignedMessage<T>,
) -> Result<Address> {
    // Recreate the message hash
    let message_bytes = prepare_message(&signed_message.payload)?;
    let hash = create_signing_hash(&message_bytes, signed_message.timestamp);
    
    // Recover signer from signature
    let signature = Signature::try_from(signed_message.signature.as_ref())?;
    let recovered = signature.recover_address_from_prehash(&hash)?;
    
    Ok(recovered)
}
```

### 2. Verification Strategies

#### Basic Verification

Only verifies signature validity:

```rust
pub fn verify_basic<T: Serialize>(
    signed_message: &SignedMessage<T>,
) -> Result<bool> {
    let recovered = recover_signer(signed_message)?;
    Ok(recovered == signed_message.signer)
}
```

#### Standard Verification

Includes timestamp validation:

```rust
pub fn verify_standard<T: Serialize>(
    signed_message: &SignedMessage<T>,
    max_age_seconds: u64,
) -> Result<bool> {
    // Check signature
    if !verify_basic(signed_message)? {
        return Ok(false);
    }
    
    // Check timestamp
    let current_time = Utc::now().timestamp() as u64;
    let message_age = current_time.saturating_sub(signed_message.timestamp);
    
    if message_age > max_age_seconds {
        return Ok(false);
    }
    
    // Check not in future (with 5 minute tolerance)
    if signed_message.timestamp > current_time + 300 {
        return Ok(false);
    }
    
    Ok(true)
}
```

#### Custom Verification

Configurable verification parameters:

```rust
pub struct VerificationConfig {
    pub check_timestamp: bool,
    pub max_age_seconds: Option<u64>,
    pub time_tolerance_seconds: u64,
    pub expected_signer: Option<Address>,
    pub check_nonce: bool,
    pub nonce_validator: Option<Box<dyn Fn(u64) -> bool>>,
}

pub fn verify_with_config<T: Serialize>(
    signed_message: &SignedMessage<T>,
    config: &VerificationConfig,
) -> Result<bool> {
    // Verify signature
    let recovered = recover_signer(signed_message)?;
    if recovered != signed_message.signer {
        return Ok(false);
    }
    
    // Check expected signer
    if let Some(expected) = &config.expected_signer {
        if recovered != *expected {
            return Ok(false);
        }
    }
    
    // Timestamp validation
    if config.check_timestamp {
        // Implementation details...
    }
    
    // Nonce validation
    if config.check_nonce {
        if let Some(nonce) = signed_message.nonce {
            if let Some(validator) = &config.nonce_validator {
                if !validator(nonce) {
                    return Ok(false);
                }
            }
        }
    }
    
    Ok(true)
}
```

## Integration Points

### Client Side

#### Request Signing

```rust
impl LloomClient {
    pub async fn send_request(&self, request: LlmRequest) -> Result<()> {
        // Sign request before sending
        let signed_request = sign_message(&request, &self.identity.wallet).await?;
        
        // Send to executor
        self.network.send_message(
            &executor_peer,
            RequestMessage::LlmRequest(signed_request),
        ).await?;
        
        Ok(())
    }
}
```

#### Response Verification

```rust
impl LloomClient {
    pub async fn handle_response(
        &self, 
        response: SignedMessage<LlmResponse>,
    ) -> Result<()> {
        // Verify executor signature
        let config = VerificationConfig {
            check_timestamp: true,
            max_age_seconds: Some(300), // 5 minutes
            expected_signer: Some(self.selected_executor),
            ..Default::default()
        };
        
        if !verify_with_config(&response, &config)? {
            return Err("Invalid executor signature");
        }
        
        // Process valid response
        self.process_response(response.payload).await?;
        
        Ok(())
    }
}
```

### Executor Side

#### Request Verification

```rust
impl LloomExecutor {
    pub async fn handle_request(
        &self,
        request: SignedMessage<LlmRequest>,
    ) -> Result<()> {
        // Verify client signature
        if !verify_standard(&request, 300)? {
            return Err("Invalid client signature");
        }
        
        // Check nonce
        if let Some(nonce) = request.nonce {
            if !self.nonce_validator.validate(request.signer, nonce)? {
                return Err("Invalid nonce - possible replay attack");
            }
        }
        
        // Process valid request
        let response = self.process_llm_request(request.payload).await?;
        
        // Sign response
        let signed_response = sign_message(&response, &self.identity.wallet).await?;
        
        // Send back to client
        self.send_response(signed_response).await?;
        
        Ok(())
    }
}
```

## Error Handling

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum SigningError {
    #[error("Serialization failed: {0}")]
    SerializationError(String),
    
    #[error("Invalid signature format")]
    InvalidSignature,
    
    #[error("Signature verification failed")]
    VerificationFailed,
    
    #[error("Message timestamp invalid: {0}")]
    InvalidTimestamp(String),
    
    #[error("Nonce validation failed")]
    InvalidNonce,
    
    #[error("Signer mismatch: expected {expected}, got {actual}")]
    SignerMismatch { expected: Address, actual: Address },
}
```

### Error Recovery

```rust
pub async fn sign_with_retry<T: Serialize>(
    message: &T,
    signer: &PrivateKeySigner,
    max_attempts: u32,
) -> Result<SignedMessage<T>> {
    let mut last_error = None;
    
    for attempt in 0..max_attempts {
        match sign_message(message, signer).await {
            Ok(signed) => return Ok(signed),
            Err(e) => {
                last_error = Some(e);
                if attempt < max_attempts - 1 {
                    tokio::time::sleep(Duration::from_millis(100 * (attempt + 1))).await;
                }
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| "Unknown error".into()))
}
```

## Security Considerations

### Replay Protection

1. **Timestamps**: Messages expire after configurable time window
2. **Nonces**: Optional sequential or random nonces per client
3. **Request Binding**: Responses reference specific request hashes

### Key Management

1. **Secure Storage**: Private keys encrypted at rest
2. **Memory Protection**: Clear keys from memory after use
3. **Rotation**: Support for key rotation without service disruption

### Algorithm Security

1. **ECDSA**: secp256k1 curve (Ethereum compatible)
2. **Hashing**: Keccak-256 for all hash operations
3. **Canonical Serialization**: Prevents signature malleability

## Performance Optimizations

### Caching

```rust
pub struct SignatureCache {
    cache: LruCache<[u8; 32], SignatureCacheEntry>,
}

struct SignatureCacheEntry {
    signature: Bytes,
    timestamp: u64,
    signer: Address,
}

impl SignatureCache {
    pub fn get_or_sign<T: Serialize>(
        &mut self,
        message: &T,
        signer: &PrivateKeySigner,
    ) -> Result<SignedMessage<T>> {
        let message_hash = hash_message(message)?;
        
        if let Some(entry) = self.cache.get(&message_hash) {
            if entry.timestamp > Utc::now().timestamp() as u64 - 300 {
                return Ok(SignedMessage {
                    payload: message.clone(),
                    signer: entry.signer,
                    signature: entry.signature.clone(),
                    timestamp: entry.timestamp,
                    nonce: None,
                });
            }
        }
        
        // Sign and cache
        let signed = sign_message(message, signer).await?;
        self.cache.put(message_hash, SignatureCacheEntry {
            signature: signed.signature.clone(),
            timestamp: signed.timestamp,
            signer: signed.signer,
        });
        
        Ok(signed)
    }
}
```

### Batch Verification

```rust
pub async fn verify_batch<T: Serialize>(
    messages: &[SignedMessage<T>],
    config: &VerificationConfig,
) -> Vec<Result<bool>> {
    // Parallel verification
    let futures: Vec<_> = messages.iter()
        .map(|msg| verify_with_config_async(msg, config))
        .collect();
    
    futures::future::join_all(futures).await
}
```

## Migration Strategy

### Phase 1: Dual Mode

Support both signed and unsigned messages:

```rust
pub enum MessageWrapper<T> {
    Unsigned(T),
    Signed(SignedMessage<T>),
}

impl<T: Serialize> MessageWrapper<T> {
    pub fn verify(&self, config: &VerificationConfig) -> Result<bool> {
        match self {
            MessageWrapper::Unsigned(_) => {
                warn!("Received unsigned message");
                Ok(config.allow_unsigned)
            }
            MessageWrapper::Signed(signed) => verify_with_config(signed, config),
        }
    }
}
```

### Phase 2: Enforcement

Require signatures with grace period:

```rust
pub struct EnforcementConfig {
    pub enforcement_start: DateTime<Utc>,
    pub warning_period_days: u32,
    pub strict_after: DateTime<Utc>,
}
```

### Phase 3: Mandatory Signing

All messages must be signed:

```rust
impl NetworkBehaviour {
    pub fn handle_message(&mut self, message: Message) -> Result<()> {
        match message {
            Message::Unsigned(_) => Err("Unsigned messages not allowed"),
            Message::Signed(signed) => {
                if !verify_standard(&signed, 300)? {
                    return Err("Invalid signature");
                }
                self.process_signed_message(signed)
            }
        }
    }
}
```

## Testing Strategy

### Unit Tests

1. Signature generation and verification
2. Edge cases (expired timestamps, invalid nonces)
3. Serialization consistency
4. Error handling

### Integration Tests

1. Full request-response flow with signing
2. Network behavior with mixed signed/unsigned
3. Performance under load
4. Key rotation scenarios

### Security Tests

1. Replay attack attempts
2. Signature malleability
3. Time-based attacks
4. Nonce exhaustion

## Future Enhancements

### Threshold Signatures

Support for multi-party signatures:

```rust
pub struct ThresholdSignature {
    pub threshold: u32,
    pub signatures: Vec<(Address, Bytes)>,
}
```

### Zero-Knowledge Proofs

Privacy-preserving verification:

```rust
pub struct ZKProof {
    pub proof: Vec<u8>,
    pub public_inputs: Vec<[u8; 32]>,
}
```

### Post-Quantum Signatures

Future-proof cryptography:

```rust
pub enum SignatureScheme {
    ECDSA(Bytes),
    Dilithium(Vec<u8>),
    Falcon(Vec<u8>),
}
```