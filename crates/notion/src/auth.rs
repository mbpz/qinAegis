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
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
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

    #[test]
    fn test_store_and_get_token() {
        let test_token = "test_token_123";
        store_notion_token(test_token).expect("store should succeed");
        let retrieved = get_notion_token().expect("get should succeed");
        assert_eq!(retrieved, Some(test_token.to_string()));
        // Cleanup
        delete_notion_token().expect("delete should succeed");
    }

    #[test]
    fn test_get_token_when_none_exists() {
        // Ensure no token exists first
        let _ = delete_notion_token();
        let result = get_notion_token().expect("should not error");
        assert_eq!(result, None);
    }
}

const SERVICE_NAME: &str = "qinAegis";
const NOTION_TOKEN_KEY: &str = "notion_access_token";

pub fn store_notion_token(token: &str) -> Result<(), AuthError> {
    let output = std::process::Command::new("security")
        .args(&["add-generic-password", "-s", SERVICE_NAME, "-a", NOTION_TOKEN_KEY, "-w", token, "-U"])
        .output()
        .map_err(|e| AuthError::Keyring(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AuthError::Keyring(format!("security command failed: {}", stderr)));
    }
    Ok(())
}

pub fn get_notion_token() -> Result<Option<String>, AuthError> {
    let output = std::process::Command::new("security")
        .args(&["find-generic-password", "-s", SERVICE_NAME, "-a", NOTION_TOKEN_KEY, "-w"])
        .output()
        .map_err(|e| AuthError::Keyring(e.to_string()))?;

    if output.status.code() == Some(44) {
        return Ok(None);
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("SecKeychainSearchCopyNext") || output.status.code() == Some(44) {
            return Ok(None);
        }
        return Err(AuthError::Keyring(format!("security command failed: {}", stderr)));
    }

    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        return Ok(None);
    }
    Ok(Some(token))
}

pub fn delete_notion_token() -> Result<(), AuthError> {
    let output = std::process::Command::new("security")
        .args(&["delete-generic-password", "-s", SERVICE_NAME, "-a", NOTION_TOKEN_KEY])
        .output()
        .map_err(|e| AuthError::Keyring(e.to_string()))?;

    // 44 means item not found, which is OK for delete
    if output.status.success() || output.status.code() == Some(44) {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(AuthError::Keyring(format!("delete failed: {}", stderr)))
}

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

#[derive(Serialize)]
struct RefreshRequest {
    grant_type: String,
    refresh_token: String,
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

        let status = resp.status();
        let body_text = resp.text().await?;

        let token_resp: TokenResponse = serde_json::from_str(&body_text)?;
        Ok(token_resp)
    }

    pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse, AuthError> {
        let client = reqwest::Client::new();
        let body = RefreshRequest {
            grant_type: "refresh_token".to_string(),
            refresh_token: refresh_token.to_string(),
        };

        let resp = client
            .post("https://api.notion.com/v1/oauth/token")
            .basic_auth(&self.client_id, None::<&str>)
            .json(&body)
            .send()
            .await?;

        let token_resp: TokenResponse = resp.json().await?;
        Ok(token_resp)
    }
}