# Smart Contract Integration

This guide demonstrates how to integrate with Lloom's smart contracts for on-chain accounting, payment settlement, and decentralized governance.

## Contract Setup

### Deploying Contracts

Deploy the Lloom contracts to your chosen network:

```bash
# Start local test network
cd ethnode
docker-compose up -d

# Deploy contracts
cd ../solidity
forge script script/Deploy.s.sol --rpc-url http://localhost:8545 --broadcast

# Verify deployment
cast call $ACCOUNTING_CONTRACT "DOMAIN_NAME()" --rpc-url http://localhost:8545
```

### Contract Addresses

```rust
use alloy::primitives::Address;

// Mainnet addresses (example)
const ACCOUNTING_CONTRACT: Address = address!("0x1234567890123456789012345678901234567890");
const REGISTRY_CONTRACT: Address = address!("0x0987654321098765432109876543210987654321");
const TOKEN_CONTRACT: Address = address!("0xabcdefabcdefabcdefabcdefabcdefabcdefabcd");

// Testnet addresses
#[cfg(test)]
const ACCOUNTING_CONTRACT: Address = address!("0x5FbDB2315678afecb367f032d93F642f64180aa3");
```

## Basic Contract Integration

### Setting Up Contract Client

```rust
use alloy::providers::{Provider, Http};
use alloy::signers::LocalWallet;
use alloy::contract::Contract;
use lloom_core::Identity;

struct ContractClient {
    provider: Provider<Http>,
    signer: LocalWallet,
    accounting_contract: AccountingV2,
}

impl ContractClient {
    async fn new(rpc_url: &str, identity: &Identity) -> Result<Self> {
        // Create provider
        let provider = Provider::<Http>::try_from(rpc_url)?;
        
        // Create signer from identity
        let signer = LocalWallet::from(identity.wallet.clone());
        
        // Load contract
        let accounting_contract = AccountingV2::new(
            ACCOUNTING_CONTRACT,
            provider.clone()
        );
        
        Ok(Self {
            provider,
            signer,
            accounting_contract,
        })
    }
}
```

### Reading Contract State

```rust
impl ContractClient {
    async fn get_domain_separator(&self) -> Result<[u8; 32]> {
        let separator = self.accounting_contract
            .DOMAIN_SEPARATOR()
            .call()
            .await?;
        
        Ok(separator.into())
    }
    
    async fn get_client_nonce(&self, client: Address) -> Result<u64> {
        let nonce = self.accounting_contract
            .nonces(client)
            .call()
            .await?;
        
        Ok(nonce)
    }
    
    async fn verify_request_commitment(
        &self,
        commitment: LlmRequestCommitment,
        signature: Bytes
    ) -> Result<Address> {
        let signer = self.accounting_contract
            .verifyRequestSignature(commitment, signature)
            .call()
            .await?;
        
        Ok(signer)
    }
}
```

## Request Commitment

### Creating and Signing Request Commitment

```rust
use alloy::sol_types::SolStruct;
use lloom_core::eip712::{sign_eip712, LLOOM_DOMAIN};

async fn create_request_commitment(
    client: &ContractClient,
    request: &LlmRequest,
    executor: Address,
) -> Result<SignedCommitment> {
    // Get current nonce
    let nonce = client.get_client_nonce(client.signer.address()).await?;
    
    // Create commitment
    let commitment = LlmRequestCommitment {
        executor,
        model: request.model.clone(),
        promptHash: keccak256(request.prompt.as_bytes()),
        systemPromptHash: request.system_prompt
            .as_ref()
            .map(|p| keccak256(p.as_bytes()))
            .unwrap_or_default(),
        maxTokens: request.max_tokens.unwrap_or(1000),
        temperature: (request.temperature.unwrap_or(0.7) * 10000.0) as u32,
        inboundPrice: U256::from_str(&request.inbound_price)?,
        outboundPrice: U256::from_str(&request.outbound_price)?,
        nonce,
        deadline: request.deadline,
    };
    
    // Sign with EIP-712
    let signature = sign_eip712(&commitment, &client.signer).await?;
    
    Ok(SignedCommitment {
        commitment,
        signature,
        signer: client.signer.address(),
    })
}
```

### Submitting Request On-Chain

