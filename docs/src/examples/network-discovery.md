# Network Discovery

This guide demonstrates how to implement network discovery in the Lloom P2P network, including peer discovery, service advertisement, and dynamic routing.

## Basic Peer Discovery

### Discovering Network Peers

```rust
use lloom_core::{Identity, LloomBehaviour, LloomEvent};
use libp2p::{Swarm, SwarmBuilder, SwarmEvent};
use std::time::Duration;

async fn discover_peers() -> Result<(), Box<dyn std::error::Error>> {
    // Create identity and swarm
    let identity = Identity::generate();
    let behaviour = LloomBehaviour::new(&identity)?;
    let mut swarm = SwarmBuilder::with_tokio_executor(
        libp2p::tcp::tokio::Transport::default(),
        behaviour,
        identity.peer_id,
    ).build();
    
    // Listen on random port
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    
    // Connect to bootstrap nodes
    let bootstrap_peers = vec![
        "/ip4/bootstrap1.lloom.network/tcp/4001/p2p/12D3KooW...",
        "/ip4/bootstrap2.lloom.network/tcp/4001/p2p/12D3KooW...",
    ];
    
    for peer_addr in bootstrap_peers {
        swarm.dial(peer_addr.parse::<libp2p::Multiaddr>()?)?;
        println!("Dialing bootstrap peer: {}", peer_addr);
    }
    
    // Start discovery
    swarm.behaviour_mut().kad.bootstrap()?;
    
    // Handle discovery events
    loop {
        match swarm.next().await {
            Some(SwarmEvent::Behaviour(LloomEvent::PeerDiscovered(peer_id))) => {
                println!("Discovered peer: {}", peer_id);
            }
            Some(SwarmEvent::ConnectionEstablished { peer_id, .. }) => {
                println!("Connected to peer: {}", peer_id);
            }
            Some(SwarmEvent::Behaviour(LloomEvent::RoutingUpdated)) => {
                let routing_table_size = swarm.behaviour().kad.kbuckets()
                    .map(|bucket| bucket.num_entries())
                    .sum::<usize>();
                println!("Routing table updated, {} peers known", routing_table_size);
            }
            _ => {}
        }
    }
}
```

### Local Network Discovery with mDNS

```rust
use lloom_core::{Identity, LloomBehaviour};
use std::collections::HashSet;

async fn mdns_discovery() -> Result<(), Box<dyn std::error::Error>> {
    let identity = Identity::generate();
    let behaviour = LloomBehaviour::with_mdns(&identity)?;
    let mut swarm = create_swarm(behaviour, &identity).await?;
    
    let mut discovered_peers = HashSet::new();
    
    loop {
        match swarm.next().await {
            Some(SwarmEvent::Behaviour(LloomEvent::Mdns(mdns::Event::Discovered(peers)))) => {
                for (peer_id, addr) in peers {
                    if discovered_peers.insert(peer_id) {
                        println!("Found local peer: {} at {}", peer_id, addr);
                        
                        // Connect to discovered peer
                        if swarm.dial(addr.clone()).is_ok() {
                            println!("Dialing local peer at {}", addr);
                        }
                    }
                }
            }
            Some(SwarmEvent::Behaviour(LloomEvent::Mdns(mdns::Event::Expired(peers)))) => {
                for (peer_id, _) in peers {
                    discovered_peers.remove(&peer_id);
                    println!("Local peer expired: {}", peer_id);
                }
            }
            _ => {}
        }
    }
}
```

## Service Discovery

### Discovering Executors

