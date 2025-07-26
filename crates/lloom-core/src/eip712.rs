use crate::{
    error::{Error, Result},
    protocol::{LlmRequest, LlmResponse},
};
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::{Address, Signature, keccak256, B256};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// EIP-712 Domain Separator structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EIP712Domain {
    pub name: String,
    pub version: String,
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    #[serde(rename = "verifyingContract")]
    pub verifying_contract: Address,
}

impl EIP712Domain {
    /// Create a new EIP712Domain for Lloom Network
    pub fn new(chain_id: u64, verifying_contract: Address) -> Self {
        Self {
            name: "Lloom Network".to_string(),
            version: "1.0.0".to_string(),
            chain_id,
            verifying_contract,
        }
    }
}

/// LLM Request Commitment for EIP-712 signing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequestCommitment {
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(rename = "clientAddress")]
    pub client_address: String,
    #[serde(rename = "executorAddress")]
    pub executor_address: String,
    #[serde(rename = "modelName")]
    pub model_name: String,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u32,
    pub temperature: String, // f32 as string for precision
    #[serde(rename = "promptHash")]
    pub prompt_hash: String,
    #[serde(rename = "maxPricePerToken")]
    pub max_price_per_token: String, // UINT256 as string
    #[serde(rename = "maxTotalCost")]
    pub max_total_cost: String,      // UINT256 as string
    pub timestamp: u64,
    pub nonce: u64,
}

/// LLM Response Commitment for EIP-712 signing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponseCommitment {
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(rename = "executorAddress")]
    pub executor_address: String,
    #[serde(rename = "responseHash")]
    pub response_hash: String,
    #[serde(rename = "inputTokens")]
    pub input_tokens: u32,
    #[serde(rename = "outputTokens")]
    pub output_tokens: u32,
    #[serde(rename = "totalTokens")]
    pub total_tokens: u32,
    #[serde(rename = "pricePerToken")]
    pub price_per_token: String, // UINT256 as string
    #[serde(rename = "totalCost")]
    pub total_cost: String,      // UINT256 as string
    pub timestamp: u64,
}

/// EIP-712 TypedData structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedData {
    pub types: HashMap<String, Vec<TypeField>>,
    #[serde(rename = "primaryType")]
    pub primary_type: String,
    pub domain: EIP712Domain,
    pub message: serde_json::Value,
}

/// Field definition for EIP-712 types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
}

impl TypedData {
    /// Create TypedData for LlmRequestCommitment
    pub fn for_request_commitment(
        domain: EIP712Domain,
        commitment: LlmRequestCommitment,
    ) -> Result<Self> {
        let mut types = HashMap::new();
        
        // EIP712Domain type
        types.insert("EIP712Domain".to_string(), vec![
            TypeField { name: "name".to_string(), field_type: "string".to_string() },
            TypeField { name: "version".to_string(), field_type: "string".to_string() },
            TypeField { name: "chainId".to_string(), field_type: "uint256".to_string() },
            TypeField { name: "verifyingContract".to_string(), field_type: "address".to_string() },
        ]);

        // LlmRequestCommitment type
        types.insert("LlmRequestCommitment".to_string(), vec![
            TypeField { name: "requestId".to_string(), field_type: "string".to_string() },
            TypeField { name: "clientAddress".to_string(), field_type: "address".to_string() },
            TypeField { name: "executorAddress".to_string(), field_type: "address".to_string() },
            TypeField { name: "modelName".to_string(), field_type: "string".to_string() },
            TypeField { name: "maxTokens".to_string(), field_type: "uint32".to_string() },
            TypeField { name: "temperature".to_string(), field_type: "string".to_string() },
            TypeField { name: "promptHash".to_string(), field_type: "bytes32".to_string() },
            TypeField { name: "maxPricePerToken".to_string(), field_type: "uint256".to_string() },
            TypeField { name: "maxTotalCost".to_string(), field_type: "uint256".to_string() },
            TypeField { name: "timestamp".to_string(), field_type: "uint64".to_string() },
            TypeField { name: "nonce".to_string(), field_type: "uint64".to_string() },
        ]);

        let message = serde_json::to_value(&commitment)
            .map_err(|e| Error::Serialization(e))?;

        Ok(TypedData {
            types,
            primary_type: "LlmRequestCommitment".to_string(),
            domain,
            message,
        })
    }

