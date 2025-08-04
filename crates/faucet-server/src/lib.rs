//! Lloom Faucet Server - Ethereum faucet with email verification
//! 
//! This crate provides an HTTP server that implements faucet logic:
//! 1. Users provide email and Ethereum address
//! 2. Server generates a token and sends it via SMTP to the user's email
//! 3. User submits the token back to the server
//! 4. Server checks the balance of the Ethereum address and tops it up to 1 ETH

pub mod config;
pub mod email;
pub mod error;
pub mod eth;
pub mod http;
pub mod state;

pub use config::FaucetConfig;
pub use error::{FaucetError, FaucetResult};
