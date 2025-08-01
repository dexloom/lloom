# P2P Networking

Lloom's peer-to-peer networking layer is built on libp2p, providing a robust, decentralized communication infrastructure. This chapter covers the networking protocols, peer discovery mechanisms, and communication patterns used in the Lloom network.

## Overview

The P2P layer enables:
- Decentralized peer discovery without central servers
- Direct communication between clients and executors
- Resilient network topology with automatic failover
- Efficient message routing and delivery
- Network-wide announcements and updates

## libp2p Foundation

### Why libp2p?

Lloom chose libp2p for several key reasons:

1. **Protocol Agnostic**: Supports multiple transport protocols
2. **Modular Design**: Use only the protocols you need
3. **Battle-tested**: Used by IPFS, Filecoin, and Ethereum 2.0
4. **Language Support**: Excellent Rust implementation
5. **Active Development**: Continuous improvements and updates

### Core Concepts

**PeerId**: Unique identifier derived from node's public key
```rust
let peer_id = PeerId::from_public_key(&keypair.public());
```

**Multiaddr**: Flexible addressing scheme
```
/ip4/192.168.1.1/tcp/4001/p2p/QmNodeId
/ip6/::1/tcp/4001/p2p/QmNodeId
/dns4/example.com/tcp/443/wss/p2p/QmNodeId
```

**Swarm**: Manages connections and protocol handlers
```rust
let swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id)
    .build();
```

## Network Protocols

### 1. Kademlia DHT

Kademlia provides distributed hash table functionality for peer discovery:

**Configuration:**
```rust
let kad_config = KademliaConfig::default()
    .set_query_timeout(Duration::from_secs(60))
    .set_replication_factor(NonZeroUsize::new(20).unwrap())
    .to_owned();

let kademlia = Kademlia::with_config(peer_id, store, kad_config);
```

**Key Operations:**
- `bootstrap()`: Connect to the network
- `get_providers()`: Find nodes providing a service
- `start_providing()`: Announce service availability
- `get_closest_peers()`: Find nodes near a key

**Service Discovery Keys:**
```rust
pub const EXECUTOR_SERVICE_KEY: &str = "/lloom/executor/1.0.0";
pub const VALIDATOR_SERVICE_KEY: &str = "/lloom/validator/1.0.0";

// Announce as executor
swarm.behaviour_mut().kademlia.start_providing(
    RecordKey::new(&EXECUTOR_SERVICE_KEY)
)?;
```

### 2. Gossipsub

Gossipsub enables pub/sub messaging for network-wide announcements:

**Topics:**
```rust
pub fn executor_topic() -> IdentTopic {
    IdentTopic::new("/lloom/executors/1.0.0")
}

pub fn network_topic() -> IdentTopic {
    IdentTopic::new("/lloom/network/1.0.0")
}
```

**Message Types:**
- Executor availability announcements
- Model updates
- Network status broadcasts
- Price changes

**Configuration:**
```rust
let gossipsub_config = Config::default()
    .heartbeat_interval(Duration::from_secs(10))
    .validation_mode(ValidationMode::Strict)
    .build()?;

let gossipsub = Gossipsub::new(
    MessageAuthenticity::Signed(keypair),
    gossipsub_config
)?;
```

### 3. Request-Response

Direct communication between clients and executors:

**Protocol Definition:**
```rust
#[derive(Debug, Clone)]
pub struct LloomProtocol;

impl ProtocolName for LloomProtocol {
    fn protocol_name(&self) -> &[u8] {
        b"/lloom/request/1.0.0"
    }
}
```

**Request Flow:**
```rust
// Client sends request
let request_id = swarm.behaviour_mut()
    .request_response
    .send_request(&executor_peer_id, RequestMessage::LlmRequest(signed_request));

// Executor handles request
match event {
    RequestResponseEvent::Message { 
        message: RequestResponseMessage::Request { request, channel, .. }
    } => {
        // Process request
        let response = process_llm_request(request).await?;
        
        // Send response
        swarm.behaviour_mut()
            .request_response
            .send_response(channel, ResponseMessage::LlmResponse(response));
    }
}
```