    /// Create TypedData for LlmResponseCommitment
    pub fn for_response_commitment(
        domain: EIP712Domain,
        commitment: LlmResponseCommitment,
    ) -> Result<Self> {
        let mut types = HashMap::new();
        
        // EIP712Domain type
        types.insert("EIP712Domain".to_string(), vec![
            TypeField { name: "name".to_string(), field_type: "string".to_string() },
            TypeField { name: "version".to_string(), field_type: "string".to_string() },
            TypeField { name: "chainId".to_string(), field_type: "uint256".to_string() },
            TypeField { name: "verifyingContract".to_string(), field_type: "address".to_string() },
        ]);

        // LlmResponseCommitment type
        types.insert("LlmResponseCommitment".to_string(), vec![
            TypeField { name: "requestId".to_string(), field_type: "string".to_string() },
            TypeField { name: "executorAddress".to_string(), field_type: "address".to_string() },
            TypeField { name: "responseHash".to_string(), field_type: "bytes32".to_string() },
            TypeField { name: "inputTokens".to_string(), field_type: "uint32".to_string() },
            TypeField { name: "outputTokens".to_string(), field_type: "uint32".to_string() },
            TypeField { name: "totalTokens".to_string(), field_type: "uint32".to_string() },
            TypeField { name: "pricePerToken".to_string(), field_type: "uint256".to_string() },
            TypeField { name: "totalCost".to_string(), field_type: "uint256".to_string() },
            TypeField { name: "timestamp".to_string(), field_type: "uint64".to_string() },
        ]);

        let message = serde_json::to_value(&commitment)
            .map_err(|e| Error::Serialization(e))?;

        Ok(TypedData {
            types,
            primary_type: "LlmResponseCommitment".to_string(),
            domain,
            message,
        })
    }
}

/// Calculate domain separator hash according to EIP-712
pub fn calculate_domain_separator(domain: &EIP712Domain) -> Result<B256> {
    let domain_type_hash = keccak256(
        "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)".as_bytes()
    );
    
    let name_hash = keccak256(domain.name.as_bytes());
    let version_hash = keccak256(domain.version.as_bytes());
    
    let mut encoded = Vec::new();
    encoded.extend_from_slice(domain_type_hash.as_slice());
    encoded.extend_from_slice(name_hash.as_slice());
    encoded.extend_from_slice(version_hash.as_slice());
    
    // Chain ID as uint256
    let mut chain_id_bytes = [0u8; 32];
    let chain_id_u64_bytes = domain.chain_id.to_be_bytes();
    chain_id_bytes[24..32].copy_from_slice(&chain_id_u64_bytes);
    encoded.extend_from_slice(&chain_id_bytes);
    
    // Verifying contract as address (20 bytes, padded to 32)
    let mut padded_addr = [0u8; 32];
    padded_addr[12..32].copy_from_slice(domain.verifying_contract.as_slice());
    encoded.extend_from_slice(&padded_addr);
    
    Ok(keccak256(&encoded))
}

/// Calculate type hash for LlmRequestCommitment
pub fn calculate_request_type_hash() -> B256 {
    keccak256(
        "LlmRequestCommitment(string requestId,address clientAddress,address executorAddress,string modelName,uint32 maxTokens,string temperature,bytes32 promptHash,uint256 maxPricePerToken,uint256 maxTotalCost,uint64 timestamp,uint64 nonce)".as_bytes()
    )
}

/// Calculate type hash for LlmResponseCommitment
pub fn calculate_response_type_hash() -> B256 {
    keccak256(
        "LlmResponseCommitment(string requestId,address executorAddress,bytes32 responseHash,uint32 inputTokens,uint32 outputTokens,uint32 totalTokens,uint256 pricePerToken,uint256 totalCost,uint64 timestamp)".as_bytes()
    )
}

