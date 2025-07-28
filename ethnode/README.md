# RETH Ethereum Node with Monitoring Stack

This Docker Compose setup provides a complete Ethereum development environment using RETH (Rust Ethereum client) in development mode, along with Prometheus monitoring and Grafana dashboards.

## 🚀 Services Included

### RETH Node (Port 8545, 8546, 8551, 9001)
- **RETH Ethereum client** in development mode
- **20 pre-funded accounts** with 10,000 ETH each
- **12-second block time** for faster development
- **Full RPC API** support (HTTP + WebSocket)
- **Metrics endpoint** for monitoring

### Prometheus (Port 9090)
- **Metrics collection** from RETH node
- **Time-series database** for monitoring data
- **5-second scrape interval** for real-time monitoring

### Grafana (Port 3000)
- **Visual dashboards** for RETH metrics
- **Pre-configured datasource** (Prometheus)
- **RETH Overview dashboard** included
- **Admin credentials**: admin/admin

## 📋 Prerequisites

- Docker Engine 20.10+
- Docker Compose 2.0+
- At least 4GB RAM available
- At least 10GB disk space

## 🏁 Getting Started

### 1. Start the Environment

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f

# View logs for specific service
docker-compose logs -f reth
```

### 2. Verify Services

Check that all services are running:
```bash
docker-compose ps
```

You should see:
- `reth-dev` (healthy)
- `prometheus` (up)
- `grafana` (up)

### 3. Access Services

| Service | URL | Purpose |
|---------|-----|---------|
| **RETH RPC** | http://localhost:8545 | JSON-RPC HTTP endpoint |
| **RETH WebSocket** | ws://localhost:8546 | WebSocket endpoint |
| **Grafana** | http://localhost:3000 | Monitoring dashboards |
| **Prometheus** | http://localhost:9090 | Metrics database |

## 🔧 Usage Examples

### Connect with Web3 Libraries

**JavaScript (ethers.js):**
```javascript
import { ethers } from 'ethers';

const provider = new ethers.JsonRpcProvider('http://localhost:8545');
const blockNumber = await provider.getBlockNumber();
console.log('Current block:', blockNumber);
```

**Python (web3.py):**
```python
from web3 import Web3

w3 = Web3(Web3.HTTPProvider('http://localhost:8545'))
print(f"Connected: {w3.is_connected()}")
print(f"Latest block: {w3.eth.block_number}")
```

### Pre-funded Development Accounts

RETH dev mode provides 20 accounts with 10,000 ETH each:

```bash
# Get the first account
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_accounts","params":[],"id":1}' \
  http://localhost:8545

# Check balance of first account
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_getBalance","params":["0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","latest"],"id":1}' \
  http://localhost:8545
```

## 💰 Test Fund Transfer Script

RETH dev mode pre-funds 20 test addresses derived from the mnemonic `"test test test test test test test test test test test junk"` with 10,000 ETH each. Use the provided script to easily transfer funds from these addresses.

### Test Addresses Overview

The 20 test addresses are derived using the standard HD wallet derivation path `m/44'/60'/0'/0/{index}` where index ranges from 0-19. Each address starts with approximately 10,000 ETH.

**First few test addresses:**
- Index 0: `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`
- Index 1: `0x70997970C51812dc3A010C7d01b50e0d17dc79C8`
- Index 2: `0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC`
- ... (up to index 19)

### Using the Transfer Script

The [`transfer_test_funds.sh`](transfer_test_funds.sh) script allows you to transfer funds from the test addresses to any target address:

```bash
# Basic usage - transfer 1.5 ETH from all 20 addresses to target
./transfer_test_funds.sh --target 0x742d35Cc6634C0532925a3b8D6f5DDA5CF21F0a7 --amount 1.5

# Transfer from specific addresses (indices 0-4)
./transfer_test_funds.sh --target 0x742d35Cc6634C0532925a3b8D6f5DDA5CF21F0a7 --amount 0.1 --start 0 --end 4

# Dry run to see what would be transferred
./transfer_test_funds.sh --target 0x742d35Cc6634C0532925a3b8D6f5DDA5CF21F0a7 --amount 2.0 --dry-run

# Use custom RPC URL
./transfer_test_funds.sh --target 0x742d35Cc6634C0532925a3b8D6f5DDA5CF21F0a7 --amount 1.0 --rpc-url http://localhost:8545
```

### Script Features

- **📊 Balance Checking**: Shows current balances before and after transfers
- **🛡️ Error Handling**: Validates addresses, amounts, and connection to RPC
- **🔍 Dry Run Mode**: Preview transfers without executing them
- **⚙️ Configurable Range**: Transfer from specific address indices
- **📝 Comprehensive Logging**: Clear output with color-coded messages
- **✅ Safety Checks**: Confirms sufficient balance before transfers
- **👤 User Confirmation**: Asks for confirmation before executing transfers

### Script Options

| Option | Description | Default |
|--------|-------------|---------|
| `--target` | Target address to receive funds (required) | - |
| `--amount` | Amount in ETH to transfer from each address (required) | - |
| `--start` | Start address index (0-19) | 0 |
| `--end` | End address index (0-19) | 19 |
| `--dry-run` | Show transfers without executing | false |
| `--rpc-url` | Custom RPC URL | http://localhost:8545 |
| `--help` | Show help message | - |

### Prerequisites

