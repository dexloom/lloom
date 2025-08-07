# Model Announcement System Testing

This directory contains comprehensive testing scripts and examples for the model announcement system. The system allows executors to announce their available models to validators, enabling clients to discover and query models dynamically.

## Overview

The model announcement system consists of three main components:

1. **Validator** - Central hub that receives model announcements and provides discovery API
2. **Executor** - Announces available models and sends periodic heartbeats  
3. **Client** - Discovers and queries available models

## Scripts

### Main Test Scripts

#### `test-model-announcement.sh` 
**Comprehensive End-to-End Test Suite**

Runs a complete test of the model announcement system covering:
- Validator startup with model announcement subscription
- Executor startup with model announcements
- Client model discovery and querying
- Heartbeat mechanism verification
- Stale executor detection
- Model updates and removal

```bash
# Run the full test suite
./scripts/test-model-announcement.sh

# Run with custom timeout
TEST_TIMEOUT=60 ./scripts/test-model-announcement.sh

# Keep processes running after test (for debugging)
CLEANUP_ON_EXIT=false ./scripts/test-model-announcement.sh
```

#### `run-system-with-announcements.sh`
**Complete System Startup with Model Announcements**

Starts the entire lloom system with model announcement features enabled:

```bash
# Start system and discover models
./scripts/run-system-with-announcements.sh

# Start system and query specific model
CLIENT_MODE=query QUERY_MODEL='gpt-4' ./scripts/run-system-with-announcements.sh

# Start system and list all models
CLIENT_MODE=list ./scripts/run-system-with-announcements.sh

# Run system in background (no auto-cleanup)
CLEANUP_ON_EXIT=false ./scripts/run-system-with-announcements.sh
```

#### `demo-model-announcement-features.sh`
**Interactive Feature Demo**

Step-by-step demonstration of each model announcement feature:

```bash
./scripts/demo-model-announcement-features.sh
```

This script walks through:
- Building the system
- Starting validator with announcements
- Starting executor with model announcements  
- Client discovery and querying
- Integration testing
- Log examination
- Full end-to-end testing

### Updated Component Scripts

The existing `.devops/` scripts have been enhanced with model announcement support:

#### `.devops/validator/run-validator.sh`
Enhanced with model announcement subscription:

```bash
# Start validator with default settings
./.devops/validator/run-validator.sh

# Start validator with custom port and announcements enabled
VALIDATOR_PORT=9090 SUBSCRIBE_ANNOUNCEMENTS=true ./.devops/validator/run-validator.sh

# Start with debug logging
LOG_LEVEL=debug ./.devops/validator/run-validator.sh
```

#### `.devops/executor/run-executor.sh` 
Enhanced with model announcement capabilities:

```bash
# Start executor with default settings
./.devops/executor/run-executor.sh

# Start executor with custom configuration
EXECUTOR_PORT=9091 VALIDATOR_ADDRESS="127.0.0.1:9090" ANNOUNCE_MODELS=true ./.devops/executor/run-executor.sh

# Start with custom heartbeat interval
HEARTBEAT_INTERVAL=5 ./.devops/executor/run-executor.sh

# Start with specific config file
CONFIG_FILE="my-config.toml" ./.devops/executor/run-executor.sh
```

#### `.devops/client/run-client-local.sh`
Enhanced with model discovery options:

```bash
# Discover all available models
DISCOVER_MODELS=true ./.devops/client/run-client-local.sh

# Query for a specific model
QUERY_MODEL='gpt-3.5-turbo' ./.devops/client/run-client-local.sh

# List all models with status
LIST_MODELS=true ./.devops/client/run-client-local.sh

# Use custom validator address
VALIDATOR_ADDRESS="192.168.1.100:8080" DISCOVER_MODELS=true ./.devops/client/run-client-local.sh
```

## Examples

### Integration Test Example

`crates/lloom-executor/examples/test_model_announcement.rs` provides a comprehensive integration test that demonstrates:

- Model announcement manager initialization
- Announcing initial models
- Starting heartbeat mechanism
- Updating model information
- Adding new models dynamically
- Load and capacity management
- Error handling
- Clean shutdown with model removal