/// Calculate struct hash for LlmRequestCommitment
pub fn calculate_request_struct_hash(commitment: &LlmRequestCommitment) -> Result<B256> {
    let type_hash = calculate_request_type_hash();
    
    let mut encoded = Vec::new();
    encoded.extend_from_slice(type_hash.as_slice());
    
    // String fields are hashed
    encoded.extend_from_slice(keccak256(commitment.request_id.as_bytes()).as_slice());
    
    // Address fields
    let client_addr: Address = commitment.client_address.parse()
        .map_err(|e| Error::Other(format!("Invalid client address: {}", e)))?;
    let mut padded_client = [0u8; 32];
    padded_client[12..32].copy_from_slice(client_addr.as_slice());
    encoded.extend_from_slice(&padded_client);
    
    let executor_addr: Address = commitment.executor_address.parse()
        .map_err(|e| Error::Other(format!("Invalid executor address: {}", e)))?;
    let mut padded_executor = [0u8; 32];
    padded_executor[12..32].copy_from_slice(executor_addr.as_slice());
    encoded.extend_from_slice(&padded_executor);
    
    // String fields are hashed
    encoded.extend_from_slice(keccak256(commitment.model_name.as_bytes()).as_slice());
    
    // uint32 maxTokens
    let mut max_tokens_bytes = [0u8; 32];
    let max_tokens_u32_bytes = commitment.max_tokens.to_be_bytes();
    max_tokens_bytes[28..32].copy_from_slice(&max_tokens_u32_bytes);
    encoded.extend_from_slice(&max_tokens_bytes);
    
    // String temperature is hashed
    encoded.extend_from_slice(keccak256(commitment.temperature.as_bytes()).as_slice());
    
    // bytes32 promptHash
    let prompt_hash = hex::decode(commitment.prompt_hash.trim_start_matches("0x"))
        .map_err(|e| Error::Other(format!("Invalid prompt hash: {}", e)))?;
    if prompt_hash.len() != 32 {
        return Err(Error::Other("Prompt hash must be 32 bytes".to_string()));
    }
    encoded.extend_from_slice(&prompt_hash);
    
    // uint256 fields
    let max_price = parse_uint256(&commitment.max_price_per_token)?;
    encoded.extend_from_slice(&max_price);
    
    let max_cost = parse_uint256(&commitment.max_total_cost)?;
    encoded.extend_from_slice(&max_cost);
    
    // uint64 fields
    let mut timestamp_bytes = [0u8; 32];
    let timestamp_u64_bytes = commitment.timestamp.to_be_bytes();
    timestamp_bytes[24..32].copy_from_slice(&timestamp_u64_bytes);
    encoded.extend_from_slice(&timestamp_bytes);
    
    let mut nonce_bytes = [0u8; 32];
    let nonce_u64_bytes = commitment.nonce.to_be_bytes();
    nonce_bytes[24..32].copy_from_slice(&nonce_u64_bytes);
    encoded.extend_from_slice(&nonce_bytes);
    
    Ok(keccak256(&encoded))
}

/// Calculate struct hash for LlmResponseCommitment
pub fn calculate_response_struct_hash(commitment: &LlmResponseCommitment) -> Result<B256> {
    let type_hash = calculate_response_type_hash();
    
    let mut encoded = Vec::new();
    encoded.extend_from_slice(type_hash.as_slice());
    
    // String fields are hashed
    encoded.extend_from_slice(keccak256(commitment.request_id.as_bytes()).as_slice());
    
    // Address field
    let executor_addr: Address = commitment.executor_address.parse()
        .map_err(|e| Error::Other(format!("Invalid executor address: {}", e)))?;
    let mut padded_executor = [0u8; 32];
    padded_executor[12..32].copy_from_slice(executor_addr.as_slice());
    encoded.extend_from_slice(&padded_executor);
    
    // bytes32 responseHash
    let response_hash = hex::decode(commitment.response_hash.trim_start_matches("0x"))
        .map_err(|e| Error::Other(format!("Invalid response hash: {}", e)))?;
    if response_hash.len() != 32 {
        return Err(Error::Other("Response hash must be 32 bytes".to_string()));
    }
    encoded.extend_from_slice(&response_hash);
    
    // uint32 fields
    let mut input_tokens_bytes = [0u8; 32];
    let input_tokens_u32_bytes = commitment.input_tokens.to_be_bytes();
    input_tokens_bytes[28..32].copy_from_slice(&input_tokens_u32_bytes);
    encoded.extend_from_slice(&input_tokens_bytes);
    
    let mut output_tokens_bytes = [0u8; 32];
    let output_tokens_u32_bytes = commitment.output_tokens.to_be_bytes();
    output_tokens_bytes[28..32].copy_from_slice(&output_tokens_u32_bytes);
    encoded.extend_from_slice(&output_tokens_bytes);
    
    let mut total_tokens_bytes = [0u8; 32];
    let total_tokens_u32_bytes = commitment.total_tokens.to_be_bytes();
    total_tokens_bytes[28..32].copy_from_slice(&total_tokens_u32_bytes);
    encoded.extend_from_slice(&total_tokens_bytes);
    
    // uint256 fields
    let price = parse_uint256(&commitment.price_per_token)?;
    encoded.extend_from_slice(&price);
    
    let total_cost = parse_uint256(&commitment.total_cost)?;
    encoded.extend_from_slice(&total_cost);
    
    // uint64 timestamp
    let mut timestamp_bytes = [0u8; 32];
    let timestamp_u64_bytes = commitment.timestamp.to_be_bytes();
    timestamp_bytes[24..32].copy_from_slice(&timestamp_u64_bytes);
    encoded.extend_from_slice(&timestamp_bytes);
    
    Ok(keccak256(&encoded))
}

