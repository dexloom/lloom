//! State management for tokens and rate limiting.

use crate::error::{FaucetError, FaucetResult};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Token information stored in memory
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub email: String,
    pub ethereum_address: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Rate limiting entry
#[derive(Debug, Clone)]
pub struct RateLimitEntry {
    pub count: u32,
    pub window_start: DateTime<Utc>,
}

/// Shared application state
#[derive(Debug)]
pub struct AppState {
    /// Active tokens (token -> TokenInfo)
    pub tokens: Arc<DashMap<String, TokenInfo>>,
    
    /// Email rate limiting (email -> RateLimitEntry)
    pub email_limits: Arc<DashMap<String, RateLimitEntry>>,
    
    /// IP rate limiting (IP -> RateLimitEntry)
    pub ip_limits: Arc<DashMap<IpAddr, RateLimitEntry>>,
    
    /// Configuration for rate limits and token expiry
    pub token_expiry_minutes: u64,
    pub max_requests_per_email_per_day: u32,
    pub max_requests_per_ip_per_hour: u32,
}

impl AppState {
    /// Create new application state
    pub fn new(
        token_expiry_minutes: u64,
        max_requests_per_email_per_day: u32,
        max_requests_per_ip_per_hour: u32,
    ) -> Self {
        Self {
            tokens: Arc::new(DashMap::new()),
            email_limits: Arc::new(DashMap::new()),
            ip_limits: Arc::new(DashMap::new()),
            token_expiry_minutes,
            max_requests_per_email_per_day,
            max_requests_per_ip_per_hour,
        }
    }
    
    /// Generate and store a new token
    pub fn create_token(&self, email: String, ethereum_address: String) -> FaucetResult<String> {
        let token = Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = now + Duration::minutes(self.token_expiry_minutes as i64);
        
        let token_info = TokenInfo {
            email: email.clone(),
            ethereum_address,
            created_at: now,
            expires_at,
        };
        
        self.tokens.insert(token.clone(), token_info);
        
        debug!("Created token for email: {}", email);
        Ok(token)
    }
    
    /// Validate and consume a token
    pub fn consume_token(&self, token: &str) -> FaucetResult<TokenInfo> {
        let token_info = self.tokens.remove(token)
            .map(|(_, info)| info)
            .ok_or(FaucetError::TokenNotFound)?;
        
        let now = Utc::now();
        if now > token_info.expires_at {
            warn!("Attempted to use expired token for email: {}", token_info.email);
            return Err(FaucetError::TokenNotFound);
        }
        
        debug!("Consumed token for email: {}", token_info.email);
        Ok(token_info)
    }
    
    /// Check if email rate limit is exceeded
    pub fn check_email_rate_limit(&self, email: &str) -> FaucetResult<()> {
        let now = Utc::now();
        
        if let Some(mut entry) = self.email_limits.get_mut(email) {
            // Check if we're in a new day window
            let hours_since_start = now.signed_duration_since(entry.window_start).num_hours();
            if hours_since_start >= 24 {
                // Reset the window
                entry.count = 1;
                entry.window_start = now;
                return Ok(());
            }
            
            if entry.count >= self.max_requests_per_email_per_day {
                return Err(FaucetError::RateLimitExceeded(
                    format!("Email {} has exceeded daily limit", email)
                ));
            }
            
            entry.count += 1;
        } else {
            // First request from this email
            self.email_limits.insert(
                email.to_string(),
                RateLimitEntry {
                    count: 1,
                    window_start: now,
                },
            );
        }
        
        Ok(())
    }
    
    /// Check if IP rate limit is exceeded
    pub fn check_ip_rate_limit(&self, ip: IpAddr) -> FaucetResult<()> {
        let now = Utc::now();
        
        if let Some(mut entry) = self.ip_limits.get_mut(&ip) {
            // Check if we're in a new hour window
            let minutes_since_start = now.signed_duration_since(entry.window_start).num_minutes();
            if minutes_since_start >= 60 {
                // Reset the window
                entry.count = 1;
                entry.window_start = now;
                return Ok(());
            }
            
            if entry.count >= self.max_requests_per_ip_per_hour {
                return Err(FaucetError::RateLimitExceeded(
                    format!("IP {} has exceeded hourly limit", ip)
                ));
            }
            
            entry.count += 1;
        } else {
            // First request from this IP
            self.ip_limits.insert(
                ip,
                RateLimitEntry {
                    count: 1,
                    window_start: now,
                },
            );
        }
        
        Ok(())
    }
    
    /// Clean up expired tokens and reset rate limits
    pub fn cleanup(&self) {
        let now = Utc::now();
        
        // Clean up expired tokens
        let mut expired_tokens = Vec::new();
        for entry in self.tokens.iter() {
            if now > entry.value().expires_at {
                expired_tokens.push(entry.key().clone());
            }
        }
        
        for token in expired_tokens {
            self.tokens.remove(&token);
        }
        
        // Clean up old email rate limit entries (older than 25 hours to be safe)
        let mut old_email_entries = Vec::new();
        for entry in self.email_limits.iter() {
            let hours_since_start = now.signed_duration_since(entry.value().window_start).num_hours();
            if hours_since_start > 25 {
                old_email_entries.push(entry.key().clone());
            }
        }
        
        for email in old_email_entries {
            self.email_limits.remove(&email);
        }
        
        // Clean up old IP rate limit entries (older than 2 hours to be safe)
        let mut old_ip_entries = Vec::new();
        for entry in self.ip_limits.iter() {
            let minutes_since_start = now.signed_duration_since(entry.value().window_start).num_minutes();
            if minutes_since_start > 120 {
                old_ip_entries.push(*entry.key());
            }
        }
        
        for ip in old_ip_entries {
            self.ip_limits.remove(&ip);
        }
        
        let token_count = self.tokens.len();
        let email_limit_count = self.email_limits.len();
        let ip_limit_count = self.ip_limits.len();
        
        info!(
            "Cleanup completed: {} active tokens, {} email limits, {} IP limits",
            token_count, email_limit_count, ip_limit_count
        );
    }
    