```rust
impl ContractClient {
    async fn submit_request(
        &self,
        commitment: LlmRequestCommitment,
        signature: Bytes,
    ) -> Result<TxHash> {
        let tx = self.accounting_contract
            .submitRequest(commitment, signature)
            .send()
            .await?
            .await?;
        
        println!("Request submitted: {:?}", tx.transaction_hash);
        
        // Extract request ID from events
        let request_id = tx.logs
            .iter()
            .find_map(|log| {
                if let Ok(event) = self.accounting_contract.decode_event::<RequestSubmitted>(log) {
                    Some(event.requestId)
                } else {
                    None
                }
            })
            .ok_or("No RequestSubmitted event found")?;
        
        Ok(request_id)
    }
}
```

## Response Commitment

### Processing Response as Executor

```rust
async fn create_response_commitment(
    executor: &ContractClient,
    request: &SignedCommitment,
    response: &LlmResponse,
) -> Result<SignedCommitment> {
    // Calculate request hash
    let request_hash = hash_request_commitment(&request.commitment);
    
    // Create response commitment
    let commitment = LlmResponseCommitment {
        requestHash: request_hash,
        client: request.commitment.client,
        model: response.model.clone(),
        contentHash: keccak256(response.content.as_bytes()),
        inboundTokens: response.prompt_tokens,
        outboundTokens: response.completion_tokens,
        inboundPrice: request.commitment.inboundPrice,
        outboundPrice: request.commitment.outboundPrice,
        timestamp: Utc::now().timestamp() as u64,
        success: response.success,
    };
    
    // Sign with executor's key
    let signature = sign_eip712(&commitment, &executor.signer).await?;
    
    Ok(SignedCommitment {
        commitment,
        signature,
        signer: executor.signer.address(),
    })
}
```

### Submitting Response On-Chain

```rust
impl ContractClient {
    async fn submit_response(
        &self,
        request_id: [u8; 32],
        commitment: LlmResponseCommitment,
        signature: Bytes,
    ) -> Result<TxHash> {
        let tx = self.accounting_contract
            .submitResponse(request_id, commitment, signature)
            .send()
            .await?
            .await?;
        
        println!("Response submitted: {:?}", tx.transaction_hash);
        
        Ok(tx.transaction_hash)
    }
}
```

## Payment Settlement

### Calculating Payment

```rust
async fn calculate_payment(
    commitment: &LlmResponseCommitment
) -> Result<U256> {
    let inbound_cost = U256::from(commitment.inboundTokens) * commitment.inboundPrice;
    let outbound_cost = U256::from(commitment.outboundTokens) * commitment.outboundPrice;
    let total_cost = inbound_cost + outbound_cost;
    
    Ok(total_cost)
}
```

### Settling Payment

```rust
impl ContractClient {
    async fn settle_payment(
        &self,
        request_id: [u8; 32],
        response_id: [u8; 32],
    ) -> Result<TxHash> {
        // Get payment amount
        let (request, response) = self.accounting_contract
            .getCommitments(request_id, response_id)
            .call()
            .await?;
        
        let payment_amount = calculate_payment(&response).await?;
        
        // Send payment with transaction
        let tx = self.accounting_contract
            .settlePayment(request_id, response_id)
            .value(payment_amount)
            .send()
            .await?
            .await?;
        
        println!("Payment settled: {} ETH", format_ether(payment_amount));
        
        Ok(tx.transaction_hash)
    }
}
```

### Batch Settlement

```rust
struct BatchSettlement {
    request_ids: Vec<[u8; 32]>,
    response_ids: Vec<[u8; 32]>,
    total_payment: U256,
}

impl ContractClient {
    async fn settle_batch(&self, batch: BatchSettlement) -> Result<TxHash> {
        let tx = self.accounting_contract
            .settleBatch(batch.request_ids, batch.response_ids)
            .value(batch.total_payment)
            .send()
            .await?
            .await?;
        
        Ok(tx.transaction_hash)
    }
}
```

## Dispute Resolution

### Raising a Dispute