/// Parse a UINT256 string to 32-byte array
fn parse_uint256(value: &str) -> Result<[u8; 32]> {
    let value = value.trim_start_matches("0x");
    if value.len() > 64 {
        return Err(Error::Other("UINT256 value too large".to_string()));
    }
    
    // Pad with leading zeros if necessary
    let padded = format!("{:0>64}", value);
    let bytes = hex::decode(padded)
        .map_err(|e| Error::Other(format!("Invalid UINT256 format: {}", e)))?;
    
    let mut result = [0u8; 32];
    result.copy_from_slice(&bytes);
    Ok(result)
}

/// Calculate EIP-712 message hash
pub fn calculate_eip712_hash(domain: &EIP712Domain, struct_hash: &B256) -> Result<B256> {
    let domain_separator = calculate_domain_separator(domain)?;
    
    let mut message = Vec::new();
    message.push(0x19);
    message.push(0x01);
    message.extend_from_slice(domain_separator.as_slice());
    message.extend_from_slice(struct_hash.as_slice());
    
    Ok(keccak256(&message))
}

/// Sign a request commitment using EIP-712
pub fn sign_request_commitment(
    private_key: &PrivateKeySigner,
    domain: &EIP712Domain,
    commitment: &LlmRequestCommitment,
) -> Result<Signature> {
    let struct_hash = calculate_request_struct_hash(commitment)?;
    let message_hash = calculate_eip712_hash(domain, &struct_hash)?;
    
    // Use the same synchronous approach as in signing.rs
    let private_key_bytes = private_key.credential().to_bytes();
    let signing_key = k256::ecdsa::SigningKey::from_bytes(&private_key_bytes)
        .map_err(|e| Error::Signature(format!("Failed to create signing key: {}", e)))?;

    let (signature, recovery_id) = signing_key
        .sign_prehash_recoverable(message_hash.as_slice())
        .map_err(|e| Error::Signature(format!("Failed to sign message hash: {}", e)))?;

    // Convert to the 65-byte format expected by Ethereum (r + s + v)
    let mut signature_bytes = [0u8; 65];
    signature_bytes[..64].copy_from_slice(&signature.to_bytes());
    signature_bytes[64] = recovery_id.to_byte();

    let alloy_signature = Signature::try_from(&signature_bytes[..])
        .map_err(|e| Error::Signature(format!("Failed to create alloy signature: {}", e)))?;
    
    Ok(alloy_signature)
}

/// Sign a response commitment using EIP-712
pub fn sign_response_commitment(
    private_key: &PrivateKeySigner,
    domain: &EIP712Domain,
    commitment: &LlmResponseCommitment,
) -> Result<Signature> {
    let struct_hash = calculate_response_struct_hash(commitment)?;
    let message_hash = calculate_eip712_hash(domain, &struct_hash)?;
    
    // Use the same synchronous approach as in signing.rs
    let private_key_bytes = private_key.credential().to_bytes();
    let signing_key = k256::ecdsa::SigningKey::from_bytes(&private_key_bytes)
        .map_err(|e| Error::Signature(format!("Failed to create signing key: {}", e)))?;

    let (signature, recovery_id) = signing_key
        .sign_prehash_recoverable(message_hash.as_slice())
        .map_err(|e| Error::Signature(format!("Failed to sign message hash: {}", e)))?;

    // Convert to the 65-byte format expected by Ethereum (r + s + v)
    let mut signature_bytes = [0u8; 65];
    signature_bytes[..64].copy_from_slice(&signature.to_bytes());
    signature_bytes[64] = recovery_id.to_byte();

    let alloy_signature = Signature::try_from(&signature_bytes[..])
        .map_err(|e| Error::Signature(format!("Failed to create alloy signature: {}", e)))?;
    
    Ok(alloy_signature)
}

/// Verify a request commitment signature
pub fn verify_request_signature(
    public_address: &Address,
    domain: &EIP712Domain,
    commitment: &LlmRequestCommitment,
    signature: &Signature,
) -> Result<bool> {
    let struct_hash = calculate_request_struct_hash(commitment)?;
    let message_hash = calculate_eip712_hash(domain, &struct_hash)?;
    
    let recovered_address = signature.recover_address_from_prehash(&message_hash)
        .map_err(|e| Error::Verification(format!("Failed to recover address: {}", e)))?;
    
    Ok(recovered_address == *public_address)
}

