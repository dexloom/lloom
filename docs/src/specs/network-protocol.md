# Network Protocol Specification

This document provides the complete specification for the Lloom P2P network protocol, built on libp2p. It defines how nodes discover each other, communicate, and maintain the network.

## Protocol Overview

The Lloom network combines multiple libp2p protocols to create a robust P2P infrastructure:

### Protocol Stack

1. **Transport Layer**: TCP with TLS/Noise encryption
2. **Multiplexing**: Yamux for stream multiplexing
3. **Discovery**: Kademlia DHT + mDNS
4. **Messaging**: Request-Response + Gossipsub
5. **NAT Traversal**: AutoNAT + Circuit Relay

### Protocol Identifiers

```
/lloom/kad/1.0.0         - Kademlia DHT
/lloom/gossip/1.0.0      - Gossipsub messaging
/lloom/req-resp/1.0.0    - Request-Response protocol
/lloom/relay/1.0.0       - Circuit relay protocol
```

## Transport Configuration

### TCP Transport

```rust
pub struct TransportConfig {
    /// Listen addresses
    pub listen_addrs: Vec<Multiaddr>,
    
    /// External address (for NAT)
    pub external_addr: Option<Multiaddr>,
    
    /// Connection limits
    pub max_connections: u32,
    pub max_connections_per_peer: u32,
    pub idle_connection_timeout: Duration,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            listen_addrs: vec!["/ip4/0.0.0.0/tcp/0".parse().unwrap()],
            external_addr: None,
            max_connections: 200,
            max_connections_per_peer: 3,
            idle_connection_timeout: Duration::from_secs(600),
        }
    }
}
```

### Security Layer

TLS 1.3 or Noise protocol for encryption:

```rust
pub enum SecurityProtocol {
    Tls(TlsConfig),
    Noise(NoiseConfig),
}

pub struct TlsConfig {
    /// Certificate for node identity
    pub certificate: Certificate,
    
    /// Private key
    pub private_key: PrivateKey,
    
    /// Verify peer certificates
    pub verify_peer: bool,
}

pub struct NoiseConfig {
    /// Noise protocol pattern
    pub pattern: NoisePattern,
    
    /// Static keypair
    pub keypair: Keypair,
}
```

## Peer Discovery

### Kademlia DHT

Distributed hash table for peer and service discovery:

```rust
pub struct KademliaConfig {
    /// Protocol identifier
    pub protocol: String, // "/lloom/kad/1.0.0"
    
    /// Replication factor (k-value)
    pub replication_factor: NonZeroUsize,
    
    /// Query parallelism (α-value)
    pub query_parallelism: NonZeroUsize,
    
    /// Record TTL
    pub record_ttl: Duration,
    
    /// Provider record TTL
    pub provider_ttl: Duration,
    
    /// Republish interval
    pub republish_interval: Duration,
}

impl Default for KademliaConfig {
    fn default() -> Self {
        Self {
            protocol: "/lloom/kad/1.0.0".to_string(),
            replication_factor: NonZeroUsize::new(20).unwrap(),
            query_parallelism: NonZeroUsize::new(3).unwrap(),
            record_ttl: Duration::from_secs(3600),
            provider_ttl: Duration::from_secs(86400),
            republish_interval: Duration::from_secs(1800),
        }
    }
}
```

#### DHT Operations

**Service Registration**:
```rust
pub async fn register_service(
    kad: &mut Kademlia<MemoryStore>,
    service: ServiceType,
    info: ServiceInfo,
) -> Result<()> {
    let key = Key::from(service.to_key_bytes());
    let record = Record {
        key: key.clone(),
        value: serde_cbor::to_vec(&info)?,
        publisher: None,
        expires: Some(Instant::now() + Duration::from_secs(3600)),
    };
    
    kad.put_record(record, Quorum::One)?;
    kad.provide(key)?;
    Ok(())
}
```

**Service Discovery**:
```rust
pub async fn discover_services(
    kad: &mut Kademlia<MemoryStore>,
    service: ServiceType,
) -> Result<Vec<ServiceInfo>> {
    let key = Key::from(service.to_key_bytes());
    
    // Get providers
    let providers = kad.get_providers(key);
    
    // Get records from providers
    let mut services = Vec::new();
    for provider in providers {
        if let Ok(record) = kad.get_record(&key, Quorum::One).await {
            let info: ServiceInfo = serde_cbor::from_slice(&record.value)?;
            services.push(info);
        }
    }
    
    Ok(services)
}
```

