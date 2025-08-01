# Validator Library

The `lloom-validator` crate provides the framework for running a validator node that monitors and validates LLM transactions on the network. It ensures network integrity by verifying request-response pairs and detecting violations.

## Overview

The validator library provides:
- **Transaction Monitoring**: Observes network transactions
- **Signature Verification**: Validates cryptographic signatures
- **Content Validation**: Optional LLM-based content verification
- **Violation Reporting**: Reports misbehavior to the network
- **Staking Integration**: Manages validator stakes and rewards

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
lloom-validator = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

### Basic Validator

Run a simple validator:

```rust
use lloom_validator::{Validator, ValidatorConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create validator with default config
    let config = ValidatorConfig::default();
    let validator = Validator::new(config).await?;
    
    // Start validating
    validator.run().await?;
    
    Ok(())
}
```

### Custom Configuration

Configure validator behavior:

```rust
use lloom_validator::{Validator, ValidatorConfig, ValidationStrategy};
use std::time::Duration;

let config = ValidatorConfig {
    // Network settings
    listen_address: "/ip4/0.0.0.0/tcp/4002".parse()?,
    bootstrap_peers: vec![
        "/ip4/bootstrap.lloom.network/tcp/4001/p2p/12D3KooW...".parse()?
    ],
    
    // Identity
    identity_path: Some("~/.lloom/validator_identity".into()),
    ethereum_key_path: Some("~/.lloom/validator_eth_key".into()),
    
    // Validation settings
    validation_strategy: ValidationStrategy::WeightedRandom {
        high_value_weight: 5.0,
        suspicious_executor_weight: 10.0,
        normal_weight: 1.0,
    },
    sampling_rate: 0.1, // Validate 10% of transactions
    
    // Validation criteria
    enable_signature_validation: true,
    enable_token_validation: true,
    enable_content_validation: false, // Requires LLM access
    
    // Storage
    database_path: "~/.lloom/validator/validator.db".into(),
    max_storage_gb: 50,
    retention_days: 30,
};

let validator = Validator::new(config).await?;
```

## Validation Strategies

### Random Sampling

Validate random subset of transactions:

```rust
use lloom_validator::{ValidationStrategy, RandomSamplingConfig};

let strategy = ValidationStrategy::Random(RandomSamplingConfig {
    sampling_rate: 0.05, // 5% of all transactions
    
    // Optional filters
    min_token_count: Some(100), // Only validate larger requests
    included_models: Some(vec!["gpt-4", "claude-2"]),
    excluded_executors: Some(vec![blacklisted_executor]),
});

validator.set_validation_strategy(strategy)?;
```

### Weighted Sampling

Focus on high-risk transactions:

```rust
use lloom_validator::{ValidationStrategy, WeightedSamplingConfig, WeightFactors};

let strategy = ValidationStrategy::WeightedRandom(WeightedSamplingConfig {
    base_rate: 0.01, // 1% base sampling
    
    weight_factors: WeightFactors {
        // Transaction value
        high_value: 5.0,      // >0.1 ETH total cost
        medium_value: 2.0,    // 0.01-0.1 ETH
        low_value: 1.0,       // <0.01 ETH
        
        // Executor reputation
        new_executor: 10.0,   // First 100 transactions
        flagged_executor: 20.0, // Previously caught violating
        trusted_executor: 0.5,  // Good track record
        
        // Model type
        expensive_model: 3.0,  // GPT-4, Claude-2, etc.
        standard_model: 1.0,   // GPT-3.5, Llama-2, etc.
        
        // Time factors
        peak_hours: 2.0,       // High traffic times
        off_peak: 0.5,         // Low traffic times
    },
});

validator.set_validation_strategy(strategy)?;
```

### Targeted Validation

Focus on specific patterns:

