# Configuration Reference

This reference provides a comprehensive guide to configuring Lloom components using configuration files, command-line arguments, and programmatic configuration.

## Configuration File Format

Lloom uses TOML format for configuration files. Configuration can also be provided via JSON or YAML.

### Basic Structure

```toml
# config.toml
[general]
network_id = "mainnet"
log_level = "info"

[network]
listen_addr = "/ip4/0.0.0.0/tcp/4001"
bootstrap_peers = [
    "/ip4/bootstrap1.lloom.network/tcp/4001/p2p/12D3KooW...",
    "/ip4/bootstrap2.lloom.network/tcp/4001/p2p/12D3KooW..."
]

[identity]
path = "~/.lloom/identity"
generate_if_missing = true
```

## Component Configuration

### Client Configuration

#### Complete Client Config

```toml
# client.toml
[general]
network_id = "mainnet"
log_level = "info"
log_format = "json"

[network]
listen_addr = "/ip4/0.0.0.0/tcp/0"
bootstrap_peers = []
enable_mdns = true
enable_relay = false

[identity]
path = "~/.lloom/client/identity"
generate_if_missing = true

[request]
default_model = "gpt-3.5-turbo"
timeout_seconds = 300
max_retries = 3
retry_delay_ms = 1000

[budget]
enabled = true
max_cost_per_request_eth = 0.001
max_cost_per_hour_eth = 0.01
max_cost_per_day_eth = 0.1
action_on_limit = "reject" # reject, warn, allow

[cache]
enabled = true
max_size_mb = 100
ttl_seconds = 3600
path = "~/.lloom/client/cache"

[api]
enabled = false
listen_addr = "127.0.0.1:8080"
cors_origins = ["http://localhost:3000"]

[metrics]
enabled = true
port = 9090
path = "/metrics"

[blockchain]
enabled = false
rpc_url = "http://localhost:8545"
chain_id = 1
accounting_contract = "0x..."
```

#### Minimal Client Config

```toml
[general]
network_id = "mainnet"

[request]
default_model = "gpt-4"
```

### Executor Configuration

#### Complete Executor Config

```toml
# executor.toml
[general]
network_id = "mainnet"
log_level = "info"

[network]
listen_addr = "/ip4/0.0.0.0/tcp/4002"
advertise_addr = "/ip4/PUBLIC_IP/tcp/4002"
bootstrap_peers = []

[identity]
path = "~/.lloom/executor/identity"

[llm]
backend = "openai" # openai, lmstudio, custom
api_key_env = "OPENAI_API_KEY"
api_url = "https://api.openai.com/v1"
timeout_seconds = 600

# Backend-specific settings
[llm.openai]
organization = ""
max_retries = 3

[llm.lmstudio]
api_url = "http://localhost:1234/v1"
model_path = ""

[llm.custom]
command = "/path/to/custom/llm"
args = ["--model", "path/to/model"]

[models]
# List of supported models
[[models.supported]]
id = "gpt-3.5-turbo"
context_length = 4096
capabilities = ["chat", "completion"]

[[models.supported]]
id = "gpt-4"
context_length = 8192
capabilities = ["chat", "completion", "function_calling"]

[resources]
max_concurrent_requests = 10
max_queue_size = 100
request_timeout_seconds = 300
gpu_memory_limit_mb = 0 # 0 = unlimited

[pricing]
base_price_per_token_wei = "1000000000000"
price_multiplier = 1.0
dynamic_pricing = true

# Model-specific pricing
[pricing.models.gpt-3-5-turbo]
input_price_per_token_wei = "500000000000"
output_price_per_token_wei = "1500000000000"

[pricing.models.gpt-4]
input_price_per_token_wei = "30000000000000"
output_price_per_token_wei = "60000000000000"

[service]
announce_interval_seconds = 300
health_check_interval_seconds = 60
capacity_update_threshold = 0.1

[storage]
data_dir = "~/.lloom/executor/data"
max_cache_size_gb = 10

[security]
require_signed_requests = true
allowed_clients = [] # Empty = allow all
rate_limit_per_client = 100 # requests per hour

[monitoring]
metrics_enabled = true
metrics_port = 9091
trace_enabled = false
trace_endpoint = "http://localhost:14268/api/traces"
```

#### GPU-Specific Executor Config

```toml
[llm]
backend = "custom"

[llm.custom]
command = "llama.cpp"
args = ["-m", "/models/llama-2-13b.gguf", "-ngl", "35"]

[resources]
gpu_memory_limit_mb = 16384
cuda_device = 0

[models.supported]
[[models.supported]]
id = "llama-2-13b"
context_length = 4096
capabilities = ["completion"]
```

### Validator Configuration

#### Complete Validator Config

