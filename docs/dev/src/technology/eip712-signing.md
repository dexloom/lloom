# EIP-712 Message Signing

EIP-712 is a standard for typed structured data signing in Ethereum. Lloom implements comprehensive EIP-712 signing for all network messages, providing cryptographic guarantees of authenticity, integrity, and non-repudiation.

## Overview

EIP-712 provides:
- **Human-readable signing**: Users can verify what they're signing
- **Replay protection**: Prevents message replay attacks
- **Domain separation**: Isolates different applications
- **Type safety**: Structured data with defined types
- **Wallet compatibility**: Works with standard Ethereum wallets

## Core Concepts

### Domain Separator

The domain separator prevents signatures from being valid across different contexts:

```rust
pub struct EIP712Domain {
    pub name: String,              // "Lloom Network"
    pub version: String,           // "1.0.0"
    pub chain_id: u64,            // Network chain ID
    pub verifying_contract: Address, // Accounting contract
}
```

**Domain Hash Calculation:**
```rust
let domain_hash = keccak256(encode([
    Token::Uint(keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)")),
    Token::Uint(keccak256("Lloom Network")),
    Token::Uint(keccak256("1.0.0")),
    Token::Uint(chain_id.into()),
    Token::Address(verifying_contract),
]));
```

### Type Hash

Each message type has a unique type hash:

```rust
pub const LLMREQUEST_TYPEHASH: &str = 
    "LlmRequestCommitment(address executor,string model,bytes32 promptHash,bytes32 systemPromptHash,uint32 maxTokens,uint32 temperature,uint256 inboundPrice,uint256 outboundPrice,uint64 nonce,uint64 deadline)";

pub const LLMRESPONSE_TYPEHASH: &str = 
    "LlmResponseCommitment(bytes32 requestHash,address client,string model,bytes32 contentHash,uint32 inboundTokens,uint32 outboundTokens,uint256 inboundPrice,uint256 outboundPrice,uint64 timestamp,bool success)";
```

## Message Structures

### Client Request Commitment

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequestCommitment {
    pub executor: Address,          // Target executor
    pub model: String,             // Model identifier
    pub prompt_hash: [u8; 32],     // Hash of prompt
    pub system_prompt_hash: [u8; 32], // Hash of system prompt
    pub max_tokens: u32,           // Max generation tokens
    pub temperature: u32,          // Temperature * 10000
    pub inbound_price: U256,       // Price per input token
    pub outbound_price: U256,      // Price per output token
    pub nonce: u64,                // Replay protection
    pub deadline: u64,             // Request expiry
}
```

**Key Design Choices:**
- **Hash-based privacy**: Prompts are hashed, not stored on-chain
- **Fixed-point temperature**: Ensures deterministic encoding
- **Price commitment**: Client agrees to specific prices upfront
- **Deadline enforcement**: Prevents indefinite validity

### Executor Response Commitment

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponseCommitment {
    pub request_hash: [u8; 32],    // Links to request
    pub client: Address,           // Original requester
    pub model: String,             // Model actually used
    pub content_hash: [u8; 32],    // Hash of response
    pub inbound_tokens: u32,       // Actual input tokens
    pub outbound_tokens: u32,      // Actual output tokens
    pub inbound_price: U256,       // Must match request
    pub outbound_price: U256,      // Must match request
    pub timestamp: u64,            // Execution time
    pub success: bool,             // Success indicator
}
```

## Signing Process

### 1. Structure Hash

Calculate the hash of the typed data:

```rust
pub fn calculate_struct_hash(commitment: &LlmRequestCommitment) -> [u8; 32] {
    let encoded = encode([
        Token::Uint(keccak256(LLMREQUEST_TYPEHASH)),
        Token::Address(commitment.executor),
        Token::Uint(keccak256(&commitment.model)),
        Token::FixedBytes(commitment.prompt_hash.to_vec()),
        Token::FixedBytes(commitment.system_prompt_hash.to_vec()),
        Token::Uint(commitment.max_tokens.into()),
        Token::Uint(commitment.temperature.into()),
        Token::Uint(commitment.inbound_price),
        Token::Uint(commitment.outbound_price),
        Token::Uint(commitment.nonce.into()),
        Token::Uint(commitment.deadline.into()),
    ]);
    
    keccak256(encoded)
}
```

### 2. Message Hash

Combine domain separator and struct hash:

```rust
pub fn calculate_message_hash(
    domain_separator: [u8; 32],
    struct_hash: [u8; 32],
) -> [u8; 32] {
    keccak256(encode([
        Token::Bytes(vec![0x19, 0x01]), // EIP-712 prefix
        Token::FixedBytes(domain_separator.to_vec()),
        Token::FixedBytes(struct_hash.to_vec()),
    ]))
}
```

