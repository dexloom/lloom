//! Identity management for the Crowd Models P2P network.
//! 
//! This module provides a unified cryptographic identity that works with both
//! libp2p networking and Ethereum blockchain operations.

use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;
use libp2p::identity::{Keypair, PeerId, secp256k1};
use crate::error::{Error, Result};

/// A unified identity for nodes in the Crowd Models network.
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