```rust
use lloom_core::{ServiceType, ServiceInfo, DiscoveryRequest};
use std::collections::HashMap;

struct ExecutorDiscovery {
    swarm: Swarm<LloomBehaviour>,
    executors: HashMap<PeerId, ExecutorInfo>,
}

#[derive(Debug, Clone)]
struct ExecutorInfo {
    peer_id: PeerId,
    models: Vec<String>,
    capacity: u32,
    pricing: PricingInfo,
    last_seen: Instant,
}

impl ExecutorDiscovery {
    async fn discover_executors(&mut self) -> Result<Vec<ExecutorInfo>, Box<dyn Error>> {
        // Query DHT for executor service
        let key = ServiceType::Executor.to_dht_key();
        self.swarm.behaviour_mut().kad.get_providers(key);
        
        let mut executors = Vec::new();
        let timeout = tokio::time::sleep(Duration::from_secs(10));
        tokio::pin!(timeout);
        
        loop {
            tokio::select! {
                event = self.swarm.next() => {
                    match event {
                        Some(SwarmEvent::Behaviour(LloomEvent::ServiceDiscovered { 
                            service: ServiceType::Executor, 
                            providers 
                        })) => {
                            for provider in providers {
                                if let Some(info) = self.query_executor_info(provider).await? {
                                    executors.push(info);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ = &mut timeout => {
                    break;
                }
            }
        }
        
        Ok(executors)
    }
    
    async fn query_executor_info(&mut self, peer_id: PeerId) -> Result<Option<ExecutorInfo>> {
        // Send direct query to executor
        let request = RequestMessage::GetInfo;
        let request_id = self.swarm.behaviour_mut().send_request(&peer_id, request);
        
        // Wait for response
        let timeout = tokio::time::sleep(Duration::from_secs(5));
        tokio::pin!(timeout);
        
        loop {
            tokio::select! {
                event = self.swarm.next() => {
                    match event {
                        Some(SwarmEvent::Behaviour(LloomEvent::ResponseReceived { 
                            peer, 
                            response: ResponseMessage::ExecutorInfo(info),
                            .. 
                        })) if peer == peer_id => {
                            return Ok(Some(ExecutorInfo {
                                peer_id,
                                models: info.models,
                                capacity: info.capacity,
                                pricing: info.pricing,
                                last_seen: Instant::now(),
                            }));
                        }
                        _ => {}
                    }
                }
                _ = &mut timeout => {
                    return Ok(None);
                }
            }
        }
    }
}
```

### Model-Specific Discovery

Find executors offering specific models:

```rust
async fn find_model_providers(
    swarm: &mut Swarm<LloomBehaviour>,
    model: &str,
) -> Result<Vec<PeerId>, Box<dyn Error>> {
    // Create model-specific key
    let model_key = format!("lloom:model:{}", model);
    let key = Key::from(model_key.as_bytes());
    
    // Query DHT
    swarm.behaviour_mut().kad.get_providers(key.clone());
    
    let mut providers = Vec::new();
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(timeout);
    
    loop {
        tokio::select! {
            event = swarm.next() => {
                match event {
                    Some(SwarmEvent::Behaviour(LloomEvent::ProvidersFound { key: k, providers: p })) 
                        if k == key => {
                        providers.extend(p);
                    }
                    _ => {}
                }
            }
            _ = &mut timeout => {
                break;
            }
        }
    }
    
    Ok(providers)
}

// Usage example
async fn find_gpt4_executors() -> Result<(), Box<dyn Error>> {
    let mut discovery = ExecutorDiscovery::new().await?;
    
    let providers = find_model_providers(&mut discovery.swarm, "gpt-4").await?;
    println!("Found {} executors offering GPT-4", providers.len());
    
    for peer_id in providers {
        if let Some(info) = discovery.query_executor_info(peer_id).await? {
            println!("Executor {}: capacity={}, price={} ETH/token", 
                peer_id, 
                info.capacity,
                info.pricing.base_price
            );
        }
    }
    
    Ok(())
}
```

## Service Advertisement

### Advertising as an Executor

```rust
use lloom_core::{ServiceType, ModelInfo};

struct ExecutorNode {
    swarm: Swarm<LloomBehaviour>,
    models: Vec<ModelInfo>,
    capacity: Arc<AtomicU32>,
}

impl ExecutorNode {
    async fn advertise_service(&mut self) -> Result<(), Box<dyn Error>> {
        // Register as executor in DHT
        let service_key = ServiceType::Executor.to_dht_key();
        self.swarm.behaviour_mut().kad.provide(service_key)?;
        
        // Register each model
        for model in &self.models {
            let model_key = Key::from(format!("lloom:model:{}", model.id).as_bytes());
            self.swarm.behaviour_mut().kad.provide(model_key)?;
        }
        
        // Announce via gossipsub
        let announcement = GossipMessage::ExecutorAvailable {
            peer_id: self.swarm.local_peer_id().clone(),
            models: self.models.iter().map(|m| m.id.clone()).collect(),
            capacity: self.capacity.load(Ordering::Relaxed),
            pricing: self.get_current_pricing(),
        };
        
        self.swarm.behaviour_mut().gossipsub.publish(
            GossipTopic::ExecutorAnnouncements.to_topic(),
            serde_json::to_vec(&announcement)?
        )?;
        
        // Re-announce periodically
        self.schedule_reannouncement();
        
        Ok(())
    }
    
    fn schedule_reannouncement(&self) {
        let swarm = self.swarm.clone();
        let models = self.models.clone();
        let capacity = self.capacity.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            
            loop {
                interval.tick().await;
                
                // Re-provide records
                for model in &models {
                    let model_key = Key::from(format!("lloom:model:{}", model.id).as_bytes());
                    swarm.behaviour_mut().kad.provide(model_key).ok();
                }
                
                // Re-announce if capacity changed
                // ... announcement logic
            }
        });
    }
}
```

