# Testing

This guide covers the comprehensive testing strategy for the Lloom project, including unit tests, integration tests, end-to-end tests, and performance benchmarks.

## Testing Philosophy

The Lloom project follows these testing principles:

1. **Test-Driven Development**: Write tests before implementation
2. **Comprehensive Coverage**: Aim for >80% code coverage
3. **Fast Feedback**: Tests should run quickly
4. **Isolation**: Tests should not depend on external services
5. **Determinism**: Tests should produce consistent results

## Running Tests

### Basic Commands

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p lloom-core

# Run tests with output displayed
cargo test -- --nocapture

# Run a specific test
cargo test test_signature_verification

# Run tests matching a pattern
cargo test network::
```

### Test Profiles

```bash
# Run only unit tests (fast)
cargo test --lib

# Run only integration tests
cargo test --test '*'

# Run only doc tests
cargo test --doc

# Run benchmarks
cargo bench
```

### Parallel Testing

```bash
# Install nextest for better test runner
cargo install cargo-nextest

# Run tests with nextest
cargo nextest run

# Run with specific parallelism
cargo nextest run -j 4

# Run with test partitioning
cargo nextest run --partition count:1/2
```

## Unit Testing

### Basic Unit Test

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_creation() {
        let request = LlmRequest {
            model: "gpt-3.5-turbo".to_string(),
            prompt: "Hello, world!".to_string(),
            max_tokens: Some(100),
            // ... other fields
        };

        assert_eq!(request.model, "gpt-3.5-turbo");
        assert_eq!(request.max_tokens, Some(100));
    }
}
```

### Async Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn test_async_signing() {
        let identity = Identity::generate();
        let message = "test message";
        
        let signed = sign_message(message, &identity.wallet).await.unwrap();
        
        assert_eq!(signed.signer, identity.evm_address);
        assert!(verify_signature(&signed).unwrap());
    }
}
```

### Property-Based Testing

Using proptest for property-based testing:

```rust
#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_serialization_roundtrip(
            model in "[a-z]{5,20}",
            prompt in "[a-zA-Z ]{10,100}",
            max_tokens in 1u32..4096,
        ) {
            let request = LlmRequest {
                model,
                prompt,
                max_tokens: Some(max_tokens),
                // ... other fields
            };

            let serialized = serde_json::to_string(&request).unwrap();
            let deserialized: LlmRequest = serde_json::from_str(&serialized).unwrap();
            
            assert_eq!(request.model, deserialized.model);
            assert_eq!(request.prompt, deserialized.prompt);
            assert_eq!(request.max_tokens, deserialized.max_tokens);
        }
    }
}
```

### Mock Testing

Using mockall for mocking:

```rust
#[cfg(test)]
mod tests {
    use mockall::*;

    #[automock]
    trait LlmClient {
        async fn complete(&self, request: LlmRequest) -> Result<LlmResponse>;
    }

    #[tokio::test]
    async fn test_executor_with_mock() {
        let mut mock_client = MockLlmClient::new();
        
        mock_client
            .expect_complete()
            .returning(|_| Ok(LlmResponse {
                content: "Mocked response".to_string(),
                total_tokens: 10,
                // ... other fields
            }));

        let executor = Executor::with_client(Box::new(mock_client));
        let response = executor.process_request(test_request()).await.unwrap();
        
        assert_eq!(response.content, "Mocked response");
    }
}
```

## Integration Testing

### Network Integration Tests

Create integration tests in `tests/` directory:

```rust
// tests/network_integration.rs
use lloom_core::{Identity, LloomBehaviour};
use libp2p::{Swarm, SwarmBuilder};

#[tokio::test]
async fn test_peer_discovery() {
    // Create two nodes
    let mut node1 = create_test_node().await;
    let mut node2 = create_test_node().await;
    
    // Connect nodes
    let node2_addr = node2.listen_address();
    node1.swarm.dial(node2_addr).unwrap();
    
    // Wait for connection
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Verify connection
    assert_eq!(node1.swarm.connected_peers().count(), 1);
    assert_eq!(node2.swarm.connected_peers().count(), 1);
}

async fn create_test_node() -> TestNode {
    let identity = Identity::generate();
    let behaviour = LloomBehaviour::new(&identity).unwrap();
    let swarm = SwarmBuilder::with_tokio_executor(
        libp2p::tcp::tokio::Transport::default(),
        behaviour,
        identity.peer_id,
    ).build();
    
    TestNode { swarm, identity }
}
```

### Database Integration Tests

```rust
// tests/database_integration.rs
use lloom_validator::EvidenceStore;
use tempfile::TempDir;