### 3. Sign Message

Sign the message hash with private key:

```rust
pub async fn sign_commitment(
    commitment: &LlmRequestCommitment,
    signer: &PrivateKeySigner,
) -> Result<Signature> {
    let domain = get_eip712_domain().await?;
    let struct_hash = calculate_struct_hash(commitment);
    let message_hash = calculate_message_hash(
        domain.separator(),
        struct_hash,
    );
    
    let signature = signer.sign_hash(&message_hash).await?;
    Ok(signature)
}
```

## Verification Process

### 1. Recover Signer

Extract the signer's address from signature:

```rust
pub fn recover_signer(
    message_hash: [u8; 32],
    signature: &Signature,
) -> Result<Address> {
    let recovery_id = RecoveryId::from_byte(signature.v())?;
    let public_key = recover(
        &Message::from_slice(&message_hash)?,
        &signature.to_compact(),
        &recovery_id,
    )?;
    
    Ok(Address::from_public_key(&public_key))
}
```

### 2. Verify Signature

Ensure signature matches claimed signer:

```rust
pub fn verify_commitment_signature(
    commitment: &LlmRequestCommitment,
    signature: &Signature,
    expected_signer: Address,
) -> Result<bool> {
    let message_hash = calculate_commitment_hash(commitment)?;
    let recovered = recover_signer(message_hash, signature)?;
    
    Ok(recovered == expected_signer)
}
```

### 3. Validate Commitment

Check business logic constraints:

```rust
pub fn validate_commitment(
    commitment: &LlmRequestCommitment,
) -> Result<()> {
    // Check deadline
    if commitment.deadline < current_timestamp() {
        return Err(Error::ExpiredCommitment);
    }
    
    // Check prices are reasonable
    if commitment.inbound_price == U256::ZERO {
        return Err(Error::InvalidPrice);
    }
    
    // Check model is supported
    if !is_supported_model(&commitment.model) {
        return Err(Error::UnsupportedModel);
    }
    
    Ok(())
}
```

## Implementation Details

### Type Encoding

EIP-712 requires specific encoding for each type:

```rust
impl Eip712Encode for LlmRequestCommitment {
    fn encode_type() -> String {
        LLMREQUEST_TYPEHASH.to_string()
    }
    
    fn encode_data(&self) -> Vec<Token> {
        vec![
            Token::Address(self.executor),
            Token::Uint(keccak256(self.model.as_bytes()).into()),
            Token::FixedBytes(self.prompt_hash.to_vec()),
            Token::FixedBytes(self.system_prompt_hash.to_vec()),
            Token::Uint(self.max_tokens.into()),
            Token::Uint(self.temperature.into()),
            Token::Uint(self.inbound_price),
            Token::Uint(self.outbound_price),
            Token::Uint(self.nonce.into()),
            Token::Uint(self.deadline.into()),
        ]
    }
}
```

### Nonce Management

Prevent replay attacks with incremental nonces:

```rust
pub struct NonceManager {
    nonces: HashMap<Address, u64>,
}

impl NonceManager {
    pub fn get_next_nonce(&mut self, address: Address) -> u64 {
        let current = self.nonces.get(&address).copied().unwrap_or(0);
        let next = current + 1;
        self.nonces.insert(address, next);
        next
    }
    
    pub fn validate_nonce(
        &self,
        address: Address,
        nonce: u64,
    ) -> bool {
        let expected = self.nonces.get(&address).copied().unwrap_or(0) + 1;
        nonce == expected
    }
}
```

### Hash Calculations

Secure hashing for sensitive data:

```rust
pub fn hash_prompt(prompt: &str) -> [u8; 32] {
    keccak256(prompt.as_bytes())
}

pub fn hash_content(content: &str) -> [u8; 32] {
    keccak256(content.as_bytes())
}

pub fn calculate_request_hash(
    commitment: &LlmRequestCommitment,
    signature: &Signature,
) -> [u8; 32] {
    let data = [
        &calculate_struct_hash(commitment)[..],
        &signature.to_bytes()[..],
    ].concat();
    
    keccak256(&data)
}
```

## Security Features

### 1. Replay Protection

Multiple layers prevent replay attacks:

- **Nonce tracking**: Sequential nonces per address
- **Deadline enforcement**: Time-limited validity
- **Request hash binding**: Response tied to specific request

### 2. Privacy Protection

Sensitive data is never exposed:

- **Prompt hashing**: Only hashes go on-chain
- **Content hashing**: Responses remain private
- **Selective disclosure**: Share only what's needed

### 3. Non-repudiation

Cryptographic proof of all actions:

- **Client commitment**: Can't deny making request
- **Executor commitment**: Can't deny processing
- **Dual signatures**: Both parties accountable

## Integration Examples

### Client-Side Signing