Run it with:
```bash
cd crates/lloom-executor
cargo run --example test_model_announcement
```

## Configuration

The test scripts use several environment variables for configuration:

### Validator Configuration
- `VALIDATOR_PORT` - Port for validator (default: 8080)
- `SUBSCRIBE_ANNOUNCEMENTS` - Enable model announcement subscription (default: true)

### Executor Configuration  
- `EXECUTOR_PORT` - Port for executor (default: 8081)
- `VALIDATOR_ADDRESS` - Validator address to connect to (default: "127.0.0.1:8080")
- `ANNOUNCE_MODELS` - Enable model announcements (default: true)
- `HEARTBEAT_INTERVAL` - Heartbeat interval in seconds (default: 10)
- `CONFIG_FILE` - Configuration file path

### Client Configuration
- `VALIDATOR_ADDRESS` - Validator address for discovery (default: "127.0.0.1:8080")
- `DISCOVER_MODELS` - Enable model discovery (default: false)
- `QUERY_MODEL` - Specific model to query
- `LIST_MODELS` - List all models (default: false)

### General Configuration
- `LOG_LEVEL` - Logging level (default: info)
- `CLEANUP_ON_EXIT` - Clean up processes on exit (default: true)
- `TEST_TIMEOUT` - Timeout for client operations (default: 30)

## Test Scenarios Covered

The test suite verifies the following scenarios:

1. **Model Announcement Flow**
   - ✅ Executor announces models on startup
   - ✅ Validator receives and stores model information
   - ✅ Multiple executors can announce different models

2. **Model Discovery**
   - ✅ Client can discover all available models
   - ✅ Client can query for specific models by name
   - ✅ Client can list models with their current status

3. **Heartbeat and Health**
   - ✅ Executors send periodic heartbeats
   - ✅ Validator tracks executor health
   - ✅ Stale executors are detected and removed

4. **Dynamic Updates**
   - ✅ Model information can be updated at runtime
   - ✅ New models can be added dynamically
   - ✅ Models can be removed cleanly

5. **Error Handling**
   - ✅ Invalid model announcements are rejected
   - ✅ Network failures are handled gracefully
   - ✅ System recovers from temporary outages

6. **Load Management**
   - ✅ Model load and capacity are tracked
   - ✅ Overloaded models are identified
   - ✅ Load balancing information is available

## Troubleshooting

### Common Issues

1. **Port already in use**
   - Change ports using environment variables
   - Kill existing processes: `pkill -f lloom`

2. **Build failures**
   - Ensure Rust is installed and up to date
   - Run `cargo clean` and rebuild

3. **Connection timeouts**
   - Check firewall settings
   - Verify services are running: `netstat -tlnp | grep 808`

4. **Model announcements not received**
   - Check validator logs for subscription status
   - Verify executor is configured to announce models
   - Ensure network connectivity between components

### Debug Mode

Run any script with debug logging:
```bash
LOG_LEVEL=debug ./scripts/test-model-announcement.sh
```

### Manual Testing

For manual testing and debugging:

1. Start validator:
   ```bash
   VALIDATOR_PORT=8080 SUBSCRIBE_ANNOUNCEMENTS=true ./.devops/validator/run-validator.sh
   ```

2. In another terminal, start executor:
   ```bash
   EXECUTOR_PORT=8081 ANNOUNCE_MODELS=true ./.devops/executor/run-executor.sh  
   ```

3. In a third terminal, test client:
   ```bash
   DISCOVER_MODELS=true ./.devops/client/run-client-local.sh
   ```

## Expected Output

When running the test suite successfully, you should see:

- ✅ Validator started and ready
- ✅ Executor started and announced models  
- ✅ Validator received model announcements
- ✅ Client successfully discovered models
- ✅ Client successfully queried specific model
- ✅ Executor heartbeat detected
- ✅ Validator processed heartbeat
- ✅ Validator detected stale executor (after timeout)

The system is working correctly when all major test scenarios pass.

## Integration with CI/CD

The test scripts are designed to be used in automated testing environments:

```bash
# In CI pipeline
./scripts/test-model-announcement.sh || exit 1
```

For containerized environments, ensure proper network configuration and port availability.