```rust
use lloom_validator::{ValidationStrategy, TargetedConfig, ValidationTarget};

let strategy = ValidationStrategy::Targeted(TargetedConfig {
    targets: vec![
        // Always validate specific executors
        ValidationTarget::Executor(suspect_executor_id),
        
        // Always validate specific models
        ValidationTarget::Model("gpt-4".to_string()),
        
        // Pattern matching
        ValidationTarget::Pattern {
            field: "total_tokens",
            operator: ">",
            value: "4000",
        },
        
        // Complex conditions
        ValidationTarget::Custom(Box::new(|transaction| {
            transaction.request.deadline - transaction.response.timestamp < 60
        })),
    ],
    
    // Still sample others at lower rate
    fallback_sampling_rate: 0.01,
});

validator.set_validation_strategy(strategy)?;
```

## Validation Process

### Transaction Observer

Monitor network transactions:

```rust
use lloom_validator::{TransactionObserver, ObserverConfig};

let observer_config = ObserverConfig {
    // Subscribe to gossipsub topics
    transaction_topics: vec![
        "lloom/transactions/1.0.0",
        "lloom/requests/1.0.0",
        "lloom/responses/1.0.0",
    ],
    
    // Buffer for matching requests/responses
    buffer_size: 10000,
    buffer_timeout: Duration::from_secs(300),
    
    // Deduplication
    dedup_window: Duration::from_secs(60),
};

let observer = TransactionObserver::new(observer_config);
validator.set_observer(observer)?;

// Handle observed transactions
validator.on_transaction(|transaction| {
    println!("Observed transaction: {} -> {}", 
        transaction.client, transaction.executor);
});
```

### Validation Pipeline

Implement validation checks:

```rust
use lloom_validator::{ValidationPipeline, ValidationCheck, ValidationResult};

let mut pipeline = ValidationPipeline::new();

// Add validation checks
pipeline.add_check("signature", Box::new(|transaction| {
    // Verify client signature
    if !verify_signed_message(&transaction.request)? {
        return ValidationResult::Failed("Invalid client signature");
    }
    
    // Verify executor signature
    if !verify_signed_message(&transaction.response)? {
        return ValidationResult::Failed("Invalid executor signature");
    }
    
    ValidationResult::Passed
}));

pipeline.add_check("token_count", Box::new(|transaction| {
    let claimed = transaction.response.total_tokens;
    let calculated = count_tokens(&transaction)?;
    
    let deviation = ((claimed as f64 - calculated as f64) / calculated as f64).abs();
    
    if deviation > 0.05 { // 5% tolerance
        return ValidationResult::Failed(format!(
            "Token count mismatch: claimed {}, actual {}", 
            claimed, calculated
        ));
    }
    
    ValidationResult::Passed
}));

pipeline.add_check("timing", Box::new(|transaction| {
    // Check if response came after deadline
    if transaction.response.timestamp > transaction.request.deadline {
        return ValidationResult::Failed("Response after deadline");
    }
    
    ValidationResult::Passed
}));

validator.set_validation_pipeline(pipeline)?;
```

### Content Validation

Validate response quality (requires LLM):

```rust
use lloom_validator::{ContentValidator, ContentValidationConfig};

let content_validator = ContentValidator::new(ContentValidationConfig {
    llm_endpoint: "https://api.openai.com/v1".to_string(),
    llm_api_key: std::env::var("OPENAI_API_KEY")?,
    validation_model: "gpt-3.5-turbo".to_string(),
    
    checks: vec![
        ContentCheck::Relevance {
            min_score: 0.7,
            prompt_template: "Rate how relevant this response is to the request on a scale of 0-1",
        },
        
        ContentCheck::Completeness {
            prompt_template: "Is this response complete or was it cut off?",
        },
        
        ContentCheck::Safety {
            prompt_template: "Does this response contain inappropriate content?",
        },
        
        ContentCheck::Accuracy {
            enabled_for_models: vec!["gpt-4"], // Only for high-stakes models
            fact_check_sources: vec!["wikipedia", "scholarly"],
        },
    ],
    
    // Batch validation requests
    batch_size: 10,
    batch_timeout: Duration::from_secs(5),
});

validator.set_content_validator(Some(content_validator))?;
```

