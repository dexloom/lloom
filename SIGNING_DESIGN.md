# Message-Level Signing Design for Lloom

## Overview
This document outlines the design for implementing message-level cryptographic signing for the Lloom P2P network. The goal is to ensure non-repudiation and create audit trails for all LLM requests and responses.

## Architecture

### 1. Signature Schema

Each signed message will follow this structure:

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

### 2. Signing Process

1. **Message Creation**: Create the original message (LlmRequest or LlmResponse)
2. **Serialization**: Serialize the message to bytes using serde_json
3. **Hashing**: Create a hash of the serialized message
4. **Signing**: Sign the hash using the node's PrivateKeySigner
5. **Wrapping**: Wrap the original message with signature metadata

### 3. Verification Process

1. **Extract Payload**: Extract the original message from SignedMessage
2. **Serialize**: Re-serialize the payload to bytes
3. **Hash**: Create a hash of the serialized payload
4. **Recover**: Recover the signer's address from the signature
5. **Verify**: Check that the recovered address matches the claimed signer

### 4. Integration Points

#### Client Side:
- Sign LlmRequest before sending to executor
- Verify LlmResponse signature after receiving

#### Executor Side:
- Verify LlmRequest signature before processing
- Sign LlmResponse before sending back

### 5. Error Handling

New error types will be added:
- `SignatureError`: For signing failures
- `VerificationError`: For verification failures
- `InvalidSigner`: When recovered address doesn't match

### 6. Backwards Compatibility

To maintain compatibility during transition:
- Accept both signed and unsigned messages initially
- Log warnings for unsigned messages
- Eventually enforce signature requirement

## Implementation Plan

### Phase 1: Core Infrastructure
1. Create SignedMessage struct and traits
2. Implement signing/verification utilities
3. Add error types

### Phase 2: Protocol Integration
1. Update protocol to use SignedMessage wrapper
2. Modify request/response handlers
3. Update serialization logic

### Phase 3: Node Integration
1. Update client to sign requests
2. Update executor to verify and sign
3. Add configuration flags

### Phase 4: Testing & Documentation
1. Unit tests for signing/verification
2. Integration tests for full flow
3. Update documentation

## Security Considerations

1. **Replay Protection**: Optional nonce field for preventing replay attacks
2. **Time Window**: Timestamp validation to reject old messages
3. **Key Management**: Leverage existing Identity management
4. **Algorithm**: Use secp256k1 (Ethereum compatible)

## Benefits

1. **Non-repudiation**: Clients cannot deny making requests
2. **Audit Trail**: All interactions are cryptographically verifiable
3. **Trust**: Executors can verify request authenticity
4. **Accountability**: Usage records can be tied to signed requests