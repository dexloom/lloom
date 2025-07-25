//! Message-level cryptographic signing for the Lloom P2P network.
//!
//! This module provides cryptographic signing and verification capabilities for protocol messages,
//! ensuring non-repudiation and creating audit trails for all LLM requests and responses.

use alloy::primitives::{Address, Bytes};
use alloy::signers::local::PrivateKeySigner;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::error::{Error, Result};

/// A wrapper struct for cryptographically signed messages.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignedMessage<T: Serialize> {
    /// The actual message payload
    pub payload: T,
    /// The signer's Ethereum address
    pub signer: Address,
    /// Signature of the serialized payload
    pub signature: Bytes,
    /// Timestamp when the message was signed (Unix timestamp in seconds)
    pub timestamp: u64,
    /// Optional nonce to prevent replay attacks
    pub nonce: Option<u64>,
}

/// Trait for messages that can be cryptographically signed.
pub trait SignableMessage: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync {
    /// Sign this message using the provided signer (blocking version).
    fn sign_blocking(&self, signer: &PrivateKeySigner) -> Result<SignedMessage<Self>> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| Error::Signature(format!("Failed to get timestamp: {}", e)))?
            .as_secs();

        sign_message_blocking(self, signer, timestamp, None)
    }

    /// Sign this message with a specific timestamp and optional nonce (blocking version).
    fn sign_with_params_blocking(
        &self,
        signer: &PrivateKeySigner,
        timestamp: u64,
        nonce: Option<u64>,
    ) -> Result<SignedMessage<Self>> {
        sign_message_blocking(self, signer, timestamp, nonce)
    }
}

/// Configuration for signature verification.
#[derive(Debug, Clone)]
pub struct VerificationConfig {
    /// Maximum age of a message in seconds (for replay protection)
    pub max_age_seconds: Option<u64>,
    /// Whether to enforce strict timestamp validation
    pub strict_timestamp: bool,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            max_age_seconds: Some(3600), // 1 hour default
            strict_timestamp: true,
        }
    }
}

impl VerificationConfig {
    /// Create a configuration that accepts messages of any age.
    pub fn permissive() -> Self {
        Self {
            max_age_seconds: None,
            strict_timestamp: false,
        }
    }

    /// Create a configuration with a specific maximum age.
    pub fn with_max_age(max_age_seconds: u64) -> Self {
        Self {
            max_age_seconds: Some(max_age_seconds),
            strict_timestamp: true,
        }
    }
}

/// Sign a message using the provided signer (blocking version).
///
/// # Arguments
/// * `message` - The message to sign
/// * `signer` - The private key signer
/// * `timestamp` - Unix timestamp in seconds
/// * `nonce` - Optional nonce for replay protection
///
/// # Returns
/// A `SignedMessage` containing the original message and signature metadata.
pub fn sign_message_blocking<T: Serialize + for<'de> Deserialize<'de>>(
    message: &T,
    signer: &PrivateKeySigner,
    timestamp: u64,
    nonce: Option<u64>,
) -> Result<SignedMessage<T>> {
    // Serialize the message to JSON bytes
    let message_bytes = serde_json::to_vec(message)
        .map_err(|e| Error::Signature(format!("Failed to serialize message: {}", e)))?;

    // Create the hash of the message
    let message_hash = alloy::primitives::keccak256(&message_bytes);

    // Get the private key bytes from the signer
    let private_key_bytes = signer.credential().to_bytes();
    
    // Create a k256 signing key directly for synchronous signing
    let signing_key = k256::ecdsa::SigningKey::from_bytes(&private_key_bytes)
        .map_err(|e| Error::Signature(format!("Failed to create signing key: {}", e)))?;

    // Sign the hash synchronously using k256
    let (signature, recovery_id) = signing_key
        .sign_prehash_recoverable(message_hash.as_slice())
        .map_err(|e| Error::Signature(format!("Failed to sign message hash: {}", e)))?;

    // Convert to the 65-byte format expected by Ethereum (r + s + v)
    let mut signature_bytes = [0u8; 65];
    signature_bytes[..64].copy_from_slice(&signature.to_bytes());
    signature_bytes[64] = recovery_id.to_byte();

    Ok(SignedMessage {
        payload: serde_json::from_slice(&message_bytes)
            .map_err(|e| Error::Signature(format!("Failed to deserialize message: {}", e)))?,
        signer: signer.address(),
        signature: signature_bytes.into(),
        timestamp,
        nonce,
    })
}

