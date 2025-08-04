//! Error handling for the faucet server.

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use thiserror::Error;

/// Faucet server error types
#[derive(Error, Debug)]
pub enum FaucetError {
    #[error("Invalid email address: {0}")]
    InvalidEmail(String),

    #[error("Invalid Ethereum address: {0}")]
    InvalidEthereumAddress(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Token not found or expired")]
    TokenNotFound,

    #[error("Email sending failed: {0}")]
    EmailError(#[from] lettre::transport::smtp::Error),

    #[error("Ethereum transaction failed: {0}")]
    EthereumError(String),

    #[error("Insufficient faucet balance")]
    InsufficientFaucetBalance,

    #[error("Address already has sufficient balance")]
    SufficientBalance,

    #[error("Configuration error: {0}")]
    ConfigError(#[from] config::ConfigError),

    #[error("Internal server error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for FaucetError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            FaucetError::InvalidEmail(_) | FaucetError::InvalidEthereumAddress(_) => {
                (StatusCode::BAD_REQUEST, self.to_string())
            }
            FaucetError::RateLimitExceeded(_) => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            FaucetError::TokenNotFound => (StatusCode::NOT_FOUND, self.to_string()),
            FaucetError::SufficientBalance => (StatusCode::CONFLICT, self.to_string()),
            FaucetError::InsufficientFaucetBalance => {
                (StatusCode::SERVICE_UNAVAILABLE, self.to_string())
            }
            FaucetError::EmailError(_) | FaucetError::EthereumError(_) | FaucetError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            FaucetError::ConfigError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error".to_string())
            }
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

/// Result type alias for faucet operations
pub type FaucetResult<T> = Result<T, FaucetError>;
