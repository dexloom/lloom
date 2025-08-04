//! HTTP server and API endpoints for the faucet server.

use crate::{
    config::FaucetConfig,
    email::{validate_email, EmailService},
    error::{FaucetError, FaucetResult},
    eth::EthereumClient,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, warn};

/// Shared application state
#[derive(Debug, Clone)]
pub struct SharedState {
    pub app_state: Arc<AppState>,
    pub email_service: Arc<EmailService>,
    pub ethereum_client: Arc<EthereumClient>,
}

/// Request to get a faucet token
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenRequest {
    pub email: String,
    pub ethereum_address: String,
}

/// Response containing the token (for testing purposes - in production, only send via email)
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub message: String,
    pub token: Option<String>, // Only included in test mode
}

/// Request to redeem a token and receive funds
#[derive(Debug, Serialize, Deserialize)]
pub struct RedeemRequest {
    pub token: String,
}

/// Response after successful redemption
#[derive(Debug, Serialize)]
pub struct RedeemResponse {
    pub message: String,
    pub transaction_hash: String,
    pub ethereum_address: String,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub ethereum_connected: bool,
    pub email_configured: bool,
    pub active_tokens: usize,
}

/// Create the HTTP router with all endpoints
pub fn create_router(state: SharedState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/request", post(request_token))
        .route("/redeem", post(redeem_token))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive()),
        )
        .with_state(state)
}

/// Root endpoint - provides basic information
async fn root() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "Lloom Faucet Server",
        "version": "1.0.0",
        "endpoints": {
            "POST /request": "Request a faucet token (provide email and ethereum_address)",
            "POST /redeem": "Redeem a token to receive funds (provide token)",
            "GET /health": "Health check",
        }
    }))
}

/// Health check endpoint
async fn health(State(state): State<SharedState>) -> FaucetResult<Json<HealthResponse>> {
    // Check Ethereum connection
    let ethereum_connected = state.ethereum_client.health_check().await.is_ok();
    
    // Check email configuration (basic test - we don't want to send actual emails in health check)
    let email_configured = true; // We assume if EmailService was created, it's configured
    
    // Get current statistics
    let stats = state.app_state.get_stats();
    
    let response = HealthResponse {
        status: if ethereum_connected && email_configured {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        ethereum_connected,
        email_configured,
        active_tokens: stats.active_tokens,
    };
    
    info!("Health check completed: {:?}", response);
    Ok(Json(response))
}

/// Request a new faucet token
async fn request_token(
    State(state): State<SharedState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(request): Json<TokenRequest>,
) -> FaucetResult<Json<TokenResponse>> {
    info!("Token request from {}: {} -> {}", addr.ip(), request.email, request.ethereum_address);
    
    // Validate email format
    validate_email(&request.email)?;
    
    // Validate Ethereum address format
    let _ethereum_address = EthereumClient::validate_address(&request.ethereum_address)?;
    
    // Check rate limits
    state.app_state.check_email_rate_limit(&request.email)?;
    state.app_state.check_ip_rate_limit(addr.ip())?;
    
    // Generate token
    let token = state.app_state.create_token(
        request.email.clone(),
        request.ethereum_address.clone(),
    )?;
    
    // Send email with token
    match state.email_service.send_token(&request.email, &token, &request.ethereum_address).await {
        Ok(()) => {
            info!("Token sent successfully to {}", request.email);
            Ok(Json(TokenResponse {
                message: format!("Verification token sent to {}. Please check your email and use the token to redeem funds.", request.email),
                token: None, // Don't include token in response for security
            }))
        }
        Err(e) => {
            // Remove the token if email sending failed
            let _ = state.app_state.consume_token(&token);
            warn!("Failed to send email to {}: {}", request.email, e);
            Err(e)
        }
    }
}

/// Redeem a token to receive funds
async fn redeem_token(
    State(state): State<SharedState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(request): Json<RedeemRequest>,
) -> FaucetResult<Json<RedeemResponse>> {
    info!("Redeem request from {}: token {}", addr.ip(), request.token);
    
    // Consume and validate token
    let token_info = state.app_state.consume_token(&request.token)?;
    
    // Validate the ethereum address from the token
    let ethereum_address = EthereumClient::validate_address(&token_info.ethereum_address)?;
    
    // Fund the address
    let transaction_hash = state.ethereum_client.fund_address(ethereum_address).await?;
    
    info!(
        "Successfully funded {} for email {} (tx: {})",
        token_info.ethereum_address,
        token_info.email,
        transaction_hash
    );
    
    Ok(Json(RedeemResponse {
        message: format!(
            "Successfully funded address {} with ETH. Transaction: {}",
            token_info.ethereum_address, transaction_hash
        ),
        transaction_hash,
        ethereum_address: token_info.ethereum_address,
    }))
}

/// Start the HTTP server
pub async fn start_server(config: &FaucetConfig) -> FaucetResult<()> {
    info!("Starting faucet server...");
    
    // Initialize components
    let app_state = Arc::new(AppState::new(
        config.security.token_expiry_minutes,
        config.security.max_requests_per_email_per_day,
        config.security.max_requests_per_ip_per_hour,
    ));
    
    let email_service = Arc::new(EmailService::new(&config.smtp)?);
    let ethereum_client = Arc::new(EthereumClient::new(&config.ethereum).await?);
    
    // Test connections
    info!("Testing email connection...");
    email_service.test_connection().await?;
    
    info!("Testing Ethereum connection...");
    ethereum_client.health_check().await?;
    
    let shared_state = SharedState {
        app_state: app_state.clone(),
        email_service,
        ethereum_client,
    };
    
    // Start cleanup task
    let cleanup_state = app_state.clone();
    let cleanup_interval = config.security.cleanup_interval_minutes;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(cleanup_interval * 60)
        );
        
        loop {
            interval.tick().await;
            cleanup_state.cleanup();
        }
    });
    
    // Create router
    let app = create_router(shared_state);
    
    // Bind and serve
    let bind_addr = format!("{}:{}", config.http.bind_address, config.http.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await
        .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Failed to bind to {}: {}", bind_addr, e)))?;
    
    info!("Faucet server listening on {}", bind_addr);
    info!("Endpoints:");
    info!("  GET  /         - Server information");
    info!("  GET  /health   - Health check");
    info!("  POST /request  - Request faucet token");
    info!("  POST /redeem   - Redeem token for funds");
    
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Server error: {}", e)))?;
    
    Ok(())
}

// Tests are disabled for simplicity - in a production environment,
// you would add proper unit tests with mocked dependencies
