//! Email functionality for sending faucet tokens.

use crate::config::SmtpConfig;
use crate::error::{FaucetError, FaucetResult};
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    Message, SmtpTransport, Transport,
};
use tracing::{debug, error, info};

/// Email service for sending faucet tokens
#[derive(Debug)]
pub struct EmailService {
    transport: SmtpTransport,
    from_address: Mailbox,
    subject: String,
}

impl EmailService {
    /// Create a new email service
    pub fn new(config: &SmtpConfig) -> FaucetResult<Self> {
        let from_address: Mailbox = config.from_address.parse()
            .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Invalid from address: {}", e)))?;

        let credentials = Credentials::new(config.username.clone(), config.password.clone());

        let transport = SmtpTransport::relay(&config.server)
            .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Failed to create SMTP transport: {}", e)))?
            .port(config.port)
            .credentials(credentials)
            .build();

        Ok(Self {
            transport,
            from_address,
            subject: config.subject.clone(),
        })
    }

    /// Send a token to the specified email address
    pub async fn send_token(&self, to_email: &str, token: &str, ethereum_address: &str) -> FaucetResult<()> {
        let to_address: Mailbox = to_email.parse()
            .map_err(|e| FaucetError::InvalidEmail(format!("Invalid email address: {}", e)))?;

        let body = self.create_email_body(token, ethereum_address);

        let email = Message::builder()
            .from(self.from_address.clone())
            .to(to_address)
            .subject(&self.subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body)
            .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Failed to build email: {}", e)))?;

        debug!("Sending token email to: {}", to_email);

        // Send email in a blocking task to avoid blocking the async runtime
        let transport = self.transport.clone();
        let result = tokio::task::spawn_blocking(move || transport.send(&email)).await
            .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Task join error: {}", e)))?;

        match result {
            Ok(response) => {
                info!("Successfully sent token email to: {} (response: {:?})", to_email, response);
                Ok(())
            }
            Err(e) => {
                error!("Failed to send email to {}: {}", to_email, e);
                Err(FaucetError::EmailError(e))
            }
        }
    }

    /// Create the email body with the token
    fn create_email_body(&self, token: &str, ethereum_address: &str) -> String {
        format!(
            r#"Welcome to the Ethereum Faucet!

You have requested funds for the Ethereum address: {}

Your verification token is: {}

To complete the funding process:
1. Use this token to verify your request
2. Send a POST request to /redeem with your token
3. Your address will be funded with 1 ETH (up to the target amount)

This token will expire in 15 minutes for security reasons.

If you did not request this token, please ignore this email.

---
Lloom Faucet Service
"#,
            ethereum_address, token
        )
    }

    /// Test the email configuration
    pub async fn test_connection(&self) -> FaucetResult<()> {
        debug!("Testing SMTP connection");

        let transport = self.transport.clone();
        let result = tokio::task::spawn_blocking(move || transport.test_connection()).await
            .map_err(|e| FaucetError::Internal(anyhow::anyhow!("Task join error: {}", e)))?;

        match result {
            Ok(true) => {
                info!("SMTP connection test successful");
                Ok(())
            }
            Ok(false) => {
                error!("SMTP connection test failed");
                Err(FaucetError::Internal(anyhow::anyhow!("SMTP connection test failed")))
            }
            Err(e) => {
                error!("SMTP connection error: {}", e);
                Err(FaucetError::EmailError(e))
            }
        }
    }
}

