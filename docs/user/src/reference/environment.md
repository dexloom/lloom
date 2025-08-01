# Environment Variables

This reference documents all environment variables used by Lloom components, their default values, and configuration options.

## Core Environment Variables

### Network Configuration

#### `LLOOM_LISTEN_ADDR`
- **Description**: Address and port for P2P network listening
- **Default**: `/ip4/0.0.0.0/tcp/0` (random port)
- **Example**: `/ip4/0.0.0.0/tcp/4001`
- **Used by**: All components

#### `LLOOM_BOOTSTRAP_PEERS`
- **Description**: Comma-separated list of bootstrap peer addresses
- **Default**: None
- **Example**: `/ip4/bootstrap1.lloom.network/tcp/4001/p2p/12D3KooW...,/ip4/bootstrap2.lloom.network/tcp/4001/p2p/12D3KooW...`
- **Used by**: All components

#### `LLOOM_NETWORK_ID`
- **Description**: Network identifier for chain separation
- **Default**: `mainnet`
- **Options**: `mainnet`, `testnet`, `devnet`, custom string
- **Used by**: All components

### Identity Configuration

#### `LLOOM_IDENTITY_PATH`
- **Description**: Path to identity file containing private key and peer ID
- **Default**: `~/.lloom/identity`
- **Example**: `/opt/lloom/identity.json`
- **Used by**: All components

#### `LLOOM_GENERATE_IDENTITY`
- **Description**: Generate new identity if file doesn't exist
- **Default**: `false`
- **Options**: `true`, `false`
- **Used by**: All components

### Logging Configuration

#### `RUST_LOG`
- **Description**: Rust logging level and filters
- **Default**: `info`
- **Examples**: 
  - `debug`
  - `lloom=debug,libp2p=info`
  - `lloom_executor=trace,lloom_core=debug`
- **Used by**: All components

#### `LLOOM_LOG_FORMAT`
- **Description**: Log output format
- **Default**: `pretty`
- **Options**: `pretty`, `json`, `compact`
- **Used by**: All components

#### `LLOOM_LOG_FILE`
- **Description**: Path to log file (stdout if not set)
- **Default**: None (logs to stdout)
- **Example**: `/var/log/lloom/executor.log`
- **Used by**: All components

## Client Environment Variables

### Request Configuration

#### `LLOOM_DEFAULT_MODEL`
- **Description**: Default model to use for requests
- **Default**: `gpt-3.5-turbo`
- **Example**: `gpt-4`, `llama-2-70b-chat`
- **Used by**: Client

#### `LLOOM_REQUEST_TIMEOUT`
- **Description**: Default timeout for LLM requests in seconds
- **Default**: `300` (5 minutes)
- **Example**: `600`
- **Used by**: Client

#### `LLOOM_MAX_RETRIES`
- **Description**: Maximum number of request retries
- **Default**: `3`
- **Example**: `5`
- **Used by**: Client

### Cost Management

#### `LLOOM_MAX_COST_PER_REQUEST`
- **Description**: Maximum cost per request in ETH
- **Default**: `0.01`
- **Example**: `0.001`
- **Used by**: Client

#### `LLOOM_BUDGET_TRACKING`
- **Description**: Enable budget tracking and enforcement
- **Default**: `true`
- **Options**: `true`, `false`
- **Used by**: Client

## Executor Environment Variables

### LLM Backend Configuration

#### `LLOOM_LLM_BACKEND`
- **Description**: LLM backend to use
- **Default**: `openai`
- **Options**: `openai`, `lmstudio`, `custom`
- **Used by**: Executor

#### `OPENAI_API_KEY`
- **Description**: OpenAI API key for OpenAI backend
- **Default**: None (required for OpenAI backend)
- **Example**: `sk-...`
- **Used by**: Executor (OpenAI backend)

#### `LMSTUDIO_API_URL`
- **Description**: LMStudio API endpoint URL
- **Default**: `http://localhost:1234/v1`
- **Example**: `http://192.168.1.100:1234/v1`
- **Used by**: Executor (LMStudio backend)

#### `LLOOM_MODEL_PATH`
- **Description**: Path to local model file for custom backends
- **Default**: None
- **Example**: `/models/llama-2-13b.gguf`
- **Used by**: Executor (custom backends)

### Resource Management

#### `LLOOM_MAX_CONCURRENT_REQUESTS`
- **Description**: Maximum concurrent LLM requests
- **Default**: `10`
- **Example**: `20`
- **Used by**: Executor