```rust
#[derive(Debug)]
enum DisputeReason {
    InvalidResponse,
    TokenCountMismatch,
    QualityIssue,
    Timeout,
}

impl ContractClient {
    async fn raise_dispute(
        &self,
        request_id: [u8; 32],
        response_id: [u8; 32],
        reason: DisputeReason,
        evidence: Vec<u8>,
    ) -> Result<TxHash> {
        let reason_code = match reason {
            DisputeReason::InvalidResponse => 1,
            DisputeReason::TokenCountMismatch => 2,
            DisputeReason::QualityIssue => 3,
            DisputeReason::Timeout => 4,
        };
        
        let tx = self.accounting_contract
            .raiseDispute(request_id, response_id, reason_code, evidence)
            .send()
            .await?
            .await?;
        
        Ok(tx.transaction_hash)
    }
}
```

### Resolving Disputes

```rust
impl ContractClient {
    async fn resolve_dispute(
        &self,
        dispute_id: [u8; 32],
        resolution: DisputeResolution,
    ) -> Result<TxHash> {
        let tx = self.accounting_contract
            .resolveDispute(dispute_id, resolution)
            .send()
            .await?
            .await?;
        
        Ok(tx.transaction_hash)
    }
}
```

## Event Monitoring

### Listening to Contract Events

```rust
use futures::StreamExt;

async fn monitor_contract_events(client: &ContractClient) -> Result<()> {
    // Subscribe to RequestSubmitted events
    let request_filter = client.accounting_contract
        .RequestSubmitted_filter()
        .from_block(BlockNumber::Latest);
    
    let mut request_stream = request_filter.subscribe().await?;
    
    // Subscribe to ResponseSubmitted events
    let response_filter = client.accounting_contract
        .ResponseSubmitted_filter()
        .from_block(BlockNumber::Latest);
    
    let mut response_stream = response_filter.subscribe().await?;
    
    // Handle events
    loop {
        tokio::select! {
            Some(event) = request_stream.next() => {
                handle_request_event(event?).await?;
            }
            Some(event) = response_stream.next() => {
                handle_response_event(event?).await?;
            }
        }
    }
}

async fn handle_request_event(event: RequestSubmittedEvent) -> Result<()> {
    println!("New request: {} from {}", event.requestId, event.client);
    
    // Process request if we're the target executor
    if event.executor == MY_EXECUTOR_ADDRESS {
        process_incoming_request(event.requestId).await?;
    }
    
    Ok(())
}
```

### Historical Event Queries

```rust
impl ContractClient {
    async fn get_client_history(
        &self,
        client: Address,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<RequestHistory>> {
        let filter = self.accounting_contract
            .RequestSubmitted_filter()
            .filter(move |event| event.client == client)
            .from_block(from_block)
            .to_block(to_block);
        
        let events = filter.query().await?;
        
        let history = events.into_iter()
            .map(|event| RequestHistory {
                request_id: event.requestId,
                executor: event.executor,
                timestamp: event.timestamp,
                model: event.model,
            })
            .collect();
        
        Ok(history)
    }
}
```

## Gas Optimization

### Batch Operations

```rust
struct GasEfficientClient {
    client: ContractClient,
    pending_requests: Vec<PendingRequest>,
    pending_responses: Vec<PendingResponse>,
}

impl GasEfficientClient {
    async fn flush_pending(&mut self) -> Result<()> {
        // Batch multiple operations in one transaction
        if !self.pending_requests.is_empty() {
            let batch_tx = self.client.accounting_contract
                .batchSubmitRequests(
                    self.pending_requests.iter().map(|r| r.commitment).collect(),
                    self.pending_requests.iter().map(|r| r.signature).collect(),
                )
                .send()
                .await?
                .await?;
            
            println!("Submitted {} requests in one tx", self.pending_requests.len());
            self.pending_requests.clear();
        }
        
        Ok(())
    }
}
```

### Storage Optimization

```rust
// Use events instead of storage for data that doesn't need to be on-chain
impl ContractClient {
    async fn emit_metadata(
        &self,
        request_id: [u8; 32],
        metadata: RequestMetadata,
    ) -> Result<()> {
        // Emit as event instead of storing
        let tx = self.accounting_contract
            .emitMetadata(request_id, encode_metadata(metadata))
            .send()
            .await?
            .await?;
        
        Ok(())
    }
}
```

## Testing Smart Contract Integration

