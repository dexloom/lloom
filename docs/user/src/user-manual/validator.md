# Running a Validator

Validators play a crucial role in the Lloom network by verifying the integrity of LLM request-response pairs and ensuring executors provide honest service. This guide covers operating a validator node.

## Overview

Validators:
- Monitor request-response transactions on the network
- Verify EIP-712 signatures from both clients and executors
- Validate response quality and accuracy
- Report violations and maintain network integrity
- Earn rewards for successful validations

## Quick Start

### Basic Setup

1. **Install validator binary**:
   ```bash
   cargo install lloom-validator
   # Or download pre-built binary
   ```

2. **Initialize configuration**:
   ```bash
   lloom-validator init
   # Creates config at ~/.lloom/validator/config.toml
   ```

3. **Start validator**:
   ```bash
   lloom-validator start
   ```

## Configuration

### Complete Configuration

```toml
# Network configuration
[network]
listen_address = "/ip4/0.0.0.0/tcp/4002"
bootstrap_peers = [
    "/ip4/bootstrap1.lloom.network/tcp/4001/p2p/12D3KooW...",
    "/ip4/bootstrap2.lloom.network/tcp/4001/p2p/12D3KooW..."
]

# Identity and keys
[identity]
private_key_path = "~/.lloom/validator/identity_key"
ethereum_private_key_path = "~/.lloom/validator/eth_key"

# Validation settings
[validation]
# Sampling rate (0.0-1.0)
sampling_rate = 0.1  # Validate 10% of transactions
validation_timeout_secs = 300
max_concurrent_validations = 20
validation_strategy = "weighted_random"  # Options: random, weighted_random, targeted

# Validation criteria
[validation.criteria]
verify_signatures = true
check_token_counts = true
validate_model_claims = true
content_quality_check = false  # Requires LLM access
response_relevance_check = false  # Requires LLM access

# LLM access for deep validation (optional)
[llm_client]
enabled = false  # Set to true for content validation
backend = "openai"
base_url = "https://api.openai.com/v1"
api_key = "${OPENAI_API_KEY}"
validation_model = "gpt-3.5-turbo"

# Reporting configuration
[reporting]
# Where to report violations
report_endpoint = "https://registry.lloom.network/api/v1/violations"
report_batch_size = 10
report_interval_secs = 60

# Local storage
[storage]
database_path = "~/.lloom/validator/validator.db"
max_storage_gb = 50
retention_days = 30

# Blockchain integration
[blockchain]
enabled = false  # Enable for on-chain reporting
rpc_url = "http://localhost:8545"
chain_id = 1
registry_contract = "0x..."
stake_amount_eth = "1.0"

# Metrics
[metrics]
enabled = true
endpoint = "0.0.0.0:9093"

# Logging
[logging]
level = "info"
file = "~/.lloom/validator/validator.log"
```

## Validation Strategies

### Random Sampling

Validate a random percentage of transactions:

```toml
[validation]
validation_strategy = "random"
sampling_rate = 0.1  # 10% of all transactions
```

### Weighted Random

Focus on high-value or suspicious transactions:

```toml
[validation]
validation_strategy = "weighted_random"

[validation.weights]
# Higher weight = more likely to validate
high_value_weight = 5.0      # Transactions > 0.1 ETH
new_executor_weight = 3.0    # First 100 transactions
flagged_executor_weight = 10.0  # Previously flagged
normal_weight = 1.0
```

### Targeted Validation

Focus on specific executors or patterns:

```toml
[validation]
validation_strategy = "targeted"

[validation.targets]
# Always validate these executors
executor_addresses = [
    "0x1234...",
    "0x5678..."
]

# Always validate these models
models = ["gpt-4", "claude-2"]

# Pattern matching
patterns = [
    { field = "token_count", operator = ">", value = 4000 },
    { field = "price_per_token", operator = ">", value = 0.0001 }
]
```

## Validation Process

### Signature Verification

Validators verify dual signatures:

1. **Client Signature**: 
   - Verify request commitment signature
   - Ensure client authorized the request
   - Check nonce and deadline

2. **Executor Signature**:
   - Verify response commitment signature
   - Ensure executor processed request
   - Validate claimed token usage

### Token Count Validation

Verify reported token counts:

```toml
[validation.token_counting]
enabled = true
tolerance_percent = 5  # Allow 5% deviation
use_tiktoken = true   # For OpenAI models
fallback_method = "estimate"  # When exact counting unavailable
```

### Model Verification

Ensure executor used claimed model:

