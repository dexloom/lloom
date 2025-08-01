# Troubleshooting

This guide helps diagnose and resolve common issues with Lloom nodes. It covers systematic debugging approaches, common problems and solutions, and tools for investigation.

## General Troubleshooting Approach

### 1. Identify the Problem

Start by gathering information:

```bash
# Check node status
lloom-client status
lloom-executor status
lloom-validator status

# View recent logs
journalctl -u lloom-executor -n 100 --no-pager
tail -f ~/.lloom/*/logs/*.log

# Check system resources
htop
nvidia-smi  # For GPU nodes
df -h       # Disk space
```

### 2. Enable Debug Logging

Increase log verbosity:

```bash
# Via environment variable
export RUST_LOG=lloom_executor=debug,lloom_core=trace

# Or in configuration
[logging]
level = "debug"
targets = ["lloom_executor::network", "lloom_core::protocol"]
```

### 3. Use Built-in Diagnostics

Run diagnostic commands:

```bash
# Network diagnostics
lloom-client diagnose network
lloom-executor diagnose --full

# Configuration validation
lloom-executor config validate
lloom-client config check
```

## Common Issues and Solutions

### Network Connectivity Issues

#### Problem: Cannot connect to bootstrap peers

**Symptoms:**
```
ERROR lloom_core::network: Failed to dial bootstrap peer: Transport error
WARN  lloom_core::network: No peers connected after 30 seconds
```

**Solutions:**

1. **Check network connectivity**:
   ```bash
   # Test bootstrap peer connectivity
   nc -zv bootstrap1.lloom.network 4001
   ping bootstrap1.lloom.network
   
   # Check local firewall
   sudo iptables -L -n | grep 4001
   sudo ufw status
   ```

2. **Verify listen address**:
   ```toml
   [network]
   # Ensure listening on all interfaces if needed
   listen_address = "/ip4/0.0.0.0/tcp/4001"
   ```

3. **Check NAT configuration**:
   ```toml
   [network]
   # Set external address for NAT
   external_address = "/ip4/YOUR_PUBLIC_IP/tcp/4001"
   enable_autonat = true
   enable_relay = true
   ```

4. **Try alternative bootstrap peers**:
   ```bash
   lloom-client --bootstrap-peer "/ip4/alternative-bootstrap.lloom.network/tcp/4001/p2p/12D3KooW..."
   ```

#### Problem: Peers disconnect frequently

**Symptoms:**
```
INFO  lloom_core::network: Peer connected: 12D3KooW...
WARN  lloom_core::network: Peer disconnected: 12D3KooW... (reason: Timeout)
```

**Solutions:**

1. **Increase timeout values**:
   ```toml
   [network.timeouts]
   connection_timeout_secs = 60
   request_timeout_secs = 300
   idle_timeout_secs = 900
   ```

2. **Check bandwidth**:
   ```bash
   # Monitor network usage
   iftop -i eth0
   nethogs
   ```

3. **Optimize connection pool**:
   ```toml
   [network.connection_pool]
   max_connections = 50  # Reduce if bandwidth limited
   max_connections_per_peer = 2
   ```

### LLM Backend Issues

#### Problem: LMStudio connection failed

**Symptoms:**
```
ERROR lloom_executor::llm_client: Failed to connect to LMStudio: Connection refused
```

**Solutions:**

1. **Verify LMStudio is running**:
   ```bash
   # Check if LMStudio server is listening
   lsof -i :1234
   curl http://localhost:1234/v1/models
   ```

2. **Start LMStudio server**:
   - Open LMStudio UI
   - Go to Server tab
   - Click "Start Server"
   - Verify port 1234 is shown

3. **Check configuration**:
   ```toml
   [llm_client]
   backend = "lmstudio"
   base_url = "http://localhost:1234/v1"  # Use 127.0.0.1 if localhost fails
   ```

4. **Test connection**:
   ```bash
   lloom-executor test-backend --verbose
   ```

#### Problem: Model not found

**Symptoms:**
```
ERROR lloom_executor: Model 'llama-2-13b' not found in backend
```

**Solutions:**

1. **List available models**:
   ```bash
   lloom-executor models list
   curl http://localhost:1234/v1/models | jq
   ```

2. **Download model in LMStudio**:
   - Open LMStudio
   - Go to Models tab
   - Search and download required model

3. **Use model discovery**:
   ```toml
   [model_discovery]
   enabled = true
   refresh_interval_secs = 300
   ```

4. **Specify exact model name**:
   ```bash
   # Get exact model ID from list
   lloom-client --model "TheBloke/Llama-2-13B-chat-GGUF/llama-2-13b-chat.Q4_K_M.gguf"
   ```