### Dynamic Service Updates

Update service information dynamically:

```rust
impl ExecutorNode {
    async fn update_capacity(&mut self, new_capacity: u32) -> Result<(), Box<dyn Error>> {
        let old_capacity = self.capacity.swap(new_capacity, Ordering::Relaxed);
        
        if new_capacity != old_capacity {
            // Announce capacity change
            let update = GossipMessage::CapacityUpdate {
                peer_id: self.swarm.local_peer_id().clone(),
                old_capacity,
                new_capacity,
                timestamp: Utc::now().timestamp() as u64,
            };
            
            self.swarm.behaviour_mut().gossipsub.publish(
                GossipTopic::ExecutorAnnouncements.to_topic(),
                serde_json::to_vec(&update)?
            )?;
            
            // Update DHT record
            self.update_dht_record().await?;
        }
        
        Ok(())
    }
    
    async fn add_model(&mut self, model: ModelInfo) -> Result<(), Box<dyn Error>> {
        // Add to local list
        self.models.push(model.clone());
        
        // Register in DHT
        let model_key = Key::from(format!("lloom:model:{}", model.id).as_bytes());
        self.swarm.behaviour_mut().kad.provide(model_key)?;
        
        // Announce new model
        let announcement = GossipMessage::ModelAdded {
            peer_id: self.swarm.local_peer_id().clone(),
            model: model.id,
            context_length: model.context_length,
            capabilities: model.capabilities,
        };
        
        self.swarm.behaviour_mut().gossipsub.publish(
            GossipTopic::ExecutorAnnouncements.to_topic(),
            serde_json::to_vec(&announcement)?
        )?;
        
        Ok(())
    }
}
```

## Advanced Discovery Patterns

### Proximity-Based Discovery

Find geographically close peers:

```rust
use maxminddb::{geoip2, Reader};

struct ProximityDiscovery {
    swarm: Swarm<LloomBehaviour>,
    geoip: Reader<Vec<u8>>,
    location_cache: HashMap<PeerId, Location>,
}

#[derive(Debug, Clone)]
struct Location {
    latitude: f64,
    longitude: f64,
    country: String,
    city: String,
}

impl ProximityDiscovery {
    async fn find_nearby_executors(
        &mut self, 
        max_distance_km: f64
    ) -> Result<Vec<ExecutorInfo>, Box<dyn Error>> {
        // Get own location
        let my_location = self.get_own_location()?;
        
        // Discover all executors
        let all_executors = self.discover_all_executors().await?;
        
        // Filter by distance
        let mut nearby = Vec::new();
        
        for executor in all_executors {
            if let Some(location) = self.get_peer_location(&executor.peer_id).await? {
                let distance = calculate_distance(&my_location, &location);
                
                if distance <= max_distance_km {
                    nearby.push((executor, distance));
                }
            }
        }
        
        // Sort by distance
        nearby.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        
        Ok(nearby.into_iter().map(|(exec, _)| exec).collect())
    }
    
    async fn get_peer_location(&mut self, peer_id: &PeerId) -> Result<Option<Location>> {
        // Check cache
        if let Some(location) = self.location_cache.get(peer_id) {
            return Ok(Some(location.clone()));
        }
        
        // Query peer for location
        let request = RequestMessage::GetLocation;
        let response = self.send_request_with_timeout(peer_id, request, Duration::from_secs(5)).await?;
        
        if let ResponseMessage::Location(location) = response {
            self.location_cache.insert(*peer_id, location.clone());
            Ok(Some(location))
        } else {
            Ok(None)
        }
    }
}

fn calculate_distance(loc1: &Location, loc2: &Location) -> f64 {
    // Haversine formula
    let r = 6371.0; // Earth radius in km
    let dlat = (loc2.latitude - loc1.latitude).to_radians();
    let dlon = (loc2.longitude - loc1.longitude).to_radians();
    
    let a = (dlat / 2.0).sin().powi(2) +
            loc1.latitude.to_radians().cos() *
            loc2.latitude.to_radians().cos() *
            (dlon / 2.0).sin().powi(2);
    
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    
    r * c
}
```