### Unit Tests with Anvil

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use alloy::node_bindings::Anvil;
    
    #[tokio::test]
    async fn test_request_submission() {
        // Start local Anvil instance
        let anvil = Anvil::new().spawn();
        
        // Deploy contracts
        let provider = Provider::try_from(anvil.endpoint()).unwrap();
        let wallet = anvil.keys()[0].clone();
        
        let accounting = deploy_accounting_contract(&provider, &wallet).await.unwrap();
        
        // Create test client
        let client = ContractClient {
            provider: provider.clone(),
            signer: wallet,
            accounting_contract: accounting,
        };
        
        // Test request submission
        let commitment = create_test_commitment();
        let signature = sign_eip712(&commitment, &client.signer).await.unwrap();
        
        let tx_hash = client.submit_request(commitment, signature).await.unwrap();
        assert!(!tx_hash.is_zero());
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_flow() {
    let client_identity = Identity::generate();
    let executor_identity = Identity::generate();
    
    // Setup contracts
    let client_contract = ContractClient::new(RPC_URL, &client_identity).await.unwrap();
    let executor_contract = ContractClient::new(RPC_URL, &executor_identity).await.unwrap();
    
    // Client submits request
    let request = create_test_request();
    let request_commitment = create_request_commitment(
        &client_contract,
        &request,
        executor_identity.evm_address,
    ).await.unwrap();
    
    let request_id = client_contract
        .submit_request(request_commitment.commitment, request_commitment.signature)
        .await
        .unwrap();
    
    // Executor submits response
    let response = process_request(&request).await.unwrap();
    let response_commitment = create_response_commitment(
        &executor_contract,
        &request_commitment,
        &response,
    ).await.unwrap();
    
    let response_tx = executor_contract
        .submit_response(request_id, response_commitment.commitment, response_commitment.signature)
        .await
        .unwrap();
    
    // Client settles payment
    let payment_tx = client_contract
        .settle_payment(request_id, response_id)
        .await
        .unwrap();
    
    assert!(!payment_tx.is_zero());
}
```

## Advanced Contract Patterns

### Proxy Pattern for Upgrades

```rust
struct UpgradeableClient {
    provider: Provider<Http>,
    proxy_address: Address,
    implementation_address: Arc<RwLock<Address>>,
}

impl UpgradeableClient {
    async fn call_implementation<T>(&self, call: T) -> Result<T::Return>
    where
        T: SolCall,
    {
        let implementation = self.implementation_address.read().await;
        
        // Call through proxy
        let data = call.abi_encode();
        let result = self.provider
            .call()
            .to(self.proxy_address)
            .data(data)
            .await?;
        
        T::abi_decode_returns(&result, true)
    }
}
```

### Multi-Sig Integration

```rust
struct MultiSigClient {
    client: ContractClient,
    multisig_address: Address,
    threshold: u32,
}

impl MultiSigClient {
    async fn propose_settlement(
        &self,
        request_id: [u8; 32],
        response_id: [u8; 32],
    ) -> Result<u256> {
        // Create settlement proposal
        let proposal_data = self.client.accounting_contract
            .settlePayment(request_id, response_id)
            .encode();
        
        let proposal_id = self.propose_transaction(
            self.client.accounting_contract.address(),
            proposal_data,
            calculate_payment_amount().await?,
        ).await?;
        
        Ok(proposal_id)
    }
}
```

## Complete Example

```rust
use lloom_core::*;
use alloy::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize
    let identity = Identity::from_file("~/.lloom/identity")?;
    let client = ContractClient::new(
        "https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY",
        &identity
    ).await?;
    
    // Create request
    let request = LlmRequest {
        model: "gpt-4".to_string(),
        prompt: "Explain smart contracts".to_string(),
        max_tokens: Some(500),
        executor_address: "0x...".to_string(),
        inbound_price: "1000000000000000".to_string(),
        outbound_price: "2000000000000000".to_string(),
        nonce: client.get_next_nonce().await?,
        deadline: Utc::now().timestamp() as u64 + 3600,
        ..Default::default()
    };
    
    // Submit on-chain
    let commitment = create_request_commitment(
        &client,
        &request,
        executor_address
    ).await?;
    
    let request_id = client.submit_request(
        commitment.commitment,
        commitment.signature
    ).await?;
    
    println!("Request submitted on-chain: {:?}", request_id);
    
    // Monitor for response
    let response = wait_for_response(&client, request_id).await?;
    
    // Settle payment
    let tx = client.settle_payment(request_id, response.id).await?;
    println!("Payment settled: {:?}", tx);
    
    Ok(())
}
```