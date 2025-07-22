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
    pub finish_reason: String,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
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
pub fn count_tokens(text: &str, model: &str) -> Result<usize> {
    use tiktoken_rs::{get_bpe_from_model, CoreBPE};
    
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
    
    #[test]
    fn test_token_counting() {
        let text = "Hello, world!";
        let count = count_tokens(text, "gpt-3.5-turbo").unwrap();
        assert!(count > 0);
        assert!(count < 10); // "Hello, world!" should be just a few tokens
    }
}