### Capability-Based Discovery

Find peers with specific capabilities:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Capability {
    name: String,
    version: String,
    properties: HashMap<String, serde_json::Value>,
}

struct CapabilityDiscovery {
    swarm: Swarm<LloomBehaviour>,
    capability_index: HashMap<String, HashSet<PeerId>>,
}

impl CapabilityDiscovery {
    async fn find_by_capability(
        &mut self,
        required_capabilities: Vec<Capability>
    ) -> Result<Vec<PeerId>, Box<dyn Error>> {
        let mut matching_peers = None;
        
        for capability in required_capabilities {
            let capability_key = format!("lloom:capability:{}:{}", 
                capability.name, 
                capability.version
            );
            
            // Query DHT
            let providers = self.query_providers(&capability_key).await?;
            
            // Filter by properties if specified
            let filtered = if !capability.properties.is_empty() {
                self.filter_by_properties(providers, &capability.properties).await?
            } else {
                providers
            };
            
            // Intersection with previous results
            matching_peers = Some(match matching_peers {
                None => filtered.into_iter().collect(),
                Some(peers) => peers.intersection(&filtered.into_iter().collect()).cloned().collect(),
            });
        }
        
        Ok(matching_peers.unwrap_or_default().into_iter().collect())
    }
    
    async fn filter_by_properties(
        &mut self,
        peers: Vec<PeerId>,
        required_properties: &HashMap<String, serde_json::Value>
    ) -> Result<Vec<PeerId>, Box<dyn Error>> {
        let mut matching = Vec::new();
        
        for peer in peers {
            let request = RequestMessage::GetCapabilities;
            if let Ok(ResponseMessage::Capabilities(caps)) = 
                self.send_request_with_timeout(&peer, request, Duration::from_secs(3)).await {
                
                let matches = caps.iter().any(|cap| {
                    required_properties.iter().all(|(key, value)| {
                        cap.properties.get(key) == Some(value)
                    })
                });
                
                if matches {
                    matching.push(peer);
                }
            }
        }
        
        Ok(matching)
    }
}
```

### Reputation-Based Discovery

Discover peers based on reputation:

```rust
use std::collections::BTreeMap;

struct ReputationDiscovery {
    swarm: Swarm<LloomBehaviour>,
    reputation_scores: HashMap<PeerId, ReputationScore>,
}

#[derive(Debug, Clone)]
struct ReputationScore {
    total_requests: u64,
    successful_requests: u64,
    average_response_time: Duration,
    uptime_percentage: f64,
    last_updated: Instant,
}

impl ReputationDiscovery {
    async fn find_reputable_executors(
        &mut self,
        min_reputation: f64,
        min_requests: u64
    ) -> Result<Vec<(PeerId, ReputationScore)>, Box<dyn Error>> {
        // Update reputation scores
        self.update_reputation_scores().await?;
        
        // Filter and sort by reputation
        let mut reputable: Vec<_> = self.reputation_scores.iter()
            .filter(|(_, score)| {
                let success_rate = score.successful_requests as f64 / score.total_requests as f64;
                success_rate >= min_reputation && score.total_requests >= min_requests
            })
            .map(|(peer, score)| (*peer, score.clone()))
            .collect();
        
        // Sort by success rate descending
        reputable.sort_by(|a, b| {
            let rate_a = a.1.successful_requests as f64 / a.1.total_requests as f64;
            let rate_b = b.1.successful_requests as f64 / b.1.total_requests as f64;
            rate_b.partial_cmp(&rate_a).unwrap()
        });
        
        Ok(reputable)
    }
    
    async fn update_reputation_scores(&mut self) -> Result<(), Box<dyn Error>> {
        // Subscribe to validator reports
        self.swarm.behaviour_mut().gossipsub.subscribe(
            &GossipTopic::ValidatorReports.to_topic()
        )?;
        
        // Collect reports for a period
        let timeout = tokio::time::sleep(Duration::from_secs(10));
        tokio::pin!(timeout);
        
        loop {
            tokio::select! {
                event = self.swarm.next() => {
                    match event {
                        Some(SwarmEvent::Behaviour(LloomEvent::MessageReceived { 
                            message: GossipMessage::ValidatorReport(report), 
                            .. 
                        })) => {
                            self.process_validator_report(report);
                        }
                        _ => {}
                    }
                }
                _ = &mut timeout => {
                    break;
                }
            }
        }
        
        Ok(())
    }
}
```

## Discovery Optimization

### Caching Discovery Results

```rust
use lru::LruCache;