#### `LLOOM_MAX_QUEUE_SIZE`
- **Description**: Maximum size of request queue
- **Default**: `100`
- **Example**: `200`
- **Used by**: Executor

#### `LLOOM_GPU_MEMORY_LIMIT`
- **Description**: GPU memory limit in MB (0 for unlimited)
- **Default**: `0`
- **Example**: `8192`
- **Used by**: Executor

### Pricing Configuration

#### `LLOOM_BASE_PRICE_PER_TOKEN`
- **Description**: Base price per token in wei
- **Default**: `1000000000000` (0.000001 ETH)
- **Example**: `2000000000000`
- **Used by**: Executor

#### `LLOOM_PRICE_MULTIPLIER`
- **Description**: Dynamic price multiplier based on load
- **Default**: `1.0`
- **Example**: `1.5`
- **Used by**: Executor

## Validator Environment Variables

### Validation Configuration

#### `LLOOM_VALIDATION_THRESHOLD`
- **Description**: Minimum validation score to pass (0.0-1.0)
- **Default**: `0.8`
- **Example**: `0.9`
- **Used by**: Validator

#### `LLOOM_VALIDATION_WORKERS`
- **Description**: Number of validation worker threads
- **Default**: `4`
- **Example**: `8`
- **Used by**: Validator

#### `LLOOM_VALIDATION_TIMEOUT`
- **Description**: Timeout for validation operations in seconds
- **Default**: `60`
- **Example**: `120`
- **Used by**: Validator

### Storage Configuration

#### `LLOOM_VALIDATION_DB_PATH`
- **Description**: Path to validation database
- **Default**: `~/.lloom/validator/validation.db`
- **Example**: `/var/lib/lloom/validation.db`
- **Used by**: Validator

#### `LLOOM_ARCHIVE_VALIDATIONS`
- **Description**: Archive old validations
- **Default**: `true`
- **Options**: `true`, `false`
- **Used by**: Validator

## Blockchain Configuration

### Ethereum RPC

#### `ETH_RPC_URL`
- **Description**: Ethereum RPC endpoint URL
- **Default**: `http://localhost:8545`
- **Example**: `https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY`
- **Used by**: All components (optional)

#### `ETH_CHAIN_ID`
- **Description**: Ethereum chain ID
- **Default**: `1` (mainnet)
- **Options**: `1` (mainnet), `11155111` (sepolia), `31337` (local)
- **Used by**: All components (optional)

### Contract Addresses

#### `LLOOM_ACCOUNTING_CONTRACT`
- **Description**: Address of the accounting smart contract
- **Default**: None
- **Example**: `0x1234567890123456789012345678901234567890`
- **Used by**: All components (optional)

#### `LLOOM_REGISTRY_CONTRACT`
- **Description**: Address of the registry smart contract
- **Default**: None
- **Example**: `0x0987654321098765432109876543210987654321`
- **Used by**: All components (optional)

## Monitoring Configuration

### Metrics

#### `LLOOM_METRICS_ENABLED`
- **Description**: Enable Prometheus metrics
- **Default**: `true`
- **Options**: `true`, `false`
- **Used by**: All components

#### `LLOOM_METRICS_PORT`
- **Description**: Port for Prometheus metrics endpoint
- **Default**: `9090`
- **Example**: `9091`
- **Used by**: All components

#### `LLOOM_METRICS_PATH`
- **Description**: Path for metrics endpoint
- **Default**: `/metrics`
- **Example**: `/prometheus/metrics`
- **Used by**: All components

### Tracing

#### `LLOOM_TRACING_ENABLED`
- **Description**: Enable distributed tracing
- **Default**: `false`
- **Options**: `true`, `false`
- **Used by**: All components

#### `LLOOM_JAEGER_ENDPOINT`
- **Description**: Jaeger collector endpoint
- **Default**: `http://localhost:14268/api/traces`
- **Example**: `http://jaeger:14268/api/traces`
- **Used by**: All components

## Development Environment Variables

### Testing

#### `LLOOM_TEST_MODE`
- **Description**: Enable test mode (uses test network, mock services)
- **Default**: `false`
- **Options**: `true`, `false`
- **Used by**: All components

#### `LLOOM_MOCK_LLM`
- **Description**: Use mock LLM responses for testing
- **Default**: `false`
- **Options**: `true`, `false`
- **Used by**: Executor

### Debugging

#### `LLOOM_DEBUG_P2P`
- **Description**: Enable detailed P2P debugging logs
- **Default**: `false`
- **Options**: `true`, `false`
- **Used by**: All components