```rust
// Create commitment
let commitment = LlmRequestCommitment {
    executor: executor_address,
    model: "llama-2-7b".to_string(),
    prompt_hash: hash_prompt(&prompt),
    system_prompt_hash: hash_prompt(&system_prompt),
    max_tokens: 100,
    temperature: 7000, // 0.7 * 10000
    inbound_price: U256::from(1000000000000000u64), // 0.001 ETH
    outbound_price: U256::from(2000000000000000u64), // 0.002 ETH
    nonce: nonce_manager.get_next_nonce(client_address),
    deadline: current_timestamp() + 3600, // 1 hour
};

// Sign commitment
let signature = sign_commitment(&commitment, &signer).await?;

// Create signed request
let signed_request = SignedLlmRequest {
    commitment,
    signature,
    prompt: prompt.clone(),
    system_prompt: Some(system_prompt.clone()),
};
```

### Executor-Side Verification

```rust
// Verify client signature
let client_address = verify_commitment_signature(
    &request.commitment,
    &request.signature,
    request.commitment.client,
)?;

// Validate commitment
validate_commitment(&request.commitment)?;

// Process request
let response = process_llm_request(&request).await?;

// Create response commitment
let response_commitment = LlmResponseCommitment {
    request_hash: calculate_request_hash(&request.commitment, &request.signature),
    client: client_address,
    model: model_used.clone(),
    content_hash: hash_content(&response.content),
    inbound_tokens: response.usage.prompt_tokens,
    outbound_tokens: response.usage.completion_tokens,
    inbound_price: request.commitment.inbound_price,
    outbound_price: request.commitment.outbound_price,
    timestamp: current_timestamp(),
    success: true,
};

// Sign response
let response_signature = sign_response_commitment(
    &response_commitment,
    &executor_signer,
).await?;
```

## Smart Contract Verification

On-chain verification in Solidity:

```solidity
function verifyRequestSignature(
    LlmRequestCommitment memory commitment,
    bytes memory signature
) public view returns (address) {
    bytes32 structHash = keccak256(abi.encode(
        LLMREQUEST_TYPEHASH,
        commitment.executor,
        keccak256(bytes(commitment.model)),
        commitment.promptHash,
        commitment.systemPromptHash,
        commitment.maxTokens,
        commitment.temperature,
        commitment.inboundPrice,
        commitment.outboundPrice,
        commitment.nonce,
        commitment.deadline
    ));
    
    bytes32 messageHash = keccak256(abi.encodePacked(
        "\x19\x01",
        DOMAIN_SEPARATOR,
        structHash
    ));
    
    return ECDSA.recover(messageHash, signature);
}
```

## Best Practices

### For Implementers

1. **Always verify signatures** before processing
2. **Check deadlines** to prevent expired requests
3. **Track nonces** to prevent replays
4. **Validate prices** match commitments
5. **Hash sensitive data** before signing

### For Users

1. **Protect private keys** - they control your identity
2. **Verify domain** - ensure signing for correct network
3. **Check commitments** - review what you're signing
4. **Set reasonable deadlines** - not too short or long
5. **Monitor nonces** - ensure sequential ordering

## Testing Signatures

### Unit Tests

```rust
#[test]
fn test_signature_verification() {
    let signer = PrivateKeySigner::random();
    let commitment = create_test_commitment();
    
    let signature = sign_commitment(&commitment, &signer)
        .await
        .unwrap();
    
    let recovered = verify_commitment_signature(
        &commitment,
        &signature,
        signer.address(),
    ).unwrap();
    
    assert!(recovered);
}
```

### Integration Tests

Test with actual smart contracts:

```rust
#[test]
async fn test_contract_verification() {
    let provider = Provider::new(test_rpc_url());
    let contract = AccountingV2::deploy(provider).await?;
    
    let commitment = create_test_commitment();
    let signature = sign_commitment(&commitment, &signer).await?;
    
    let result = contract
        .verify_signature(commitment, signature.to_bytes())
        .call()
        .await?;
    
    assert_eq!(result, signer.address());
}
```

## Troubleshooting

### Common Issues

**Invalid signature recovery**
- Check domain parameters match
- Verify type hash is correct
- Ensure proper encoding

**Nonce mismatch**
- Sync with on-chain state
- Handle concurrent requests
- Implement retry logic

**Deadline exceeded**
- Use appropriate time windows
- Account for network delays
- Implement grace periods

## Summary

EIP-712 signing in Lloom provides:

- **Strong authentication** for all messages
- **Privacy protection** through hashing
- **Replay prevention** via nonces and deadlines
- **Ethereum compatibility** with standard tools
- **Extensible design** for future message types

This comprehensive signing system ensures that all interactions in the Lloom network are cryptographically secure, verifiable, and non-repudiable.