```toml
[validation.model_verification]
enabled = true
methods = ["signature_analysis", "response_pattern", "api_headers"]
confidence_threshold = 0.8
```

### Content Quality Validation

Optional deep validation using LLM:

```toml
[validation.content_quality]
enabled = true
checks = [
    "response_relevance",
    "instruction_following",
    "factual_accuracy",
    "safety_compliance"
]

# Prompts for validation
[validation.content_quality.prompts]
relevance = """
Given this request: {request}
And this response: {response}
Rate the relevance from 0-10 and explain.
"""
```

## Reporting Violations

### Violation Types

```toml
[violations]
# Severity levels and actions
signature_mismatch = { severity = "critical", action = "immediate_report" }
token_count_mismatch = { severity = "high", action = "batch_report" }
model_mismatch = { severity = "high", action = "batch_report" }
quality_below_threshold = { severity = "medium", action = "accumulate" }
timeout = { severity = "low", action = "log_only" }
```

### Report Format

Violations are reported in standardized format:

```json
{
  "violation_type": "token_count_mismatch",
  "severity": "high",
  "transaction_id": "0xabc123...",
  "executor_address": "0x1234...",
  "client_address": "0x5678...",
  "evidence": {
    "claimed_tokens": 1500,
    "actual_tokens": 2100,
    "deviation_percent": 40
  },
  "validator_signature": "0xdef456...",
  "timestamp": 1704067200
}
```

### Evidence Collection

Configure evidence storage:

```toml
[evidence]
store_full_content = false  # Privacy: only store hashes
store_duration_days = 90
compression = "zstd"
encryption_enabled = true
encryption_key_path = "~/.lloom/validator/evidence_key"
```

## Staking and Rewards

### Validator Staking

Stake tokens to become active validator:

```toml
[staking]
enabled = true
stake_amount_eth = "1.0"
auto_restake_rewards = true
minimum_stake_duration_days = 30
withdrawal_delay_days = 7
```

### Reward Distribution

Earn rewards for successful validations:

```toml
[rewards]
# Reward calculation
base_reward_per_validation = "0.0001"  # ETH
quality_bonus_multiplier = 1.5
false_positive_penalty = "0.001"
missed_violation_penalty = "0.01"

# Claiming settings
auto_claim = true
claim_threshold_eth = "0.1"
claim_gas_price_gwei = 20
```

## Performance Optimization

### Batch Processing

Process validations in batches:

```toml
[performance]
batch_size = 50
batch_timeout_ms = 1000
parallel_validations = 10
queue_size = 1000
```

### Caching

Cache validation results:

```toml
[cache]
enabled = true
backend = "rocksdb"
max_entries = 100000
ttl_seconds = 3600
bloom_filter_enabled = true
```

### Resource Management

Control resource usage:

```toml
[resources]
max_memory_mb = 4096
max_cpu_percent = 50
io_priority = "low"  # Don't interfere with system
nice_level = 10
```

## Monitoring

### Metrics

Available at `http://localhost:9093/metrics`:

```prometheus
# Validation metrics
lloom_validator_validations_total{result="pass"} 8234
lloom_validator_validations_total{result="fail"} 156
lloom_validator_validation_duration_seconds{quantile="0.99"} 2.3

# Violation metrics
lloom_validator_violations_detected_total{type="signature_mismatch"} 12
lloom_validator_violations_reported_total 168
lloom_validator_false_positives_total 2

# Reward metrics
lloom_validator_rewards_earned_total{currency="ETH"} 0.823
lloom_validator_penalties_total{currency="ETH"} 0.002

# Network metrics
lloom_validator_transactions_observed_total 82340
lloom_validator_peer_count 127
```

### Alerting

Configure alerts:

```toml
[alerting]
enabled = true

[[alerts]]
name = "high_violation_rate"
condition = "violation_rate > 0.05"  # More than 5% violations
severity = "critical"
notification = "webhook"

[[alerts]]
name = "low_validation_rate"
condition = "validations_per_minute < 10"
severity = "warning"
notification = "log"

[alerting.webhook]
url = "https://alerts.example.com/webhook"
auth_token = "${ALERT_WEBHOOK_TOKEN}"
```

## High Availability

### Redundancy

Run multiple validators:

```toml
[ha]
enabled = true
mode = "active-active"  # All validators active
coordinator_endpoint = "http://coordinator:8080"
heartbeat_interval_secs = 10

# Deduplication
[ha.deduplication]
enabled = true
backend = "redis"
redis_url = "redis://localhost:6379"
dedup_window_secs = 300
```