## Violation Handling

### Violation Detection

Define violation types and severities:

```rust
use lloom_validator::{ViolationType, ViolationSeverity, ViolationHandler};

#[derive(Debug)]
enum CustomViolation {
    SignatureMismatch,
    TokenCountFraud { claimed: u32, actual: u32 },
    ResponseTimeout,
    ContentMismatch,
    PriceManipulation,
}

impl ViolationType for CustomViolation {
    fn severity(&self) -> ViolationSeverity {
        match self {
            Self::SignatureMismatch => ViolationSeverity::Critical,
            Self::TokenCountFraud { .. } => ViolationSeverity::High,
            Self::ResponseTimeout => ViolationSeverity::Medium,
            Self::ContentMismatch => ViolationSeverity::Medium,
            Self::PriceManipulation => ViolationSeverity::High,
        }
    }
    
    fn description(&self) -> String {
        match self {
            Self::TokenCountFraud { claimed, actual } => {
                format!("Token fraud: claimed {}, actual {}", claimed, actual)
            },
            _ => format!("{:?}", self),
        }
    }
}
```

### Violation Reporting

Report violations to network:

```rust
use lloom_validator::{ViolationReporter, ReportingConfig, ViolationEvidence};

let reporter = ViolationReporter::new(ReportingConfig {
    // Where to report
    report_endpoints: vec![
        "https://registry.lloom.network/violations",
        "https://backup.lloom.network/violations",
    ],
    
    // Batching
    batch_size: 10,
    batch_timeout: Duration::from_secs(60),
    
    // Retry logic
    max_retries: 3,
    retry_delay: Duration::from_secs(5),
    
    // Evidence storage
    store_evidence: true,
    evidence_retention_days: 90,
    compress_evidence: true,
});

// Report a violation
let evidence = ViolationEvidence {
    transaction_id: transaction.id,
    violation_type: CustomViolation::TokenCountFraud { 
        claimed: 1000, 
        actual: 1500 
    },
    timestamp: Utc::now(),
    
    // Cryptographic proof
    request_hash: hash(&transaction.request),
    response_hash: hash(&transaction.response),
    
    // Additional evidence
    metadata: serde_json::json!({
        "token_calculation_method": "tiktoken",
        "model_claimed": "gpt-3.5-turbo",
        "deviation_percent": 50.0,
    }),
};

reporter.report_violation(evidence).await?;
```

### Evidence Management

Store and manage evidence:

```rust
use lloom_validator::{EvidenceStore, EvidenceQuery};

let evidence_store = EvidenceStore::new("~/.lloom/validator/evidence")?;

// Store evidence
evidence_store.store(&evidence)?;

// Query evidence
let query = EvidenceQuery::new()
    .executor(executor_address)
    .after(Utc::now() - Duration::from_days(7))
    .violation_type("TokenCountFraud")
    .min_severity(ViolationSeverity::High);

let results = evidence_store.query(query)?;
for evidence in results {
    println!("Found violation: {:?}", evidence);
}

// Export evidence for investigation
evidence_store.export_case(case_id, "/tmp/evidence_export.zip")?;
```

## Staking and Economics

### Stake Management

Manage validator stake:

```rust
use lloom_validator::{StakeManager, StakeConfig};

let stake_manager = StakeManager::new(StakeConfig {
    // Staking contract
    contract_address: "0x1234...".parse()?,
    rpc_endpoint: "https://eth-mainnet.g.alchemy.com/v2/...".to_string(),
    
    // Stake amount
    stake_amount_eth: 1.0,
    
    // Auto-staking
    auto_stake: true,
    min_balance_eth: 0.1, // Keep for gas
    
    // Rewards
    auto_claim_rewards: true,
    claim_threshold_eth: 0.1,
    compound_rewards: true,
});

// Check stake status
let status = stake_manager.status().await?;
println!("Staked: {} ETH", status.staked_amount);
println!("Rewards: {} ETH", status.pending_rewards);

// Stake additional funds
stake_manager.stake_additional(0.5).await?;

// Claim rewards
let claimed = stake_manager.claim_rewards().await?;
println!("Claimed {} ETH in rewards", claimed);
```

