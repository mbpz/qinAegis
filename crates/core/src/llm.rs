// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

// ============================================================================
// LLM error
// ============================================================================

#[derive(Error, Debug)]
pub enum LlmError {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("missing API key")]
    NoApiKey,
}

// ============================================================================
// Chat options (model-specific)
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
    /// Override default max_tokens for this call
    pub max_tokens: Option<u32>,
    /// Sampling temperature
    pub temperature: Option<f32>,
    /// Whether to enable vision/image input support
    pub vision: Option<bool>,
    /// JSON schema to constrain response format (model-dependent)
    pub json_schema: Option<String>,
    /// Conversation history to include (e.g. for system prompt)
    pub system_prompt: Option<String>,
}

impl ChatOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_vision(mut self) -> Self {
        self.vision = Some(true);
        self
    }

    pub fn with_json_schema(mut self, schema: impl Into<String>) -> Self {
        self.json_schema = Some(schema.into());
        self
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }
}

// ============================================================================
// LLM client trait (for testability and abstraction)
// ============================================================================

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, messages: &[Message]) -> Result<String, LlmError>;

    async fn chat_with_options(
        &self,
        messages: &[Message],
        options: ChatOptions,
    ) -> Result<String, LlmError> {
        let _ = options;
        self.chat(messages).await
    }
}

#[derive(Clone)]
pub struct ArcLlmClient(pub Arc<dyn LlmClient>);

impl ArcLlmClient {
    pub fn new(client: impl LlmClient + 'static) -> Self {
        Self(Arc::new(client))
    }
}

#[async_trait]
impl LlmClient for ArcLlmClient {
    async fn chat(&self, messages: &[Message]) -> Result<String, LlmError> {
        self.0.chat(messages).await
    }

    async fn chat_with_options(
        &self,
        messages: &[Message],
        options: ChatOptions,
    ) -> Result<String, LlmError> {
        self.0.chat_with_options(messages, options).await
    }
}

// ============================================================================
// Message types
// ============================================================================

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }
}

// ============================================================================
// MiniMax client
// ============================================================================

#[derive(Clone)]
pub struct MiniMaxClient {
    base_url: String,
    api_key: String,
    model: String,
    client: Client,
    default_options: ChatOptions,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

impl MiniMaxClient {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        Self {
            base_url,
            api_key,
            model,
            client: Client::new(),
            default_options: ChatOptions::new(),
        }
    }

    pub fn with_options(mut self, options: ChatOptions) -> Self {
        self.default_options = options;
        self
    }
}

#[async_trait]
impl LlmClient for MiniMaxClient {
    async fn chat(&self, messages: &[Message]) -> Result<String, LlmError> {
        self.chat_with_options(messages, self.default_options.clone()).await
    }

    async fn chat_with_options(
        &self,
        messages: &[Message],
        options: ChatOptions,
    ) -> Result<String, LlmError> {
        let max_tokens = options.max_tokens.or(self.default_options.max_tokens).unwrap_or(1024);

        let body = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            max_tokens: Some(max_tokens),
        };

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Api(format!("{}: {}", status, body)));
        }

        let chat_resp: ChatResponse = resp.json().await?;

        chat_resp
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| LlmError::Api("no choices in response".to_string()))
    }
}

// Internal request type
#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: Option<u32>,
}

// Additional message content types
#[derive(Serialize, Clone)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Serialize, Clone)]
pub struct ContentPart {
    #[serde(rename = "type")]
    pub part_type: String,
    pub text: Option<String>,
    pub image_url: Option<ImageUrl>,
}

#[derive(Serialize, Clone)]
pub struct ImageUrl {
    pub url: String,
}