#[tokio::test]
async fn test_evidence_storage() {
    let temp_dir = TempDir::new().unwrap();
    let store = EvidenceStore::new(temp_dir.path()).unwrap();
    
    // Store evidence
    let evidence = create_test_evidence();
    store.store(&evidence).await.unwrap();
    
    // Query evidence
    let results = store.query(
        EvidenceQuery::new()
            .executor(evidence.executor)
            .after(Utc::now() - Duration::from_hours(1))
    ).await.unwrap();
    
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, evidence.id);
}
```

## End-to-End Testing

### Full System Test

```rust
// tests/e2e.rs
use lloom_test_utils::{TestNetwork, TestClient, TestExecutor};

#[tokio::test]
async fn test_full_request_flow() {
    // Start test network
    let mut network = TestNetwork::new()
        .with_bootstrap_nodes(2)
        .with_executors(3)
        .with_validators(1)
        .start()
        .await
        .unwrap();
    
    // Create client
    let client = network.create_client().await.unwrap();
    
    // Make request
    let response = client
        .complete("What is the capital of France?")
        .await
        .unwrap();
    
    // Verify response
    assert!(response.content.contains("Paris"));
    assert!(response.total_tokens > 0);
    
    // Check validator caught the transaction
    let validator_logs = network.validator_logs().await;
    assert!(validator_logs.contains("Transaction validated"));
}
```

### Docker-based E2E Tests

```bash
# Run E2E tests with Docker Compose
docker-compose -f docker-compose.test.yml up --abort-on-container-exit
```

```yaml
# docker-compose.test.yml
version: '3.8'

services:
  bootstrap:
    image: lloom-validator:test
    command: ["--bootstrap", "--test-mode"]
    
  executor:
    image: lloom-executor:test
    depends_on:
      - bootstrap
    environment:
      - LLOOM_TEST_MODE=true
      - LLOOM_BOOTSTRAP_PEERS=/dns4/bootstrap/tcp/4001
      
  client:
    image: lloom-client:test
    depends_on:
      - executor
    command: ["test", "e2e"]
    environment:
      - LLOOM_BOOTSTRAP_PEERS=/dns4/bootstrap/tcp/4001
```

## Performance Testing

### Benchmarks

Create benchmarks in `benches/` directory:

```rust
// benches/signing_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lloom_core::{Identity, signing::sign_message};

fn bench_signing(c: &mut Criterion) {
    let identity = Identity::generate();
    let message = "benchmark message";
    
    c.bench_function("sign_message", |b| {
        b.iter(|| {
            let _ = sign_message(black_box(message), &identity.wallet);
        });
    });
}

fn bench_verification(c: &mut Criterion) {
    let identity = Identity::generate();
    let signed = sign_message("test", &identity.wallet).unwrap();
    
    c.bench_function("verify_signature", |b| {
        b.iter(|| {
            let _ = verify_signature(black_box(&signed));
        });
    });
}

criterion_group!(benches, bench_signing, bench_verification);
criterion_main!(benches);
```

Run benchmarks:

```bash
cargo bench

# Run specific benchmark
cargo bench signing

# Save baseline
cargo bench -- --save-baseline main

# Compare with baseline
cargo bench -- --baseline main
```

### Load Testing

```rust
// tests/load_test.rs
use tokio::time::Instant;

#[tokio::test]
async fn test_executor_load() {
    let executor = create_test_executor().await;
    let start = Instant::now();
    let num_requests = 1000;
    
    // Send concurrent requests
    let mut handles = vec![];
    for i in 0..num_requests {
        let executor = executor.clone();
        let handle = tokio::spawn(async move {
            let request = create_test_request(i);
            executor.process_request(request).await
        });
        handles.push(handle);
    }
    
    // Wait for all requests
    let results: Vec<_> = futures::future::join_all(handles).await;
    let duration = start.elapsed();
    
    // Verify results
    let successful = results.iter().filter(|r| r.is_ok()).count();
    let requests_per_second = num_requests as f64 / duration.as_secs_f64();
    
    println!("Processed {} requests in {:?}", num_requests, duration);
    println!("Success rate: {:.2}%", successful as f64 / num_requests as f64 * 100.0);
    println!("Throughput: {:.2} req/s", requests_per_second);
    
    assert!(successful as f64 / num_requests as f64 > 0.99); // 99% success rate
    assert!(requests_per_second > 100.0); // At least 100 req/s
}
```

## Test Coverage

### Generate Coverage Report

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage

# With more options
cargo tarpaulin \
    --workspace \
    --exclude-files "*/tests/*" \
    --exclude-files "*/benches/*" \
    --ignore-panics \
    --timeout 300 \
    --out Lcov

# Upload to coveralls
cargo tarpaulin --coveralls $COVERALLS_TOKEN
```

### Coverage Requirements

Maintain minimum coverage levels:

```toml
# .tarpaulin.toml
[coverage]
minimum = 80.0

[exclude]
packages = ["lloom-test-utils"]
exclude-files = ["*/tests/*", "*/examples/*"]
```

## Test Utilities

### Test Fixtures