### Reward Calculation

Track validation rewards:

```rust
use lloom_validator::{RewardTracker, RewardConfig};

let reward_tracker = RewardTracker::new(RewardConfig {
    // Base rewards
    base_reward_per_validation: "100000000000000", // 0.0001 ETH
    
    // Bonuses
    violation_detection_bonus: "1000000000000000", // 0.001 ETH
    
    // Penalties  
    false_positive_penalty: "500000000000000", // 0.0005 ETH
    missed_violation_penalty: "2000000000000000", // 0.002 ETH
    
    // Multipliers
    accuracy_multiplier: true, // Higher rewards for accurate validators
    stake_multiplier: true,    // Higher rewards for larger stakes
});

validator.set_reward_tracker(reward_tracker)?;

// Get reward statistics
let stats = validator.reward_stats();
println!("Total earned: {} ETH", stats.total_earned);
println!("Success rate: {}%", stats.accuracy * 100.0);
```

## Monitoring and Metrics

### Performance Metrics

Track validator performance:

```rust
use lloom_validator::{MetricsCollector, MetricType};

let metrics = MetricsCollector::new();

// Register metrics
metrics.register(MetricType::Counter("validations_total"));
metrics.register(MetricType::Histogram("validation_duration_seconds"));
metrics.register(MetricType::Gauge("active_validations"));

// Track metrics in validation
validator.on_validation_complete(|result, duration| {
    metrics.increment("validations_total", &[
        ("result", result.status()),
        ("severity", result.severity()),
    ]);
    
    metrics.observe("validation_duration_seconds", duration.as_secs_f64());
});

// Export metrics
metrics.export_prometheus("0.0.0.0:9093")?;
```

### Health Monitoring

Monitor validator health:

```rust
use lloom_validator::{HealthMonitor, HealthCheck};

let health_monitor = HealthMonitor::new();

// Add health checks
health_monitor.add_check("network", Box::new(|| {
    let peer_count = validator.peer_count();
    if peer_count < 5 {
        return HealthCheck::Unhealthy("Too few peers".to_string());
    }
    HealthCheck::Healthy
}));

health_monitor.add_check("validation_rate", Box::new(|| {
    let rate = validator.validation_rate();
    if rate < 0.001 { // Less than 0.1% sampling
        return HealthCheck::Warning("Low validation rate".to_string());
    }
    HealthCheck::Healthy
}));

health_monitor.add_check("storage", Box::new(|| {
    let storage_usage = validator.storage_usage_percent();
    if storage_usage > 90.0 {
        return HealthCheck::Critical("Storage almost full".to_string());
    }
    HealthCheck::Healthy
}));

// Expose health endpoint
health_monitor.serve("0.0.0.0:8082")?;
```

## Advanced Features

### Custom Validation Rules

Add domain-specific validation:

```rust
use lloom_validator::{CustomValidator, ValidationContext};

struct FinancialValidator;

#[async_trait]
impl CustomValidator for FinancialValidator {
    async fn validate(
        &self,
        ctx: &ValidationContext,
        transaction: &Transaction,
    ) -> Result<ValidationResult> {
        // Check if this is a financial query
        if !is_financial_query(&transaction.request.prompt) {
            return Ok(ValidationResult::Skipped);
        }
        
        // Extract financial data from response
        let amounts = extract_amounts(&transaction.response.content);
        
        // Verify calculations
        for amount in amounts {
            if !verify_calculation(&amount, &transaction.request.prompt)? {
                return Ok(ValidationResult::Failed(
                    "Incorrect financial calculation"
                ));
            }
        }
        
        Ok(ValidationResult::Passed)
    }
}

validator.add_custom_validator(Box::new(FinancialValidator))?;
```