### 4. Identify

Exchange peer information and capabilities:

```rust
let identify = Identify::new(
    Config::new("/lloom/1.0.0".to_string(), keypair.public())
        .with_agent_version(format!("lloom/{}", env!("CARGO_PKG_VERSION")))
);
```

**Exchanged Information:**
- Protocol version
- Supported protocols
- Listen addresses
- Agent version
- Public key

## Peer Discovery

### Bootstrap Process

1. **Connect to Validators**
```rust
for addr in bootstrap_nodes {
    swarm.dial(addr)?;
}
```

2. **Join DHT**
```rust
swarm.behaviour_mut().kademlia.bootstrap()?;
```

3. **Discover Services**
```rust
// Find executors
swarm.behaviour_mut()
    .kademlia
    .get_providers(RecordKey::new(&EXECUTOR_SERVICE_KEY));
```

### Continuous Discovery

The network maintains peer connections through:

**Periodic Bootstrap:**
```rust
interval.tick().await;
if swarm.behaviour().kademlia.bootstrap().is_err() {
    warn!("Failed to bootstrap Kademlia");
}
```

**mDNS Local Discovery:**
```rust
let mdns = Mdns::new(Default::default())?;

// Handle discovered peers
match event {
    MdnsEvent::Discovered(peers) => {
        for (peer_id, addr) in peers {
            swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
        }
    }
}
```

## Connection Management

### Transport Configuration

Lloom uses TCP with Noise encryption and Yamux multiplexing:

```rust
let transport = tcp::tokio::Transport::new(tcp::Config::default())
    .upgrade(Version::V1)
    .authenticate(noise::Config::new(&keypair)?)
    .multiplex(yamux::Config::default())
    .boxed();
```

**Security Features:**
- Noise protocol for encryption
- Authentication via keypairs
- Perfect forward secrecy

### Connection Pooling

Efficient connection reuse:

```rust
pub struct ConnectionPool {
    max_connections: usize,
    idle_timeout: Duration,
    connections: HashMap<PeerId, Connection>,
}
```

### NAT Traversal

Support for nodes behind NAT:

**Strategies:**
- UPnP port mapping
- NAT hole punching
- Relay nodes (planned)

## Message Patterns

### 1. One-to-One (Request-Response)

Direct client-executor communication:

```
Client ──────────> Executor
       Request
       
Client <────────── Executor
       Response
```

**Use Cases:**
- LLM inference requests
- Model availability queries
- Price negotiations

### 2. One-to-Many (Gossipsub)

Broadcast to interested peers:

```
Executor ─────┬────> Client A
              ├────> Client B
              └────> Client C
         Announcement
```

**Use Cases:**
- Model availability updates
- Network status broadcasts
- Price changes

### 3. Many-to-Many (DHT)

Distributed information sharing:

```
Peer A ←──→ DHT ←──→ Peer B
             ↑
             │
          Peer C
```

**Use Cases:**
- Service discovery
- Peer routing
- Distributed storage

## Network Events

### Event Handling

```rust
pub enum LloomEvent {
    // Connection events
    ConnectionEstablished { peer_id: PeerId, endpoint: ConnectedPoint },
    ConnectionClosed { peer_id: PeerId, cause: Option<ConnectionError> },
    
    // Discovery events
    ExecutorDiscovered { peer_id: PeerId, models: Vec<String> },
    ValidatorDiscovered { peer_id: PeerId, address: Multiaddr },
    
    // Message events
    RequestReceived { peer_id: PeerId, request: SignedLlmRequest },
    ResponseReceived { peer_id: PeerId, response: SignedLlmResponse },
    
    // Network events
    BootstrapComplete,
    NetworkError { error: NetworkError },
}
```

### Event Processing Loop

