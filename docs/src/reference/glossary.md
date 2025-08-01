# Glossary

This glossary defines key terms and concepts used throughout the Lloom documentation and codebase.

## A

### Accounting Contract
The smart contract responsible for tracking request/response commitments, managing payments, and settling disputes between clients and executors.

### Alloy
A Rust library for Ethereum interaction, used by Lloom for blockchain operations, transaction signing, and smart contract interactions.

### API Key
Authentication credential used by executors to access LLM backend services like OpenAI or other providers.

## B

### Bootstrap Peer
A known peer address used to join the P2P network. New nodes connect to bootstrap peers to discover other network participants.

### Backend
The underlying LLM service provider (e.g., OpenAI, LMStudio, custom model) that executors use to process requests.

## C

### Chain ID
Ethereum network identifier (1 for mainnet, 11155111 for Sepolia, etc.) used to prevent cross-chain replay attacks.

### Client
A node that submits LLM requests to the network and pays for responses. Clients discover executors and validate responses.

### Commitment
A cryptographically signed promise representing either a request for LLM services or a response to such a request.

### Consensus
Agreement among multiple validators about the validity of a response. Required for high-value or sensitive requests.

### Context Length
The maximum number of tokens a language model can process in a single request, including both input and output.

## D

### DHT (Distributed Hash Table)
Kademlia-based distributed storage system used for peer discovery and service advertisement in the P2P network.

### Dispute
A formal challenge to a response's validity, initiated by a client when they believe an executor provided incorrect or substandard service.

### Domain Separator
An EIP-712 construct that ensures signatures are unique to a specific contract and chain, preventing signature replay attacks.

## E

### EIP-712
Ethereum Improvement Proposal defining a standard for structured data signing, used by Lloom for all cryptographic commitments.

### Executor
A node that processes LLM requests using backend services and returns responses. Executors advertise capabilities and set pricing.

### EVM Address
Ethereum Virtual Machine address derived from a node's identity, used for on-chain interactions and payment settlement.

## F

### Foundry
A fast, portable, and modular toolkit for Ethereum development, used by Lloom for smart contract development and testing.

## G

### Gas
Computational cost unit in Ethereum, required for executing smart contract operations and settling payments.

### Gossipsub
A pub/sub protocol used in libp2p for broadcasting messages across the P2P network, such as executor announcements.

### GPU Memory
Video memory used by local LLM models. Executors can set limits to prevent out-of-memory errors.

## H

### Hash
A fixed-size cryptographic fingerprint of data. Used for content verification and request/response matching.

### Hotfix
An urgent software update addressing critical bugs or security vulnerabilities, released outside the normal release cycle.

## I

### Identity
A cryptographic keypair that uniquely identifies a node in the network. Includes both P2P identity and Ethereum wallet.

### Inbound Price
The cost per token for input/prompt tokens in an LLM request, set by executors and agreed to by clients.

## J

### JWT (JSON Web Token)
Token format potentially used for API authentication in future versions of Lloom.

## K

### Kademlia
A distributed hash table protocol used by libp2p for peer discovery and content routing in the P2P network.

### Keccak256
Cryptographic hash function used in Ethereum, employed by Lloom for creating content hashes and identifiers.

## L

### Libp2p
A modular network protocol stack used by Lloom for P2P communication, peer discovery, and data exchange.

### LLM (Large Language Model)
AI models capable of understanding and generating human-like text, such as GPT-4, Llama, or Claude.

### LMStudio
A desktop application for running local LLM models, supported as a backend option for Lloom executors.

### Lloom
The decentralized P2P network for LLM inference, enabling trustless interactions between clients and compute providers.

## M

### mDNS (Multicast DNS)
Protocol for discovering peers on the local network without centralized servers, useful for development and testing.

### Metrics
Performance and operational data exposed in Prometheus format for monitoring system health and performance.

### Model
A specific LLM variant (e.g., "gpt-4", "llama-2-70b") with defined capabilities, context length, and pricing.

### Multiaddr
A self-describing network address format used by libp2p, encoding multiple protocols (e.g., `/ip4/127.0.0.1/tcp/4001`).

## N

### Network ID
Identifier separating different Lloom networks (mainnet, testnet, devnet) to prevent cross-network communication.

### Nonce
A number used once to prevent replay attacks. Each account has an incrementing nonce for request ordering.