### Request Processing Issues

#### Problem: Requests timing out

**Symptoms:**
```
ERROR lloom_client: Request timeout after 120 seconds
```

**Solutions:**

1. **Increase timeout**:
   ```bash
   lloom-client --timeout 300 --prompt "..."
   
   # Or in config
   [defaults]
   request_timeout_secs = 300
   ```

2. **Check executor load**:
   ```bash
   # Monitor executor metrics
   curl http://executor:9092/metrics | grep queue_size
   ```

3. **Use streaming**:
   ```bash
   lloom-client --stream --prompt "..."
   ```

4. **Reduce max tokens**:
   ```bash
   lloom-client --max-tokens 500 --prompt "..."
   ```

#### Problem: Signature verification failed

**Symptoms:**
```
ERROR lloom_core::signing: Signature verification failed: Invalid signature
ERROR lloom_client: Response rejected: Invalid executor signature
```

**Solutions:**

1. **Check time synchronization**:
   ```bash
   # Ensure system time is accurate
   timedatectl status
   sudo ntpdate -s time.nist.gov
   ```

2. **Verify key configuration**:
   ```bash
   # Check key files exist and have correct permissions
   ls -la ~/.lloom/*/keys/
   stat ~/.lloom/executor/eth_key
   ```

3. **Regenerate keys if corrupted**:
   ```bash
   # Backup old keys first
   mv ~/.lloom/executor/eth_key ~/.lloom/executor/eth_key.backup
   lloom-executor generate-key
   ```

4. **Check for version mismatch**:
   ```bash
   lloom-client --version
   lloom-executor --version
   # Ensure compatible versions
   ```

### Resource Issues

#### Problem: Out of memory

**Symptoms:**
```
ERROR lloom_executor: Failed to load model: Out of memory
thread 'main' panicked at 'allocation failed'
```

**Solutions:**

1. **Check available memory**:
   ```bash
   free -h
   nvidia-smi  # For GPU memory
   ```

2. **Reduce model size or use quantized version**:
   ```toml
   [[models]]
   name = "llama-2-13b-chat.Q4_K_M.gguf"  # Use Q4 instead of Q8
   ```

3. **Limit concurrent requests**:
   ```toml
   [request_handling]
   max_concurrent_requests = 2  # Reduce from default
   ```

4. **Enable swap (temporary solution)**:
   ```bash
   sudo fallocate -l 16G /swapfile
   sudo chmod 600 /swapfile
   sudo mkswap /swapfile
   sudo swapon /swapfile
   ```

5. **Use memory limits**:
   ```toml
   [resources]
   max_memory_gb = 28  # Leave some for system
   enable_memory_limit = true
   ```

#### Problem: GPU not detected

**Symptoms:**
```
WARN  lloom_executor: No CUDA devices found, falling back to CPU
```

**Solutions:**

1. **Check GPU drivers**:
   ```bash
   nvidia-smi
   nvcc --version
   ```

2. **Install/update CUDA**:
   ```bash
   # Check CUDA installation
   ldconfig -p | grep cuda
   
   # Install if missing
   sudo apt update
   sudo apt install nvidia-cuda-toolkit
   ```

3. **Set environment variables**:
   ```bash
   export CUDA_VISIBLE_DEVICES=0,1
   export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH
   ```

4. **Check GPU allocation**:
   ```toml
   [gpu_allocation]
   devices = [0]  # Specify GPU index
   ```

### Validation Issues

#### Problem: High false positive rate

**Symptoms:**
```
WARN  lloom_validator: False positive rate 15% exceeds threshold 5%
```

**Solutions:**

1. **Adjust validation tolerances**:
   ```toml
   [validation.token_counting]
   tolerance_percent = 10  # Increase from 5%
   
   [validation.criteria]
   strict_mode = false
   ```

2. **Update validation model**:
   ```toml
   [llm_client]
   validation_model = "gpt-4"  # Use more accurate model
   ```

3. **Exclude problematic validators**:
   ```toml
   [validation]
   skip_validation_types = ["content_quality"]  # Temporarily disable
   ```

### Performance Issues

#### Problem: Slow request processing

**Symptoms:**
- High request latency
- Queue buildup
- Low throughput

**Solutions:**

1. **Profile the application**:
   ```bash
   # Enable profiling
   export LLOOM_PROFILE=true
   lloom-executor start
   
   # Analyze profile
   go tool pprof cpu.prof
   ```