```rust
// src/test_utils/fixtures.rs
use once_cell::sync::Lazy;

pub static TEST_IDENTITY: Lazy<Identity> = Lazy::new(|| {
    Identity::from_str("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80")
        .unwrap()
});

pub fn create_test_request() -> LlmRequest {
    LlmRequest {
        model: "test-model".to_string(),
        prompt: "test prompt".to_string(),
        max_tokens: Some(100),
        executor_address: TEST_IDENTITY.evm_address.to_string(),
        // ... other fields with test values
    }
}

pub fn create_test_response(request: &LlmRequest) -> LlmResponse {
    LlmResponse {
        model: request.model.clone(),
        content: "test response".to_string(),
        total_tokens: 50,
        // ... other fields
    }
}
```

### Test Helpers

```rust
// src/test_utils/helpers.rs

/// Wait for condition with timeout
pub async fn wait_for<F, Fut>(
    condition: F,
    timeout: Duration,
) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: Future<Output = bool>,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if condition().await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("Timeout waiting for condition")
}

/// Create test network with multiple nodes
pub async fn create_test_network(num_nodes: usize) -> Vec<Swarm<LloomBehaviour>> {
    let mut swarms = vec![];
    
    for _ in 0..num_nodes {
        let identity = Identity::generate();
        let behaviour = LloomBehaviour::new(&identity).unwrap();
        let swarm = create_swarm(behaviour, &identity).await;
        swarms.push(swarm);
    }
    
    // Connect all nodes
    for i in 0..num_nodes {
        for j in i+1..num_nodes {
            let addr = swarms[j].listen_addresses().next().unwrap();
            swarms[i].dial(addr).unwrap();
        }
    }
    
    swarms
}
```

## Continuous Integration Testing

### GitHub Actions

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: [stable, nightly]
    
    steps:
      - uses: actions/checkout@v3
      
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: ${{ matrix.rust }}
          
      - uses: Swatinem/rust-cache@v2
      
      - name: Run tests
        run: cargo test --all-features
        
      - name: Run clippy
        run: cargo clippy -- -D warnings
        
      - name: Check formatting
        run: cargo fmt -- --check
        
      - name: Generate coverage
        run: cargo tarpaulin --out Xml
        
      - name: Upload coverage
        uses: codecov/codecov-action@v3
```

### Pre-commit Hooks

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: cargo-test
        name: Cargo test
        entry: cargo test
        language: system
        pass_filenames: false
        
      - id: cargo-clippy
        name: Cargo clippy
        entry: cargo clippy -- -D warnings
        language: system
        pass_filenames: false
        
      - id: cargo-fmt
        name: Cargo fmt
        entry: cargo fmt -- --check
        language: system
        pass_filenames: false
```

## Testing Best Practices

### Test Organization

1. **Unit tests**: Next to the code they test
2. **Integration tests**: In `tests/` directory
3. **Benchmarks**: In `benches/` directory
4. **Test utilities**: In `src/test_utils/` or separate crate

### Test Naming

```rust
#[test]
fn test_<module>_<functionality>_<expected_behavior>() {
    // Example: test_signing_valid_message_returns_signature
}
```

### Test Independence

```rust
// Bad: Tests depend on execution order
static mut COUNTER: u32 = 0;

#[test]
fn test_1() {
    unsafe { COUNTER += 1; }
    assert_eq!(unsafe { COUNTER }, 1);
}

// Good: Tests are independent
#[test]
fn test_independent() {
    let counter = AtomicU32::new(0);
    counter.fetch_add(1, Ordering::Relaxed);
    assert_eq!(counter.load(Ordering::Relaxed), 1);
}
```

### Async Testing

```rust
// Use tokio::test for async tests
#[tokio::test]
async fn test_async_operation() {
    // Test implementation
}

// Configure runtime if needed
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_parallel_operation() {
    // Test implementation
}
```

## Debugging Tests

### Verbose Output

```bash
# Show println! output
cargo test -- --nocapture

# Show test execution order
cargo test -- --test-threads=1

# Enable debug logging
RUST_LOG=debug cargo test

# Run with backtrace
RUST_BACKTRACE=1 cargo test
```

### Test Filtering

```bash
# Run tests containing "network"
cargo test network

# Run exact test
cargo test --exact test_peer_discovery

# Exclude tests
cargo test --skip integration

# Run ignored tests
cargo test -- --ignored
```

### IDE Debugging

Most IDEs support debugging Rust tests:

1. **VS Code**: Use CodeLLDB extension
2. **RustRover**: Built-in debugging support
3. **CLion**: With Rust plugin

## Test Maintenance

### Regular Tasks

1. **Weekly**: Run full test suite including ignored tests
2. **Monthly**: Review and update test coverage
3. **Quarterly**: Benchmark performance regression
4. **Before Release**: Full E2E test suite

### Test Refactoring

Keep tests maintainable:

```rust
// Extract common setup
fn setup() -> TestContext {
    // Common initialization
}

// Use builder pattern for test data
TestRequest::builder()
    .model("gpt-3.5-turbo")
    .prompt("test")
    .build()
```