struct CachedDiscovery {
    swarm: Swarm<LloomBehaviour>,
    executor_cache: Arc<Mutex<LruCache<String, Vec<ExecutorInfo>>>>,
    cache_ttl: Duration,
}

impl CachedDiscovery {
    async fn find_executors_cached(
        &mut self,
        model: &str
    ) -> Result<Vec<ExecutorInfo>, Box<dyn Error>> {
        let cache_key = format!("executors:{}", model);
        
        // Check cache
        {
            let mut cache = self.executor_cache.lock().unwrap();
            if let Some(cached) = cache.get(&cache_key) {
                if cached.iter().all(|e| e.last_seen.elapsed() < self.cache_ttl) {
                    return Ok(cached.clone());
                }
            }
        }
        
        // Cache miss - perform discovery
        let executors = self.discover_model_executors(model).await?;
        
        // Update cache
        {
            let mut cache = self.executor_cache.lock().unwrap();
            cache.put(cache_key, executors.clone());
        }
        
        Ok(executors)
    }
}
```

### Parallel Discovery

```rust
async fn parallel_discovery(
    models: Vec<String>
) -> Result<HashMap<String, Vec<ExecutorInfo>>, Box<dyn Error>> {
    let mut discovery_tasks = Vec::new();
    
    for model in models {
        let task = tokio::spawn(async move {
            let mut discovery = ExecutorDiscovery::new().await?;
            let executors = discovery.find_model_executors(&model).await?;
            Ok::<_, Box<dyn Error>>((model, executors))
        });
        discovery_tasks.push(task);
    }
    
    let results = futures::future::try_join_all(discovery_tasks).await?;
    
    let mut executor_map = HashMap::new();
    for result in results {
        let (model, executors) = result?;
        executor_map.insert(model, executors);
    }
    
    Ok(executor_map)
}
```

## Complete Discovery Example

```rust
use lloom_core::*;
use tokio::time::interval;

struct DiscoveryService {
    swarm: Swarm<LloomBehaviour>,
    discovered_services: HashMap<ServiceType, Vec<ServiceInfo>>,
    discovery_interval: Duration,
}

impl DiscoveryService {
    async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Initial bootstrap
        self.bootstrap().await?;
        
        // Start periodic discovery
        let mut discovery_timer = interval(self.discovery_interval);
        
        loop {
            tokio::select! {
                _ = discovery_timer.tick() => {
                    self.perform_discovery().await?;
                }
                
                event = self.swarm.next() => {
                    self.handle_event(event).await?;
                }
            }
        }
    }
    
    async fn perform_discovery(&mut self) -> Result<(), Box<dyn Error>> {
        // Discover all service types
        for service_type in &[ServiceType::Executor, ServiceType::Validator] {
            let providers = self.discover_service(*service_type).await?;
            
            println!("Found {} {} nodes", providers.len(), service_type);
            
            self.discovered_services.insert(*service_type, providers);
        }
        
        // Update metrics
        self.update_discovery_metrics();
        
        Ok(())
    }
    
    async fn handle_event(&mut self, event: SwarmEvent<LloomEvent>) -> Result<(), Box<dyn Error>> {
        match event {
            SwarmEvent::Behaviour(LloomEvent::MessageReceived { message, .. }) => {
                match message {
                    GossipMessage::ExecutorAvailable { peer_id, models, .. } => {
                        println!("Executor {} announced models: {:?}", peer_id, models);
                        self.update_executor_info(peer_id, models);
                    }
                    GossipMessage::ServiceOffline { peer_id, service_type } => {
                        println!("Service {} went offline: {}", service_type, peer_id);
                        self.remove_service(peer_id, service_type);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let identity = Identity::generate();
    let behaviour = LloomBehaviour::new(&identity)?;
    let swarm = create_swarm(behaviour, &identity).await?;
    
    let mut discovery = DiscoveryService {
        swarm,
        discovered_services: HashMap::new(),
        discovery_interval: Duration::from_secs(300), // 5 minutes
    };
    
    discovery.run().await
}
```