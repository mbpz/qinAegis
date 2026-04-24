use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LlmError {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("missing API key")]
    NoApiKey,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

pub struct MiniMaxClient {
    base_url: String,
    api_key: String,
    model: String,
    client: Client,
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

    pub async fn chat(&self, messages: &[Message]) -> Result<String, LlmError> {
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

// Add to Message struct:
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