/// Verify a response commitment signature
pub fn verify_response_signature(
    public_address: &Address,
    domain: &EIP712Domain,
    commitment: &LlmResponseCommitment,
    signature: &Signature,
) -> Result<bool> {
    let struct_hash = calculate_response_struct_hash(commitment)?;
    let message_hash = calculate_eip712_hash(domain, &struct_hash)?;
    
    let recovered_address = signature.recover_address_from_prehash(&message_hash)
        .map_err(|e| Error::Verification(format!("Failed to recover address: {}", e)))?;
    
    Ok(recovered_address == *public_address)
}

/// Convert LlmRequest to LlmRequestCommitment
pub fn request_to_commitment(
    request: &LlmRequest,
    request_id: String,
    client_address: String,
    max_price_per_token: String,
    max_total_cost: String,
) -> Result<LlmRequestCommitment> {
    // Calculate prompt hash
    let prompt_hash = hex::encode(keccak256(request.prompt.as_bytes()));
    
    Ok(LlmRequestCommitment {
        request_id,
        client_address,
        executor_address: request.executor_address.clone(),
        model_name: request.model.clone(),
        max_tokens: request.max_tokens.unwrap_or(1000),
        temperature: request.temperature.unwrap_or(1.0).to_string(),
        prompt_hash,
        max_price_per_token,
        max_total_cost,
        timestamp: request.deadline,
        nonce: request.nonce,
    })
}

/// Convert LlmResponse to LlmResponseCommitment
pub fn response_to_commitment(
    response: &LlmResponse,
    request_id: String,
    executor_address: String,
    price_per_token: String,
) -> Result<LlmResponseCommitment> {
    // Calculate response hash
    let response_hash = hex::encode(keccak256(response.content.as_bytes()));
    
    Ok(LlmResponseCommitment {
        request_id,
        executor_address,
        response_hash,
        input_tokens: response.inbound_tokens as u32,
        output_tokens: response.outbound_tokens as u32,
        total_tokens: (response.inbound_tokens + response.outbound_tokens) as u32,
        price_per_token,
        total_cost: response.total_cost.clone(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::signers::local::PrivateKeySigner;

    #[test]
    fn test_domain_separator() {
        let domain = EIP712Domain::new(1, "0x1234567890123456789012345678901234567890".parse().unwrap());
        let separator = calculate_domain_separator(&domain).unwrap();
        assert_eq!(separator.len(), 32);
    }

    #[test]
    fn test_type_hashes() {
        let request_hash = calculate_request_type_hash();
        let response_hash = calculate_response_type_hash();
        
        assert_eq!(request_hash.len(), 32);
        assert_eq!(response_hash.len(), 32);
        assert_ne!(request_hash, response_hash);
    }

    #[test]
    fn test_uint256_parsing() {
        // Test basic functionality - just verify the function works
        let result = parse_uint256("1000").unwrap();
        assert_eq!(result.len(), 32);
        
        let result = parse_uint256("0x1000").unwrap();
        assert_eq!(result.len(), 32);
        
        // Test that non-zero values are placed correctly
        let result = parse_uint256("ff").unwrap();
        assert_eq!(result[31], 255);
    }

    #[test]
    fn test_sign_and_verify_request() {
        let signer: PrivateKeySigner = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse()
            .expect("Valid private key");
        
        let domain = EIP712Domain::new(1, "0x1234567890123456789012345678901234567890".parse().unwrap());
        
        let commitment = LlmRequestCommitment {
            request_id: "req_123".to_string(),
            client_address: signer.address().to_string(),
            executor_address: "0x1234567890123456789012345678901234567890".to_string(),
            model_name: "gpt-3.5-turbo".to_string(),
            max_tokens: 100,
            temperature: "0.7".to_string(),
            prompt_hash: hex::encode([0u8; 32]),
            max_price_per_token: "1000000000000000000".to_string(),
            max_total_cost: "100000000000000000000".to_string(),
            timestamp: 1640995200,
            nonce: 1,
        };
        
        let signature = sign_request_commitment(&signer, &domain, &commitment).unwrap();
        let is_valid = verify_request_signature(&signer.address(), &domain, &commitment, &signature).unwrap();
        
        assert!(is_valid);
    }
}