# EIP-712 Accounting Specification

This document provides the complete technical specification for the EIP-712 structured data signing scheme used in Lloom's decentralized LLM network accounting system.

## Overview

The Lloom accounting system implements a comprehensive EIP-712 signing scheme that provides:
- **Dual Signature Verification**: Both clients and executors sign commitments
- **Pricing Transparency**: Clear per-token pricing for inbound and outbound tokens
- **Replay Protection**: Nonce-based replay prevention
- **Privacy Preservation**: Prompt hashing to prevent content leakage
- **Smart Contract Integration**: On-chain verification capabilities

## Domain Structure

### Domain Separator

The EIP-712 domain separator ensures signatures are unique to the Lloom network:

```solidity
struct EIP712Domain {
    string name;                // "Lloom Network"
    string version;             // "1.0.0"
    uint256 chainId;           // Network chain ID
    address verifyingContract; // Accounting contract address
}
```

### Domain Hash Calculation

```solidity
bytes32 DOMAIN_SEPARATOR = keccak256(abi.encode(
    keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"),
    keccak256(bytes("Lloom Network")),
    keccak256(bytes("1.0.0")),
    chainId,
    address(this)
));
```

## Type Definitions

### LlmRequestCommitment

Client's signed commitment for an LLM request:

```solidity
struct LlmRequestCommitment {
    address executor;          // Chosen executor address
    string model;              // Model identifier  
    bytes32 promptHash;        // keccak256 of prompt content
    bytes32 systemPromptHash;  // keccak256 of system prompt
    uint32 maxTokens;          // Maximum tokens to generate
    uint32 temperature;        // Temperature * 10000
    uint256 inboundPrice;      // Price per inbound token (wei)
    uint256 outboundPrice;     // Price per outbound token (wei)
    uint64 nonce;              // Client nonce
    uint64 deadline;           // Unix timestamp deadline
}
```

**Type Hash:**
```solidity
bytes32 constant LLMREQUEST_TYPEHASH = keccak256(
    "LlmRequestCommitment(address executor,string model,bytes32 promptHash,bytes32 systemPromptHash,uint32 maxTokens,uint32 temperature,uint256 inboundPrice,uint256 outboundPrice,uint64 nonce,uint64 deadline)"
);
```

### LlmResponseCommitment

Executor's signed commitment for the response:

```solidity
struct LlmResponseCommitment {
    bytes32 requestHash;       // Hash of the original request
    address client;            // Client address
    string model;              // Model actually used
    bytes32 contentHash;       // keccak256 of response content
    uint32 inboundTokens;      // Actual inbound tokens used
    uint32 outboundTokens;     // Actual outbound tokens generated
    uint256 inboundPrice;      // Confirmed inbound price
    uint256 outboundPrice;     // Confirmed outbound price
    uint64 timestamp;          // Response timestamp
    bool success;              // Execution success status
}
```

**Type Hash:**
```solidity
bytes32 constant LLMRESPONSE_TYPEHASH = keccak256(
    "LlmResponseCommitment(bytes32 requestHash,address client,string model,bytes32 contentHash,uint32 inboundTokens,uint32 outboundTokens,uint256 inboundPrice,uint256 outboundPrice,uint64 timestamp,bool success)"
);
```

## Signing Process

### Client Request Signing

1. **Prepare Request Data**:
   ```rust
   let request_commitment = LlmRequestCommitment {
       executor: executor_address,
       model: "gpt-3.5-turbo",
       promptHash: keccak256(prompt.as_bytes()),
       systemPromptHash: keccak256(system_prompt.as_bytes()),
       maxTokens: 1000,
       temperature: 7000, // 0.7 * 10000
       inboundPrice: "500000000000000",    // 0.0005 ETH/token
       outboundPrice: "1000000000000000",  // 0.001 ETH/token
       nonce: get_next_nonce(),
       deadline: current_time() + 3600,
   };
   ```

2. **Calculate Struct Hash**:
   ```rust
   let struct_hash = keccak256(encode_packed(&[
       LLMREQUEST_TYPEHASH,
       request_commitment.executor,
       keccak256(request_commitment.model),
       request_commitment.promptHash,
       request_commitment.systemPromptHash,
       request_commitment.maxTokens,
       request_commitment.temperature,
       request_commitment.inboundPrice,
       request_commitment.outboundPrice,
       request_commitment.nonce,
       request_commitment.deadline,
   ]));
   ```

3. **Create EIP-712 Digest**:
   ```rust
   let digest = keccak256(encode_packed(&[
       "\x19\x01",
       DOMAIN_SEPARATOR,
       struct_hash,
   ]));
   ```

4. **Sign with Private Key**:
   ```rust
   let signature = sign_digest(digest, private_key);
   ```

### Executor Response Signing

1. **Process Request and Generate Response**
2. **Create Response Commitment**:
   ```rust
   let response_commitment = LlmResponseCommitment {
       requestHash: hash_request(&original_request),
       client: client_address,
       model: "gpt-3.5-turbo",
       contentHash: keccak256(response_content.as_bytes()),
       inboundTokens: count_inbound_tokens(&request),
       outboundTokens: count_outbound_tokens(&response),
       inboundPrice: request.inboundPrice,
       outboundPrice: request.outboundPrice,
       timestamp: current_time(),
       success: true,
   };
   ```

3. **Sign Using Same EIP-712 Process**

## Verification Process

### On-Chain Verification

Smart contract verification implementation:

```solidity
function verifyRequestSignature(
    LlmRequestCommitment memory commitment,
    bytes memory signature
) public view returns (address) {
    bytes32 structHash = keccak256(abi.encode(
        LLMREQUEST_TYPEHASH,
        commitment
    ));
    
    bytes32 digest = keccak256(abi.encodePacked(
        "\x19\x01",
        DOMAIN_SEPARATOR,
        structHash
    ));
    
    return ecrecover(digest, signature);
}
```

### Off-Chain Verification

Rust implementation for P2P verification:

```rust
pub fn verify_request_signature(
    commitment: &LlmRequestCommitment,
    signature: &Signature,
    expected_signer: &Address,
) -> Result<bool> {
    let recovered = recover_signer(commitment, signature)?;
    Ok(recovered == *expected_signer)
}
```

## Nonce Management

### Client Nonce Tracking

Clients maintain sequential nonces per executor:

```rust
pub struct NonceManager {
    // Map of executor address to next nonce
    nonces: HashMap<Address, u64>,
}

impl NonceManager {
    pub fn get_next_nonce(&mut self, executor: &Address) -> u64 {
        let nonce = self.nonces.get(executor).unwrap_or(&0);
        let next = *nonce;
        self.nonces.insert(*executor, next + 1);
        next
    }
}
```

### Executor Nonce Validation

Executors track used nonces to prevent replay:

```rust
pub struct NonceValidator {
    // Map of client address to set of used nonces
    used_nonces: HashMap<Address, HashSet<u64>>,
}

impl NonceValidator {
    pub fn validate_and_record(
        &mut self, 
        client: &Address, 
        nonce: u64
    ) -> Result<()> {
        let client_nonces = self.used_nonces.entry(*client)
            .or_insert_with(HashSet::new);
        
        if client_nonces.contains(&nonce) {
            return Err("Nonce already used");
        }
        
        client_nonces.insert(nonce);
        Ok(())
    }
}
```

## Price Calculation

### Token Cost Computation

Calculate total cost based on token usage:

```solidity
function calculateTotalCost(
    uint32 inboundTokens,
    uint32 outboundTokens,
    uint256 inboundPrice,
    uint256 outboundPrice
) public pure returns (uint256) {
    uint256 inboundCost = uint256(inboundTokens) * inboundPrice;
    uint256 outboundCost = uint256(outboundTokens) * outboundPrice;
    return inboundCost + outboundCost;
}
```

### Price Validation

Ensure price agreement between client and executor:

```rust
pub fn validate_pricing(
    request: &LlmRequestCommitment,
    response: &LlmResponseCommitment,
) -> Result<()> {
    if request.inboundPrice != response.inboundPrice {
        return Err("Inbound price mismatch");
    }
    
    if request.outboundPrice != response.outboundPrice {
        return Err("Outbound price mismatch");
    }
    
    Ok(())
}
```

## Security Considerations

### Replay Attack Prevention

1. **Client Nonces**: Each client maintains separate nonce sequences per executor
2. **Timestamps**: Requests include deadlines to prevent old request replay
3. **Request Binding**: Responses include request hash to prevent mix-and-match attacks

### Privacy Protection

1. **Content Hashing**: Prompts and responses are hashed, not stored on-chain
2. **Selective Disclosure**: Only necessary metadata is included in signatures
3. **Off-Chain Storage**: Full content remains between client and executor

### Signature Security

1. **Domain Separation**: Prevents cross-application signature reuse
2. **Type Safety**: Structured data prevents ambiguity
3. **Chain Binding**: ChainId prevents cross-chain replay

## Implementation Notes

### Gas Optimization

1. **Struct Packing**: Group same-size fields together
2. **Hash Caching**: Pre-compute type hashes as constants
3. **Minimal On-Chain Data**: Store only essential verification data

### Compatibility

1. **EIP-712 Compliance**: Full compliance with the standard
2. **Wallet Support**: Compatible with MetaMask, WalletConnect, etc.
3. **Multi-Chain**: Supports deployment on any EVM chain

### Upgrade Path

1. **Version Field**: Domain version allows future upgrades
2. **Type Evolution**: New fields can be added to commitments
3. **Backward Compatibility**: Old signatures remain valid

## Testing Requirements

### Unit Tests

1. Signature generation and verification
2. Nonce management
3. Price calculations
4. Hash computations

### Integration Tests

1. Full request-response flow
2. Multi-client scenarios
3. Replay attack attempts
4. Cross-chain verification

### Security Audits

1. Signature malleability
2. Replay attack vectors
3. Integer overflow/underflow
4. Access control

## Appendix: Reference Implementation

### Rust Types

```rust
use alloy::sol_types::sol;

sol! {
    struct LlmRequestCommitment {
        address executor;
        string model;
        bytes32 promptHash;
        bytes32 systemPromptHash;
        uint32 maxTokens;
        uint32 temperature;
        uint256 inboundPrice;
        uint256 outboundPrice;
        uint64 nonce;
        uint64 deadline;
    }
    
    struct LlmResponseCommitment {
        bytes32 requestHash;
        address client;
        string model;
        bytes32 contentHash;
        uint32 inboundTokens;
        uint32 outboundTokens;
        uint256 inboundPrice;
        uint256 outboundPrice;
        uint64 timestamp;
        bool success;
    }
}
```

### Smart Contract Interface

```solidity
interface ILloomAccounting {
    function verifyAndRecordRequest(
        LlmRequestCommitment calldata commitment,
        bytes calldata signature
    ) external returns (bytes32 requestId);
    
    function verifyAndRecordResponse(
        LlmResponseCommitment calldata commitment,
        bytes calldata signature
    ) external returns (bool);
    
    function settlePayment(
        bytes32 requestId,
        bytes32 responseId
    ) external payable;
}
```