### mDNS Discovery

Local network discovery:

```rust
pub struct MdnsConfig {
    /// Enable mDNS
    pub enabled: bool,
    
    /// Service name
    pub service_name: String,
    
    /// Query interval
    pub query_interval: Duration,
    
    /// TTL for announcements
    pub ttl: Duration,
}

impl Default for MdnsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            service_name: "_lloom._tcp.local".to_string(),
            query_interval: Duration::from_secs(30),
            ttl: Duration::from_secs(120),
        }
    }
}
```

### Bootstrap Nodes

Initial peers for network entry:

```rust
pub struct BootstrapConfig {
    /// Bootstrap peer addresses
    pub peers: Vec<Multiaddr>,
    
    /// Minimum connected peers
    pub min_peers: usize,
    
    /// Bootstrap retry interval
    pub retry_interval: Duration,
    
    /// Parallel connection attempts
    pub parallel_connections: usize,
}

pub const DEFAULT_BOOTSTRAP_PEERS: &[&str] = &[
    "/dns4/bootstrap1.lloom.network/tcp/4001/p2p/12D3KooWH3uVF6wv47WnArKHk5ZDVmGb6CVbEDqVyemfPnCaQnQM",
    "/dns4/bootstrap2.lloom.network/tcp/4001/p2p/12D3KooWLRPJgqrZxc7CZ7ojbVXKmQ3pYAe2dCTmJbHN6F5RdVkE",
    "/dns4/bootstrap3.lloom.network/tcp/4001/p2p/12D3KooWBUJifCTgaxAUrcM9JysqCcS4CS8xVHyVc5a8S6z5FuRJ",
];
```

## Messaging Protocols

### Gossipsub

Pub/sub protocol for network-wide announcements:

```rust
pub struct GossipsubConfig {
    /// Message validation
    pub validation_mode: ValidationMode,
    
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    
    /// Mesh network parameters
    pub mesh_n: usize,          // Target mesh degree
    pub mesh_n_low: usize,      // Lower bound
    pub mesh_n_high: usize,     // Upper bound
    pub mesh_outbound_min: usize, // Min outbound connections
    
    /// Message cache
    pub history_length: usize,
    pub history_gossip: usize,
    
    /// Scoring parameters
    pub peer_score_params: Option<PeerScoreParams>,
}
```

#### Topics

Pre-defined gossipsub topics:

```rust
pub enum GossipTopic {
    /// Executor availability announcements
    ExecutorAnnouncements,
    
    /// Network status updates
    NetworkStatus,
    
    /// Price updates
    PriceUpdates,
    
    /// Validator reports
    ValidatorReports,
}

impl GossipTopic {
    pub fn to_topic(&self) -> Topic {
        let topic_str = match self {
            Self::ExecutorAnnouncements => "/lloom/announcements/executor/1.0.0",
            Self::NetworkStatus => "/lloom/status/network/1.0.0",
            Self::PriceUpdates => "/lloom/updates/price/1.0.0",
            Self::ValidatorReports => "/lloom/reports/validator/1.0.0",
        };
        Topic::new(topic_str)
    }
}
```

#### Message Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipMessage {
    /// Executor availability
    ExecutorAvailable {
        peer_id: PeerId,
        models: Vec<String>,
        capacity: u32,
        pricing: PricingInfo,
    },
    
    /// Executor going offline
    ExecutorOffline {
        peer_id: PeerId,
        reason: Option<String>,
    },
    
    /// Network metrics
    NetworkMetrics {
        total_peers: u32,
        active_executors: u32,
        active_requests: u32,
        average_latency_ms: u32,
    },
    
    /// Model price update
    PriceUpdate {
        executor: PeerId,
        model: String,
        inbound_price: String,
        outbound_price: String,
        effective_at: u64,
    },
}
```

### Request-Response Protocol

Direct peer-to-peer messaging:

```rust
pub struct RequestResponseConfig {
    /// Protocol name
    pub protocol: String, // "/lloom/req-resp/1.0.0"
    
    /// Request timeout
    pub request_timeout: Duration,
    
    /// Maximum request size
    pub max_request_size: usize,
    
    /// Maximum response size
    pub max_response_size: usize,
    
