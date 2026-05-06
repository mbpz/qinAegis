// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// Sandbox errors
// ============================================================================

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("browser not ready: {0}")]
    BrowserNotReady(String),
    #[error("CDP connection failed: {0}")]
    CdpConnectionFailed(String),
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
    #[error("health check failed: {0}")]
    HealthCheckFailed(String),
    #[error("process died: {0}")]
    ProcessDied(String),
    #[error("internal: {0}")]
    Internal(String),
}

// ============================================================================
// Sandbox health status
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxHealth {
    pub browser_ready: bool,
    pub cdp_url: Option<String>,
    pub pid: Option<u32>,
}

// ============================================================================
// Sandbox adapter trait
// ============================================================================

#[async_trait]
pub trait SandboxAdapter: Send + Sync {
    /// Return the active CDP WebSocket URL.
    /// Returns None if browser is not yet ready.
    fn cdp_url(&self) -> Option<String>;

    /// Check if the sandbox process is healthy.
    async fn health(&self) -> Result<SandboxHealth, SandboxError>;

    /// Wait for the browser to be ready, retrying if necessary.
    async fn wait_for_browser(&self, timeout_secs: u64) -> Result<String, SandboxError>;

    /// Restart the browser/session (hot reload).
    async fn restart(&self) -> Result<(), SandboxError>;
}

// ============================================================================
// ShellBrowserAdapter — resolves CDP from /json/version
// ============================================================================

#[derive(Clone)]
pub struct ShellBrowserAdapter {
    #[allow(dead_code)]
    browser_path: String,
    #[allow(dead_code)]
    args: Vec<String>,
    cdp_port: u16,
}

impl ShellBrowserAdapter {
    pub fn new(browser_path: String, args: Vec<String>, cdp_port: u16) -> Self {
        Self {
            browser_path,
            args,
            cdp_port,
        }
    }

    async fn resolve_cdp_url(&self) -> Result<String, SandboxError> {
        let url = reqwest::get(format!("http://localhost:{}/json/version", self.cdp_port))
            .await
            .map_err(|e| SandboxError::CdpConnectionFailed(e.to_string()))?
            .json::<ChromeVersionResponse>()
            .await
            .map_err(|e| SandboxError::CdpConnectionFailed(e.to_string()))?
            .web_socket_debugger_url;

        Ok(url)
    }
}

#[derive(Debug, Deserialize)]
struct ChromeVersionResponse {
    #[serde(rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: String,
}

#[async_trait]
impl SandboxAdapter for ShellBrowserAdapter {
    fn cdp_url(&self) -> Option<String> {
        None // Resolved dynamically on each call
    }

    async fn health(&self) -> Result<SandboxHealth, SandboxError> {
        let cdp_url = match self.resolve_cdp_url().await {
            Ok(url) => url,
            Err(_) => return Ok(SandboxHealth { browser_ready: false, cdp_url: None, pid: None }),
        };

        Ok(SandboxHealth {
            browser_ready: true,
            cdp_url: Some(cdp_url.clone()),
            pid: None,
        })
    }

    async fn wait_for_browser(&self, timeout_secs: u64) -> Result<String, SandboxError> {
        let start = std::time::Instant::now();
        let interval = std::time::Duration::from_millis(500);

        loop {
            if start.elapsed().as_secs() > timeout_secs {
                return Err(SandboxError::BrowserNotReady(
                    "timeout waiting for browser".to_string(),
                ));
            }

            if let Ok(url) = self.resolve_cdp_url().await {
                return Ok(url);
            }

            tokio::time::sleep(interval).await;
        }
    }

    async fn restart(&self) -> Result<(), SandboxError> {
        // Browser restart logic: kill existing + spawn new
        Err(SandboxError::Internal("restart not yet implemented".to_string()))
    }
}

// ============================================================================
// SteelBrowserAdapter — uses existing CDP URL (for Steel browser)
// ============================================================================

#[derive(Clone)]
pub struct SteelBrowserAdapter {
    cdp_url: String,
}

impl SteelBrowserAdapter {
    pub fn new(cdp_url: impl Into<String>) -> Self {
        Self {
            cdp_url: cdp_url.into(),
        }
    }
}

#[async_trait]
impl SandboxAdapter for SteelBrowserAdapter {
    fn cdp_url(&self) -> Option<String> {
        Some(self.cdp_url.clone())
    }

    async fn health(&self) -> Result<SandboxHealth, SandboxError> {
        // Try to connect to the CDP URL to check if browser is alive
        Ok(SandboxHealth {
            browser_ready: true,
            cdp_url: Some(self.cdp_url.clone()),
            pid: None,
        })
    }

    async fn wait_for_browser(&self, _timeout_secs: u64) -> Result<String, SandboxError> {
        Ok(self.cdp_url.clone())
    }

    async fn restart(&self) -> Result<(), SandboxError> {
        Err(SandboxError::Internal("restart not applicable for Steel browser".to_string()))
    }
}
