# Configuration

Lloom nodes are highly configurable to suit different deployment scenarios. This guide covers all configuration options for clients, executors, and validators.

## Configuration File Format

Lloom uses TOML format for configuration files. The default locations are:

- **System-wide**: `/etc/lloom/config.toml`
- **User-specific**: `~/.lloom/config.toml`
- **Custom path**: Via `--config` flag or `LLOOM_CONFIG_PATH` environment variable

## Common Configuration

These settings apply to all node types:

```toml
# Network configuration
[network]
# Bootstrap nodes for initial connection
bootstrap_nodes = [
    "/ip4/boot1.lloom.network/tcp/4001/p2p/QmBootstrap1",
    "/ip4/boot2.lloom.network/tcp/4001/p2p/QmBootstrap2"
]

# Listen addresses for P2P connections
listen_addresses = [
    "/ip4/0.0.0.0/tcp/4001",
    "/ip6/::/tcp/4001"
]

# Enable mDNS for local peer discovery
enable_mdns = true

# Connection limits
max_connections = 200
max_pending_connections = 50

# Logging configuration
[logging]
# Log level: trace, debug, info, warn, error
level = "info"

# Log format: json, pretty, compact
format = "pretty"

# Log file path (optional)
file = "/var/log/lloom/node.log"

# Identity configuration
[identity]
# Path to identity file
path = "~/.lloom/identity.json"

# Generate new identity if missing
auto_generate = false
```

## Executor Configuration

Executor-specific settings for LLM service providers:

```toml
# Executor role configuration
[executor]
# Enable executor functionality
enabled = true

# Maximum concurrent requests
max_concurrent_requests = 10

# Request timeout in seconds
request_timeout = 300

# LLM Backend configurations
[[llm_backends]]
# Backend name (unique identifier)
name = "openai"

# API endpoint
endpoint = "https://api.openai.com/v1"

# API key (can use environment variable)
api_key = "${OPENAI_API_KEY}"

# Supported models (empty = all available)
supported_models = ["gpt-3.5-turbo", "gpt-4"]

# Rate limiting (requests per minute)
rate_limit = 60

# Request timeout for this backend
timeout = 120

# Priority (higher = preferred)
priority = 10

# Second backend example - LMStudio
[[llm_backends]]
name = "lmstudio"
endpoint = "http://localhost:1234/api/v0"
# No API key needed for local LMStudio
supported_models = [] # Auto-discover
rate_limit = 100
timeout = 300
priority = 5

# Custom backend with specific configuration
[[llm_backends]]
name = "custom_llama"
endpoint = "http://llama-server:8080"
api_key = "${CUSTOM_API_KEY}"
supported_models = ["llama-2-70b", "code-llama-34b"]
rate_limit = 30
timeout = 600
priority = 8

# Headers for custom backends
[llm_backends.headers]
"X-Custom-Header" = "value"
"Authorization" = "Bearer ${CUSTOM_TOKEN}"

# Pricing configuration
[pricing]
# Default prices in wei per token
default_inbound_price = "1000000000000000"  # 0.001 ETH
default_outbound_price = "2000000000000000" # 0.002 ETH

# Model-specific pricing
[pricing.models]
"gpt-3.5-turbo" = { inbound = "500000000000000", outbound = "1000000000000000" }
"gpt-4" = { inbound = "5000000000000000", outbound = "10000000000000000" }
"llama-2-70b" = { inbound = "2000000000000000", outbound = "4000000000000000" }

# Blockchain configuration
[blockchain]
# Enable blockchain integration
enabled = true

# RPC endpoint
rpc_url = "https://eth-mainnet.g.alchemy.com/v2/${ALCHEMY_API_KEY}"

# Accounting contract address
contract_address = "0x742d35Cc6634C0532925a3b844Bc9e7195Ed5E8D"

# Chain ID (1 = mainnet, 11155111 = sepolia)
chain_id = 1

# Gas price strategy: legacy, eip1559, oracle
gas_price_strategy = "eip1559"

# Maximum gas price in gwei
max_gas_price = 100

# Submission batch size
batch_size = 10

# Submission interval in seconds
submission_interval = 3600

# Resource limits
[limits]
# Maximum prompt length in characters
max_prompt_length = 10000

# Maximum tokens per request
max_tokens_per_request = 4096

# Maximum requests per client per hour
max_requests_per_client = 100

# Memory limit for LLM processes (MB)
memory_limit = 8192

# Monitoring
[monitoring]
# Enable Prometheus metrics
enabled = true

# Metrics endpoint
endpoint = "0.0.0.0:9001"

# Metrics path
path = "/metrics"
```

