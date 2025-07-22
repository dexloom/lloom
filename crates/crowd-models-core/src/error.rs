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