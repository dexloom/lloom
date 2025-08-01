# Cryptographic Signing

The signing module provides EIP-712 compliant message signing and verification functionality. It ensures message authenticity, integrity, and non-repudiation across the Lloom network.

## Overview

The signing system provides:
- **EIP-712 Compliance**: Ethereum-compatible structured data signing
- **Message Authentication**: Cryptographic proof of message origin
- **Replay Protection**: Timestamps and nonces prevent replay attacks
- **Flexible Verification**: Multiple verification strategies
- **Async/Sync Support**: Both async and blocking signing methods

## Core Types

### `SignedMessage<T>`

Wrapper for signed messages:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedMessage<T: Serialize> {
    /// The original message payload
    pub payload: T,
    
    /// Ethereum address of the signer
    pub signer: Address,
    
    /// ECDSA signature bytes
    pub signature: Bytes,
    
    /// Unix timestamp when signed
    pub timestamp: u64,
    
    /// Optional nonce for additional replay protection
    pub nonce: Option<u64>,
}
```

### `SignableMessage` Trait

Trait for types that can be signed:

```rust
pub trait SignableMessage: Serialize + Sized {
    /// Sign message asynchronously
    fn sign(&self, signer: &PrivateKeySigner) 
        -> impl Future<Output = Result<SignedMessage<Self>>>;
    
    /// Sign message synchronously (blocking)
    fn sign_blocking(&self, signer: &PrivateKeySigner) 
        -> Result<SignedMessage<Self>>;
    
    /// Get EIP-712 type hash for this message type
    fn type_hash() -> B256;
}
```

## Basic Signing

### Sign a Message

Sign any serializable message:

```rust
use lloom_core::{Identity, signing::SignableMessage};
use lloom_core::protocol::LlmRequest;

let identity = Identity::generate();
let request = LlmRequest {
    model: "gpt-3.5-turbo".to_string(),
    prompt: "Hello, world!".to_string(),
    // ... other fields
};

// Async signing
let signed = request.sign(&identity.wallet).await?;

// Blocking signing
let signed = request.sign_blocking(&identity.wallet)?;

// Access signature components
println!("Signer: {}", signed.signer);
println!("Signature: 0x{}", hex::encode(&signed.signature));
println!("Timestamp: {}", signed.timestamp);
```

### Sign Arbitrary Data

Sign custom messages:

```rust
use lloom_core::signing::{sign_message, SignableCustom};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, SignableCustom)]
#[signable(type_hash = "0x1234...")]
struct CustomMessage {
    data: String,
    value: u64,
}

let message = CustomMessage {
    data: "important data".to_string(),
    value: 42,
};

// Sign using the utility function
let signed = sign_message(&message, &identity.wallet).await?;
```

## Verification

### Basic Verification

Verify signature validity:

```rust
use lloom_core::signing::{verify_signed_message, verify_signed_message_basic};

// Basic verification (signature only)
let is_valid = verify_signed_message_basic(&signed)?;
println!("Signature valid: {}", is_valid);

// Standard verification (includes timestamp check)
let is_valid = verify_signed_message(&signed)?;
println!("Message valid: {}", is_valid);

// Get recovered signer address
use lloom_core::signing::recover_signer;
let recovered_address = recover_signer(&signed)?;
assert_eq!(recovered_address, signed.signer);
```

### Custom Verification

Configure verification parameters:

```rust
use lloom_core::signing::{VerificationConfig, verify_with_config};

let config = VerificationConfig {
    /// Maximum age of message in seconds
    max_age_secs: Some(3600), // 1 hour
    
    /// Whether to check timestamp
    check_timestamp: true,
    
    /// Allowed time drift in seconds
    time_tolerance_secs: 300, // 5 minutes
    
    /// Expected signer address (if known)
    expected_signer: Some(executor_address),
    
    /// Whether to check nonce
    check_nonce: true,
    
    /// Nonce validation function
    nonce_validator: Some(Box::new(|nonce| {
        // Custom nonce validation logic
        nonce > last_seen_nonce
    })),
};

let is_valid = verify_with_config(&signed, &config)?;
```

### Batch Verification

Verify multiple signatures efficiently:

```rust
use lloom_core::signing::{batch_verify, BatchVerificationResult};

let messages = vec![signed1, signed2, signed3];

let results = batch_verify(&messages)?;
for (index, result) in results.into_iter().enumerate() {
    match result {
        BatchVerificationResult::Valid => {
            println!("Message {} is valid", index);
        }
        BatchVerificationResult::Invalid(reason) => {
            println!("Message {} invalid: {}", index, reason);
        }
    }
}
```

## EIP-712 Implementation

### Domain Separator

Configure EIP-712 domain:

```rust
use lloom_core::signing::{Eip712Domain, LLOOM_DOMAIN};

// Default Lloom domain
let domain = LLOOM_DOMAIN;
println!("Domain name: {}", domain.name);
println!("Version: {}", domain.version);
println!("Chain ID: {}", domain.chain_id);

