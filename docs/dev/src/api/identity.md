# Identity Management

The identity module provides cryptographic identity functionality for all Lloom network participants. It manages key pairs for both P2P networking and Ethereum-compatible operations.

## Overview

Each Lloom node has a unified identity that serves multiple purposes:
- **P2P Networking**: libp2p peer identification and authentication
- **Message Signing**: EIP-712 compliant signatures for requests/responses
- **Blockchain Integration**: Ethereum address for on-chain operations
- **Service Discovery**: Identifying nodes by their capabilities

## Core Types

### `Identity`

The main identity structure combining P2P and Ethereum identities:

```rust
pub struct Identity {
    /// libp2p keypair for P2P operations
    pub keypair: Keypair,
    
    /// Derived peer ID for network identification
    pub peer_id: PeerId,
    
    /// Ethereum wallet for signing operations
    pub wallet: PrivateKeySigner,
    
    /// Ethereum address derived from wallet
    pub evm_address: Address,
}
```

## Creating Identities

### Generate New Identity

Create a completely new random identity:

```rust
use lloom_core::Identity;

let identity = Identity::generate();
println!("New Peer ID: {}", identity.peer_id);
println!("Ethereum Address: {}", identity.evm_address);
```

### From Private Key

Create identity from existing private key bytes:

```rust
use lloom_core::Identity;

// 32-byte private key
let private_key = [
    0xac, 0x09, 0x74, 0xbe, 0xc3, 0x9a, 0x17, 0xe3,
    0x6b, 0xa4, 0xa6, 0xb4, 0xd2, 0x38, 0xff, 0x94,
    0x4b, 0xac, 0xb4, 0x78, 0xcb, 0xed, 0x5e, 0xfc,
    0xae, 0x78, 0x4d, 0x7b, 0xf4, 0xf2, 0xff, 0x80
];

let identity = Identity::from_private_key(&private_key)?;
```

### From Hex String

Create identity from hex-encoded private key:

```rust
use lloom_core::Identity;

let hex_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
let identity = Identity::from_str(hex_key)?;
```

### From Mnemonic (BIP39)

Create identity from mnemonic phrase:

```rust
use lloom_core::identity::Identity;
use bip39::{Mnemonic, Language};

let mnemonic = Mnemonic::from_phrase(
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    Language::English
)?;

let identity = Identity::from_mnemonic(&mnemonic, 0)?; // index 0
```

## Persistence

### Save to File

Save identity to encrypted file:

```rust
use lloom_core::Identity;

let identity = Identity::generate();

// Save with encryption
identity.save_to_file_encrypted("~/.lloom/identity", "password123")?;

// Save unencrypted (not recommended for production)
identity.save_to_file("~/.lloom/identity")?;
```

### Load from File

Load previously saved identity:

```rust
use lloom_core::Identity;

// Load encrypted identity
let identity = Identity::from_file_encrypted("~/.lloom/identity", "password123")?;

// Load unencrypted
let identity = Identity::from_file("~/.lloom/identity")?;
```

## Key Derivation

### P2P Components

The P2P identity uses Ed25519 keys:

```rust
use lloom_core::Identity;
use libp2p::identity::Keypair;

let identity = Identity::generate();

// Access libp2p keypair
let keypair: &Keypair = &identity.keypair;

// Get public key
let public_key = keypair.public();

// Derive peer ID (deterministic from public key)
let peer_id = PeerId::from_public_key(&public_key);
assert_eq!(peer_id, identity.peer_id);
```

### Ethereum Components

The Ethereum identity uses secp256k1 keys:

```rust
use lloom_core::Identity;
use alloy::signers::Signer;

let identity = Identity::generate();

// Access Ethereum wallet
let wallet: &PrivateKeySigner = &identity.wallet;

// Get Ethereum address
let address = wallet.address();
assert_eq!(address, identity.evm_address);

// Sign message
let message = "Hello, Lloom!";
let signature = wallet.sign_message(message).await?;
```

## Identity Conversion

### Export Formats

Export identity in various formats:

```rust
use lloom_core::Identity;

let identity = Identity::generate();

// Export as hex private key
let hex_key = identity.to_hex_string();

// Export as bytes
let key_bytes = identity.to_bytes();

// Export public information only
let public_info = identity.public_info();
println!("PeerId: {}", public_info.peer_id);
println!("Address: {}", public_info.evm_address);
```

### Import Formats

Import from various sources:

```rust
use lloom_core::Identity;

// From Ethereum keystore JSON
let keystore = std::fs::read_to_string("keystore.json")?;
let identity = Identity::from_keystore(&keystore, "password")?;

// From raw private key file
let key_bytes = std::fs::read("private.key")?;
let identity = Identity::from_private_key(&key_bytes)?;
```

## Multi-Identity Management

### Identity Pool

Manage multiple identities:

```rust
use lloom_core::identity::IdentityPool;

let mut pool = IdentityPool::new();

// Add identities
pool.add_identity("client", Identity::generate());
pool.add_identity("executor", Identity::generate());

// Access by name
let client_id = pool.get("client")?;

// List all identities
for (name, identity) in pool.list() {
    println!("{}: {}", name, identity.peer_id);
}

// Save pool to directory
pool.save_to_directory("~/.lloom/identities")?;

// Load pool from directory
let pool = IdentityPool::load_from_directory("~/.lloom/identities")?;
```

### HD Wallet Derivation

Derive multiple identities from single seed:

```rust
use lloom_core::identity::{Identity, HDIdentity};

let seed = "your seed phrase here";
let hd_identity = HDIdentity::from_seed(seed)?;

// Derive identities at different paths
let client_identity = hd_identity.derive(0)?;     // m/44'/60'/0'/0/0
let executor_identity = hd_identity.derive(1)?;   // m/44'/60'/0'/0/1
let validator_identity = hd_identity.derive(2)?;  // m/44'/60'/0'/0/2
```

## Security Considerations

### Key Storage Best Practices

1. **Encryption at Rest**:
   ```rust
   // Always encrypt when storing
   identity.save_to_file_encrypted(path, password)?;
   
   // Use strong passwords
   let password = generate_secure_password(32);
   ```

2. **Memory Protection**:
   ```rust
   use zeroize::Zeroize;
   
   // Clear sensitive data from memory
   let mut private_key = identity.to_bytes();
   // ... use private_key
   private_key.zeroize();
   ```

3. **Access Control**:
   ```rust
   use std::fs;
   use std::os::unix::fs::PermissionsExt;
   
   // Set restrictive file permissions
   let path = "~/.lloom/identity";
   identity.save_to_file(path)?;
   
   let metadata = fs::metadata(path)?;
   let mut permissions = metadata.permissions();
   permissions.set_mode(0o600); // Read/write for owner only
   fs::set_permissions(path, permissions)?;
   ```

### Hardware Security Module (HSM) Integration

For production environments:

```rust
use lloom_core::identity::HsmIdentity;

// Initialize HSM
let hsm = HsmIdentity::new("pkcs11:token=lloom;pin=1234")?;

// Generate key in HSM
let identity = hsm.generate_identity("executor-key-1")?;

// Sign without exposing private key
let message = b"sign this";
let signature = hsm.sign_message(&identity, message)?;
```

### Key Rotation

Implement key rotation policies:

```rust
use lloom_core::identity::{Identity, RotationPolicy};

let policy = RotationPolicy {
    max_age_days: 90,
    max_signatures: 1_000_000,
    rotation_callback: |old_identity, new_identity| {
        // Update network peers
        announce_key_rotation(old_identity, new_identity)?;
        Ok(())
    },
};

let rotator = IdentityRotator::new(policy);
let new_identity = rotator.rotate_if_needed(&current_identity)?;
```

## Utilities

### Identity Verification

Verify identity properties:

```rust
use lloom_core::identity::{Identity, verify_identity};

let identity = Identity::from_file("identity.key")?;

// Verify consistency
assert!(verify_identity(&identity)?);

// Verify ownership
let message = b"test message";
let signature = identity.sign_message(message)?;
assert!(identity.verify_signature(message, &signature)?);
```

