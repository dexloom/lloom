//! Crowd Models Accountant Library
//! 
//! This crate provides functionality for the accountant node.

pub mod network_utils {
    use crowd_models_core::{Identity, Result};
    use std::collections::HashSet;
    use libp2p::PeerId;

    /// Helper functions for accountant network operations
    pub fn load_identity_from_file(path: Option<&std::path::Path>) -> Result<Identity> {
        if let Some(path) = path {
            if path.exists() {
                let key_hex = std::fs::read_to_string(path)
                    .map_err(|e| crowd_models_core::Error::Io(e))?;
                let key_hex = key_hex.trim();
                Identity::from_str(key_hex)
            } else {
                let identity = Identity::generate();
                let key_hex = hex::encode(identity.wallet.to_bytes());
                std::fs::write(path, key_hex)
                    .map_err(|e| crowd_models_core::Error::Io(e))?;
                Ok(identity)
            }
        } else {
            Ok(Identity::generate())
        }
    }

    /// Track and manage known executors
    #[derive(Debug, Default)]
    pub struct ExecutorTracker {
        known_executors: HashSet<PeerId>,
    }

    impl ExecutorTracker {
        pub fn new() -> Self {
            Self {
                known_executors: HashSet::new(),
            }
        }

        pub fn add_executor(&mut self, peer_id: PeerId) -> bool {
            self.known_executors.insert(peer_id)
        }

        pub fn remove_executor(&mut self, peer_id: &PeerId) -> bool {
            self.known_executors.remove(peer_id)
        }

        pub fn get_executor_count(&self) -> usize {
            self.known_executors.len()
        }

        pub fn contains_executor(&self, peer_id: &PeerId) -> bool {
            self.known_executors.contains(peer_id)
        }

        pub fn get_all_executors(&self) -> Vec<PeerId> {
            self.known_executors.iter().cloned().collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::network_utils::*;
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