2. **Optimize batching**:
   ```toml
   [batching]
   enabled = true
   max_batch_size = 8
   batch_timeout_ms = 50
   ```

3. **Enable caching**:
   ```toml
   [cache]
   enabled = true
   backend = "memory"
   max_size_mb = 1000
   ```

4. **Check disk I/O**:
   ```bash
   iostat -x 1
   iotop
   ```

## Advanced Debugging

### Packet Capture

Analyze network traffic:

```bash
# Capture P2P traffic
sudo tcpdump -i any -w lloom.pcap 'port 4001'

# Analyze with Wireshark
wireshark lloom.pcap
```

### Core Dumps

Enable core dumps for crash analysis:

```bash
# Enable core dumps
ulimit -c unlimited
echo "/tmp/core-%e-%p-%t" | sudo tee /proc/sys/kernel/core_pattern

# Run with core dump enabled
lloom-executor start

# Analyze core dump
gdb lloom-executor /tmp/core-lloom-executor-*
```

### Tracing

Enable detailed tracing:

```toml
[tracing]
enabled = true
backend = "jaeger"
endpoint = "http://localhost:14268/api/traces"
sampling_rate = 1.0  # Trace all requests for debugging
```

View traces in Jaeger UI: http://localhost:16686

### Memory Profiling

Track memory usage:

```bash
# Use heaptrack
heaptrack lloom-executor

# Analyze results
heaptrack --analyze heaptrack.lloom-executor.12345.gz
```

### Strace Analysis

Trace system calls:

```bash
# Trace file operations
strace -e trace=file lloom-executor 2>&1 | grep -E "(ENOENT|EACCES)"

# Trace network operations
strace -e trace=network lloom-executor 2>&1
```

## Emergency Procedures

### Node Hanging

If a node becomes unresponsive:

1. **Get thread dump**:
   ```bash
   kill -QUIT $(pgrep lloom-executor)
   # Check logs for thread dump
   ```

2. **Force restart**:
   ```bash
   systemctl restart lloom-executor
   # Or
   kill -9 $(pgrep lloom-executor)
   ```

3. **Clear state if corrupted**:
   ```bash
   # Backup first
   mv ~/.lloom/executor/state ~/.lloom/executor/state.backup
   lloom-executor start --clean-state
   ```

### Data Recovery

If data corruption occurs:

1. **Check filesystem**:
   ```bash
   sudo fsck -f /dev/sda1
   ```

2. **Restore from backup**:
   ```bash
   # Stop service
   systemctl stop lloom-executor
   
   # Restore data
   tar -xzf lloom-backup-20240115.tar.gz -C ~/.lloom/
   
   # Verify integrity
   lloom-executor verify-state
   ```

3. **Rebuild from network**:
   ```bash
   lloom-executor sync --full
   ```

## Getting Help

### Collect Diagnostic Information

When reporting issues, collect:

```bash
# Generate diagnostic bundle
lloom-executor diagnose --output diagnostic-bundle.tar.gz

# This includes:
# - Configuration (sanitized)
# - Recent logs
# - System information
# - Network status
# - Resource usage
```

### Community Support

1. **Discord**: Join #troubleshooting channel
2. **GitHub Issues**: Search existing issues first
3. **Stack Overflow**: Tag with `lloom`

### Information to Provide

When asking for help, include:

1. **Version information**:
   ```bash
   lloom-client --version
   lloom-executor --version
   rustc --version
   ```

2. **Error messages** (full output)

3. **Configuration** (remove sensitive data):
   ```bash
   cat ~/.lloom/executor/config.toml | grep -v "key\|token\|password"
   ```

4. **Steps to reproduce**

5. **What you've already tried**

### Log Analysis Tools

Useful commands for log analysis:

```bash
# Find errors in logs
grep -i error ~/.lloom/*/logs/*.log | tail -50

# Count occurrences
grep "pattern" logfile | sort | uniq -c | sort -nr

# Time-based analysis
awk '{print $1}' logfile | sort | uniq -c

# Extract stack traces
sed -n '/panic/,/^$/p' logfile
```

## Prevention Best Practices

1. **Regular Maintenance**:
   - Update software regularly
   - Monitor disk space
   - Review logs weekly

2. **Monitoring Setup**:
   - Configure alerts
   - Track key metrics
   - Set up health checks

3. **Backup Strategy**:
   - Automated daily backups
   - Test restore procedures
   - Off-site backup storage

4. **Documentation**:
   - Document custom configurations
   - Maintain runbooks
   - Track known issues

5. **Testing**:
   - Test updates in staging
   - Load test before scaling
   - Chaos engineering exercises