### Collaborative Validation

Work with other validators:

```rust
use lloom_validator::{CollaborativeValidation, ConsensusConfig};

let collab_config = CollaborativeValidation::new(ConsensusConfig {
    // Minimum validators for consensus
    min_validators: 3,
    
    // Agreement threshold
    consensus_threshold: 0.66, // 2/3 agreement
    
    // Timeout for collecting opinions
    consensus_timeout: Duration::from_secs(30),
    
    // Weight opinions by stake
    stake_weighted: true,
});

// Enable collaborative validation for high-value transactions
validator.set_collaborative_validation(Some(collab_config))?;

// Handle consensus requests
validator.on_consensus_request(|request| {
    // Perform validation
    let result = validate_transaction(&request.transaction)?;
    
    // Submit opinion
    validator.submit_consensus_opinion(request.id, result)
});
```

### Machine Learning Integration

Use ML for anomaly detection:

```rust
use lloom_validator::{AnomalyDetector, MLModel};

let anomaly_detector = AnomalyDetector::new(
    MLModel::load("~/.lloom/models/anomaly_v1.onnx")?
);

// Features for ML model
anomaly_detector.set_feature_extractors(vec![
    Box::new(|tx| tx.response.total_tokens as f32),
    Box::new(|tx| tx.response.timestamp as f32 - tx.request.timestamp as f32),
    Box::new(|tx| calculate_price_ratio(tx)),
    Box::new(|tx| get_executor_history_score(tx.executor)),
]);

// Set threshold
anomaly_detector.set_threshold(0.95); // Flag top 5% as anomalies

// Use in validation
validator.add_anomaly_detector(anomaly_detector)?;
```

## Testing

### Mock Validator

Test validation logic:

```rust
#[cfg(test)]
mod tests {
    use lloom_validator::test_utils::{MockValidator, MockTransaction};
    
    #[tokio::test]
    async fn test_validation() {
        let mut validator = MockValidator::new();
        
        // Create test transaction
        let transaction = MockTransaction::new()
            .with_token_count(100, 150) // claimed vs actual
            .with_valid_signatures()
            .build();
        
        // Validate
        let result = validator.validate(transaction).await?;
        
        assert_eq!(result.status, ValidationStatus::Failed);
        assert_eq!(result.violation_type, "TokenCountFraud");
    }
}
```

### Integration Testing

Test full validator:

```rust
#[cfg(test)]
mod integration_tests {
    use lloom_validator::test_utils::{spawn_test_network, inject_violations};
    
    #[tokio::test]
    async fn test_violation_detection() {
        // Spawn test network
        let network = spawn_test_network(10).await?; // 10 nodes
        
        // Add validator
        let validator = network.add_validator().await?;
        
        // Inject violations
        let violations = inject_violations(&network, vec![
            ViolationType::TokenFraud,
            ViolationType::InvalidSignature,
        ]).await?;
        
        // Wait for detection
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Check if violations detected
        let detected = validator.get_detected_violations();
        assert_eq!(detected.len(), violations.len());
    }
}
```

## Best Practices

1. **Sampling Strategy**
   - Start with low sampling rates
   - Focus on high-risk transactions
   - Adjust based on violation rates

2. **Resource Management**
   - Monitor storage usage
   - Implement data retention policies
   - Compress old evidence

3. **False Positives**
   - Set appropriate tolerances
   - Validate validation logic
   - Track false positive rates

4. **Network Participation**
   - Maintain good peer connections
   - Participate in consensus when requested
   - Share violation data promptly

5. **Security**
   - Secure validator keys
   - Validate all incoming data
   - Monitor for attacks on validator