```rust
loop {
    select! {
        event = swarm.next() => {
            match event {
                Some(SwarmEvent::Behaviour(event)) => {
                    handle_behaviour_event(event).await?;
                }
                Some(SwarmEvent::ConnectionEstablished { peer_id, .. }) => {
                    info!("Connected to peer: {}", peer_id);
                }
                // ... handle other events
            }
        }
    }
}
```

## Performance Optimization

### 1. Connection Reuse

Maintain persistent connections to frequently used peers:

```rust
impl ConnectionManager {
    pub fn should_keep_alive(&self, peer_id: &PeerId) -> bool {
        self.is_validator(peer_id) || 
        self.recent_interactions(peer_id) > KEEP_ALIVE_THRESHOLD
    }
}
```

### 2. Message Compression

Optional compression for large payloads:

```rust
pub fn compress_message(msg: &[u8]) -> Result<Vec<u8>> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(msg)?;
    Ok(encoder.finish()?)
}
```

### 3. Parallel Requests

Handle multiple requests concurrently:

```rust
let mut futures = FuturesUnordered::new();

for executor in discovered_executors {
    futures.push(send_request(executor, request.clone()));
}

while let Some(result) = futures.next().await {
    if let Ok(response) = result {
        return Ok(response); // Use first successful response
    }
}
```

## Network Monitoring

### Metrics Collection

Track network health:

```rust
pub struct NetworkMetrics {
    connected_peers: Gauge,
    messages_sent: Counter,
    messages_received: Counter,
    discovery_latency: Histogram,
    request_duration: Histogram,
}
```

### Health Checks

Monitor peer connectivity:

```rust
async fn health_check(swarm: &mut Swarm<LloomBehaviour>) {
    let connected_peers = swarm.connected_peers().count();
    
    if connected_peers < MIN_PEERS {
        warn!("Low peer count: {}", connected_peers);
        // Attempt to discover more peers
        swarm.behaviour_mut().kademlia.bootstrap()?;
    }
}
```

## Security Considerations

### 1. Sybil Resistance

Protect against identity attacks:
- Proof of work for identity generation (future)
- Reputation systems
- Economic stakes

### 2. Message Validation

Verify all incoming messages:
```rust
pub fn validate_message(msg: &SignedMessage) -> Result<()> {
    // Check signature
    verify_signature(&msg)?;
    
    // Check timestamp
    if msg.timestamp < SystemTime::now() - MAX_MESSAGE_AGE {
        return Err(Error::MessageTooOld);
    }
    
    // Check nonce if present
    if let Some(nonce) = msg.nonce {
        check_nonce(msg.signer, nonce)?;
    }
    
    Ok(())
}
```

### 3. Rate Limiting

Prevent resource exhaustion:
```rust
pub struct RateLimiter {
    requests_per_peer: HashMap<PeerId, TokenBucket>,
    global_limit: TokenBucket,
}
```

## Best Practices

### For Node Operators

1. **Stable Connectivity**: Use reliable internet connections
2. **Open Ports**: Ensure P2P ports are accessible
3. **Resource Monitoring**: Track bandwidth and connection usage
4. **Regular Updates**: Keep node software current

### For Developers

1. **Handle Disconnections**: Implement retry logic
2. **Validate Peers**: Verify peer identities
3. **Efficient Serialization**: Use binary formats
4. **Timeout Handling**: Set appropriate timeouts

## Future Enhancements

### Planned Features

1. **Circuit Relay**: Support for nodes behind strict NATs
2. **PubSub Sharding**: Topic-based network partitioning
3. **Advanced Discovery**: ML-based peer selection
4. **QoS Guarantees**: Priority message delivery

### Research Areas

- Incentivized relay networks
- Privacy-preserving discovery
- Quantum-resistant protocols
- Cross-chain bridges

## Summary

Lloom's P2P networking layer provides:

- Robust peer discovery through Kademlia DHT
- Efficient messaging via Gossipsub and Request-Response
- Secure connections with authentication and encryption
- Scalable architecture supporting thousands of nodes
- Extensible design for future protocol additions

This foundation enables truly decentralized LLM services without central points of failure or control.