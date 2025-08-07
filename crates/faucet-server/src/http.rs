//! HTTP server and API endpoints for the faucet server.

use crate::{
    config::FaucetConfig,
    email::{validate_email, EmailService},
    error::{FaucetError, FaucetResult},
    eth::EthereumClient,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, Path, State},
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{fs::OpenOptions, io::Write, net::SocketAddr, sync::Arc};
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

/// Request to subscribe (accepts any JSON data)
#[derive(Debug, Serialize, Deserialize)]
pub struct SubscribeRequest {
    #[serde(flatten)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

/// Response after successful subscription
#[derive(Debug, Serialize)]
pub struct SubscribeResponse {
    pub message: String,
    pub status: String,
}

/// Create the HTTP router with all endpoints
pub fn create_router(state: SharedState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/request", post(request_token))
        .route("/redeem", post(redeem_token))
        .route("/redeem/:token", get(redeem_token_get))
        .route("/subscribe", post(subscribe))
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
            "POST /redeem": "Redeem a token to receive funds (provide token) - JSON response",
            "GET /redeem/{token}": "Redeem a token to receive funds (token in URL) - HTML response",
            "POST /subscribe": "Subscribe with JSON data - saves to subscribers.csv",
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
    
    // Use the shared redemption logic
    let response = process_redeem_request(&state, &request).await?;
    Ok(Json(response))
}

/// GET endpoint for redeem functionality - returns HTML response
///
/// # Endpoint
/// `GET /redeem/{token}`
///
/// # Parameters
/// - `token`: The redemption token (path parameter)
///
/// # Returns
/// HTML page showing success or error message
async fn redeem_token_get(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> Result<Html<String>, Html<String>> {
    info!("GET redeem request for token: {}", token);
    
    let redeem_request = RedeemRequest { token: token.clone() };
    
    // Reuse the core redemption logic
    match process_redeem_request(&state, &redeem_request).await {
        Ok(response) => {
            let html = generate_success_html(&response);
            Ok(Html(html))
        }
        Err(e) => {
            warn!("GET redeem failed for token {}: {}", token, e);
            let error_html = generate_error_html(&e.to_string());
            Err(Html(error_html))
        }
    }
}

/// Subscribe endpoint - accepts JSON data and appends to CSV file
async fn subscribe(
    State(_state): State<SharedState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(request): Json<SubscribeRequest>,
) -> FaucetResult<Json<SubscribeResponse>> {
    info!("Subscribe request from {}: {:?}", addr.ip(), request.data);
    
    // Convert JSON data to CSV format
    let csv_line = json_to_csv(&request.data)?;
    
    // Append to subscribers.csv file
    append_to_csv_file("subscribers.csv", &csv_line)?;
    
    info!("Successfully appended subscription data to CSV file");
    
    Ok(Json(SubscribeResponse {
        message: "Successfully subscribed and data saved".to_string(),
        status: "success".to_string(),
    }))
}

/// Convert JSON data to CSV format
fn json_to_csv(data: &serde_json::Map<String, serde_json::Value>) -> FaucetResult<String> {
    // Create a CSV record with key-value pairs
    let mut csv_fields = Vec::new();
    
    // Add timestamp
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    csv_fields.push(timestamp);
    
    // Sort keys for consistent CSV format
    let mut sorted_keys: Vec<_> = data.keys().collect();
    sorted_keys.sort();
    
    // Add values in sorted key order
    for key in sorted_keys {
        let value = match data.get(key) {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Number(n)) => n.to_string(),
            Some(serde_json::Value::Bool(b)) => b.to_string(),
            Some(serde_json::Value::Null) => "".to_string(),
            Some(other) => other.to_string(),
            None => "".to_string(),
        };
        csv_fields.push(format!("{}:{}", key, value));
    }
    
    Ok(csv_fields.join(","))
}

/// Append data to CSV file
fn append_to_csv_file(filename: &str, csv_line: &str) -> FaucetResult<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(filename)
        .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Failed to open CSV file: {}", e)))?;
    
    writeln!(file, "{}", csv_line)
        .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Failed to write to CSV file: {}", e)))?;
    
    Ok(())
}

/// Extract core redemption logic to be shared between POST and GET endpoints
async fn process_redeem_request(
    state: &SharedState,
    request: &RedeemRequest,
) -> FaucetResult<RedeemResponse> {
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
    
    Ok(RedeemResponse {
        message: format!(
            "Successfully funded address {} with ETH. Transaction: {}",
            token_info.ethereum_address, transaction_hash
        ),
        transaction_hash,
        ethereum_address: token_info.ethereum_address,
    })
}

/// Generate HTML for successful redemption
fn generate_success_html(response: &RedeemResponse) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Faucet - Redemption Successful</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            max-width: 600px;
            margin: 50px auto;
            padding: 20px;
            background-color: #f5f5f5;
        }}
        .container {{
            background: white;
            padding: 30px;
            border-radius: 10px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            text-align: center;
        }}
        .success {{
            color: #28a745;
            font-size: 24px;
            margin-bottom: 20px;
        }}
        .details {{
            background: #f8f9fa;
            padding: 15px;
            border-radius: 5px;
            margin: 20px 0;
            text-align: left;
        }}
        .detail-row {{
            margin: 10px 0;
            word-break: break-all;
        }}
        .label {{
            font-weight: bold;
            color: #333;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1 class="success">✅ Redemption Successful!</h1>
        <p>{}</p>
        <div class="details">
            <div class="detail-row">
                <span class="label">Address:</span> {}
            </div>
            <div class="detail-row">
                <span class="label">Transaction Hash:</span> {}
            </div>
        </div>
        <p><em>Your tokens have been successfully sent to your wallet!</em></p>
    </div>
</body>
</html>"#,
        response.message,
        response.ethereum_address,
        response.transaction_hash
    )
}

/// Generate HTML for redemption error
fn generate_error_html(error_message: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Faucet - Redemption Error</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            max-width: 600px;
            margin: 50px auto;
            padding: 20px;
            background-color: #f5f5f5;
        }}
        .container {{
            background: white;
            padding: 30px;
            border-radius: 10px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            text-align: center;
        }}
        .error {{
            color: #dc3545;
            font-size: 24px;
            margin-bottom: 20px;
        }}
        .error-details {{
            background: #f8d7da;
            color: #721c24;
            padding: 15px;
            border-radius: 5px;
            margin: 20px 0;
            border: 1px solid #f5c6cb;
        }}
        .help-text {{
            color: #6c757d;
            font-style: italic;
            margin-top: 20px;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1 class="error">❌ Redemption Failed</h1>
        <div class="error-details">
            <strong>Error:</strong> {}
        </div>
        <p class="help-text">
            Please check that your redemption token is valid and hasn't already been used.
            If you continue to experience issues, please contact support.
        </p>
    </div>
</body>
</html>"#,
        error_message
    )
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
    info!("  GET  /            - Server information");
    info!("  GET  /health      - Health check");
    info!("  POST /request     - Request faucet token");
    info!("  POST /redeem      - Redeem token for funds (JSON)");
    info!("  GET  /redeem/{{token}} - Redeem token for funds (HTML)");
    info!("  POST /subscribe   - Subscribe with JSON data (saves to CSV)");
    
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
