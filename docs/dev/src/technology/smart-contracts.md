# Smart Contracts

The Lloom network utilizes Ethereum smart contracts to provide transparent accounting, payment settlement, and trust mechanisms for the decentralized LLM network. Our contracts are developed using Foundry and leverage EIP-712 for structured data signing.

## Architecture Overview

The smart contract system consists of two primary contracts:

### AccountingV2 Contract

The core accounting contract that manages:
- **Dual Signature Verification**: Both clients and executors sign commitments
- **Token Usage Tracking**: Precise tracking of inbound and outbound token consumption
- **Price Transparency**: Clear pricing structure for both prompt and completion tokens
- **Dispute Resolution**: Built-in mechanisms for handling disagreements

### ECDSA Library

A gas-optimized ECDSA signature verification library that provides:
- Signature recovery functions
- EIP-712 compliant message hashing
- Protection against signature malleability

## EIP-712 Implementation

Our contracts implement EIP-712 structured data signing for security and user experience:

```solidity
// Domain separator for the Lloom Network
bytes32 public constant DOMAIN_TYPEHASH = keccak256(
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
);

// Type hash for LLM request commitments
bytes32 public constant LLMREQUEST_TYPEHASH = keccak256(
    "LlmRequestCommitment(address executor,string model,bytes32 promptHash,bytes32 systemPromptHash,uint32 maxTokens,uint32 temperature,uint256 inboundPrice,uint256 outboundPrice,uint64 nonce,uint64 deadline)"
);

// Type hash for LLM response commitments
bytes32 public constant LLMRESPONSE_TYPEHASH = keccak256(
    "LlmResponseCommitment(bytes32 requestHash,address client,string model,bytes32 contentHash,uint32 inboundTokens,uint32 outboundTokens,uint256 inboundPrice,uint256 outboundPrice,uint64 timestamp,bool success)"
);
```

## Request-Response Flow

1. **Client Creates Request Commitment**
   - Selects executor and model
   - Specifies token limits and pricing
   - Signs commitment with EIP-712

2. **Executor Validates Commitment**
   - Verifies client signature
   - Checks pricing and limits
   - Processes LLM request

3. **Executor Creates Response Commitment**
   - Records actual token usage
   - Includes content hash
   - Signs response with EIP-712

4. **Settlement**
   - Both parties have signed commitments
   - On-chain verification if disputes arise
   - Automatic payment settlement

## Key Features

### Privacy-Preserving Design

- Prompt and response content are hashed, not stored on-chain
- Only metadata and commitments are recorded
- Full content remains off-chain between parties

### Flexible Pricing Model

```solidity
struct LlmRequestCommitment {
    uint256 inboundPrice;     // Price per inbound token (wei per token)
    uint256 outboundPrice;    // Price per outbound token (wei per token)
    // ... other fields
}
```

Separate pricing for:
- **Inbound tokens**: Prompt and system prompt tokens
- **Outbound tokens**: Generated completion tokens

### Replay Protection

- Client-controlled nonces prevent replay attacks
- Deadline timestamps ensure timely execution
- Request hashes link responses to specific requests

### Gas Optimization

- Efficient struct packing to minimize storage costs
- Batch operations for multiple commitments
- Optimized ECDSA recovery implementation

## Contract Deployment

### Local Development

```bash
# Start local Ethereum node
cd ethnode
docker-compose up -d

# Deploy contracts
forge script script/Deploy.s.sol --rpc-url http://localhost:8545
```

### Testnet Deployment

```bash
# Deploy to Sepolia testnet
forge script script/Deploy.s.sol --rpc-url $SEPOLIA_RPC_URL --private-key $PRIVATE_KEY --broadcast
```

## Integration with Rust Crates

The Rust codebase integrates with contracts via:

### Alloy Framework
- Type-safe contract bindings
- Automatic ABI encoding/decoding
- EIP-712 message construction

### Example Integration

```rust
use alloy::sol_types::SolStruct;

// Define contract types
sol! {
    struct LlmRequestCommitment {
        address executor;
        string model;
        bytes32 promptHash;
        // ... other fields
    }
}

// Create and sign commitment
let commitment = LlmRequestCommitment {
    executor: executor_address,
    model: "gpt-4".to_string(),
    promptHash: keccak256(prompt.as_bytes()),
    // ... fill other fields
};

let signature = sign_eip712_commitment(&commitment, &signer).await?;
```

## Security Considerations

### Signature Verification
- All signatures are verified using ecrecover
- Protection against signature malleability attacks
- Strict type checking for EIP-712 compliance

### Access Control
- Only authorized parties can submit commitments
- Rate limiting through nonces
- Deadline enforcement

### Economic Security
- Staking mechanisms for executors (planned)
- Slashing for misbehavior (planned)
- Reputation tracking (planned)

## Testing

The contracts include comprehensive test coverage:

```bash
# Run all tests
forge test

# Run with gas reporting
forge test --gas-report

# Run specific test
forge test --match-test testDualSignatureVerification -vvv
```

### Test Categories

1. **Unit Tests**: Individual function testing
2. **Integration Tests**: Full flow testing
3. **Fuzz Tests**: Property-based testing
4. **Gas Optimization Tests**: Ensuring efficient execution

## Future Enhancements

### Planned Features

1. **Staking System**
   - Executor stakes for quality assurance
   - Client deposits for payment guarantee

2. **Reputation System**
   - On-chain reputation scores
   - Historical performance tracking

3. **Batch Settlement**
   - Aggregate multiple requests
   - Reduced gas costs

4. **Cross-chain Support**
   - Deploy on multiple chains
   - Bridge commitments between chains

### Governance

- Upgradeable proxy pattern for contract evolution
- Multi-sig control for critical parameters
- Community governance for fee structures

## Resources

- [Foundry Documentation](https://book.getfoundry.sh/)
- [EIP-712 Specification](https://eips.ethereum.org/EIPS/eip-712)
- [Alloy Documentation](https://alloy.rs/)
- [Contract Source Code](/solidity/src/)