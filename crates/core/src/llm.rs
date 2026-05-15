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

// ============================================================================
// LlmRouter — multi-provider routing with auto-fallback
// ============================================================================

/// Configuration for a single LLM provider in the router.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl ProviderConfig {
    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty() && !self.base_url.is_empty() && !self.model.is_empty()
    }
}

/// Routing strategy for selecting between multiple LLM providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingStrategy {
    /// Use primary only, fallback to secondary on failure.
    PrimaryWithFallback,
    /// Route based on task complexity: simple → primary, complex → secondary.
    ComplexityBased,
}

impl Default for RoutingStrategy {
    fn default() -> Self {
        Self::PrimaryWithFallback
    }
}

/// Multi-LLM router that implements LlmClient for transparent replacement.
///
/// Supports:
/// - Auto-fallback: try primary, retry with secondary on failure
/// - Complexity-based routing: route complex/vision tasks to secondary
pub struct LlmRouter {
    primary: ArcLlmClient,
    secondary: Option<ArcLlmClient>,
    strategy: RoutingStrategy,
}

impl LlmRouter {
    /// Create a new router with a primary provider and optional secondary.
    pub fn new(primary: ArcLlmClient, secondary: Option<ArcLlmClient>) -> Self {
        Self {
            primary,
            secondary,
            strategy: RoutingStrategy::default(),
        }
    }

    /// Create from ProviderConfig values (convenience constructor).
    pub fn from_configs(
        primary_cfg: ProviderConfig,
        secondary_cfg: Option<ProviderConfig>,
    ) -> Result<Self, LlmError> {
        let primary = ArcLlmClient::new(MiniMaxClient::new(
            primary_cfg.base_url,
            primary_cfg.api_key,
            primary_cfg.model,
        ));

        let secondary = secondary_cfg
            .filter(|c| c.is_configured())
            .map(|cfg| {
                ArcLlmClient::new(MiniMaxClient::new(
                    cfg.base_url,
                    cfg.api_key,
                    cfg.model,
                ))
            });

        Ok(Self::new(primary, secondary))
    }

    /// Set the routing strategy.
    pub fn with_strategy(mut self, strategy: RoutingStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Check if a secondary provider is configured.
    pub fn has_secondary(&self) -> bool {
        self.secondary.is_some()
    }

    /// Select which client to use based on the routing strategy.
    fn select_client(&self, options: &ChatOptions) -> &ArcLlmClient {
        match self.strategy {
            RoutingStrategy::PrimaryWithFallback => &self.primary,
            RoutingStrategy::ComplexityBased => {
                // If vision is explicitly requested or max_tokens is high, use secondary
                let needs_vision = options.vision.unwrap_or(false);
                let is_complex = options.max_tokens.unwrap_or(0) > 2048;
                if (needs_vision || is_complex) && self.secondary.is_some() {
                    self.secondary.as_ref().unwrap()
                } else {
                    &self.primary
                }
            }
        }
    }
}

#[async_trait]
impl LlmClient for LlmRouter {
    async fn chat(&self, messages: &[Message]) -> Result<String, LlmError> {
        self.chat_with_options(messages, ChatOptions::default()).await
    }

    async fn chat_with_options(
        &self,
        messages: &[Message],
        options: ChatOptions,
    ) -> Result<String, LlmError> {
        let client = self.select_client(&options);

        // Try primary (or complexity-routed) client first
        match client.chat_with_options(messages, options.clone()).await {
            Ok(result) => Ok(result),
            Err(e) => {
                // If we already used secondary, don't try again
                if self.secondary.is_none()
                    || std::ptr::eq(client as *const _, self.secondary.as_ref().unwrap() as *const _)
                {
                    return Err(e);
                }

                // Fallback to secondary
                tracing::warn!("primary LLM failed ({:?}), falling back to secondary: {}", e, e);
                self.secondary
                    .as_ref()
                    .unwrap()
                    .chat_with_options(messages, options)
                    .await
            }
        }
    }
}
