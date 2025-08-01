# Development Environment

This guide will help you set up a complete development environment for Lloom, including a local Ethereum node, monitoring stack, and all necessary tools.

## Overview

The Lloom development environment includes:
- **RETH Ethereum Node**: Local blockchain for testing
- **Prometheus**: Metrics collection
- **Grafana**: Monitoring dashboards
- **Pre-funded accounts**: For testing transactions
- **Smart contracts**: Deployed accounting contracts

## Prerequisites

- Docker and Docker Compose installed
- Foundry toolchain installed
- At least 8GB RAM available
- 20GB free disk space

## Quick Setup

### 1. Start the Ethereum Development Stack

Navigate to the ethnode directory and start all services:

```bash
cd ethnode
docker-compose up -d
```

This starts:
- RETH node on ports 8545 (HTTP) and 8546 (WebSocket)
- Prometheus on port 9090
- Grafana on port 3000

### 2. Verify Services

Check all services are running:

```bash
docker-compose ps
```

Expected output:
```
NAME                STATUS              PORTS
reth-dev           running (healthy)   0.0.0.0:8545->8545/tcp, 0.0.0.0:8546->8546/tcp
prometheus         running             0.0.0.0:9090->9090/tcp
grafana           running             0.0.0.0:3000->3000/tcp
```

### 3. Access Monitoring

Open Grafana dashboard:
- URL: http://localhost:3000
- Username: admin
- Password: admin

The RETH dashboard shows:
- Block height
- Transaction pool
- Network peers
- Resource usage

## Funded Test Accounts

RETH provides 20 pre-funded accounts with 10,000 ETH each:

```bash
# List all test accounts
cast accounts --mnemonic "test test test test test test test test test test test junk"

# Check balance of first account
cast balance 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --rpc-url http://localhost:8545
```

### Transfer Test Funds

Use the provided script to fund your development addresses:

```bash
# Fund a specific address with 10 ETH from test accounts
./transfer_test_funds.sh \
  --target 0xYourAddress \
  --amount 10 \
  --rpc-url http://localhost:8545
```

## Deploy Smart Contracts

### 1. Compile Contracts

```bash
cd solidity
forge build
```

### 2. Deploy Accounting Contract

```bash
# Deploy to local network
forge create src/Accounting.sol:AccountingV2 \
  --rpc-url http://localhost:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

Note the deployed contract address for configuration.

### 3. Verify Deployment

```bash
# Check contract code
cast code <CONTRACT_ADDRESS> --rpc-url http://localhost:8545

# Call contract function
cast call <CONTRACT_ADDRESS> "DOMAIN_SEPARATOR()" --rpc-url http://localhost:8545
```

## Configure Lloom Components

### 1. Update Executor Configuration

Edit `executor-config.toml`:

```toml
[blockchain]
enabled = true
rpc_url = "http://localhost:8545"
contract_address = "0xYourDeployedContract"
chain_id = 31337  # Local network chain ID

[network]
bootstrap_nodes = ["/ip4/127.0.0.1/tcp/4001"]

[[llm_backends]]
name = "lmstudio"
endpoint = "http://localhost:1234/api/v0"
```

### 2. Set Environment Variables

```bash
export LLOOM_RPC_URL="http://localhost:8545"
export LLOOM_CONTRACT_ADDRESS="0xYourDeployedContract"
export LLOOM_CHAIN_ID="31337"
```

## Development Workflow

### 1. Start Core Services

```bash
# Terminal 1: Ethereum node and monitoring
cd ethnode && docker-compose up

# Terminal 2: Validator
lloom-validator --identity validator-identity.json --listen /ip4/0.0.0.0/tcp/4001

# Terminal 3: Executor with blockchain
lloom-executor --identity executor-identity.json --config executor-config.toml
```

### 2. Run Integration Tests

```bash
# Test full flow with blockchain
cargo test --features integration-tests

# Test specific component
cargo test -p lloom-executor --features blockchain
```

### 3. Monitor Activity

Watch real-time metrics:
- Grafana dashboards: http://localhost:3000
- Direct metrics: http://localhost:9001/metrics
- Blockchain explorer: Use `cast` commands

## Advanced Configuration

### Custom Network Parameters

Modify `docker-compose.yml` for different settings:

```yaml
services:
  reth:
    command:
      - node
      - --dev
      - --dev.block-time 6  # Faster blocks
      - --dev.accounts 50   # More test accounts
```

### Multiple Executors

Run multiple executors on different ports:

```bash
# Executor 1
lloom-executor --identity executor1.json --listen /ip4/0.0.0.0/tcp/5001

# Executor 2  
lloom-executor --identity executor2.json --listen /ip4/0.0.0.0/tcp/5002
```

### Contract Interaction

Interact with deployed contracts:

```bash
# Submit usage record (example)
cast send <CONTRACT_ADDRESS> \
  "processRequest(bytes,bytes,bytes,bytes)" \
  0x... 0x... 0x... 0x... \
  --rpc-url http://localhost:8545 \
  --private-key 0x...
```

## Debugging Tools

### 1. Network Inspection

```bash
# View all peers
lloom-client peers --bootstrap /ip4/127.0.0.1/tcp/4001

# Trace network messages
RUST_LOG=libp2p=debug lloom-client request ...
```

### 2. Blockchain Debugging

```bash
# Get latest block
cast block latest --rpc-url http://localhost:8545

# Trace transaction
cast trace --tx-hash 0x... --rpc-url http://localhost:8545

# View contract storage
cast storage <CONTRACT_ADDRESS> 0 --rpc-url http://localhost:8545
```

### 3. Log Analysis

```bash
# Filter logs by component
docker-compose logs reth | grep -i error

# Follow specific service logs
docker-compose logs -f prometheus

# Export logs
docker-compose logs > lloom-dev.log
```

## Testing Scenarios

### 1. Load Testing

```bash
# Run parallel requests
for i in {1..10}; do
  lloom-client request \
    --bootstrap /ip4/127.0.0.1/tcp/4001 \
    --model "llama-2-7b" \
    --prompt "Test $i" &
done
```

### 2. Failure Testing

```bash
# Test executor disconnection
# Stop executor mid-request

# Test blockchain unavailability  
# Stop RETH node and observe behavior
```

### 3. Performance Testing

Monitor metrics during load:
- Request latency
- Token throughput
- Resource usage
- Blockchain confirmation times

## Cleanup

### Stop Services

```bash
# Stop all Docker services
cd ethnode && docker-compose down

# Remove volumes (deletes blockchain data)
docker-compose down -v

# Stop Lloom nodes
# Ctrl+C in each terminal
```

### Reset State

```bash
# Clear local data
rm -rf ~/.lloom/cache
rm -rf ~/.lloom/logs

# Reset blockchain
cd ethnode && docker-compose down -v && docker-compose up -d
```

## Troubleshooting

### Common Issues

**Docker containers won't start**
- Check port availability: `lsof -i :8545`
- Ensure Docker daemon is running
- Check disk space

**Can't connect to blockchain**
- Verify RETH is healthy: `docker-compose ps`
- Check RPC endpoint: `curl http://localhost:8545`
- Review RETH logs: `docker-compose logs reth`

**Grafana shows no data**
- Check Prometheus targets: http://localhost:9090/targets
- Verify RETH metrics: http://localhost:9001/metrics
- Restart Grafana: `docker-compose restart grafana`

## Next Steps

With your development environment ready:

1. [Build from source](../development/building.md) with custom modifications
2. [Run tests](../development/testing.md) against local network
3. [Deploy contracts](../examples/smart-contracts.md) for production
4. [Configure production nodes](./configuration.md) with real settings

Happy developing!