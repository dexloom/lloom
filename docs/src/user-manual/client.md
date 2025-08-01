# Running a Client

The Lloom client allows you to submit LLM requests to the decentralized network and receive responses from executors. This guide covers running, configuring, and using the client effectively.

## Quick Start

### Basic Usage

Run the client with a simple prompt:

```bash
lloom-client --prompt "Explain quantum computing in simple terms"
```

### Specify a Model

Request a specific model:

```bash
lloom-client --prompt "Write a haiku about rust programming" --model "gpt-4"
```

### Use System Prompts

Include a system prompt for better control:

```bash
lloom-client \
  --system-prompt "You are a helpful coding assistant" \
  --prompt "How do I implement a binary search tree in Rust?"
```

## Configuration

### Configuration File

Create a configuration file at `~/.lloom/client.toml`:

```toml
# Network configuration
[network]
listen_address = "/ip4/0.0.0.0/tcp/0"
bootstrap_peers = [
    "/ip4/bootstrap1.lloom.network/tcp/4001/p2p/12D3KooW...",
    "/ip4/bootstrap2.lloom.network/tcp/4001/p2p/12D3KooW..."
]

# Identity configuration
[identity]
private_key_path = "~/.lloom/client_key"

# Request defaults
[defaults]
model = "gpt-3.5-turbo"
max_tokens = 1000
temperature = 0.7
request_timeout_secs = 120

# Executor selection
[executor_selection]
strategy = "best_price"  # Options: best_price, fastest, specific
preferred_executors = []  # List of executor peer IDs
blacklist = []           # Executors to avoid

# Metrics and monitoring
[metrics]
enabled = true
endpoint = "0.0.0.0:9091"
```

### Environment Variables

Override configuration with environment variables:

```bash
export LLOOM_CLIENT_KEY_PATH="~/.lloom/my_client_key"
export LLOOM_BOOTSTRAP_PEERS="/ip4/localhost/tcp/4001/p2p/12D3KooW..."
export LLOOM_DEFAULT_MODEL="llama-2-70b"
```

### Command Line Options

All configuration can be overridden via CLI:

```bash
lloom-client \
  --config ~/.lloom/custom_config.toml \
  --listen-address "/ip4/0.0.0.0/tcp/5555" \
  --bootstrap-peer "/ip4/localhost/tcp/4001/p2p/12D3KooW..." \
  --private-key ~/.lloom/alt_key \
  --model "gpt-4" \
  --max-tokens 2000 \
  --temperature 0.5 \
  --prompt "Your prompt here"
```

## Executor Discovery and Selection

### Automatic Discovery

The client automatically discovers executors through:

1. **DHT Queries**: Searches the Kademlia DHT for executor announcements
2. **Gossipsub**: Subscribes to executor availability topics
3. **mDNS**: Discovers local network executors

### Manual Executor Selection

Specify a preferred executor:

```bash
lloom-client \
  --executor "12D3KooWQcD3cmHSXqHV2WpbDHDCZqhKUdVfTzQ5KjDa6EqnGcWz" \
  --prompt "Your prompt"
```

### Selection Strategies

Configure how the client chooses executors:

```toml
[executor_selection]
strategy = "best_price"

[executor_selection.weights]
price = 0.5
latency = 0.3
reliability = 0.2
```

Available strategies:
- `best_price`: Lowest cost per token
- `fastest`: Lowest latency
- `balanced`: Weighted combination
- `specific`: Use only specified executors

## Request Management

### Synchronous Requests

Default behavior waits for response:

```bash
lloom-client --prompt "Tell me a joke"
```

### Asynchronous Requests

Submit without waiting:

```bash
lloom-client --async --prompt "Generate a long story" --callback-url "http://localhost:8080/webhook"
```

### Batch Requests

Submit multiple prompts:

```bash
lloom-client --batch requests.json
```

Where `requests.json`:
```json
[
  {
    "prompt": "First question",
    "model": "gpt-3.5-turbo",
    "max_tokens": 500
  },
  {
    "prompt": "Second question",
    "model": "gpt-4",
    "max_tokens": 1000
  }
]
```

### Request Tracking

Track request status:

```bash
# Get request ID from submission
lloom-client --prompt "..." --async
# Output: Request ID: 0x1234...

# Check status
lloom-client status 0x1234...
```

## Advanced Features

### Streaming Responses

Enable streaming for real-time output:

```bash
lloom-client --prompt "Write a long essay" --stream
```

### Request Signing

All requests are automatically signed with EIP-712:

```bash
# View request commitment before sending
lloom-client --prompt "..." --dry-run

# Output:
# Request Commitment:
#   Executor: 0x1234...
#   Model: gpt-3.5-turbo
#   Prompt Hash: 0xabcd...
#   Max Tokens: 1000
#   Inbound Price: 0.00001 ETH/token
#   Outbound Price: 0.00002 ETH/token
#   Signature: 0x5678...
```

### Price Limits

Set maximum acceptable prices:

```bash
lloom-client \
  --prompt "..." \
  --max-inbound-price "0.00001" \
  --max-outbound-price "0.00002"
```

### Request Priority

Set priority for urgent requests:

```bash
lloom-client --prompt "..." --priority high --max-price "0.001"
```

## Response Handling

### Response Verification

The client automatically verifies:
1. Executor's EIP-712 signature
2. Response matches request
3. Token usage is within limits
4. Content hash matches

### Output Formats

Choose output format:

```bash
# Plain text (default)
lloom-client --prompt "..." --format text

# JSON with metadata
lloom-client --prompt "..." --format json

# Markdown
lloom-client --prompt "..." --format markdown
```

### Saving Responses

Save to file:

```bash
lloom-client --prompt "..." --output response.txt

# Append to file
lloom-client --prompt "..." --append-output responses.log
```

## Error Handling

### Common Errors

1. **No Executors Found**
   ```
   Error: No executors available for model 'gpt-4'
   Solution: Try a different model or wait for executors to come online
   ```

2. **Request Timeout**
   ```
   Error: Request timed out after 120 seconds
   Solution: Increase timeout with --timeout 300
   ```

3. **Price Too High**
   ```
   Error: Executor price (0.0001 ETH/token) exceeds limit (0.00005 ETH/token)
   Solution: Increase price limit or find cheaper executor
   ```

4. **Signature Verification Failed**
   ```
   Error: Invalid executor signature
   Solution: Report executor, client will blacklist automatically
   ```

### Retry Configuration

Configure automatic retries:

```toml
[retry]
max_attempts = 3
initial_delay_ms = 1000
max_delay_ms = 30000
exponential_base = 2.0
```

## Monitoring and Metrics

### Prometheus Metrics

When metrics are enabled, access them at `http://localhost:9091/metrics`:

```
# Request metrics
lloom_client_requests_total{status="success"} 42
lloom_client_requests_total{status="failed"} 3
lloom_client_request_duration_seconds{quantile="0.99"} 2.5

# Token usage
lloom_client_tokens_used_total{type="inbound"} 15234
lloom_client_tokens_used_total{type="outbound"} 28456

# Cost tracking
lloom_client_cost_total{currency="ETH"} 0.0234
```

### Logging

Configure logging verbosity:

```bash
# Set log level
export RUST_LOG=lloom_client=debug

# Or via CLI
lloom-client --log-level debug --prompt "..."
```

### Request History

View recent requests:

```bash
# List recent requests
lloom-client history

# Show specific request details
lloom-client show-request 0x1234...
```

## Security Best Practices

### Key Management

1. **Secure Storage**: Keep private keys encrypted
   ```bash
   # Generate encrypted key
   lloom-client generate-key --password
   ```

2. **Key Rotation**: Regularly rotate keys
   ```bash
   lloom-client rotate-key --old-key ~/.lloom/old_key
   ```

3. **Hardware Wallets**: Use hardware wallet integration
   ```bash
   lloom-client --wallet-type ledger --prompt "..."
   ```

### Network Security

1. **TLS for API Calls**: Always use HTTPS for callbacks
2. **Firewall Rules**: Restrict metrics endpoint access
3. **Peer Verification**: Validate executor identities

### Data Privacy

1. **Local Prompt Storage**: Prompts are only stored locally
2. **Hash Verification**: Only hashes go on-chain
3. **Encrypted Transport**: All P2P communication is encrypted

## Performance Tuning

### Connection Pool

Optimize network connections:

```toml
[network.connection_pool]
max_connections = 50
idle_timeout_secs = 300
connection_timeout_secs = 10
```

### Caching

Enable response caching:

```toml
[cache]
enabled = true
max_size_mb = 100
ttl_seconds = 3600
```

### Parallel Requests

Configure parallelism:

```toml
[performance]
max_concurrent_requests = 10
request_buffer_size = 100
```

## Troubleshooting

### Debug Mode

Enable detailed debugging:

```bash
lloom-client --debug --prompt "..."
```

This shows:
- Network discovery process
- Executor selection logic
- Request/response details
- Signature verification steps

### Common Issues

1. **Slow Network Discovery**
   - Check bootstrap peer connectivity
   - Verify firewall allows P2P traffic
   - Try alternative bootstrap peers

2. **High Latency**
   - Select geographically closer executors
   - Check network bandwidth
   - Consider local executor setup

3. **Frequent Timeouts**
   - Increase timeout values
   - Check executor reliability scores
   - Use retry configuration

### Getting Help

```bash
# Show help
lloom-client --help

# Show version and build info
lloom-client --version --verbose

# Run diagnostic checks
lloom-client diagnose
```

## Integration Examples

### Python Script

```python
import subprocess
import json

def query_lloom(prompt, model="gpt-3.5-turbo"):
    result = subprocess.run([
        "lloom-client",
        "--prompt", prompt,
        "--model", model,
        "--format", "json"
    ], capture_output=True, text=True)
    
    if result.returncode == 0:
        return json.loads(result.stdout)
    else:
        raise Exception(f"Lloom error: {result.stderr}")

# Use it
response = query_lloom("What is the capital of France?")
print(response['content'])
```

### Shell Script

```bash
#!/bin/bash

# Function to query Lloom
query_lloom() {
    local prompt="$1"
    local model="${2:-gpt-3.5-turbo}"
    
    lloom-client \
        --prompt "$prompt" \
        --model "$model" \
        --format text
}

# Use in script
RESULT=$(query_lloom "Generate a random number between 1 and 100")
echo "Lloom says: $RESULT"
```

### Continuous Integration

```yaml
# .github/workflows/lloom-test.yml
name: Test with Lloom

on: [push]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Install Lloom Client
        run: |
          curl -L https://github.com/lloom/lloom/releases/latest/download/lloom-client-linux-amd64 -o lloom-client
          chmod +x lloom-client
          sudo mv lloom-client /usr/local/bin/
      
      - name: Test Documentation
        run: |
          lloom-client \
            --prompt "Review this documentation and suggest improvements" \
            --model "gpt-4" \
            < README.md
```