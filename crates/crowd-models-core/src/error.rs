//! Error types for the Crowd Models P2P network.

use thiserror::Error;

/// The main error type for the Crowd Models system.
#[derive(Error, Debug)]
pub enum Error {
    /// Network-related errors
    #[error("Network error: {0}")]
    Network(String),
    
    /// Identity-related errors
    #[error("Identity error: {0}")]
    Identity(String),
    
    /// Protocol-related errors
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    /// Blockchain-related errors
    #[error("Blockchain error: {0}")]
    Blockchain(String),
    
    /// Signature-related errors
    #[error("Signature error: {0}")]
    Signature(String),
    
    /// Verification-related errors
    #[error("Verification error: {0}")]
    Verification(String),
    
    /// Invalid signer error
    #[error("Invalid signer: expected {expected}, but recovered {recovered}")]
    InvalidSigner {
        expected: alloy::primitives::Address,
        recovered: alloy::primitives::Address,
    },
    
    /// Generic I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    /// Libp2p error
    #[error("Libp2p error: {0}")]
    Libp2p(String),
    
    /// Alloy error
    #[error("Alloy error: {0}")]
    Alloy(String),
    
    /// Other errors
    #[error("{0}")]
    Other(String),
}

/// Convenience type alias for Results using our Error type.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::error::Error as StdError;

    #[test]
    fn test_error_display() {
        let network_error = Error::Network("Connection failed".to_string());
        assert_eq!(format!("{}", network_error), "Network error: Connection failed");

        let identity_error = Error::Identity("Invalid key".to_string());
        assert_eq!(format!("{}", identity_error), "Identity error: Invalid key");

        let protocol_error = Error::Protocol("Malformed message".to_string());
        assert_eq!(format!("{}", protocol_error), "Protocol error: Malformed message");

        let blockchain_error = Error::Blockchain("Transaction failed".to_string());
        assert_eq!(format!("{}", blockchain_error), "Blockchain error: Transaction failed");

        let libp2p_error = Error::Libp2p("Peer unreachable".to_string());
        assert_eq!(format!("{}", libp2p_error), "Libp2p error: Peer unreachable");

        let alloy_error = Error::Alloy("RPC error".to_string());
        assert_eq!(format!("{}", alloy_error), "Alloy error: RPC error");

        let other_error = Error::Other("Unknown error".to_string());
        assert_eq!(format!("{}", other_error), "Unknown error");
    }

    #[test]
    fn test_error_debug() {
        let network_error = Error::Network("Connection failed".to_string());
        let debug_str = format!("{:?}", network_error);
        assert!(debug_str.contains("Network"));
        assert!(debug_str.contains("Connection failed"));
    }

    #[test]
    fn test_from_io_error() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let converted: Error = io_error.into();
        
        match converted {
            Error::Io(_) => (),
            _ => panic!("Expected IO error"),
        }
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json");
        assert!(json_error.is_err());
        
        let serde_error = json_error.unwrap_err();
        let converted: Error = serde_error.into();
        
        match converted {
            Error::Serialization(_) => (),
            _ => panic!("Expected Serialization error"),
        }
    }

    #[test]
    fn test_error_source() {
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let wrapped_error = Error::Io(io_error);
        
        assert!(StdError::source(&wrapped_error).is_some());
    }

    #[test]
    fn test_result_type_alias() {
        fn success_function() -> Result<i32> {
            Ok(42)
        }
        
        fn error_function() -> Result<i32> {
            Err(Error::Other("Test error".to_string()))
        }
        
        assert_eq!(success_function().unwrap(), 42);
        assert!(error_function().is_err());
    }

    #[test]
    fn test_error_chain() {
        let root_cause = io::Error::new(io::ErrorKind::BrokenPipe, "Broken pipe");
        let wrapped = Error::Io(root_cause);
        
        // Test that the error has a source
        let error_as_std: &dyn StdError = &wrapped;
        assert!(error_as_std.source().is_some());
        
        // Test the error message contains expected text
        let error_string = wrapped.to_string();
        assert!(error_string.contains("I/O error"));
    }

    #[test]
    fn test_error_variants() {
        let errors = vec![
            Error::Network("net".to_string()),
            Error::Identity("id".to_string()),
            Error::Protocol("proto".to_string()),
            Error::Blockchain("chain".to_string()),
            Error::Signature("sig".to_string()),
            Error::Verification("verify".to_string()),
            Error::InvalidSigner {
                expected: "0x0000000000000000000000000000000000000001".parse().unwrap(),
                recovered: "0x0000000000000000000000000000000000000002".parse().unwrap(),
            },
            Error::Libp2p("p2p".to_string()),
            Error::Alloy("alloy".to_string()),
            Error::Other("other".to_string()),
        ];
        
        // Ensure each error variant can be created and displayed
        for error in errors {
            let error_string = error.to_string();
            assert!(!error_string.is_empty());
        }
    }

    #[test]
    fn test_send_sync() {
        // Test that Error implements Send and Sync (compile-time test)
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        
        assert_send::<Error>();
        assert_sync::<Error>();
    }

    #[test]
    fn test_signature_error() {
        let signature_error = Error::Signature("Failed to sign message".to_string());
        assert_eq!(format!("{}", signature_error), "Signature error: Failed to sign message");
    }

    #[test]
    fn test_verification_error() {
        let verification_error = Error::Verification("Invalid signature".to_string());
        assert_eq!(format!("{}", verification_error), "Verification error: Invalid signature");
    }

    #[test]
    fn test_invalid_signer_error() {
        let expected: alloy::primitives::Address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".parse().unwrap();
        let recovered: alloy::primitives::Address = "0x123456789abcdef0123456789abcdef012345678".parse().unwrap();
        
        let invalid_signer_error = Error::InvalidSigner { expected, recovered };
        let error_string = format!("{}", invalid_signer_error);
        
        assert!(error_string.contains("Invalid signer"));
        assert!(error_string.contains(&format!("{}", expected)));
        assert!(error_string.contains(&format!("{}", recovered)));
    }
}