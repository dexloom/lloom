# Network Protocol

The network module implements the P2P networking layer for Lloom using libp2p. It provides service discovery, message routing, and peer management functionality.

## Overview

The Lloom network uses a combination of libp2p protocols:
- **Kademlia DHT**: Distributed hash table for service discovery
- **Gossipsub**: Pub/sub protocol for network-wide announcements
- **Request-Response**: Direct peer-to-peer messaging for LLM requests
- **mDNS**: Local network peer discovery
- **AutoNAT**: Automatic NAT traversal

## Core Types

### `LloomBehaviour`

The main network behavior combining all protocols:

```rust
pub struct LloomBehaviour {
    /// Kademlia DHT for peer and service discovery
    pub kad: Kademlia<MemoryStore>,
    
    /// Gossipsub for broadcast messaging
    pub gossipsub: Gossipsub,
    
    /// Request-response protocol for LLM interactions
    pub request_response: RequestResponse<LloomCodec>,
    
    /// mDNS for local peer discovery
    pub mdns: Mdns,
    
    /// Internal state and peer management
    peers: HashMap<PeerId, PeerInfo>,
    pending_requests: HashMap<RequestId, PendingRequest>,
}
```

### `LloomEvent`

Events emitted by the network behavior:

```rust
pub enum LloomEvent {
    // Gossipsub events
    MessageReceived {
        peer_id: PeerId,
        message: GossipMessage,
    },
    
    // Kademlia events
    RoutingUpdated {
        peer: PeerId,
        addresses: Vec<Multiaddr>,
    },
    ServiceDiscovered {
        service: ServiceType,
        providers: Vec<PeerId>,
    },
    
    // Request-response events
    RequestReceived {
        peer: PeerId,
        request: RequestMessage,
        channel: ResponseChannel<ResponseMessage>,
    },
    ResponseReceived {
        peer: PeerId,
        request_id: RequestId,
        response: ResponseMessage,
    },
    RequestFailed {
        peer: PeerId,
        request_id: RequestId,
        error: RequestError,
    },
    
    // mDNS events
    PeerDiscovered(PeerId),
    PeerExpired(PeerId),
    
    // Connection events
    ConnectionEstablished {
        peer_id: PeerId,
        endpoint: ConnectedPoint,
    },
    ConnectionClosed {
        peer_id: PeerId,
        cause: Option<ConnectionError>,
    },
}
```

## Creating Network Behavior

### Basic Setup

Create a network behavior with default configuration:

```rust
use lloom_core::{Identity, LloomBehaviour};

let identity = Identity::generate();
let behaviour = LloomBehaviour::new(&identity)?;
```

### Custom Configuration

Create with custom network parameters:

```rust
use lloom_core::network::{NetworkConfig, LloomBehaviour};

let config = NetworkConfig {
    // Kademlia configuration
    kad_protocol: "/lloom/kad/1.0.0".to_string(),
    kad_replication_factor: 20,
    kad_query_timeout: Duration::from_secs(60),
    
    // Gossipsub configuration
    gossipsub_config: GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(ValidationMode::Strict)
        .build()?,
    
    // Request-response configuration  
    request_timeout: Duration::from_secs(120),
    max_request_size: 10 * 1024 * 1024, // 10MB
    
    // Connection limits
    max_connections: 200,
    max_connections_per_peer: 3,
    
    // Enable optional protocols
    enable_mdns: true,
    enable_autonat: true,
    enable_relay: true,
};

let behaviour = LloomBehaviour::with_config(&identity, config)?;
```

## Service Discovery

### Advertising Services

Register as a service provider:

```rust
use lloom_core::network::ServiceType;

// Advertise as an executor
behaviour.advertise_service(ServiceType::Executor {
    models: vec!["gpt-3.5-turbo", "llama-2-13b"],
    capacity: 10,
})?;

// Advertise as a validator
behaviour.advertise_service(ServiceType::Validator {
    stake: "1000000000000000000", // 1 ETH
})?;

// Update service info
behaviour.update_service_info(ServiceType::Executor {
    models: vec!["gpt-4"], // Updated models
    capacity: 5,
})?;
```

### Discovering Services

Find service providers:

```rust
// Discover executors
let executors = behaviour.discover_executors().await?;
for (peer_id, info) in executors {
    println!("Executor {} offers models: {:?}", peer_id, info.models);
}

// Discover specific model
let providers = behaviour.find_model_providers("gpt-4").await?;

// Discover validators
let validators = behaviour.discover_validators().await?;
```