```toml
# validator.toml
[general]
network_id = "mainnet"
log_level = "info"

[network]
listen_addr = "/ip4/0.0.0.0/tcp/4003"
bootstrap_peers = []

[identity]
path = "~/.lloom/validator/identity"

[validation]
enabled = true
threshold = 0.8
workers = 4
timeout_seconds = 60

[validation.rules]
# Validation rules
check_token_counts = true
check_content_hash = true
check_model_match = true
check_timing = true
max_time_variance_percent = 20

[storage]
database_path = "~/.lloom/validator/validation.db"
archive_enabled = true
archive_after_days = 30
max_storage_gb = 100

[reputation]
enabled = true
update_interval_seconds = 3600
min_validations_for_reputation = 10

[reporting]
enabled = true
report_interval_seconds = 300
batch_size = 100

[consensus]
enabled = false
min_validators = 3
consensus_timeout_seconds = 30

[monitoring]
metrics_enabled = true
metrics_port = 9092
```

## Command-Line Arguments

### Client CLI Arguments

```bash
lloom-client [OPTIONS] [COMMAND]

OPTIONS:
    -c, --config <FILE>          Configuration file path
    -n, --network <ID>           Network ID (mainnet, testnet, devnet)
    -i, --identity <FILE>        Identity file path
    -b, --bootstrap <PEERS>      Comma-separated bootstrap peers
    -m, --model <MODEL>          Default model
    -v, --verbose               Increase verbosity
    -q, --quiet                 Decrease verbosity
    --json                      Output JSON format
    --no-cache                  Disable response cache
    --metrics-port <PORT>       Metrics server port

COMMANDS:
    complete <PROMPT>           Send completion request
    chat                       Interactive chat mode
    list-models                List available models
    stats                      Show statistics
```

### Executor CLI Arguments

```bash
lloom-executor [OPTIONS]

OPTIONS:
    -c, --config <FILE>          Configuration file path
    -n, --network <ID>           Network ID
    -i, --identity <FILE>        Identity file path
    -b, --backend <TYPE>         LLM backend (openai, lmstudio, custom)
    --api-key <KEY>             API key for backend
    --api-url <URL>             API endpoint URL
    --model-path <PATH>         Path to local model
    --max-requests <N>          Max concurrent requests
    --gpu-memory <MB>           GPU memory limit
    --price <WEI>               Base price per token
    --advertise-addr <ADDR>     Public address to advertise
    --no-announce               Don't announce to network
    -v, --verbose               Increase verbosity
```

### Validator CLI Arguments

```bash
lloom-validator [OPTIONS]

OPTIONS:
    -c, --config <FILE>          Configuration file path
    -n, --network <ID>           Network ID
    -i, --identity <FILE>        Identity file path
    --threshold <SCORE>         Validation threshold (0.0-1.0)
    --workers <N>               Number of validation workers
    --db-path <PATH>            Database file path
    --no-archive                Disable validation archiving
    --consensus                 Enable consensus validation
    -v, --verbose               Increase verbosity
```

## Programmatic Configuration

### Rust Configuration

```rust
use lloom_core::config::{Config, NetworkConfig, IdentityConfig};

// Build configuration programmatically
let config = Config::builder()
    .network(NetworkConfig {
        network_id: "mainnet".to_string(),
        listen_addr: "/ip4/0.0.0.0/tcp/0".parse()?,
        bootstrap_peers: vec![],
        enable_mdns: true,
        ..Default::default()
    })
    .identity(IdentityConfig {
        path: Some("~/.lloom/identity".into()),
        generate_if_missing: true,
    })
    .build()?;

// Or load from file and override
let mut config = Config::from_file("config.toml")?;
config.network.listen_addr = "/ip4/0.0.0.0/tcp/4001".parse()?;

// Apply to component
let client = Client::with_config(config).await?;
```

### Configuration Builder Pattern

```rust
use lloom_executor::{Executor, ExecutorConfig};

let executor = Executor::builder()
    .network_id("testnet")
    .llm_backend("openai")
    .api_key(std::env::var("OPENAI_API_KEY")?)
    .max_concurrent_requests(20)
    .gpu_memory_limit_mb(16384)
    .base_price_per_token_wei("2000000000000")
    .metrics_port(9091)
    .build()
    .await?;
```

## Configuration Precedence

Configuration is loaded and merged in the following order (later overrides earlier):

1. **Default values** - Built-in defaults
2. **Configuration file** - From `--config` or default location
3. **Environment variables** - `LLOOM_*` prefixed variables
4. **Command-line arguments** - Highest precedence

### Example Precedence

```toml
# config.toml
[network]
listen_addr = "/ip4/0.0.0.0/tcp/4001"
```

```bash
# Environment variable
export LLOOM_LISTEN_ADDR="/ip4/0.0.0.0/tcp/4002"

# Command line (highest precedence)
lloom-executor --listen-addr "/ip4/0.0.0.0/tcp/4003"

# Result: listen_addr = "/ip4/0.0.0.0/tcp/4003"
```

## Configuration Validation

### Schema Validation

