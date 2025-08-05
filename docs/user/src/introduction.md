# Introduction

Welcome to Lloom, a decentralized peer-to-peer network for Large Language Model (LLM) services built with Rust. Lloom enables distributed AI inference by connecting clients who need LLM services with executors who provide computational resources, all coordinated through a trustless P2P network.

## What is Lloom?

Lloom is a decentralized marketplace for LLM services that allows:

- **Clients** to request LLM inference from the network without relying on centralized providers
- **Executors** to monetize their computational resources by serving LLM requests
- **Validators** to maintain network integrity and facilitate peer discovery

The system uses cryptographic signatures, blockchain-based accounting, and peer-to-peer networking to create a trustless environment where participants can interact directly without intermediaries.

## Key Features

### Decentralized Architecture
- No single point of failure or control
- Direct peer-to-peer communication between clients and executors
- Distributed discovery through Kademlia DHT

### Cryptographic Security
- EIP-712 structured data signing for all messages
- Ethereum-compatible identity system
- Tamper-proof message authentication
- Replay attack protection

### Flexible LLM Support
- Multiple backend support (OpenAI, LMStudio, custom)
- Model discovery and selection
- Automatic executor matching based on capabilities

### Transparent Accounting
- On-chain settlement with smart contracts
- Granular token usage tracking
- Dual-signature verification (client + executor)
- Fair pricing through market dynamics

### Developer-Friendly
- Comprehensive Rust libraries
- Clear API boundaries
- Extensive documentation
- Active development environment

## Network Participants

### Clients
Clients are users or applications that need LLM inference services. They:
- Create and sign requests with specific model requirements
- Discover available executors through the P2P network
- Pay for services based on token usage

### Executors
Executors are nodes that provide LLM inference services. They:
- Advertise available models and pricing
- Process incoming requests from clients
- Sign responses with usage information
- Receive compensation for services rendered

### Validators
Validators are special nodes that help maintain network stability. They:
- Act as bootstrap nodes for new participants
- Facilitate peer discovery
- Maintain network topology information
- Ensure protocol compliance

## Technology Stack

Lloom is built on modern, battle-tested technologies:

- **Rust**: For performance, safety, and reliability
- **libp2p**: For robust peer-to-peer networking
- **Ethereum**: For identity management and settlements
- **EIP-712**: For structured, verifiable message signing
- **Docker**: For easy deployment and development
- **Prometheus/Grafana**: For monitoring and observability

## Use Cases

Lloom enables various decentralized AI applications:

1. **Distributed AI Services**: Build applications that leverage multiple LLM providers
2. **Private AI Infrastructure**: Run your own LLM services without cloud dependencies
3. **Cost Optimization**: Access competitive pricing through market dynamics
4. **Research Networks**: Share computational resources for AI research
5. **Censorship Resistance**: Access AI services without geographical restrictions

## Getting Started

Ready to dive in? Here are your next steps:

1. **[Installation](./getting-started/installation.md)**: Set up the Lloom tools and dependencies
2. **[Quick Start](./getting-started/quick-start.md)**: Run your first client-executor interaction
3. **[Testnet](./getting-started/testnet.md)**: Connect to our public testnet for development
4. **[Architecture Overview](./technology/architecture.md)**: Understand the system design
5. **[Development Environment](./getting-started/development-environment.md)**: Set up a local test network

## Community and Support

Lloom is an open-source project welcoming contributions from developers, researchers, and users:

- **GitHub**: [github.com/lloom/lloom](https://github.com/lloom/lloom)
- **Documentation**: You're reading it!
- **Contributing**: See our [contribution guide](./development/contributing.md)

Join us in building the decentralized future of AI services!