### Continuous Discovery

Set up automatic service discovery:

```rust
use tokio::time::interval;

// Start periodic discovery
let mut discovery_interval = interval(Duration::from_secs(300));

loop {
    tokio::select! {
        _ = discovery_interval.tick() => {
            behaviour.refresh_services();
        }
        event = swarm.next() => {
            match event {
                Some(SwarmEvent::Behaviour(LloomEvent::ServiceDiscovered { service, providers })) => {
                    println!("Found {} providers for {:?}", providers.len(), service);
                }
                _ => {}
            }
        }
    }
}
```

## Peer Management

### Bootstrap Peers

Connect to bootstrap peers:

```rust
// Add bootstrap peers
let bootstrap_peers = [
    "/ip4/bootstrap1.lloom.network/tcp/4001/p2p/12D3KooWH3uVF6wv47WnArKHk5ZDVmGb6CVbEDqVyemfPnCaQnQM",
    "/ip4/bootstrap2.lloom.network/tcp/4001/p2p/12D3KooWLRPJgqrZxc7CZ7ojbVXKmQ3pYAe2dCTmJbHN6F5RdVkE",
];

for addr in &bootstrap_peers {
    behaviour.add_bootstrap_peer(addr)?;
}

// Manually bootstrap
behaviour.bootstrap()?;
```

### Peer Information

Track and query peer information:

```rust
// Get peer info
if let Some(info) = behaviour.peer_info(&peer_id) {
    println!("Peer {} info:", peer_id);
    println!("  Addresses: {:?}", info.addresses);
    println!("  Protocols: {:?}", info.protocols);
    println!("  Agent: {}", info.agent_version);
    println!("  Services: {:?}", info.services);
}

// Get all connected peers
let peers = behaviour.connected_peers();
println!("Connected to {} peers", peers.len());

// Check if peer is connected
if behaviour.is_connected(&peer_id) {
    println!("Peer {} is connected", peer_id);
}
```

### Connection Management

Control peer connections:

```rust
// Dial a specific peer
behaviour.dial_peer(&peer_id, vec![multiaddr])?;

// Disconnect from peer
behaviour.disconnect_peer(&peer_id);

// Ban a peer
behaviour.ban_peer(&peer_id, Duration::from_secs(3600));

// Set connection limits
behaviour.set_connection_limits(ConnectionLimits {
    max_connections: 100,
    max_connections_per_peer: 2,
    max_pending_connections: 30,
});
```

## Message Protocol

### Request-Response Pattern

Send LLM requests and receive responses:

```rust
use lloom_core::protocol::{RequestMessage, ResponseMessage, SignedLlmRequest};

// Send a request
let signed_request = llm_request.sign(&identity.wallet).await?;
let request_id = behaviour.send_request(
    &executor_peer_id,
    RequestMessage::LlmRequest(signed_request)
)?;

// Track pending request
behaviour.track_request(request_id, RequestMetadata {
    peer: executor_peer_id,
    timeout: Instant::now() + Duration::from_secs(120),
    retry_count: 0,
});

// Handle response in event loop
match event {
    LloomEvent::ResponseReceived { peer, request_id, response } => {
        match response {
            ResponseMessage::LlmResponse(signed_response) => {
                // Verify and process response
                if verify_signed_message(&signed_response)? {
                    println!("Received valid response: {}", signed_response.payload.content);
                }
            }
            ResponseMessage::Error(error) => {
                eprintln!("Request failed: {}", error);
            }
        }
    }
    _ => {}
}
```

### Handling Incoming Requests

Process requests as an executor:

```rust
match event {
    LloomEvent::RequestReceived { peer, request, channel } => {
        match request {
            RequestMessage::LlmRequest(signed_request) => {
                // Verify signature
                if !verify_signed_message(&signed_request)? {
                    behaviour.send_error_response(channel, "Invalid signature");
                    continue;
                }
                
                // Process request asynchronously
                tokio::spawn(async move {
                    let response = process_llm_request(signed_request).await;
                    behaviour.send_response(channel, ResponseMessage::LlmResponse(response));
                });
            }
            RequestMessage::Ping => {
                behaviour.send_response(channel, ResponseMessage::Pong);
            }
        }
    }
    _ => {}
}
```