```rust
use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
struct NetworkConfig {
    #[validate(length(min = 1))]
    network_id: String,
    
    #[validate(custom = "validate_multiaddr")]
    listen_addr: String,
    
    #[validate(range(min = 1024, max = 65535))]
    port: u16,
}

fn validate_config(config: &Config) -> Result<()> {
    config.validate()?;
    
    // Additional custom validation
    if config.network.bootstrap_peers.is_empty() 
        && !config.network.enable_mdns {
        return Err(anyhow!("Must specify bootstrap peers or enable mDNS"));
    }
    
    Ok(())
}
```

### Runtime Validation

```rust
impl Config {
    pub fn validate_runtime(&self) -> Result<()> {
        // Check file paths exist
        if let Some(path) = &self.identity.path {
            if !path.exists() && !self.identity.generate_if_missing {
                return Err(anyhow!("Identity file not found"));
            }
        }
        
        // Check network connectivity
        if self.blockchain.enabled {
            self.test_rpc_connection()?;
        }
        
        Ok(())
    }
}
```

## Configuration Templates

### Development Template

```toml
# dev.toml - Development configuration
[general]
network_id = "devnet"
log_level = "debug"

[network]
listen_addr = "/ip4/127.0.0.1/tcp/0"
enable_mdns = true

[request]
timeout_seconds = 60
max_retries = 5

[llm]
backend = "mock"

[monitoring]
metrics_enabled = true
trace_enabled = true
```

### Production Template

```toml
# prod.toml - Production configuration
[general]
network_id = "mainnet"
log_level = "info"
log_format = "json"

[network]
listen_addr = "/ip4/0.0.0.0/tcp/4001"
bootstrap_peers = [
    "/ip4/bootstrap1.lloom.network/tcp/4001/p2p/...",
    "/ip4/bootstrap2.lloom.network/tcp/4001/p2p/..."
]

[security]
enable_tls = true
require_signed_requests = true

[monitoring]
metrics_enabled = true
metrics_port = 9090
trace_enabled = true
trace_endpoint = "http://jaeger:14268/api/traces"

[blockchain]
enabled = true
rpc_url = "https://eth-mainnet.g.alchemy.com/v2/KEY"
```

## Dynamic Configuration

### Hot Reloading

```rust
use notify::{Watcher, RecursiveMode, watcher};
use std::sync::mpsc::channel;

pub struct ConfigWatcher {
    config: Arc<RwLock<Config>>,
    watcher: Notify,
}

impl ConfigWatcher {
    pub fn watch(path: &Path) -> Result<Self> {
        let (tx, rx) = channel();
        let mut watcher = watcher(tx, Duration::from_secs(2))?;
        watcher.watch(path, RecursiveMode::NonRecursive)?;
        
        tokio::spawn(async move {
            while let Ok(event) = rx.recv() {
                if let Ok(new_config) = Config::from_file(path) {
                    *config.write().await = new_config;
                    info!("Configuration reloaded");
                }
            }
        });
        
        Ok(ConfigWatcher { config, watcher })
    }
}
```

### Runtime Updates

```rust
pub trait ConfigUpdate {
    async fn update_pricing(&mut self, pricing: PricingConfig) -> Result<()>;
    async fn update_resources(&mut self, resources: ResourceConfig) -> Result<()>;
    async fn add_bootstrap_peer(&mut self, peer: Multiaddr) -> Result<()>;
}

impl ConfigUpdate for Executor {
    async fn update_pricing(&mut self, pricing: PricingConfig) -> Result<()> {
        self.config.pricing = pricing;
        self.apply_pricing_changes().await?;
        Ok(())
    }
}
```

## Migration Guide

### Migrating from v0.1.x to v0.2.x

```toml
# Old format (v0.1.x)
listen_address = "0.0.0.0:4001"
bootstrap_nodes = ["node1", "node2"]

# New format (v0.2.x)
[network]
listen_addr = "/ip4/0.0.0.0/tcp/4001"
bootstrap_peers = [
    "/ip4/node1/tcp/4001/p2p/...",
    "/ip4/node2/tcp/4001/p2p/..."
]
```

### Automatic Migration

```rust
pub fn migrate_config(old_config: &str) -> Result<Config> {
    let old: OldConfig = toml::from_str(old_config)?;
    
    let new = Config {
        network: NetworkConfig {
            listen_addr: format!("/ip4/{}", old.listen_address).parse()?,
            bootstrap_peers: old.bootstrap_nodes.into_iter()
                .map(|node| format!("/ip4/{}/tcp/4001", node).parse())
                .collect::<Result<Vec<_>>>()?,
            ..Default::default()
        },
        ..Default::default()
    };
    
    Ok(new)
}
```

## Best Practices

1. **Use configuration files** for production deployments
2. **Keep secrets in environment variables**, not config files
3. **Validate configuration** on startup
4. **Log configuration values** (except secrets) for debugging
5. **Use sensible defaults** for all optional values
6. **Document all configuration options** with examples
7. **Version configuration schemas** for backward compatibility
8. **Test configuration loading** in your test suite