    /// Connection keep-alive
    pub keep_alive: Duration,
}
```

#### Message Flow

1. **Client → Executor Request**:
   ```rust
   let request = RequestMessage::LlmRequest(signed_request);
   let request_id = behaviour.send_request(&executor_peer, request)?;
   ```

2. **Executor Processing**:
   ```rust
   match request {
       RequestMessage::LlmRequest(req) => {
           let response = process_llm_request(req).await?;
           let signed_response = sign_message(response)?;
           ResponseMessage::LlmResponse(signed_response)
       }
   }
   ```

3. **Executor → Client Response**:
   ```rust
   behaviour.send_response(channel, response)?;
   ```

#### Codec Implementation

```rust
pub struct LloomCodec;

impl RequestResponseCodec for LloomCodec {
    type Protocol = String;
    type Request = RequestMessage;
    type Response = ResponseMessage;
    
    async fn read_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;
        serde_cbor::from_slice(&buf).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, e)
        })
    }
    
    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let buf = serde_cbor::to_vec(&req).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, e)
        })?;
        io.write_all(&buf).await
    }
    
    // Similar implementations for read_response and write_response
}
```

## NAT Traversal

### AutoNAT Protocol

Automatic NAT detection and external address discovery:

```rust
pub struct AutoNatConfig {
    /// Enable AutoNAT server
    pub enable_server: bool,
    
    /// Confidence threshold
    pub confidence_threshold: usize,
    
    /// Throttle server responses
    pub throttle_interval: Duration,
    
    /// Only test global IPs
    pub only_global_ips: bool,
    
    /// Boot delay
    pub boot_delay: Duration,
}
```

### Circuit Relay

Relay protocol for nodes behind NAT:

```rust
pub struct RelayConfig {
    /// Enable relay client
    pub enable_client: bool,
    
    /// Enable relay server
    pub enable_server: bool,
    
    /// Maximum circuits
    pub max_circuits: usize,
    
    /// Circuit duration
    pub max_circuit_duration: Duration,
    
    /// Maximum data relayed per circuit
    pub max_circuit_bytes: u64,
}
```

#### Relay Usage

```rust
// Connect through relay
let relay_addr = "/p2p/12D3KooWRelay/p2p-circuit/p2p/12D3KooWTarget";
swarm.dial(relay_addr.parse::<Multiaddr>()?)?;

// Listen through relay
let relay_listener = "/p2p/12D3KooWRelay/p2p-circuit";
swarm.listen_on(relay_listener.parse()?)?;
```

## Connection Management

### Connection Limits

```rust
pub struct ConnectionLimits {
    /// Maximum total connections
    pub max_connections: u32,
    
    /// Maximum connections per peer
    pub max_connections_per_peer: u32,
    
    /// Maximum pending connections
    pub max_pending_connections: u32,
    
    /// Connection timeout
    pub connection_timeout: Duration,
}
```

### Peer Scoring

```rust
pub struct PeerScore {
    /// Connection reliability (0.0 - 1.0)
    pub reliability: f32,
    
    /// Response time percentile
    pub latency_p50: Duration,
    pub latency_p99: Duration,
    
    /// Success rate
    pub success_rate: f32,
    
    /// Total interactions
    pub total_interactions: u64,
    
    /// Last seen timestamp
    pub last_seen: Instant,
}

pub trait PeerScoring {
    fn update_score(&mut self, peer: &PeerId, interaction: Interaction);
    fn get_score(&self, peer: &PeerId) -> Option<&PeerScore>;
    fn prune_low_score_peers(&mut self, threshold: f32) -> Vec<PeerId>;
}
```

## Protocol Negotiation

### Multistream Select

Protocol negotiation for connections:

```rust
pub struct ProtocolNegotiation {
    /// Supported protocols in preference order
    pub protocols: Vec<StreamProtocol>,
    
    /// Negotiation timeout
    pub timeout: Duration,
    
    /// Fallback protocol
    pub fallback: Option<StreamProtocol>,
}

impl Default for ProtocolNegotiation {
    fn default() -> Self {
        Self {
            protocols: vec![
                StreamProtocol::new("/lloom/req-resp/1.0.0"),
                StreamProtocol::new("/lloom/req-resp/0.9.0"), // Backward compat
            ],
            timeout: Duration::from_secs(10),
            fallback: None,
        }
    }
}
```

## Security Considerations

### Message Authentication

All critical messages must be signed:

```rust
pub struct SecurityPolicy {
    /// Require signed LLM requests
    pub require_signed_requests: bool,
    
