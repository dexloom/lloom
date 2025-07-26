//! # Lloom Validator Library
//!
//! This library provides reusable components for building validator nodes in the Lloom P2P network.
//! Validators serve as stable supernodes for network bootstrap, discovery, and coordination.
//!
//! ## Usage
//!
//! ### As a Library
//!
//! Use this crate to build custom validator implementations:
//!
//! ```rust,no_run
//! use lloom_validator::{ExecutorTracker, identity::load_identity_from_file};
//! use lloom_validator::registry::ValidatorRegistry;
//!
//! // Load identity
//! let identity = load_identity_from_file(Some(std::path::Path::new("validator.key"))).unwrap();
//!
//! // Track executors
//! let mut tracker = ExecutorTracker::new();
//! // tracker.add_executor(peer_id);
//!
//! // Set up validator registry
//! let registry = ValidatorRegistry::new(identity);
//! ```
//!
//! ### As a Binary
//!
//! Run the included binary for a complete validator node:
//!
//! ```bash
//! lloom-validator --p2p-port 9000 --external-addr /ip4/1.2.3.4/tcp/9000
//! ```

/// Identity management utilities for validator nodes
pub mod identity {
    use lloom_core::{Identity, Result};
    use std::path::Path;

    /// Load identity from file or generate a new one
    pub fn load_identity_from_file(path: Option<&Path>) -> Result<Identity> {
        if let Some(path) = path {
            if path.exists() {
                let key_hex = std::fs::read_to_string(path)
                    .map_err(|e| lloom_core::Error::Io(e))?;
                let key_hex = key_hex.trim();
                Identity::from_str(key_hex)
            } else {
                let identity = Identity::generate();
                let key_hex = hex::encode(identity.wallet.to_bytes());
                std::fs::write(path, key_hex)
                    .map_err(|e| lloom_core::Error::Io(e))?;
                Ok(identity)
            }
        } else {
            Ok(Identity::generate())
        }
    }

    /// Asynchronous version of identity loading
    pub async fn load_identity_from_file_async(path: Option<&Path>) -> Result<Identity> {
        if let Some(path) = path {
            if path.exists() {
                let key_hex = tokio::fs::read_to_string(path).await
                    .map_err(|e| lloom_core::Error::Io(e))?;
                let key_hex = key_hex.trim();
                Identity::from_str(key_hex)
            } else {
                let identity = Identity::generate();
                let key_hex = hex::encode(identity.wallet.to_bytes());
                tokio::fs::write(path, key_hex).await
                    .map_err(|e| lloom_core::Error::Io(e))?;
                Ok(identity)
            }
        } else {
            Ok(Identity::generate())
        }
    }
}

/// Executor tracking and discovery utilities
pub mod tracking {
    use std::collections::HashSet;
    use libp2p::PeerId;

    /// Track and manage known executors
    #[derive(Debug, Default, Clone)]
    pub struct ExecutorTracker {
        known_executors: HashSet<PeerId>,
    }

    impl ExecutorTracker {
        /// Create a new executor tracker
        pub fn new() -> Self {
            Self {
                known_executors: HashSet::new(),
            }
        }

        /// Add an executor to the tracker
        /// Returns true if the executor was newly added, false if it was already known
        pub fn add_executor(&mut self, peer_id: PeerId) -> bool {
            self.known_executors.insert(peer_id)
        }

        /// Remove an executor from the tracker
        /// Returns true if the executor was removed, false if it wasn't found
        pub fn remove_executor(&mut self, peer_id: &PeerId) -> bool {
            self.known_executors.remove(peer_id)
        }

        /// Get the number of known executors
        pub fn get_executor_count(&self) -> usize {
            self.known_executors.len()
        }

        /// Check if an executor is known
        pub fn contains_executor(&self, peer_id: &PeerId) -> bool {
            self.known_executors.contains(peer_id)
        }

        /// Get all known executors
        pub fn get_all_executors(&self) -> Vec<PeerId> {
            self.known_executors.iter().cloned().collect()
        }

        /// Clear all known executors
        pub fn clear(&mut self) {
            self.known_executors.clear();
        }

        /// Check if the tracker is empty
        pub fn is_empty(&self) -> bool {
            self.known_executors.is_empty()
        }
    }
}

/// Validator registry and coordination utilities
pub mod registry {
    use lloom_core::Identity;
    use libp2p::PeerId;
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Information about a registered validator
    #[derive(Debug, Clone)]
    pub struct ValidatorInfo {
        pub peer_id: PeerId,
        pub last_seen: u64,
        pub external_address: Option<String>,
    }

