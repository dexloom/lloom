//! Identity management for the Lloom P2P network.
//!
//! This module provides a unified cryptographic identity that works with both
//! libp2p networking and Ethereum blockchain operations.

use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;
use libp2p::identity::{Keypair, PeerId, secp256k1};
use crate::error::{Error, Result};

/// A unified identity for nodes in the Lloom network.
///
/// This struct combines a secp256k1 private key that can be used for both
/// P2P networking (via libp2p) and blockchain operations (via alloy).
#[derive(Clone)]
pub struct Identity {
    /// The wallet containing the secp256k1 private key.
    pub wallet: PrivateKeySigner,
    /// The libp2p keypair, derived from the wallet's private key.
    pub p2p_keypair: Keypair,
    /// The libp2p PeerId, derived from the p2p_keypair's public key.
    pub peer_id: PeerId,
    /// The EVM-compatible address, derived from the wallet's public key.
    pub evm_address: Address,
}

impl Identity {
    /// Creates a new identity from a wallet.
    pub fn new(wallet: PrivateKeySigner) -> Result<Self> {
        // Convert the wallet's private key to libp2p secp256k1 keypair
        let private_key_bytes = wallet.to_bytes();
        
        // Convert FixedBytes to a mutable array
        let mut key_bytes = private_key_bytes.0;
        
        // Create libp2p secp256k1 secret key from the wallet's private key bytes
        let secret_key = secp256k1::SecretKey::try_from_bytes(&mut key_bytes)
            .map_err(|e| Error::Identity(format!("Failed to create secp256k1 secret key: {:?}", e)))?;
        
        // Create the libp2p keypair
        let p2p_keypair = Keypair::from(secp256k1::Keypair::from(secret_key));
        let peer_id = p2p_keypair.public().to_peer_id();
        let evm_address = wallet.address();

        Ok(Self {
            wallet,
            p2p_keypair,
            peer_id,
            evm_address,
        })
    }

    /// Generates a completely new, random identity.
    pub fn generate() -> Self {
        let wallet = PrivateKeySigner::random();
        Self::new(wallet).expect("Failed to create identity from new wallet")
    }

    /// Loads an identity from a hex-encoded private key string.
    pub fn from_str(private_key: &str) -> Result<Self> {
        let wallet: PrivateKeySigner = private_key.parse()
            .map_err(|e| Error::Identity(format!("Failed to parse private key: {}", e)))?;
        Self::new(wallet)
    }
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("peer_id", &self.peer_id)
            .field("evm_address", &self.evm_address)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_identity() {
        let identity = Identity::generate();
        
        // Ensure we have valid fields
        assert!(!identity.peer_id.to_string().is_empty());
        assert_ne!(identity.evm_address, Address::ZERO);
    }

    #[test]
    fn test_identity_from_wallet() {
        let wallet = PrivateKeySigner::random();
        let identity = Identity::new(wallet.clone()).unwrap();
        
        // The identity should have the same EVM address as the wallet
        assert_eq!(identity.evm_address, wallet.address());
    }

    #[test]
    fn test_identity_from_str() {
        // Test with a valid private key
        let private_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let identity = Identity::from_str(private_key).unwrap();
        
        // Should produce consistent results
        assert!(!identity.peer_id.to_string().is_empty());
        assert_ne!(identity.evm_address, Address::ZERO);
        
        // Creating another identity with the same key should produce the same results
        let identity2 = Identity::from_str(private_key).unwrap();
        assert_eq!(identity.peer_id, identity2.peer_id);
        assert_eq!(identity.evm_address, identity2.evm_address);
    }

    #[test]
    fn test_identity_from_invalid_str() {
        // Test with invalid private key
        let result = Identity::from_str("invalid_key");
        assert!(result.is_err());
        
        // Test with empty string
        let result = Identity::from_str("");
        assert!(result.is_err());
        
        // Test with wrong length
        let result = Identity::from_str("1234");
        assert!(result.is_err());
    }

    #[test]
    fn test_identity_deterministic() {
        let private_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let identity1 = Identity::from_str(private_key).unwrap();
        let identity2 = Identity::from_str(private_key).unwrap();
        
        // Same private key should always produce the same identity
        assert_eq!(identity1.peer_id, identity2.peer_id);
        assert_eq!(identity1.evm_address, identity2.evm_address);
    }

    #[test]
    fn test_identity_uniqueness() {
        let identity1 = Identity::generate();
        let identity2 = Identity::generate();
        
        // Different identities should have different peer IDs and addresses
        assert_ne!(identity1.peer_id, identity2.peer_id);
        assert_ne!(identity1.evm_address, identity2.evm_address);
    }

    #[test]
    fn test_debug_format() {
        let identity = Identity::generate();
        let debug_str = format!("{:?}", identity);
        
        // Debug format should contain peer_id and evm_address
        assert!(debug_str.contains("peer_id"));
        assert!(debug_str.contains("evm_address"));
        // But should not contain sensitive private key information
        assert!(!debug_str.contains("wallet"));
        assert!(!debug_str.contains("keypair"));
    }

    #[test]
    fn test_identity_clone() {
        let identity1 = Identity::generate();
        let identity2 = identity1.clone();
        
        // Cloned identity should be identical
        assert_eq!(identity1.peer_id, identity2.peer_id);
        assert_eq!(identity1.evm_address, identity2.evm_address);
    }
}