    /// Require signed LLM responses
    pub require_signed_responses: bool,
    
    /// Verify gossipsub message signatures
    pub verify_gossip_signatures: bool,
    
    /// Maximum message age
    pub max_message_age: Duration,
}
```

### DoS Protection

```rust
pub struct DosProtection {
    /// Rate limiting per peer
    pub rate_limit: RateLimit,
    
    /// Request size limits
    pub max_request_size: usize,
    
    /// Connection limits
    pub connection_limits: ConnectionLimits,
    
    /// Blacklist duration
    pub blacklist_duration: Duration,
}

pub struct RateLimit {
    /// Maximum requests per minute
    pub requests_per_minute: u32,
    
    /// Maximum bandwidth per second
    pub bytes_per_second: u64,
    
    /// Burst allowance
    pub burst_size: u32,
}
```

### Privacy

```rust
pub struct PrivacyConfig {
    /// Hide client addresses in gossip
    pub anonymous_gossip: bool,
    
    /// Use relay for all connections
    pub force_relay: bool,
    
    /// Onion routing layers
    pub onion_layers: Option<u8>,
}
```

## Network Events

### Event Types

```rust
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Peer connected
    PeerConnected {
        peer_id: PeerId,
        endpoint: ConnectedPoint,
        num_established: u32,
    },
    
    /// Peer disconnected
    PeerDisconnected {
        peer_id: PeerId,
        cause: Option<ConnectionError>,
    },
    
    /// New listen address
    NewListenAddr {
        listener_id: ListenerId,
        address: Multiaddr,
    },
    
    /// External address discovered
    ExternalAddrDiscovered {
        address: Multiaddr,
        source: AddressSource,
    },
    
    /// Protocol event
    Protocol(ProtocolEvent),
}

#[derive(Debug, Clone)]
pub enum ProtocolEvent {
    Kademlia(KademliaEvent),
    Gossipsub(GossipsubEvent),
    RequestResponse(RequestResponseEvent),
    Mdns(MdnsEvent),
}
```

### Event Handling

```rust
pub trait NetworkEventHandler {
    fn on_peer_connected(&mut self, peer: PeerId, endpoint: ConnectedPoint);
    fn on_peer_disconnected(&mut self, peer: PeerId, cause: Option<ConnectionError>);
    fn on_message_received(&mut self, peer: PeerId, message: Message);
    fn on_protocol_event(&mut self, event: ProtocolEvent);
}
```

## Metrics and Monitoring

### Network Metrics

```rust
pub struct NetworkMetrics {
    /// Connection metrics
    pub connected_peers: Gauge,
    pub connection_duration: Histogram,
    pub bytes_sent: Counter,
    pub bytes_received: Counter,
    
    /// Protocol metrics
    pub kad_queries: Counter,
    pub gossip_messages: Counter,
    pub requests_sent: Counter,
    pub requests_received: Counter,
    
    /// Performance metrics
    pub request_latency: Histogram,
    pub message_processing_time: Histogram,
}
```

### Health Monitoring

```rust
pub struct NetworkHealth {
    /// Minimum connected peers
    pub min_peers: usize,
    
    /// Maximum peer churn rate
    pub max_churn_rate: f32,
    
    /// Required bootstrap peers
    pub required_bootstraps: usize,
    
    /// DHT health threshold
    pub dht_routing_table_size: usize,
}

pub fn check_network_health(swarm: &Swarm<LloomBehaviour>) -> HealthStatus {
    let connected_peers = swarm.connected_peers().count();
    let routing_table_size = swarm.behaviour().kad.kbuckets().count();
    
    HealthStatus {
        healthy: connected_peers >= 5 && routing_table_size >= 10,
        connected_peers,
        routing_table_size,
        // ... other metrics
    }
}
```

## Protocol Upgrades

### Version Management

```rust
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

pub trait VersionedProtocol {
    fn current_version() -> ProtocolVersion;
    fn supported_versions() -> Vec<ProtocolVersion>;
    fn negotiate_version(peer_version: ProtocolVersion) -> Option<ProtocolVersion>;
}
```

### Migration Strategy

```rust
pub struct ProtocolMigration {
    /// Old protocol ID
    pub from: StreamProtocol,
    
    /// New protocol ID
    pub to: StreamProtocol,
    
    /// Migration deadline
    pub deadline: SystemTime,
    
    /// Compatibility mode
    pub compatibility_mode: bool,
}
```