### Node
Any participant in the Lloom network - can be a client, executor, validator, or a combination of roles.

## O

### ONNX
Open Neural Network Exchange format, supported by Lloom for cross-platform model deployment.

### Outbound Price
The cost per token for output/completion tokens in an LLM response, set by executors and agreed to by clients.

## P

### P2P (Peer-to-Peer)
Network architecture where nodes communicate directly without central servers, used by Lloom for all networking.

### Peer ID
Unique identifier for a node in the libp2p network, derived from the node's cryptographic public key.

### Pricing
The cost structure for LLM services, including separate rates for input and output tokens.

### Prometheus
Open-source monitoring system used by Lloom for collecting and exposing metrics data.

### Protocol
The message format and communication rules used between Lloom nodes for request/response exchange.

## Q

### Queue
Request buffer in executors that holds pending requests when all processing slots are occupied.

### Query
A search operation in the DHT to find peers offering specific services or models.

## R

### Registry Contract
Smart contract maintaining on-chain records of executor capabilities and reputation scores.

### Request
A client's ask for LLM processing, including prompt, model choice, parameters, and pricing agreement.

### Response
An executor's answer to a request, including generated content, token counts, and cryptographic proof.

### RPC (Remote Procedure Call)
Protocol for interacting with Ethereum nodes, used by Lloom for blockchain operations.

## S

### Service Discovery
The process of finding peers offering specific services (e.g., executors supporting particular models).

### Signature
Cryptographic proof that a message was created by a specific identity, used for all commitments.

### Smart Contract
Self-executing code on Ethereum that handles accounting, payments, and dispute resolution for Lloom.

### Swarm
The libp2p networking component managing peer connections, protocols, and message routing.

## T

### Temperature
LLM parameter controlling response randomness (0 = deterministic, higher = more creative).

### Token
1. In LLM context: A unit of text (roughly 4 characters) used for pricing
2. In blockchain context: Digital asset used for payments

### Topic
A gossipsub channel for broadcasting specific types of messages across the network.

### Transaction
An Ethereum blockchain operation, such as submitting a request commitment or settling a payment.

## U

### Upgradeable
Smart contract pattern allowing contract logic updates while preserving state and addresses.

## V

### Validation
The process of verifying that a response matches its commitment and meets quality standards.

### Validator
A node that verifies response correctness and quality, providing trust and dispute resolution services.

### Verification
Cryptographic proof checking, ensuring signatures are valid and data hasn't been tampered with.

## W

### Wallet
Ethereum account used for signing transactions and receiving payments, derived from node identity.

### Wei
The smallest unit of Ether (1 ETH = 10^18 wei), used for precise price calculations.

### WebSocket
Persistent connection protocol used for real-time communication and response streaming.

## X

### XOR Distance
Metric used in Kademlia DHT for determining peer proximity in the overlay network.

## Y

### Yank
Crates.io operation to mark a published version as unsuitable for use, typically due to critical bugs.

## Z

### Zero-Knowledge Proof
Cryptographic method to prove knowledge without revealing the information itself (planned feature).

---

## Acronyms

- **API**: Application Programming Interface
- **CLI**: Command Line Interface
- **DHT**: Distributed Hash Table
- **EIP**: Ethereum Improvement Proposal
- **ETH**: Ether (Ethereum native currency)
- **GPU**: Graphics Processing Unit
- **JSON**: JavaScript Object Notation
- **JWT**: JSON Web Token
- **LLM**: Large Language Model
- **P2P**: Peer-to-Peer
- **RPC**: Remote Procedure Call
- **TLS**: Transport Layer Security
- **TOML**: Tom's Obvious, Minimal Language
- **UUID**: Universally Unique Identifier

## Common Patterns

### Request-Response Flow
1. Client creates and signs request commitment
2. Client discovers suitable executor
3. Client sends request to executor
4. Executor processes request with LLM backend
5. Executor creates and signs response commitment
6. Client validates response
7. Payment is settled (optionally on-chain)

### Service Discovery Flow
1. Executor advertises capabilities via DHT
2. Executor announces availability via gossipsub
3. Client queries DHT for specific model
4. Client receives list of capable executors
5. Client selects executor based on price/reputation

### Validation Flow
1. Validator observes request/response pair
2. Validator checks cryptographic signatures
3. Validator verifies token counts
4. Validator assesses response quality
5. Validator publishes validation report
6. Network aggregates validation consensus