## Client Configuration

Client-specific settings:

```toml
# Client configuration
[client]
# Default model preference
preferred_models = ["gpt-3.5-turbo", "llama-2-7b"]

# Executor selection strategy: random, lowest_price, fastest
selection_strategy = "lowest_price"

# Request retry configuration
[client.retry]
# Maximum retry attempts
max_attempts = 3

# Initial retry delay in milliseconds
initial_delay = 1000

# Exponential backoff multiplier
multiplier = 2.0

# Maximum retry delay in milliseconds
max_delay = 30000

# Request defaults
[client.defaults]
# Default temperature
temperature = 0.7

# Default max tokens
max_tokens = 1000

# Default system prompt
system_prompt = ""

# Timeout for requests in seconds
request_timeout = 120

# Caching configuration
[client.cache]
# Enable response caching
enabled = true

# Cache directory
directory = "~/.lloom/cache"

# Maximum cache size in MB
max_size = 1024

# Cache TTL in seconds
ttl = 3600
```

## Validator Configuration

Validator-specific settings:

```toml
# Validator configuration
[validator]
# Enable validator functionality
enabled = true

# DHT configuration
[validator.dht]
# Replication factor
replication_factor = 20

# Record TTL in seconds
record_ttl = 3600

# Query timeout in seconds
query_timeout = 60

# Peer management
[validator.peers]
# Maximum peers to track
max_peers = 1000

# Peer cleanup interval in seconds
cleanup_interval = 600

# Peer timeout in seconds
peer_timeout = 1800

# Network statistics
[validator.stats]
# Enable statistics collection
enabled = true

# Statistics interval in seconds
interval = 60

# Statistics retention in hours
retention = 168
```

## Environment Variables

Lloom supports environment variable substitution in configuration files:

```toml
# Use ${VAR_NAME} syntax
api_key = "${OPENAI_API_KEY}"
rpc_url = "https://eth-mainnet.g.alchemy.com/v2/${ALCHEMY_API_KEY}"

# With default values
api_key = "${OPENAI_API_KEY:-sk-default-key}"

# Nested variables
endpoint = "https://${API_HOST:-api.openai.com}:${API_PORT:-443}/v1"
```

### Common Environment Variables

```bash
# Core settings
export LLOOM_CONFIG_PATH="/path/to/config.toml"
export LLOOM_IDENTITY_PATH="/path/to/identity.json"
export LLOOM_LOG_LEVEL="debug"

# Network settings
export LLOOM_BOOTSTRAP_NODES="/ip4/1.2.3.4/tcp/4001"
export LLOOM_LISTEN_ADDRESS="/ip4/0.0.0.0/tcp/4001"

# API keys
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."

# Blockchain settings
export LLOOM_RPC_URL="https://eth-mainnet.g.alchemy.com/v2/..."
export LLOOM_CONTRACT_ADDRESS="0x..."
export LLOOM_PRIVATE_KEY="0x..."
```

## Configuration Precedence

Configuration values are loaded in this order (later overrides earlier):

1. Built-in defaults
2. System configuration file (`/etc/lloom/config.toml`)
3. User configuration file (`~/.lloom/config.toml`)
4. Custom configuration file (via `--config`)
5. Environment variables
6. Command-line flags

## Configuration Examples

### Minimal Client Configuration

```toml
[network]
bootstrap_nodes = ["/ip4/boot.lloom.network/tcp/4001"]

[client]
preferred_models = ["gpt-3.5-turbo"]
```

### Production Executor Configuration

