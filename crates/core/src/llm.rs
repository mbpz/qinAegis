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
// LLM client trait (for testability and abstraction)
// ============================================================================

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, messages: &[Message]) -> Result<String, LlmError>;
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
}

// ============================================================================
// Message types
// ============================================================================

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
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
        }
    }
}

#[async_trait]
impl LlmClient for MiniMaxClient {
    async fn chat(&self, messages: &[Message]) -> Result<String, LlmError> {
        let body = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            max_tokens: Some(1024),
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