// Custom domain for testing
let custom_domain = Eip712Domain {
    name: "Test Network".to_string(),
    version: "1.0.0".to_string(),
    chain_id: 31337, // Local test chain
    verifying_contract: Some(contract_address),
    salt: None,
};
```

### Structured Data Encoding

Encode typed data:

```rust
use lloom_core::signing::{encode_eip712, TypedData};

// Define typed data
let typed_data = TypedData {
    domain: LLOOM_DOMAIN,
    primary_type: "LlmRequest".to_string(),
    types: hashmap! {
        "LlmRequest" => vec![
            ("model", "string"),
            ("prompt", "string"),
            ("maxTokens", "uint32"),
            ("nonce", "uint64"),
        ],
    },
    message: serde_json::json!({
        "model": "gpt-3.5-turbo",
        "prompt": "Hello, world!",
        "maxTokens": 100,
        "nonce": 1,
    }),
};

// Encode for signing
let encoded = encode_eip712(&typed_data)?;
```

### Type Hash Calculation

Calculate EIP-712 type hashes:

```rust
use lloom_core::signing::{calculate_type_hash, encode_type};

// Calculate type hash for a struct
let type_string = encode_type("LlmRequest", &[
    ("model", "string"),
    ("prompt", "string"),
    ("maxTokens", "uint32"),
    ("temperature", "uint32"),
    ("nonce", "uint64"),
    ("deadline", "uint64"),
]);

let type_hash = calculate_type_hash(&type_string);
println!("Type hash: 0x{}", hex::encode(type_hash));
```

## Advanced Signing

### Multi-Signature

Implement multi-sig patterns:

```rust
use lloom_core::signing::{MultiSigMessage, ThresholdSigner};

// Create multi-sig message
let mut multi_sig = MultiSigMessage::new(message, 2); // 2-of-N threshold

// Collect signatures
multi_sig.add_signature(&identity1.wallet).await?;
multi_sig.add_signature(&identity2.wallet).await?;

// Verify threshold met
assert!(multi_sig.is_complete());
assert!(multi_sig.verify()?);

// Extract signers
let signers = multi_sig.signers();
println!("Signed by: {:?}", signers);
```

### Delegated Signing

Sign on behalf of another party:

```rust
use lloom_core::signing::{DelegatedSigner, DelegationProof};

// Create delegation proof
let delegation = DelegationProof {
    delegator: principal_address,
    delegate: signer_address,
    permissions: vec!["sign_requests".to_string()],
    expiry: Utc::now().timestamp() as u64 + 86400, // 24 hours
    signature: delegation_signature,
};

// Create delegated signer
let delegated_signer = DelegatedSigner::new(
    &identity.wallet,
    delegation
);

// Sign with delegation
let signed = message.sign_delegated(&delegated_signer).await?;
assert_eq!(signed.signer, principal_address); // Shows principal, not delegate
```

### Blind Signatures

Implement privacy-preserving signatures:

```rust
use lloom_core::signing::{BlindSigner, unblind_signature};

// Client blinds message
let (blinded_message, blinding_factor) = blind_message(&message)?;

// Server signs blinded message
let blind_signature = signer.sign_blind(&blinded_message).await?;

// Client unblinds signature
let signature = unblind_signature(
    &blind_signature,
    &blinding_factor,
    &signer_public_key
)?;

// Signature is valid for original message
assert!(verify_signature(&message, &signature, &signer_address)?);
```

## Signature Storage

### Signature Cache

Cache signatures for performance:

```rust
use lloom_core::signing::{SignatureCache, CacheConfig};

let cache = SignatureCache::new(CacheConfig {
    max_entries: 10000,
    ttl_seconds: 3600,
    persistent: true,
    storage_path: Some("~/.lloom/signature_cache".into()),
});

// Check cache before signing
let message_hash = hash_message(&message);
if let Some(cached) = cache.get(&message_hash)? {
    return Ok(cached);
}

// Sign and cache
let signed = message.sign(&identity.wallet).await?;
cache.insert(&message_hash, &signed)?;
```

### Signature Database

Store signatures for audit trails:

```rust
use lloom_core::signing::{SignatureStore, QueryFilter};

let store = SignatureStore::open("~/.lloom/signatures.db")?;

// Store signature with metadata
store.insert(&signed, &metadata)?;

// Query signatures
let filter = QueryFilter {
    signer: Some(identity.evm_address),
    after_timestamp: Some(yesterday),
    message_type: Some("LlmRequest"),
    limit: 100,
};

let signatures = store.query(&filter)?;
for sig in signatures {
    println!("Found signature: {:?}", sig);
}
```

## Security Features

### Replay Protection

Implement nonce-based replay protection:

```rust
use lloom_core::signing::{NonceManager, NonceStrategy};

let nonce_manager = NonceManager::new(NonceStrategy::Sequential);

// Get next nonce
let nonce = nonce_manager.next_nonce(&identity.evm_address)?;

// Create message with nonce
let message = LlmRequest {
    nonce,
    // ... other fields
};