/// Verify a signed message.
///
/// # Arguments
/// * `signed_message` - The signed message to verify
/// * `config` - Verification configuration
///
/// # Returns
/// `Ok(())` if the signature is valid, otherwise an error.
pub fn verify_signed_message<T: Serialize>(
    signed_message: &SignedMessage<T>,
    config: &VerificationConfig,
) -> Result<()> {
    // Validate timestamp if required
    if config.strict_timestamp {
        validate_timestamp(signed_message.timestamp, config.max_age_seconds)?;
    }

    // Serialize the payload to get the original message bytes
    let message_bytes = serde_json::to_vec(&signed_message.payload)
        .map_err(|e| Error::Verification(format!("Failed to serialize payload: {}", e)))?;

    // Create the hash of the message
    let message_hash = alloy::primitives::keccak256(&message_bytes);

    // Convert signature bytes back to signature
    let signature_bytes: [u8; 65] = signed_message.signature.as_ref().try_into()
        .map_err(|_| Error::Verification("Invalid signature length".to_string()))?;
    
    // Parse the signature using the correct alloy API
    let signature = alloy::primitives::Signature::try_from(&signature_bytes[..])
        .map_err(|e| Error::Verification(format!("Failed to parse signature: {}", e)))?;

    // Recover the signer's address from the signature
    let recovered_address = signature.recover_address_from_prehash(&message_hash)
        .map_err(|e| Error::Verification(format!("Failed to recover address: {}", e)))?;

    // Verify that the recovered address matches the claimed signer
    if recovered_address != signed_message.signer {
        return Err(Error::InvalidSigner {
            expected: signed_message.signer,
            recovered: recovered_address,
        });
    }

    Ok(())
}

/// Validate a timestamp against the current time and maximum age.
fn validate_timestamp(timestamp: u64, max_age_seconds: Option<u64>) -> Result<()> {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Verification(format!("Failed to get current time: {}", e)))?
        .as_secs();

    // Check if the timestamp is from the future (with a small tolerance for clock skew)
    const CLOCK_SKEW_TOLERANCE: u64 = 300; // 5 minutes
    if timestamp > current_time + CLOCK_SKEW_TOLERANCE {
        return Err(Error::Verification(format!(
            "Message timestamp is too far in the future: {} > {}",
            timestamp, current_time + CLOCK_SKEW_TOLERANCE
        )));
    }

    // Check if the message is too old
    if let Some(max_age) = max_age_seconds {
        if current_time > timestamp && (current_time - timestamp) > max_age {
            return Err(Error::Verification(format!(
                "Message is too old: age {} seconds exceeds maximum {} seconds",
                current_time - timestamp,
                max_age
            )));
        }
    }

    Ok(())
}

/// Verify a signed message with basic validation (uses default config).
pub fn verify_signed_message_basic<T: Serialize>(
    signed_message: &SignedMessage<T>,
) -> Result<()> {
    verify_signed_message(signed_message, &VerificationConfig::default())
}

/// Verify a signed message with permissive validation (no timestamp checks).
pub fn verify_signed_message_permissive<T: Serialize>(
    signed_message: &SignedMessage<T>,
) -> Result<()> {
    verify_signed_message(signed_message, &VerificationConfig::permissive())
}

impl<T: Serialize> SignedMessage<T> {
    /// Verify this signed message with a time window for replay protection.
    ///
    /// # Arguments
    /// * `max_age_seconds` - Maximum age of the message in seconds
    ///
    /// # Returns
    /// The signer's address if verification succeeds, otherwise an error.
    pub fn verify_with_time_window(&self, max_age_seconds: u64) -> Result<alloy::primitives::Address> {
        let config = VerificationConfig::with_max_age(max_age_seconds);
        verify_signed_message(self, &config)?;
        Ok(self.signer)
    }
    