/// Validate email address format
pub fn validate_email(email: &str) -> FaucetResult<()> {
    // Basic email validation - could be enhanced with a proper email validation library
    if !email.contains('@') || !email.contains('.') || email.len() < 5 {
        return Err(FaucetError::InvalidEmail(email.to_string()));
    }

    // Check for invalid characters
    if email.contains(' ') || email.starts_with('@') || email.ends_with('@') {
        return Err(FaucetError::InvalidEmail(email.to_string()));
    }

    // Basic domain validation
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(FaucetError::InvalidEmail(email.to_string()));
    }

    let domain = parts[1];
    if !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
        return Err(FaucetError::InvalidEmail(email.to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SmtpConfig;

    fn get_test_smtp_config() -> SmtpConfig {
        SmtpConfig {
            server: "smtp.example.com".to_string(),
            port: 587,
            username: "test@example.com".to_string(),
            password: "password".to_string(),
            from_address: "faucet@example.com".to_string(),
            subject: "Test Faucet Token".to_string(),
        }
    }

    #[test]
    fn test_create_email_body() {
        let config = get_test_smtp_config();
        let email_service = EmailService::new(&config).unwrap();
        
        let body = email_service.create_email_body("test-token-123", "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a");
        
        assert!(body.contains("test-token-123"));
        assert!(body.contains("0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a"));
        assert!(body.contains("verification token"));
        assert!(body.contains("15 minutes"));
    }

    #[test]
    fn test_validate_email_valid() {
        assert!(validate_email("test@example.com").is_ok());
        assert!(validate_email("user.name@domain.co.uk").is_ok());
        assert!(validate_email("test123@test-domain.com").is_ok());
    }

    #[test]
    fn test_validate_email_invalid() {
        assert!(validate_email("invalid").is_err());
        assert!(validate_email("@example.com").is_err());
        assert!(validate_email("test@").is_err());
        assert!(validate_email("test @example.com").is_err());
        assert!(validate_email("test@.com").is_err());
        assert!(validate_email("test@com.").is_err());
        assert!(validate_email("").is_err());
        assert!(validate_email("a@b").is_err()); // too short domain
    }

    #[test]
    fn test_email_service_creation() {
        let config = get_test_smtp_config();
        let result = EmailService::new(&config);
        
        // Should succeed with valid config
        assert!(result.is_ok());
    }

    #[test]
    fn test_email_service_creation_invalid_from() {
        let mut config = get_test_smtp_config();
        config.from_address = "invalid-email".to_string();
        
        let result = EmailService::new(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_email_body_formatting() {
        let config = get_test_smtp_config();
        let email_service = EmailService::new(&config).unwrap();
        
        let token = "abc123def456";
        let address = "0x1234567890abcdef1234567890abcdef12345678";
        let body = email_service.create_email_body(token, address);
        
        // Check that both token and address are present
        assert!(body.contains(token));
        assert!(body.contains(address));
        
        // Check for key phrases
        assert!(body.contains("verification token"));
        assert!(body.contains("POST request to /redeem"));
        assert!(body.contains("expire in 15 minutes"));
        assert!(body.contains("Lloom Faucet Service"));
    }

    // Note: We don't test actual email sending in unit tests as it requires
    // real SMTP credentials and network access. Those would be integration tests.
    
    #[test]
    fn test_email_validation_edge_cases() {
        // Test minimum valid email
        assert!(validate_email("a@b.c").is_ok());
        
        // Test with numbers and special characters
        assert!(validate_email("test123@example-domain.com").is_ok());
        assert!(validate_email("user.name+tag@domain.co.uk").is_ok());
        
        // Test invalid cases
        assert!(validate_email("test@@example.com").is_err());
        assert!(validate_email("test@example@com").is_err());
        assert!(validate_email("test@").is_err());
        assert!(validate_email("@test.com").is_err());
    }

    #[test]
    fn test_smtp_config_validation() {
        let config = get_test_smtp_config();
        
        // Valid config should create service successfully
        assert!(EmailService::new(&config).is_ok());
        
        // Test with different ports
        let mut config_465 = config.clone();
        config_465.port = 465;
        assert!(EmailService::new(&config_465).is_ok());
        
        let mut config_25 = config.clone();
        config_25.port = 25;
        assert!(EmailService::new(&config_25).is_ok());
    }
}