    /// Get statistics about current state
    pub fn get_stats(&self) -> StateStats {
        StateStats {
            active_tokens: self.tokens.len(),
            email_limits: self.email_limits.len(),
            ip_limits: self.ip_limits.len(),
        }
    }
}

/// Statistics about the current state
#[derive(Debug, Clone)]
pub struct StateStats {
    pub active_tokens: usize,
    pub email_limits: usize,
    pub ip_limits: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::thread;
    use std::time::Duration as StdDuration;

    #[test]
    fn test_create_and_consume_token() {
        let state = AppState::new(15, 1, 5);
        let email = "test@example.com".to_string();
        let address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string();
        
        // Create token
        let token = state.create_token(email.clone(), address.clone()).unwrap();
        assert!(!token.is_empty());
        
        // Consume token
        let token_info = state.consume_token(&token).unwrap();
        assert_eq!(token_info.email, email);
        assert_eq!(token_info.ethereum_address, address);
        
        // Token should be gone now
        assert!(state.consume_token(&token).is_err());
    }
    
    #[test]
    fn test_expired_token() {
        let state = AppState::new(0, 1, 5); // 0 minute expiry
        let email = "test@example.com".to_string();
        let address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string();
        
        let token = state.create_token(email, address).unwrap();
        
        // Wait a bit to ensure expiry
        thread::sleep(StdDuration::from_millis(10));
        
        // Token should be expired
        assert!(state.consume_token(&token).is_err());
    }
    
    #[test]
    fn test_email_rate_limiting() {
        let state = AppState::new(15, 2, 5); // Max 2 requests per day
        let email = "test@example.com";
        
        // First request should succeed
        assert!(state.check_email_rate_limit(email).is_ok());
        
        // Second request should succeed
        assert!(state.check_email_rate_limit(email).is_ok());
        
        // Third request should fail
        assert!(state.check_email_rate_limit(email).is_err());
    }
    
    #[test]
    fn test_ip_rate_limiting() {
        let state = AppState::new(15, 1, 2); // Max 2 requests per hour
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        
        // First request should succeed
        assert!(state.check_ip_rate_limit(ip).is_ok());
        
        // Second request should succeed
        assert!(state.check_ip_rate_limit(ip).is_ok());
        
        // Third request should fail
        assert!(state.check_ip_rate_limit(ip).is_err());
    }
    
    #[test]
    fn test_cleanup() {
        let state = AppState::new(15, 1, 5);
        let email = "test@example.com".to_string();
        let address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string();
        
        // Create some tokens
        let _token1 = state.create_token(email.clone(), address.clone()).unwrap();
        let _token2 = state.create_token("test2@example.com".to_string(), address).unwrap();
        
        assert_eq!(state.tokens.len(), 2);
        
        // Add some rate limit entries
        state.check_email_rate_limit(&email).unwrap();
        state.check_ip_rate_limit(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))).unwrap();
        
        // Cleanup shouldn't remove anything yet (tokens not expired)
        state.cleanup();
        assert_eq!(state.tokens.len(), 2);
    }
    
    #[test]
    fn test_state_stats() {
        let state = AppState::new(15, 1, 5);
        let stats = state.get_stats();
        
        assert_eq!(stats.active_tokens, 0);
        assert_eq!(stats.email_limits, 0);
        assert_eq!(stats.ip_limits, 0);
        
        // Add some data
        let _token = state.create_token("test@example.com".to_string(), "0x123".to_string()).unwrap();
        state.check_email_rate_limit("test@example.com").unwrap();
        state.check_ip_rate_limit(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))).unwrap();
        
        let stats = state.get_stats();
        assert_eq!(stats.active_tokens, 1);
        assert_eq!(stats.email_limits, 1);
        assert_eq!(stats.ip_limits, 1);
    }
    
    #[test]
    fn test_nonexistent_token() {
        let state = AppState::new(15, 1, 5);
        let result = state.consume_token("nonexistent-token");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;
        
        let state = Arc::new(AppState::new(15, 10, 10));
        let mut handles = vec![];
        
        // Spawn multiple threads creating tokens
        for i in 0..5 {
            let state_clone = Arc::clone(&state);
            let handle = thread::spawn(move || {
                let email = format!("test{}@example.com", i);
                let address = "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a".to_string();
                state_clone.create_token(email, address).unwrap()
            });
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        let mut tokens = vec![];
        for handle in handles {
            tokens.push(handle.join().unwrap());
        }
        
        // Should have 5 tokens
        assert_eq!(state.tokens.len(), 5);
        
        // All tokens should be consumable
        for token in tokens {
            assert!(state.consume_token(&token).is_ok());
        }
        
        // No tokens should remain
        assert_eq!(state.tokens.len(), 0);
    }
}