### State Synchronization

Sync state between validators:

```toml
[state_sync]
enabled = true
peers = [
    "validator2.example.com:7000",
    "validator3.example.com:7000"
]
sync_interval_secs = 60
conflict_resolution = "most_recent"
```

## Security

### Access Control

Secure validator endpoints:

```toml
[api]
enabled = true
listen_address = "127.0.0.1:8081"
auth_required = true
auth_token = "${VALIDATOR_API_TOKEN}"
allowed_ips = ["127.0.0.1", "10.0.0.0/8"]
```

### Validation Signing

Sign all validation reports:

```toml
[signing]
sign_all_reports = true
signature_algorithm = "secp256k1"
include_timestamp = true
include_nonce = true
```

### Anti-Gaming Measures

Prevent validation gaming:

```toml
[anti_gaming]
# Randomize validation timing
random_delay_ms = { min = 0, max = 5000 }

# Hide validation targets
obfuscate_targets = true

# Validate own test transactions
honeypot_enabled = true
honeypot_frequency = 0.01  # 1% of validations
```

## Troubleshooting

### Common Issues

1. **Low Validation Rate**
   ```
   Issue: Validator processing few transactions
   
   Check:
   - Network connectivity to peers
   - Sampling rate configuration
   - Resource limits
   
   Solution:
   - Increase sampling_rate
   - Add more bootstrap peers
   - Increase resource limits
   ```

2. **High False Positive Rate**
   ```
   Issue: Incorrectly flagging valid transactions
   
   Check:
   - Token counting accuracy
   - Validation criteria thresholds
   - LLM model for content validation
   
   Solution:
   - Increase tolerance thresholds
   - Update validation logic
   - Use better validation model
   ```

3. **Staking Issues**
   ```
   Issue: Cannot stake or claim rewards
   
   Check:
   - Wallet balance
   - Gas price settings
   - Contract connection
   
   Solution:
   - Ensure sufficient ETH for gas
   - Increase gas price
   - Verify RPC endpoint
   ```

### Debug Mode

Enable detailed debugging:

```bash
RUST_LOG=lloom_validator=trace lloom-validator start --debug
```

Shows:
- Each validation step
- Decision making process
- Network communication
- Report generation

### Diagnostic Tools

```bash
# Check validator status
lloom-validator status

# Test validation logic
lloom-validator test-validate --file sample_transaction.json

# Benchmark performance
lloom-validator benchmark

# Check earnings
lloom-validator rewards summary

# Export validation history
lloom-validator export --from 2024-01-01 --to 2024-01-31
```

## Best Practices

### Operational

1. **Regular Monitoring**
   - Set up alerts for anomalies
   - Review validation accuracy weekly
   - Monitor resource usage

2. **Security**
   - Secure private keys
   - Use hardware security modules
   - Regular security audits

3. **Performance**
   - Tune batch sizes for throughput
   - Use SSD for database storage
   - Monitor network latency

### Economic

1. **Staking Strategy**
   - Stake enough for good rewards
   - Don't over-stake unnecessarily
   - Monitor ROI regularly

2. **Validation Strategy**
   - Balance thoroughness with cost
   - Focus on high-value transactions
   - Avoid over-validation

3. **Risk Management**
   - Set aside funds for penalties
   - Maintain good false positive rate
   - Regular strategy review

## Advanced Features

### Custom Validation Rules

Add domain-specific validation:

```toml
[[custom_rules]]
name = "medical_accuracy"
enabled = true
applies_to = { model = "med-llm-*" }
validator_script = "/usr/lib/lloom/validators/medical.wasm"
weight = 2.0

[[custom_rules]]
name = "code_execution"
enabled = true
applies_to = { prompt_contains = ["code", "function", "implement"] }
validator_script = "/usr/lib/lloom/validators/code.wasm"
timeout_secs = 60
```

### Machine Learning Integration

Use ML for anomaly detection:

```toml
[ml_detection]
enabled = true
model_path = "/usr/lib/lloom/models/anomaly_detector.onnx"
features = [
    "token_ratio",
    "response_time",
    "price_deviation",
    "executor_history"
]
threshold = 0.95
update_model_daily = true
```

### Integration with External Systems

```toml
[[integrations]]
name = "splunk"
type = "syslog"
endpoint = "splunk.example.com:514"
format = "json"
events = ["violations", "rewards"]

[[integrations]]
name = "datadog"
type = "statsd"
endpoint = "localhost:8125"
prefix = "lloom.validator"
tags = ["env:prod", "validator:1"]
```