### Address Formatting

Format addresses for display:

```rust
use lloom_core::identity::format_utils;

let identity = Identity::generate();

// Short peer ID
let short_peer_id = format_utils::short_peer_id(&identity.peer_id);
// "12D3Ko...cWz"

// Checksummed Ethereum address
let checksum_address = format_utils::to_checksum_address(&identity.evm_address);
// "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a"

// ENS-style display
let ens_display = format_utils::ens_style(&identity.evm_address);
// "0x742d...5d3a"
```

### Identity Metadata

Attach metadata to identities:

```rust
use lloom_core::identity::{Identity, IdentityMetadata};

let mut identity = Identity::generate();

let metadata = IdentityMetadata {
    name: Some("Main Executor".to_string()),
    created_at: chrono::Utc::now(),
    tags: vec!["executor", "gpu-enabled"],
    custom: serde_json::json!({
        "region": "us-east-1",
        "capabilities": ["gpt-4", "llama-2"]
    }),
};

identity.set_metadata(metadata);
identity.save_with_metadata("identity.json")?;
```

## Testing Utilities

### Mock Identities

Create deterministic identities for testing:

```rust
#[cfg(test)]
mod tests {
    use lloom_core::identity::test_utils;
    
    #[test]
    fn test_with_fixed_identity() {
        // Always generates same identity
        let identity = test_utils::fixed_identity(0);
        assert_eq!(
            identity.peer_id.to_string(),
            "12D3KooWQcD3cmHSXqHV2WpbDHDCZqhKUdVfTzQ5KjDa6EqnGcWz"
        );
        
        // Different index = different identity
        let identity2 = test_utils::fixed_identity(1);
        assert_ne!(identity.peer_id, identity2.peer_id);
    }
}
```

### Identity Assertions

Test helpers for identity validation:

```rust
#[cfg(test)]
use lloom_core::identity::test_utils::{assert_valid_identity, assert_identities_match};

#[test]
fn test_identity_creation() {
    let identity = Identity::generate();
    assert_valid_identity(&identity);
    
    let loaded = Identity::from_file("test.key").unwrap();
    assert_identities_match(&identity, &loaded);
}
```

## Common Patterns

### Lazy Identity Loading

Load identity only when needed:

```rust
use once_cell::sync::Lazy;
use lloom_core::Identity;

static IDENTITY: Lazy<Identity> = Lazy::new(|| {
    Identity::from_file_encrypted(
        "~/.lloom/identity",
        std::env::var("LLOOM_KEY_PASSWORD").expect("Password required")
    ).expect("Failed to load identity")
});

fn get_identity() -> &'static Identity {
    &IDENTITY
}
```

### Identity with Retry

Handle transient failures:

```rust
use lloom_core::Identity;
use tokio_retry::{Retry, strategy::ExponentialBackoff};

async fn load_identity_with_retry() -> Result<Identity> {
    let retry_strategy = ExponentialBackoff::from_millis(100)
        .max_delay(Duration::from_secs(2))
        .take(5);
    
    Retry::spawn(retry_strategy, || async {
        Identity::from_file_encrypted("~/.lloom/identity", get_password().await?)
    }).await
}
```

## Troubleshooting

### Common Errors

1. **Invalid Private Key Format**
   ```
   Error: Invalid private key length: expected 32 bytes, got 31
   Solution: Ensure hex strings don't have '0x' prefix and are exactly 64 characters
   ```

2. **Key Derivation Mismatch**
   ```
   Error: PeerId doesn't match expected value
   Solution: Ensure using same key derivation method (Ed25519 for P2P)
   ```

3. **Permission Denied**
   ```
   Error: Permission denied accessing identity file
   Solution: Check file permissions and ownership
   ```

### Debug Utilities

Enable detailed logging:

```rust
use tracing::debug;

let identity = Identity::generate();
debug!("Identity created: peer_id={}, address={}", 
    identity.peer_id, 
    identity.evm_address
);
```