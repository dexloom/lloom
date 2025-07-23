//! LLM client for interacting with various language model backends.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use reqwest::{Client, header};
use std::time::Duration;
use crate::config::LlmBackendConfig;

/// OpenAI-compatible chat completion request
#[derive(Debug, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

/// Chat message for OpenAI-compatible APIs
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// OpenAI-compatible chat completion response
#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ChatMessage,
    #[allow(dead_code)]
    pub finish_reason: String,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    #[allow(dead_code)]
    pub prompt_tokens: u32,
    #[allow(dead_code)]
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Client for interacting with LLM backends
pub struct LlmClient {
    http_client: Client,
    backend_config: LlmBackendConfig,
}

impl LlmClient {
    /// Create a new LLM client for the given backend
    pub fn new(backend_config: LlmBackendConfig) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?;
            
        Ok(Self {
            http_client,
            backend_config,
        })
    }
    
    /// Execute a chat completion request
    pub async fn chat_completion(
        &self,
        model: &str,
        prompt: &str,
        system_prompt: Option<&str>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<(String, u32)> {
        // Check if the model is supported
        if !self.backend_config.supported_models.contains(&model.to_string()) {
            return Err(anyhow!("Model {} not supported by backend {}", model, self.backend_config.name));
        }
        
        // Build messages
        let mut messages = vec![];
        if let Some(system) = system_prompt {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: system.to_string(),
            });
        }
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        });
        
        // Build request
        let request = ChatCompletionRequest {
            model: model.to_string(),
            messages,
            temperature,
            max_tokens,
        };
        
        // Get API key from environment variable or config
        let api_key = match &self.backend_config.api_key {
            Some(key) => key.clone(),
            None => {
                let env_var = format!("{}_API_KEY", self.backend_config.name.to_uppercase());
                std::env::var(&env_var)
                    .map_err(|_| anyhow!("API key not found in config or {} environment variable", env_var))?
            }
        };
        
        // Make the request
        let url = format!("{}/chat/completions", self.backend_config.endpoint);
        let response = self.http_client
            .post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", api_key))
            .header(header::CONTENT_TYPE, "application/json")
            .json(&request)
            .send()
            .await?;
            
        // Check status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(anyhow!("LLM API error ({}): {}", status, error_text));
        }
        
        // Parse response
        let completion: ChatCompletionResponse = response.json().await?;
        
        // Extract content and token count
        let content = completion.choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow!("No completion choices returned"))?;
            
        let token_count = completion.usage.total_tokens;
        
        Ok((content, token_count))
    }
}

