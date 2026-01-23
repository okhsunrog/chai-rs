//! OpenRouter API client utilities
//!
//! This module provides shared types and utilities for interacting with the OpenRouter API.
//! It is primarily used by examples and CLI tools for testing different prompts and models.

use crate::http::get_client;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Request payload for OpenRouter chat completions API
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
}

impl ChatRequest {
    /// Create a new chat request with a single user message
    pub fn new(model: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: vec![Message::user(content)],
            temperature: None,
            max_tokens: None,
            response_format: None,
        }
    }

    /// Set the temperature for sampling
    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Set the maximum number of tokens in the response
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// Request JSON response format
    pub fn json_format(mut self) -> Self {
        self.response_format = Some(ResponseFormat {
            format_type: "json_object".to_string(),
        });
        self
    }
}

/// A message in the chat conversation
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

/// Response format specification
#[derive(Debug, Serialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String,
}

/// Response from OpenRouter chat completions API
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

impl ChatResponse {
    /// Get the content of the first choice, if available
    pub fn content(&self) -> Option<&str> {
        self.choices.first().map(|c| c.message.content.as_str())
    }

    /// Get the content of the first choice, or an error if not available
    pub fn content_or_err(&self) -> Result<&str> {
        self.content()
            .context("No response content from API (empty choices)")
    }
}

/// A single response choice
#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ResponseMessage,
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// The message content in a response choice
#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    pub content: String,
    #[serde(default)]
    pub role: Option<String>,
}

/// Token usage information
#[derive(Debug, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// Re-export strip_markdown_json from http module for backwards compatibility
pub use crate::http::strip_markdown_json;

/// Send a chat completion request to OpenRouter API
///
/// # Arguments
/// * `request` - The chat request payload
/// * `api_key` - OpenRouter API key
///
/// # Returns
/// The parsed response from the API
pub async fn chat_completion(request: &ChatRequest, api_key: &str) -> Result<ChatResponse> {
    let client = get_client();

    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(request)
        .send()
        .await
        .context("Failed to send request to OpenRouter API")?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("OpenRouter API error {}: {}", status, text);
    }

    response
        .json()
        .await
        .context("Failed to parse OpenRouter API response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_request_builder() {
        let request = ChatRequest::new("gpt-4", "Hello")
            .temperature(0.7)
            .max_tokens(100)
            .json_format();

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(100));
        assert!(request.response_format.is_some());
    }

    #[test]
    fn test_message_constructors() {
        let user = Message::user("Hello");
        assert_eq!(user.role, "user");
        assert_eq!(user.content, "Hello");

        let system = Message::system("You are helpful");
        assert_eq!(system.role, "system");

        let assistant = Message::assistant("Hi there");
        assert_eq!(assistant.role, "assistant");
    }
}