#### `LLOOM_TRACE_REQUESTS`
- **Description**: Trace all requests with detailed timing
- **Default**: `false`
- **Options**: `true`, `false`
- **Used by**: All components

## Security Environment Variables

### API Keys and Secrets

#### `LLOOM_API_KEY`
- **Description**: API key for authenticated endpoints
- **Default**: None
- **Example**: Generated UUID
- **Used by**: All components (optional)

#### `LLOOM_JWT_SECRET`
- **Description**: Secret for JWT token generation
- **Default**: Random generated
- **Example**: 32-byte hex string
- **Used by**: All components (optional)

### Network Security

#### `LLOOM_ENABLE_TLS`
- **Description**: Enable TLS for P2P connections
- **Default**: `true`
- **Options**: `true`, `false`
- **Used by**: All components

#### `LLOOM_TLS_CERT_PATH`
- **Description**: Path to TLS certificate
- **Default**: Auto-generated
- **Example**: `/etc/lloom/tls/cert.pem`
- **Used by**: All components

#### `LLOOM_TLS_KEY_PATH`
- **Description**: Path to TLS private key
- **Default**: Auto-generated
- **Example**: `/etc/lloom/tls/key.pem`
- **Used by**: All components

## Docker-Specific Variables

### Container Configuration

#### `LLOOM_DATA_DIR`
- **Description**: Base directory for all Lloom data
- **Default**: `/data/lloom`
- **Example**: `/var/lib/lloom`
- **Used by**: All components (Docker)

#### `LLOOM_CONFIG_FILE`
- **Description**: Path to configuration file (overrides env vars)
- **Default**: None
- **Example**: `/etc/lloom/config.toml`
- **Used by**: All components

## Environment Variable Precedence

Variables are loaded in the following order (later overrides earlier):

1. Default values (built into binary)
2. Configuration file (`LLOOM_CONFIG_FILE`)
3. Environment variables
4. Command-line arguments

## Example Environment Files

### Development Environment (.env.development)

```bash
# Network
LLOOM_NETWORK_ID=devnet
LLOOM_LISTEN_ADDR=/ip4/127.0.0.1/tcp/4001

# Logging
RUST_LOG=debug
LLOOM_LOG_FORMAT=pretty

# Testing
LLOOM_TEST_MODE=true
LLOOM_MOCK_LLM=true

# Ethereum
ETH_RPC_URL=http://localhost:8545
ETH_CHAIN_ID=31337
```

### Production Environment (.env.production)

```bash
# Network
LLOOM_NETWORK_ID=mainnet
LLOOM_BOOTSTRAP_PEERS=/ip4/bootstrap1.lloom.network/tcp/4001/p2p/12D3KooW...

# Logging
RUST_LOG=info
LLOOM_LOG_FORMAT=json
LLOOM_LOG_FILE=/var/log/lloom/app.log

# Security
LLOOM_ENABLE_TLS=true
LLOOM_API_KEY=${GENERATED_API_KEY}

# Monitoring
LLOOM_METRICS_ENABLED=true
LLOOM_TRACING_ENABLED=true

# Ethereum
ETH_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/${ALCHEMY_KEY}
ETH_CHAIN_ID=1
```

### Executor-Specific (.env.executor)

```bash
# LLM Backend
LLOOM_LLM_BACKEND=openai
OPENAI_API_KEY=${OPENAI_API_KEY}

# Resources
LLOOM_MAX_CONCURRENT_REQUESTS=20
LLOOM_GPU_MEMORY_LIMIT=16384

# Pricing
LLOOM_BASE_PRICE_PER_TOKEN=2000000000000
```

## Loading Environment Variables

### Using dotenv

```rust
use dotenv::dotenv;

fn main() {
    // Load .env file
    dotenv().ok();
    
    // Load specific env file
    dotenv::from_filename(".env.production").ok();
    
    // Access variables
    let network_id = std::env::var("LLOOM_NETWORK_ID")
        .unwrap_or_else(|_| "mainnet".to_string());
}
```

### Using config crate

```rust
use config::{Config, Environment};

let config = Config::builder()
    .add_source(config::File::with_name("config/default"))
    .add_source(Environment::with_prefix("LLOOM"))
    .build()?;
```

## Best Practices

1. **Security**: Never commit sensitive environment variables to version control
2. **Documentation**: Document all custom environment variables
3. **Defaults**: Provide sensible defaults for all variables
4. **Validation**: Validate environment variables on startup
5. **Logging**: Log configuration values (except secrets) on startup
6. **Naming**: Use consistent `LLOOM_` prefix for all custom variables