/// Count tokens in text using tiktoken
#[allow(dead_code)]
pub fn count_tokens(text: &str, model: &str) -> Result<usize> {
    use tiktoken_rs::get_bpe_from_model;
    
    // Map model names to tiktoken model names
    let tiktoken_model = match model {
        "gpt-4" | "gpt-4-turbo" => "gpt-4",
        "gpt-3.5-turbo" => "gpt-3.5-turbo",
        _ => "gpt-3.5-turbo", // Default fallback
    };
    
    let bpe = get_bpe_from_model(tiktoken_model)?;
    let tokens = bpe.encode_with_special_tokens(text);
    Ok(tokens.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    
    #[test]
    fn test_token_counting() {
        let text = "Hello, world!";
        let count = count_tokens(text, "gpt-3.5-turbo").unwrap();
        assert!(count > 0);
        assert!(count < 10); // "Hello, world!" should be just a few tokens
    }

    #[test]
    fn test_token_counting_empty_string() {
        let count = count_tokens("", "gpt-3.5-turbo").unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_token_counting_different_models() {
        let text = "This is a test message.";
        
        let gpt35_count = count_tokens(text, "gpt-3.5-turbo").unwrap();
        let gpt4_count = count_tokens(text, "gpt-4").unwrap();
        let unknown_count = count_tokens(text, "unknown-model").unwrap();
        
        // All should return some positive count
        assert!(gpt35_count > 0);
        assert!(gpt4_count > 0);
        assert!(unknown_count > 0);
    }

    #[test]
    fn test_llm_client_creation() {
        let backend_config = LlmBackendConfig {
            name: "test-backend".to_string(),
            endpoint: "https://api.test.com/v1".to_string(),
            api_key: Some("test-key".to_string()),
            supported_models: vec!["test-model".to_string()],
            rate_limit: Some(100),
        };

        let client = LlmClient::new(backend_config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_chat_message_structure() {
        let message = ChatMessage {
            role: "user".to_string(),
            content: "Test message".to_string(),
        };

        assert_eq!(message.role, "user");
        assert_eq!(message.content, "Test message");
    }

    #[test]
    fn test_chat_completion_request_structure() {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are a helpful assistant".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
        ];

        let request = ChatCompletionRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages,
            temperature: Some(0.7),
            max_tokens: Some(150),
        };

        assert_eq!(request.model, "gpt-3.5-turbo");
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(150));
    }

    #[test]
    fn test_chat_completion_request_minimal() {
        let messages = vec![
            ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
        ];

        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages,
            temperature: None,
            max_tokens: None,
        };

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.messages.len(), 1);
        assert!(request.temperature.is_none());
        assert!(request.max_tokens.is_none());
    }

    #[tokio::test]
    async fn test_chat_completion_unsupported_model() {
        let backend_config = LlmBackendConfig {
            name: "test-backend".to_string(),
            endpoint: "https://api.test.com/v1".to_string(),
            api_key: Some("test-key".to_string()),
            supported_models: vec!["gpt-3.5-turbo".to_string()],
            rate_limit: Some(100),
        };

        let client = LlmClient::new(backend_config).unwrap();

        let result = client.chat_completion(
            "unsupported-model",
            "Hello",
            None,
            None,
            None,
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }

    #[tokio::test]
    async fn test_chat_completion_success() {
        // Start a mock server
        let mock_server = MockServer::start().await;

        // Create mock response
        let mock_response = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you today?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        });

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
            .mount(&mock_server)
            .await;

        let backend_config = LlmBackendConfig {
            name: "test-backend".to_string(),
            endpoint: mock_server.uri(),
            api_key: Some("test-key".to_string()),
            supported_models: vec!["gpt-3.5-turbo".to_string()],
            rate_limit: Some(100),
        };

        let client = LlmClient::new(backend_config).unwrap();

        let result = client.chat_completion(
            "gpt-3.5-turbo",
            "Hello",
            Some("You are a helpful assistant"),
            Some(0.7),
            Some(150),
        ).await;

        assert!(result.is_ok());
        let (content, token_count) = result.unwrap();
        assert_eq!(content, "Hello! How can I help you today?");
        assert_eq!(token_count, 18);
    }

    #[tokio::test]
    async fn test_chat_completion_api_error() {
        // Start a mock server that returns an error
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(400).set_body_string("Bad Request"))
            .mount(&mock_server)
            .await;

        let backend_config = LlmBackendConfig {
            name: "test-backend".to_string(),
            endpoint: mock_server.uri(),
            api_key: Some("test-key".to_string()),
            supported_models: vec!["gpt-3.5-turbo".to_string()],
            rate_limit: Some(100),
        };

        let client = LlmClient::new(backend_config).unwrap();

        let result = client.chat_completion(
            "gpt-3.5-turbo",
            "Hello",
            None,
            None,
            None,
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("LLM API error"));
    }

    #[tokio::test]
    async fn test_chat_completion_no_choices() {
        // Start a mock server
        let mock_server = MockServer::start().await;

        // Create mock response with no choices
        let mock_response = serde_json::json!({
            "choices": [],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 0,
                "total_tokens": 10
            }
        });

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
            .mount(&mock_server)
            .await;

        let backend_config = LlmBackendConfig {
            name: "test-backend".to_string(),
            endpoint: mock_server.uri(),
            api_key: Some("test-key".to_string()),
            supported_models: vec!["gpt-3.5-turbo".to_string()],
            rate_limit: Some(100),
        };

        let client = LlmClient::new(backend_config).unwrap();

        let result = client.chat_completion(
            "gpt-3.5-turbo",
            "Hello",
            None,
            None,
            None,
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No completion choices"));
    }

    #[test]
    fn test_serialization() {
        let request = ChatCompletionRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![
                ChatMessage {
                    role: "user".to_string(),
                    content: "Hello".to_string(),
                },
            ],
            temperature: Some(0.5),
            max_tokens: Some(100),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: ChatCompletionRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(request.model, deserialized.model);
        assert_eq!(request.messages.len(), deserialized.messages.len());
        assert_eq!(request.temperature, deserialized.temperature);
        assert_eq!(request.max_tokens, deserialized.max_tokens);
    }

    #[test]
    fn test_response_deserialization() {
        let json_response = r#"{
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Test response"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 2,
                "total_tokens": 7
            }
        }"#;

        let response: ChatCompletionResponse = serde_json::from_str(json_response).unwrap();
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].message.content, "Test response");
        assert_eq!(response.usage.total_tokens, 7);
        assert_eq!(response.usage.prompt_tokens, 5);
        assert_eq!(response.usage.completion_tokens, 2);
    }

    #[test]
    fn test_backend_config_clone() {
        let config = LlmBackendConfig {
            name: "test".to_string(),
            endpoint: "https://api.test.com".to_string(),
            api_key: Some("key".to_string()),
            supported_models: vec!["model1".to_string()],
            rate_limit: Some(60),
        };

        let cloned = config.clone();
        assert_eq!(config.name, cloned.name);
        assert_eq!(config.endpoint, cloned.endpoint);
        assert_eq!(config.api_key, cloned.api_key);
        assert_eq!(config.supported_models, cloned.supported_models);
        assert_eq!(config.rate_limit, cloned.rate_limit);
    }
}