// Verify nonce on receiver side
if !nonce_manager.verify_nonce(&signed.signer, signed.nonce.unwrap())? {
    return Err("Invalid nonce - possible replay attack");
}
```

### Time-based Validation

Prevent old message acceptance:

```rust
use lloom_core::signing::{TimestampValidator, TimeWindow};

let validator = TimestampValidator::new(TimeWindow {
    past_tolerance: Duration::from_secs(300),   // 5 minutes
    future_tolerance: Duration::from_secs(60),  // 1 minute
});

// Validate timestamp
if !validator.is_valid(signed.timestamp)? {
    return Err("Message timestamp outside acceptable window");
}

// For messages with deadlines
if message.deadline < Utc::now().timestamp() as u64 {
    return Err("Message deadline has passed");
}
```

### Signature Aggregation

Aggregate multiple signatures:

```rust
use lloom_core::signing::{aggregate_signatures, AggregateSignature};

// Collect signatures from multiple parties
let signatures = vec![sig1, sig2, sig3];

// Create aggregate signature
let aggregate = aggregate_signatures(&signatures)?;

// Verify aggregate
let signers = vec![addr1, addr2, addr3];
assert!(verify_aggregate(&message, &aggregate, &signers)?);

// More efficient than individual verification
println!("Aggregate size: {} bytes", aggregate.size());
```

## Error Handling

### Common Errors

Handle signing errors:

```rust
use lloom_core::signing::{SigningError, VerificationError};

match message.sign(&wallet).await {
    Ok(signed) => println!("Signed successfully"),
    Err(SigningError::InvalidPrivateKey) => {
        eprintln!("Invalid private key");
    }
    Err(SigningError::SerializationFailed(e)) => {
        eprintln!("Failed to serialize message: {}", e);
    }
    Err(SigningError::SignatureFailed(e)) => {
        eprintln!("Signature operation failed: {}", e);
    }
    Err(e) => eprintln!("Unexpected error: {}", e),
}
```

### Recovery Strategies

Implement fallback mechanisms:

```rust
use lloom_core::signing::{SigningStrategy, FallbackSigner};

let strategy = SigningStrategy::new()
    .primary(&hardware_signer)
    .fallback(&software_signer)
    .retry_count(3)
    .timeout(Duration::from_secs(10));

let signed = strategy.sign(&message).await?;
```

## Testing

### Mock Signers

Create test signers:

```rust
#[cfg(test)]
mod tests {
    use lloom_core::signing::test_utils::{mock_signer, deterministic_signer};
    
    #[test]
    fn test_signing() {
        // Random mock signer
        let signer = mock_signer();
        
        // Deterministic signer (same key every time)
        let det_signer = deterministic_signer(0);
        assert_eq!(
            det_signer.address().to_string(),
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
        );
    }
}
```

### Signature Validation

Test signature properties:

```rust
#[cfg(test)]
use lloom_core::signing::test_utils::{
    assert_valid_signature,
    assert_signatures_equal,
    generate_test_signatures
};

#[test]
fn test_signature_properties() {
    let signatures = generate_test_signatures(10);
    
    for sig in &signatures {
        assert_valid_signature(sig);
    }
    
    // Signatures should be unique
    for i in 0..signatures.len() {
        for j in i+1..signatures.len() {
            assert!(!assert_signatures_equal(&signatures[i], &signatures[j]));
        }
    }
}
```

## Performance Optimization

### Batch Operations

Sign multiple messages efficiently:

```rust
use lloom_core::signing::{batch_sign, SignBatch};

let messages = vec![msg1, msg2, msg3, msg4, msg5];

// Sign all at once
let signed_messages = batch_sign(&messages, &identity.wallet).await?;

// Or use SignBatch for more control
let mut batch = SignBatch::new(&identity.wallet);
for msg in messages {
    batch.add(msg);
}
let results = batch.execute().await?;
```

### Signature Caching

Implement smart caching:

```rust
use lloom_core::signing::{CachedSigner, CacheStrategy};

let cached_signer = CachedSigner::new(
    &identity.wallet,
    CacheStrategy {
        cache_duration: Duration::from_secs(300),
        max_cache_size: 1000,
        cache_identical: true, // Cache identical messages
    }
);

// First call signs
let sig1 = cached_signer.sign(&message).await?;

// Second call returns cached
let sig2 = cached_signer.sign(&message).await?;
assert_eq!(sig1, sig2);
```

## Best Practices

1. **Always Verify**
   - Verify all incoming signed messages
   - Use appropriate verification config
   - Log verification failures

2. **Secure Key Storage**
   - Never expose private keys
   - Use hardware security modules when possible
   - Implement key rotation

3. **Prevent Replay Attacks**
   - Always include timestamps
   - Use nonces for critical operations
   - Implement message expiry

4. **Error Handling**
   - Handle all error cases explicitly
   - Provide meaningful error messages
   - Implement retry logic for transient failures

5. **Performance**
   - Cache signatures when appropriate
   - Use batch operations for multiple signatures
   - Consider async signing for better throughput