    /// Registry for managing validator network coordination
    #[derive(Debug)]
    pub struct ValidatorRegistry {
        pub identity: Identity,
        pub validators: HashMap<PeerId, ValidatorInfo>,
    }

    impl ValidatorRegistry {
        /// Create a new validator registry
        pub fn new(identity: Identity) -> Self {
            Self {
                identity,
                validators: HashMap::new(),
            }
        }

        /// Register a validator
        pub fn register_validator(&mut self, peer_id: PeerId, external_address: Option<String>) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            self.validators.insert(peer_id, ValidatorInfo {
                peer_id,
                last_seen: now,
                external_address,
            });
        }

        /// Get all registered validators
        pub fn get_validators(&self) -> Vec<&ValidatorInfo> {
            self.validators.values().collect()
        }

        /// Remove stale validators (older than threshold)
        pub fn cleanup_stale_validators(&mut self, max_age_secs: u64) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            self.validators.retain(|_, info| {
                now - info.last_seen < max_age_secs
            });
        }
    }
}

// Re-export commonly used types
pub use tracking::ExecutorTracker;
pub use registry::{ValidatorRegistry, ValidatorInfo};

// Backward compatibility - re-export under old module name
#[deprecated(since = "0.1.0", note = "Use the new modular API instead")]
pub mod network_utils {
    pub use crate::identity::load_identity_from_file;
    pub use crate::tracking::ExecutorTracker;
}

#[cfg(test)]
mod tests {
    use super::{tracking::*, identity::*};
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_executor_tracker() {
        let mut tracker = ExecutorTracker::new();
        
        // Initially empty
        assert_eq!(tracker.get_executor_count(), 0);
        
        // Add some executors
        let peer1 = libp2p::PeerId::random();
        let peer2 = libp2p::PeerId::random();
        
        assert!(tracker.add_executor(peer1));
        assert_eq!(tracker.get_executor_count(), 1);
        assert!(tracker.contains_executor(&peer1));
        
        assert!(tracker.add_executor(peer2));
        assert_eq!(tracker.get_executor_count(), 2);
        
        // Adding the same executor again should return false
        assert!(!tracker.add_executor(peer1));
        assert_eq!(tracker.get_executor_count(), 2);
        
        // Remove an executor
        assert!(tracker.remove_executor(&peer1));
        assert_eq!(tracker.get_executor_count(), 1);
        assert!(!tracker.contains_executor(&peer1));
        assert!(tracker.contains_executor(&peer2));
        
        // Removing non-existent executor should return false
        assert!(!tracker.remove_executor(&peer1));
        assert_eq!(tracker.get_executor_count(), 1);
        
        // Get all executors
        let all_executors = tracker.get_all_executors();
        assert_eq!(all_executors.len(), 1);
        assert!(all_executors.contains(&peer2));
    }

    #[test]
    fn test_load_identity_from_file_new_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_owned();
        
        // Delete the file so it doesn't exist
        drop(temp_file);
        
        let identity = load_identity_from_file(Some(&path)).unwrap();
        
        // Should have created the file and generated a valid identity
        assert!(path.exists());
        assert!(!identity.peer_id.to_string().is_empty());
    }

    #[test]
    fn test_load_identity_from_file_existing_file() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        let test_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        writeln!(temp_file, "{}", test_key)?;
        
        let identity = load_identity_from_file(Some(temp_file.path())).unwrap();
        
        // Should load the same identity consistently
        let identity2 = load_identity_from_file(Some(temp_file.path())).unwrap();
        assert_eq!(identity.peer_id, identity2.peer_id);
        assert_eq!(identity.evm_address, identity2.evm_address);
        
        Ok(())
    }

    #[test]
    fn test_load_identity_without_file() {
        let identity = load_identity_from_file(None).unwrap();
        
        // Should generate a random identity
        assert!(!identity.peer_id.to_string().is_empty());
        
        // Each call should generate a different identity
        let identity2 = load_identity_from_file(None).unwrap();
        assert_ne!(identity.peer_id, identity2.peer_id);
    }

    #[test]
    fn test_executor_tracker_debug() {
        let tracker = ExecutorTracker::new();
        let debug_str = format!("{:?}", tracker);
        assert!(debug_str.contains("ExecutorTracker"));
        assert!(debug_str.contains("known_executors"));
    }

    #[test]
    fn test_executor_tracker_default() {
        let tracker = ExecutorTracker::default();
        assert_eq!(tracker.get_executor_count(), 0);
    }
}