```toml
[network]
bootstrap_nodes = [
    "/ip4/boot1.lloom.network/tcp/4001",
    "/ip4/boot2.lloom.network/tcp/4001",
    "/ip4/boot3.lloom.network/tcp/4001"
]
listen_addresses = ["/ip4/0.0.0.0/tcp/4001"]

[executor]
enabled = true
max_concurrent_requests = 50

[[llm_backends]]
name = "openai"
endpoint = "https://api.openai.com/v1"
api_key = "${OPENAI_API_KEY}"
supported_models = ["gpt-3.5-turbo", "gpt-4"]
rate_limit = 60
priority = 10

[blockchain]
enabled = true
rpc_url = "${ETH_RPC_URL}"
contract_address = "${ACCOUNTING_CONTRACT}"
chain_id = 1

[monitoring]
enabled = true
endpoint = "0.0.0.0:9001"

[logging]
level = "info"
format = "json"
file = "/var/log/lloom/executor.log"
```

### High-Availability Validator Configuration

```toml
[network]
listen_addresses = [
    "/ip4/0.0.0.0/tcp/4001",
    "/ip6/::/tcp/4001"
]
max_connections = 500

[validator]
enabled = true

[validator.dht]
replication_factor = 30
record_ttl = 7200

[validator.peers]
max_peers = 2000
cleanup_interval = 300

[monitoring]
enabled = true
endpoint = "0.0.0.0:9001"

[logging]
level = "info"
format = "json"
```

## Configuration Validation

Lloom validates configuration on startup:

```bash
# Validate configuration without starting
lloom-executor --config config.toml --validate

# Test configuration with dry run
lloom-executor --config config.toml --dry-run
```

Common validation errors:

- **Invalid addresses**: Check multiaddr format
- **Missing API keys**: Set required environment variables
- **Invalid models**: Verify model names match backend
- **Port conflicts**: Ensure ports are available

## Dynamic Configuration

Some settings can be updated without restart:

- Log levels (via signals)
- Rate limits (via API)
- Model availability (auto-discovered)

## Security Considerations

### Sensitive Data

Never commit sensitive data to version control:

```toml
# Bad - API key in config
api_key = "sk-actual-api-key"

# Good - Use environment variable
api_key = "${OPENAI_API_KEY}"
```

### File Permissions

Secure configuration files:

```bash
# Set appropriate permissions
chmod 600 ~/.lloom/config.toml
chmod 600 ~/.lloom/identity.json

# Verify ownership
chown $USER:$USER ~/.lloom/*
```

### Network Security

Configure firewall rules:

```bash
# Allow P2P port
sudo ufw allow 4001/tcp

# Allow metrics (internal only)
sudo ufw allow from 10.0.0.0/8 to any port 9001
```

## Troubleshooting Configuration

### Debug Configuration Loading

```bash
# Show effective configuration
RUST_LOG=lloom_core=debug lloom-executor --show-config

# Trace configuration sources
RUST_LOG=config=trace lloom-executor
```

### Common Issues

**Environment variable not found**
```bash
# Check variable is exported
echo $OPENAI_API_KEY

# Export if needed
export OPENAI_API_KEY="sk-..."
```

**Configuration file not found**
```bash
# Check file exists
ls -la ~/.lloom/config.toml

# Create directory if needed
mkdir -p ~/.lloom
```

**Invalid TOML syntax**
```bash
# Validate TOML syntax
toml-cli validate config.toml

# Common issues:
# - Missing quotes around strings
# - Invalid table syntax
# - Duplicate keys
```

## Best Practices

1. **Use environment variables** for sensitive data
2. **Version control** configuration templates, not actual configs
3. **Document** custom settings and their purposes
4. **Monitor** configuration changes in production
5. **Test** configuration changes in development first
6. **Backup** working configurations before changes
7. **Use descriptive names** for custom backends
8. **Set appropriate timeouts** based on your network
9. **Configure logging** for debugging and monitoring
10. **Regular review** configuration for optimization

## Next Steps

- [Running a Client](../user-manual/client.md) - Use your configuration
- [Running an Executor](../user-manual/executor.md) - Set up service provider
- [Monitoring](../user-manual/monitoring.md) - Track node performance
- [Troubleshooting](../user-manual/troubleshooting.md) - Solve common issues