### Request Timeouts and Retries

Implement retry logic:

```rust
use lloom_core::network::RetryStrategy;

// Configure retry strategy
let retry_strategy = RetryStrategy {
    max_attempts: 3,
    initial_delay: Duration::from_secs(1),
    max_delay: Duration::from_secs(30),
    exponential_base: 2.0,
};

// Send request with retry
let request_id = behaviour.send_request_with_retry(
    &peer_id,
    request,
    retry_strategy
)?;

// Handle timeout events
match event {
    LloomEvent::RequestFailed { peer, request_id, error } => {
        match error {
            RequestError::Timeout => {
                println!("Request {} timed out, retrying...", request_id);
                behaviour.retry_request(request_id)?;
            }
            RequestError::ConnectionClosed => {
                println!("Connection lost to {}", peer);
                // Try alternative peer
                if let Some(alt_peer) = behaviour.find_alternative_peer(&peer) {
                    behaviour.resend_request(request_id, &alt_peer)?;
                }
            }
            _ => {}
        }
    }
    _ => {}
}
```

## Gossipsub Messaging

### Publishing Messages

Broadcast messages to the network:

```rust
use lloom_core::network::{GossipMessage, GossipTopic};

// Subscribe to topics
behaviour.subscribe_topic(GossipTopic::ExecutorAnnouncements)?;
behaviour.subscribe_topic(GossipTopic::NetworkStatus)?;

// Publish executor availability
let announcement = GossipMessage::ExecutorAvailable {
    peer_id: identity.peer_id,
    models: vec!["gpt-3.5-turbo", "llama-2-13b"],
    capacity: 10,
    pricing: PricingInfo {
        inbound_price: "500000000000000",   // Wei per token
        outbound_price: "1000000000000000",
    },
};

behaviour.publish_message(GossipTopic::ExecutorAnnouncements, announcement)?;
```

### Receiving Messages

Handle gossipsub messages:

```rust
match event {
    LloomEvent::MessageReceived { peer_id, message } => {
        match message {
            GossipMessage::ExecutorAvailable { peer_id, models, .. } => {
                println!("Executor {} is offering models: {:?}", peer_id, models);
                behaviour.update_executor_info(peer_id, models);
            }
            GossipMessage::NetworkMetrics { total_peers, active_requests, .. } => {
                println!("Network stats: {} peers, {} active requests", total_peers, active_requests);
            }
            GossipMessage::PriceUpdate { model, new_price } => {
                println!("Price update for {}: {}", model, new_price);
            }
        }
    }
    _ => {}
}
```

### Topic Management

Manage gossipsub topics dynamically:

```rust
// Create custom topic
let custom_topic = behaviour.create_topic("lloom/custom/1.0.0")?;

// Subscribe/unsubscribe
behaviour.subscribe_topic(custom_topic.clone())?;
behaviour.unsubscribe_topic(custom_topic)?;

// Get subscribed topics
let topics = behaviour.subscribed_topics();

// Get topic peers
let peers = behaviour.topic_peers(&GossipTopic::ExecutorAnnouncements);
```

## NAT Traversal

### AutoNAT

Enable automatic NAT detection:

```rust
use lloom_core::network::AutoNatConfig;

let autonat_config = AutoNatConfig {
    enable_server: true,
    confidence_threshold: 3,
    throttle_interval: Duration::from_secs(10),
    only_global_ips: true,
};

behaviour.enable_autonat(autonat_config)?;

// Check NAT status
let nat_status = behaviour.nat_status();
match nat_status {
    NatStatus::Public(addr) => println!("Public address: {}", addr),
    NatStatus::Private => println!("Behind NAT"),
    NatStatus::Unknown => println!("NAT status unknown"),
}
```

### Relay Protocol

Use relay for NAT traversal:

```rust
// Enable relay client
behaviour.enable_relay_client()?;

// Connect through relay
let relay_addr = "/ip4/relay.lloom.network/tcp/4001/p2p/12D3KooW.../p2p-circuit/p2p/12D3KooW...";
behaviour.dial_peer_through_relay(&peer_id, relay_addr)?;

// Become a relay (if public IP)
if behaviour.nat_status().is_public() {
    behaviour.enable_relay_server()?;
}
```

## Metrics and Monitoring

### Network Metrics

Track network performance:

