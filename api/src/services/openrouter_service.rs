///! OpenRouter LLM Integration Service
///!
///! Provides integration with OpenRouter API for accessing various LLM models
///! including GPT-4, Claude, and other leading models through a unified interface.
use anyhow::{Context, Result};
use futures_util::stream::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tracing::{debug, info, warn};

/// OpenRouter API endpoint
const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// OpenRouter LLM service
pub struct OpenRouterService {
    client: Client,
    api_key: String,
    default_model: String,
    site_url: Option<String>,
    site_name: Option<String>,
}

/// Chat message for LLM conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system", "user", "assistant"
    pub content: String,
}

/// Request to OpenRouter API
#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

/// Response from OpenRouter API
#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChatMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// LLM completion result
#[derive(Debug, Clone)]
pub struct CompletionResult {
    pub content: String,
    pub finish_reason: Option<String>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

/// Streaming chunk from OpenRouter API
#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

/// Stream event for SSE
#[derive(Debug, Clone, Serialize)]
pub struct StreamEvent {
    pub content: String,
    pub done: bool,
}

impl OpenRouterService {
    /// Create a new OpenRouter service from environment variables
    ///
    /// Required env vars:
    /// - OPENROUTER_API_KEY
    ///
    /// Optional env vars:
    /// - OPENROUTER_MODEL (default: anthropic/claude-3.5-sonnet)
    /// - OPENROUTER_SITE_URL
    /// - OPENROUTER_SITE_NAME
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .context("OPENROUTER_API_KEY environment variable is required")?;

        let default_model = std::env::var("OPENROUTER_MODEL")
            .unwrap_or_else(|_| "anthropic/claude-3.5-sonnet".to_string());

        let site_url = std::env::var("OPENROUTER_SITE_URL").ok();
        let site_name = std::env::var("OPENROUTER_SITE_NAME").ok();

        info!(
            "Initializing OpenRouter service with model: {}",
            default_model
        );

        Ok(Self {
            client: Client::new(),
            api_key,
            default_model,
            site_url,
            site_name,
        })
    }

    /// Create completion with custom model
    pub async fn complete_with_model(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<CompletionResult> {
        debug!(
            "Sending completion request to OpenRouter (model: {}, messages: {})",
            model,
            messages.len()
        );

        let request = OpenRouterRequest {
            model: model.to_string(),
            messages,
            temperature,
            max_tokens,
            top_p: None,
            stream: None,
        };

        let mut request_builder = self
            .client
            .post(OPENROUTER_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request);

        // Add optional site info headers
        if let Some(site_url) = &self.site_url {
            request_builder = request_builder.header("HTTP-Referer", site_url);
        }
        if let Some(site_name) = &self.site_name {
            request_builder = request_builder.header("X-Title", site_name);
        }

        let response = request_builder
            .send()
            .await
            .context("Failed to send request to OpenRouter")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!(
                "OpenRouter API error ({}): {}",
                status,
                error_text
            ));
        }

        let openrouter_response: OpenRouterResponse = response
            .json()
            .await
            .context("Failed to parse OpenRouter response")?;

        let choice = openrouter_response
            .choices
            .into_iter()
            .next()
            .context("No choices in OpenRouter response")?;

        let result = CompletionResult {
            content: choice.message.content,
            finish_reason: choice.finish_reason,
            prompt_tokens: openrouter_response.usage.as_ref().map(|u| u.prompt_tokens),
            completion_tokens: openrouter_response
                .usage
                .as_ref()
                .map(|u| u.completion_tokens),
            total_tokens: openrouter_response.usage.as_ref().map(|u| u.total_tokens),
        };

        debug!(
            "Received completion from OpenRouter (tokens: {:?})",
            result.total_tokens
        );

        Ok(result)
    }

    /// Create completion with default model
    pub async fn complete(
        &self,
        messages: Vec<ChatMessage>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<CompletionResult> {
        self.complete_with_model(&self.default_model, messages, temperature, max_tokens)
            .await
    }

    /// Simple question-answering helper
    pub async fn ask(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            },
        ];

        let result = self.complete(messages, Some(0.7), Some(2000)).await?;
        Ok(result.content)
    }

    /// Stream completion with custom model
    ///
    /// Returns a stream of content chunks that can be sent via Server-Sent Events
    pub async fn stream_completion_with_model(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        debug!(
            "Starting streaming completion request to OpenRouter (model: {}, messages: {})",
            model,
            messages.len()
        );

        let request = OpenRouterRequest {
            model: model.to_string(),
            messages,
            temperature,
            max_tokens,
            top_p: None,
            stream: Some(true),
        };

        let mut request_builder = self
            .client
            .post(OPENROUTER_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request);

        // Add optional site info headers
        if let Some(site_url) = &self.site_url {
            request_builder = request_builder.header("HTTP-Referer", site_url);
        }
        if let Some(site_name) = &self.site_name {
            request_builder = request_builder.header("X-Title", site_name);
        }

        let response = request_builder
            .send()
            .await
            .context("Failed to send streaming request to OpenRouter")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!(
                "OpenRouter API error ({}): {}",
                status,
                error_text
            ));
        }

        // Convert response into a stream of events
        let byte_stream = response.bytes_stream();

        let event_stream = byte_stream.map(|chunk_result| {
            match chunk_result {
                Ok(bytes) => {
                    // Parse SSE format: "data: {...}\n\n"
                    let text = String::from_utf8_lossy(&bytes);

                    // Handle the special [DONE] message
                    if text.contains("[DONE]") {
                        return Ok(StreamEvent {
                            content: String::new(),
                            done: true,
                        });
                    }

                    // Parse each line that starts with "data: "
                    for line in text.lines() {
                        if let Some(json_str) = line.strip_prefix("data: ") {
                            if json_str.trim() == "[DONE]" {
                                return Ok(StreamEvent {
                                    content: String::new(),
                                    done: true,
                                });
                            }

                            match serde_json::from_str::<StreamChunk>(json_str) {
                                Ok(chunk) => {
                                    if let Some(choice) = chunk.choices.first() {
                                        if let Some(content) = &choice.delta.content {
                                            return Ok(StreamEvent {
                                                content: content.clone(),
                                                done: choice.finish_reason.is_some(),
                                            });
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to parse streaming chunk: {} - JSON: {}",
                                        e, json_str
                                    );
                                }
                            }
                        }
                    }

                    // Return empty event if no content found
                    Ok(StreamEvent {
                        content: String::new(),
                        done: false,
                    })
                }
                Err(e) => Err(anyhow::anyhow!("Stream error: {}", e)),
            }
        });

        Ok(Box::pin(event_stream))
    }

    /// Stream completion with default model
    pub async fn stream_completion(
        &self,
        messages: Vec<ChatMessage>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        self.stream_completion_with_model(&self.default_model, messages, temperature, max_tokens)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_serialization() {
        let message = ChatMessage {
            role: "user".to_string(),
            content: "Hello!".to_string(),
        };

        let json = serde_json::to_string(&message).expect("Failed to serialize");
        let deserialized: ChatMessage = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(message.role, deserialized.role);
        assert_eq!(message.content, deserialized.content);
    }
}