- **Foundry** must be installed (`cast` command available) - [Install Foundry](https://getfoundry.sh)
- **RETH node** running in dev mode (use `docker-compose up -d`)
- **Node accessible** at the specified RPC URL

### Example Output

```
🚀 RETH Test Fund Transfer Script
=================================

[INFO] Checking RPC connection to http://localhost:8545...
[SUCCESS] Connected to RPC at http://localhost:8545

Current balances of test addresses:
----------------------------------------
Idx  Address                                    Balance (ETH)
----------------------------------------
0    0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266    10000.0
1    0x70997970C51812dc3A010C7d01b50e0d17dc79C8    10000.0
2    0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC    10000.0
...
----------------------------------------

[INFO] Starting transfers...
[INFO] Target address: 0x742d35Cc6634C0532925a3b8D6f5DDA5CF21F0a7
[INFO] Amount per transfer: 1.5 ETH (1500000000000000000 wei)
[INFO] Address range: 0 to 2

Proceed with transfers? [y/N] y

[INFO] Transferring from address 0...
[SUCCESS] Transfer successful! TX: 0x123abc...
[INFO] Transferring from address 1...
[SUCCESS] Transfer successful! TX: 0x456def...

[SUCCESS] Script completed successfully!
```

### Common Use Cases

**1. Fund a development wallet:**
```bash
# Transfer 5 ETH from first 5 test addresses to your development wallet
./transfer_test_funds.sh --target 0x1234567890123456789012345678901234567890 --amount 5.0 --start 0 --end 4
```

**2. Create multiple funded accounts:**
```bash
# Transfer 1 ETH each to create 10 funded accounts
for addr in 0x1111111111111111111111111111111111111111 0x2222222222222222222222222222222222222222 0x3333333333333333333333333333333333333333; do
  ./transfer_test_funds.sh --target $addr --amount 1.0 --start 0 --end 9
done
```

**3. Test with minimal funds:**
```bash
# Transfer small amounts for testing
./transfer_test_funds.sh --target 0x4444444444444444444444444444444444444444 --amount 0.01 --start 0 --end 0
```

## 📊 Monitoring & Dashboards

### Grafana Dashboards

1. **Access Grafana**: http://localhost:3000
2. **Login**: admin/admin (change password on first login)
3. **View dashboards**: Home → Dashboards → RETH Node Overview

The included dashboard shows:
- Current block height
- Transaction pool size
- Connected peers
- RPC request rate
- Memory usage

### Prometheus Metrics

Direct access to metrics: http://localhost:9090

Key RETH metrics to monitor:
- `reth_blockchain_height` - Current block number
- `reth_txpool_pending_pool_size` - Pending transactions
- `reth_network_peers` - Connected peers
- `reth_rpc_requests_total` - RPC request counter
- `reth_db_memory_usage_bytes` - Database memory usage

## 🛠 Management Commands

### Start/Stop Services

```bash
# Start all services
docker-compose up -d

# Stop all services
docker-compose down

# Stop and remove volumes (⚠️ deletes blockchain data)
docker-compose down -v

# Restart specific service
docker-compose restart reth
```

### View Logs

```bash
# All services
docker-compose logs -f

# Specific service
docker-compose logs -f reth
docker-compose logs -f prometheus
docker-compose logs -f grafana

# Last 100 lines
docker-compose logs --tail=100 reth
```

### Health Checks

```bash
# Check RETH node health
curl -f http://localhost:8545

# Check if RETH is syncing (should return false in dev mode)
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}' \
  http://localhost:8545

# Check current gas price
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_gasPrice","params":[],"id":1}' \
  http://localhost:8545
```

## 🔒 Security Notes

- **Development Only**: This setup is configured for development with open CORS and authentication disabled
- **JWT Secret**: Uses a static JWT secret (change for production)
- **Default Passwords**: Change Grafana admin password on first login
- **Network Exposure**: Services are bound to localhost only

## 📂 File Structure

```
ethnode/
├── docker-compose.yml          # Main orchestration file
├── jwt.hex                     # JWT secret for Engine API
├── prometheus.yml              # Prometheus configuration
├── grafana/
│   ├── provisioning/
│   │   ├── datasources/        # Grafana datasource config
│   │   └── dashboards/         # Dashboard provisioning config
│   └── dashboards/
│       └── reth-overview.json  # RETH monitoring dashboard
└── README.md                   # This file
```

## 🐛 Troubleshooting

### RETH Won't Start
- Check available disk space (needs ~10GB)
- Verify Docker has enough memory allocated
- Check logs: `docker-compose logs reth`

### Can't Connect to RPC
- Verify RETH is healthy: `docker-compose ps`
- Check if port 8545 is available: `netstat -an | grep 8545`
- Test with curl: `curl http://localhost:8545`

### Grafana Dashboard Empty
- Check Prometheus is collecting metrics: http://localhost:9090/targets
- Verify RETH metrics endpoint: http://localhost:9001/metrics
- Check Grafana datasource configuration

### Performance Issues
- Increase Docker memory limit to 8GB+
- Monitor host system resources
- Consider adjusting block time in docker-compose.yml

## 📚 Additional Resources

- [RETH Documentation](https://reth.rs/)
- [Ethereum JSON-RPC API](https://ethereum.org/en/developers/docs/apis/json-rpc/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Prometheus Documentation](https://prometheus.io/docs/)

## 🤝 Support

For issues specific to this setup, check the logs and verify all services are running. For RETH-specific issues, consult the official RETH documentation.