```rust
use lloom_core::network::NetworkMetrics;

let metrics = behaviour.metrics();
println!("Network metrics:");
println!("  Connected peers: {}", metrics.connected_peers);
println!("  Total messages sent: {}", metrics.messages_sent);
println!("  Total messages received: {}", metrics.messages_received);
println!("  Active requests: {}", metrics.active_requests);
println!("  Bandwidth: {} KB/s in, {} KB/s out", 
    metrics.bandwidth_in_kbps, 
    metrics.bandwidth_out_kbps
);

// Get per-protocol metrics
let kad_metrics = behaviour.kad_metrics();
println!("Kademlia: {} records, {} peers in routing table", 
    kad_metrics.stored_records, 
    kad_metrics.routing_table_size
);
```

### Event Monitoring

Monitor network events:

```rust
use lloom_core::network::EventMonitor;

let monitor = EventMonitor::new();

// Count events
monitor.on_event(&event);

// Get event statistics
let stats = monitor.statistics();
println!("Event statistics:");
for (event_type, count) in stats {
    println!("  {}: {}", event_type, count);
}

// Export metrics
monitor.export_prometheus_metrics();
```

## Advanced Features

### Custom Protocols

Add custom protocol handlers:

```rust
use lloom_core::network::{CustomProtocol, ProtocolHandler};

#[derive(Clone)]
struct MyProtocol;

impl ProtocolHandler for MyProtocol {
    type Request = MyRequest;
    type Response = MyResponse;
    
    fn protocol_name(&self) -> &'static str {
        "/lloom/custom/1.0.0"
    }
    
    async fn handle_request(
        &mut self,
        peer: PeerId,
        request: Self::Request,
    ) -> Result<Self::Response> {
        // Handle custom request
        Ok(MyResponse { data: "response".to_string() })
    }
}

// Register custom protocol
behaviour.add_protocol(MyProtocol)?;
```

### Network Simulation

Test network behavior:

```rust
#[cfg(test)]
mod tests {
    use lloom_core::network::test_utils::{NetworkSimulator, SimulationConfig};
    
    #[tokio::test]
    async fn test_network_discovery() {
        let config = SimulationConfig {
            num_nodes: 10,
            network_delay: Duration::from_millis(50),
            packet_loss: 0.01,
        };
        
        let mut sim = NetworkSimulator::new(config);
        sim.spawn_executor_nodes(5);
        sim.spawn_client_nodes(5);
        
        // Run simulation
        sim.run_for(Duration::from_secs(30)).await;
        
        // Check all nodes discovered each other
        assert!(sim.all_nodes_connected());
    }
}
```

### Connection Pooling

Optimize connection management:

```rust
use lloom_core::network::ConnectionPool;

let pool_config = ConnectionPoolConfig {
    min_connections: 10,
    max_connections: 100,
    idle_timeout: Duration::from_secs(300),
    connection_ttl: Duration::from_secs(3600),
};

behaviour.set_connection_pool(pool_config)?;

// Pre-connect to important peers
let important_peers = vec![executor1, executor2, validator1];
for peer in important_peers {
    behaviour.maintain_connection(&peer)?;
}
```

## Troubleshooting

### Common Issues

1. **Connection Refused**
   ```rust
   // Enable detailed logging
   behaviour.enable_debug_logging();
   
   // Check listen addresses
   let addrs = behaviour.listened_addresses();
   println!("Listening on: {:?}", addrs);
   ```

2. **Peer Discovery Failures**
   ```rust
   // Manually add peers
   behaviour.add_peer_addr(&peer_id, multiaddr)?;
   
   // Force DHT refresh
   behaviour.kad_bootstrap()?;
   ```

3. **Message Delivery Issues**
   ```rust
   // Check gossipsub mesh
   let mesh_peers = behaviour.gossipsub_mesh_peers(&topic);
   println!("Mesh peers for {:?}: {}", topic, mesh_peers.len());
   
   // Increase gossipsub D parameter
   behaviour.set_gossipsub_mesh_size(8, 12)?;
   ```

### Debug Utilities

Enable comprehensive debugging:

```rust
// Enable all debug features
behaviour.enable_debug_mode(DebugConfig {
    log_all_events: true,
    trace_messages: true,
    dump_routing_table: true,
    export_metrics: true,
});

// Export network state
let state = behaviour.export_state();
std::fs::write("network_state.json", serde_json::to_string_pretty(&state)?)?;
```