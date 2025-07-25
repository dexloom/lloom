//! Lloom Client Library
//!
//! This crate provides functionality for the client application.

pub mod client_utils {
    use lloom_core::protocol::LlmRequest;

    /// Validate and parse bootstrap node addresses
    pub fn parse_bootstrap_nodes(addrs: &[String]) -> std::result::Result<Vec<String>, String> {
        for addr_str in addrs {
            // Basic validation - check if it looks like a multiaddr
            if !addr_str.starts_with('/') {
                return Err(format!("Invalid multiaddr format: {}", addr_str));
            }
            if !addr_str.contains("/tcp/") && !addr_str.contains("/udp/") {
                return Err(format!("Missing transport protocol in: {}", addr_str));
            }
        }
        Ok(addrs.to_vec())
    }

    /// Create an LLM request from command line arguments
    pub fn create_llm_request(
        model: String,
        prompt: String,
        system_prompt: Option<String>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> LlmRequest {
        LlmRequest {
            model,
            prompt,
            system_prompt,
            temperature,
            max_tokens,
        }
    }

    /// Validate temperature parameter
    pub fn validate_temperature(temp: f32) -> bool {
        temp >= 0.0 && temp <= 2.0
    }

    /// Validate max_tokens parameter
    pub fn validate_max_tokens(max_tokens: u32) -> bool {
        max_tokens > 0 && max_tokens <= 4096
    }

    /// Select the best executor from a set of discovered executors
    pub fn select_executor_index(executor_count: usize) -> Option<usize> {
        // For now, just select the first one (index 0)
        // In a real implementation, this could consider latency, load, reputation, etc.
        if executor_count > 0 {
            Some(0)
        } else {
            None
        }
    }

    /// Format response for display
    pub fn format_response(
        content: &str,
        model_used: &str,
        token_count: u32,
    ) -> String {
        format!(
            "Model: {}\nTokens: {}\n---\n{}",
            model_used, token_count, content
        )
    }
}

#[cfg(test)]
mod tests {
    use super::client_utils::*;

    #[test]
    fn test_parse_bootstrap_nodes_valid() {
        let addrs = vec![
            "/ip4/127.0.0.1/tcp/9000".to_string(),
            "/ip4/192.168.1.1/tcp/8000".to_string(),
        ];
        
        let result = parse_bootstrap_nodes(&addrs);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn test_parse_bootstrap_nodes_invalid() {
        let addrs = vec![
            "invalid-multiaddr".to_string(),
            "/ip4/127.0.0.1/tcp/9000".to_string(),
        ];
        
        let result = parse_bootstrap_nodes(&addrs);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_bootstrap_nodes_empty() {
        let addrs: Vec<String> = vec![];
        let result = parse_bootstrap_nodes(&addrs);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_bootstrap_nodes_missing_protocol() {
        let addrs = vec!["/ip4/127.0.0.1".to_string()];
        let result = parse_bootstrap_nodes(&addrs);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_llm_request_minimal() {
        let request = create_llm_request(
            "gpt-3.5-turbo".to_string(),
            "Hello world".to_string(),
            None,
            None,
            None,
        );
        
        assert_eq!(request.model, "gpt-3.5-turbo");
        assert_eq!(request.prompt, "Hello world");
        assert_eq!(request.system_prompt, None);
        assert_eq!(request.temperature, None);
        assert_eq!(request.max_tokens, None);
    }

    #[test]
    fn test_create_llm_request_full() {
        let request = create_llm_request(
            "gpt-4".to_string(),
            "Test prompt".to_string(),
            Some("You are a helpful assistant".to_string()),
            Some(0.7),
            Some(100),
        );
        
        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.prompt, "Test prompt");
        assert_eq!(request.system_prompt, Some("You are a helpful assistant".to_string()));
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(100));
    }

    #[test]
    fn test_validate_temperature() {
        assert!(validate_temperature(0.0));
        assert!(validate_temperature(1.0));
        assert!(validate_temperature(2.0));
        assert!(validate_temperature(0.5));
        
        assert!(!validate_temperature(-0.1));
        assert!(!validate_temperature(2.1));
        assert!(!validate_temperature(-1.0));
        assert!(!validate_temperature(3.0));
    }

    #[test]
    fn test_validate_max_tokens() {
        assert!(validate_max_tokens(1));
        assert!(validate_max_tokens(100));
        assert!(validate_max_tokens(4096));
        assert!(validate_max_tokens(2048));
        
        assert!(!validate_max_tokens(0));
        assert!(!validate_max_tokens(4097));
        assert!(!validate_max_tokens(10000));
    }

    #[test]
    fn test_select_executor_index() {
        // Empty set
        assert_eq!(select_executor_index(0), None);
        
        // Single executor
        assert_eq!(select_executor_index(1), Some(0));
        
        // Multiple executors (should return index 0)
        assert_eq!(select_executor_index(3), Some(0));
        assert_eq!(select_executor_index(10), Some(0));
    }

    #[test]
    fn test_format_response() {
        let formatted = format_response(
            "This is a test response",
            "gpt-3.5-turbo",
            15,
        );
        
        let expected = "Model: gpt-3.5-turbo\nTokens: 15\n---\nThis is a test response";
        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_format_response_empty_content() {
        let formatted = format_response("", "gpt-4", 0);
        let expected = "Model: gpt-4\nTokens: 0\n---\n";
        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_format_response_long_content() {
        let long_content = "This is a very long response ".repeat(10);
        let formatted = format_response(&long_content, "gpt-4-turbo", 300);
        
        assert!(formatted.contains("Model: gpt-4-turbo"));
        assert!(formatted.contains("Tokens: 300"));
        assert!(formatted.contains(&long_content));
    }
}