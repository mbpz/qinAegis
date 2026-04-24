// crates/notion/src/auth.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("keyring error: {0}")]
    Keyring(String),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("callback missing code param")]
    MissingCode,
}

pub struct NotionAuth {
    client_id: String,
    redirect_port: u16,
}

impl NotionAuth {
    pub fn new(client_id: String, redirect_port: u16) -> Self {
        Self { client_id, redirect_port }
    }

    pub fn authorization_url(&self) -> String {
        format!(
            "https://api.notion.com/v1/oauth/authorize\
             ?client_id={}\
             &redirect_uri=http://localhost:{}/callback\
             &response_type=code\
             &owner=user",
            self.client_id, self.redirect_port
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authorization_url_format() {
        let auth = NotionAuth::new("my-client-id".to_string(), 54321);
        let url = auth.authorization_url();
        assert!(url.contains("client_id=my-client-id"));
        assert!(url.contains("redirect_uri=http://localhost:54321/callback"));
        assert!(url.contains("response_type=code"));
    }
}

// Add to crates/notion/src/auth.rs

use keyring::Entry;

const SERVICE_NAME: &str = "qinAegis";
const NOTION_TOKEN_KEY: &str = "notion_access_token";

pub fn store_notion_token(token: &str) -> Result<(), AuthError> {
    let entry = Entry::new(SERVICE_NAME, NOTION_TOKEN_KEY)
        .map_err(|e| AuthError::Keyring(e.to_string()))?;
    entry
        .set_password(token)
        .map_err(|e| AuthError::Keyring(e.to_string()))?;
    Ok(())
}

pub fn get_notion_token() -> Result<Option<String>, AuthError> {
    let entry = Entry::new(SERVICE_NAME, NOTION_TOKEN_KEY)
        .map_err(|e| AuthError::Keyring(e.to_string()))?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AuthError::Keyring(e.to_string())),
    }
}

pub fn delete_notion_token() -> Result<(), AuthError> {
    let entry = Entry::new(SERVICE_NAME, NOTION_TOKEN_KEY)
        .map_err(|e| AuthError::Keyring(e.to_string()))?;
    entry
        .delete_credential()
        .map_err(|e| AuthError::Keyring(e.to_string()))?;
    Ok(())
}

// Add to crates/notion/src/auth.rs

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub workspace_id: String,
    pub workspace_name: String,
}

#[derive(Serialize)]
struct TokenRequest {
    grant_type: String,
    code: String,
    redirect_uri: String,
}

impl NotionAuth {
    pub async fn exchange_code(&self, code: &str, client_secret: &str) -> Result<TokenResponse, AuthError> {
        let client = reqwest::Client::new();
        let body = TokenRequest {
            grant_type: "authorization_code".to_string(),
            code: code.to_string(),
            redirect_uri: format!("http://localhost:{}/callback", self.redirect_port),
        };

        let resp = client
            .post("https://api.notion.com/v1/oauth/token")
            .basic_auth(&self.client_id, Some(client_secret))
            .json(&body)
            .send()
            .await?;

        let token_resp: TokenResponse = resp.json().await?;
        Ok(token_resp)
    }
}