    /// Verify this signed message with basic validation (default config).
    pub fn verify_basic(&self) -> Result<alloy::primitives::Address> {
        verify_signed_message_basic(self)?;
        Ok(self.signer)
    }
    
    /// Verify this signed message with permissive validation (no timestamp checks).
    pub fn verify_permissive(&self) -> Result<alloy::primitives::Address> {
        verify_signed_message_permissive(self)?;
        Ok(self.signer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{LlmRequest, LlmResponse};
    use alloy::signers::local::PrivateKeySigner;

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct TestMessage {
        content: String,
        value: u32,
    }

    impl SignableMessage for TestMessage {}

    fn create_test_signer() -> PrivateKeySigner {
        "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse()
            .expect("Valid private key")
    }

    #[tokio::test]
    async fn test_sign_and_verify_message() {
        let signer = create_test_signer();
        let message = TestMessage {
            content: "Hello, world!".to_string(),
            value: 42,
        };

        // Sign the message
        let signed_message = message.sign_blocking(&signer).unwrap();

        // Verify the signed message
        assert!(verify_signed_message_basic(&signed_message).is_ok());

        // Check that the signature contains expected fields
        assert_eq!(signed_message.signer, signer.address());
        assert_eq!(signed_message.payload, message);
        assert!(signed_message.signature.len() == 65); // secp256k1 signature length
        assert!(signed_message.timestamp > 0);
    }

    #[test]
    fn test_sign_with_params() {
        let signer = create_test_signer();
        let message = TestMessage {
            content: "Test with params".to_string(),
            value: 123,
        };
        let timestamp = 1234567890;
        let nonce = Some(42);

        let signed_message = message.sign_with_params_blocking(&signer, timestamp, nonce).unwrap();

        assert_eq!(signed_message.timestamp, timestamp);
        assert_eq!(signed_message.nonce, nonce);
        assert_eq!(signed_message.payload, message);
    }

    #[test]
    fn test_verify_with_wrong_signer() {
        let signer1 = create_test_signer();
        let signer2 = PrivateKeySigner::random();
        let message = TestMessage {
            content: "Wrong signer test".to_string(),
            value: 999,
        };

        let mut signed_message = message.sign_blocking(&signer1).unwrap();
        // Tamper with the signer address
        signed_message.signer = signer2.address();

        let result = verify_signed_message_basic(&signed_message);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidSigner { expected, recovered } => {
                assert_eq!(expected, signer2.address());
                assert_eq!(recovered, signer1.address());
            }
            _ => panic!("Expected InvalidSigner error"),
        }
    }

    #[test]
    fn test_verify_tampered_payload() {
        let signer = create_test_signer();
        let message = TestMessage {
            content: "Original content".to_string(),
            value: 42,
        };

        let mut signed_message = message.sign_blocking(&signer).unwrap();
        // Tamper with the payload
        signed_message.payload.content = "Tampered content".to_string();

        let result = verify_signed_message_basic(&signed_message);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_timestamp_validation() {
        let signer = create_test_signer();
        let message = TestMessage {
            content: "Timestamp test".to_string(),
            value: 456,
        };

        // Test with very old timestamp
        let old_timestamp = 1000000000; // Year 2001
        let signed_message = message.sign_with_params_blocking(&signer, old_timestamp, None).unwrap();

        let config = VerificationConfig::with_max_age(3600); // 1 hour max age
        let result = verify_signed_message(&signed_message, &config);
        assert!(result.is_err());

        // Test with permissive config (should pass)
        let result = verify_signed_message(&signed_message, &VerificationConfig::permissive());
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_future_timestamp_validation() {
        let signer = create_test_signer();
        let message = TestMessage {
            content: "Future timestamp test".to_string(),
            value: 789,
        };

        // Test with timestamp far in the future
        let future_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() + 7200; // 2 hours in the future

        let signed_message = message.sign_with_params_blocking(&signer, future_timestamp, None).unwrap();

        let result = verify_signed_message_basic(&signed_message);
        assert!(result.is_err());
    }

    #[test]
    fn test_verification_config() {
        let default_config = VerificationConfig::default();
        assert_eq!(default_config.max_age_seconds, Some(3600));
        assert!(default_config.strict_timestamp);

        let permissive_config = VerificationConfig::permissive();
        assert_eq!(permissive_config.max_age_seconds, None);
        assert!(!permissive_config.strict_timestamp);

        let custom_config = VerificationConfig::with_max_age(7200);
        assert_eq!(custom_config.max_age_seconds, Some(7200));
        assert!(custom_config.strict_timestamp);
    }

    #[test]
    fn test_llm_request_signing() {
        let signer = create_test_signer();
        let request = LlmRequest {
            model: "gpt-4".to_string(),
            prompt: "Test prompt".to_string(),
            system_prompt: Some("System prompt".to_string()),
            temperature: Some(0.7),
            max_tokens: Some(150),
        };

        let signed_request = request.sign_blocking(&signer).unwrap();
        assert!(verify_signed_message_basic(&signed_request).is_ok());
        // Note: Can't directly compare LlmRequest as it doesn't implement PartialEq
        assert_eq!(signed_request.payload.model, request.model);
        assert_eq!(signed_request.payload.prompt, request.prompt);
    }

    #[test]
    fn test_llm_response_signing() {
        let signer = create_test_signer();
        let response = LlmResponse {
            content: "Generated response".to_string(),
            token_count: 25,
            model_used: "gpt-4".to_string(),
            error: None,
        };

        let signed_response = response.sign_blocking(&signer).unwrap();
        assert!(verify_signed_message_basic(&signed_response).is_ok());
        assert_eq!(signed_response.payload, response);
    }

    #[test]
    fn test_signature_serialization() {
        let signer = create_test_signer();
        let message = TestMessage {
            content: "Serialization test".to_string(),
            value: 111,
        };

        let signed_message = message.sign_blocking(&signer).unwrap();

        // Serialize to JSON and back
        let serialized = serde_json::to_string(&signed_message).unwrap();
        let deserialized: SignedMessage<TestMessage> = serde_json::from_str(&serialized).unwrap();

        // Should still verify after round-trip
        assert!(verify_signed_message_basic(&deserialized).is_ok());
        assert_eq!(deserialized.payload, message);
        assert_eq!(deserialized.signer, signed_message.signer);
        assert_eq!(deserialized.signature, signed_message.signature);
        assert_eq!(deserialized.timestamp, signed_message.timestamp);
    }

    #[test]
    fn test_invalid_signature_length() {
        let signer = create_test_signer();
        let message = TestMessage {
            content: "Invalid signature test".to_string(),
            value: 222,
        };

        let mut signed_message = message.sign_blocking(&signer).unwrap();
        // Corrupt the signature length
        signed_message.signature = Bytes::from(vec![0u8; 32]); // Wrong length

        let result = verify_signed_message_basic(&signed_message);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid signature length"));
    }

    #[test]
    fn test_deterministic_signing() {
        let signer = create_test_signer();
        let message = TestMessage {
            content: "Deterministic test".to_string(),
            value: 333,
        };
        let timestamp = 1234567890;
        let nonce = Some(42);

        // Sign the same message twice with same parameters
        let signed1 = message.sign_with_params_blocking(&signer, timestamp, nonce).unwrap();
        let signed2 = message.sign_with_params_blocking(&signer, timestamp, nonce).unwrap();

        // Should produce identical signatures
        assert_eq!(signed1.signature, signed2.signature);
        assert_eq!(signed1.signer, signed2.signer);
        assert_eq!(signed1.timestamp, signed2.timestamp);
        assert_eq!(signed1.nonce, signed2.nonce);
    }
}