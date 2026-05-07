// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! Sandbox adapter for browser automation.
//!
//! This module provides PlaywrightBrowserAdapter which launches Chromium
//! directly without requiring Docker.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use std::thread::sleep;
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
// PlaywrightBrowserAdapter — launches Chromium via Playwright (no Docker)
// ============================================================================

/// Browser launcher for Playwright-managed or system Chromium.
/// This enables running without Docker.
#[derive(Clone)]
pub struct PlaywrightBrowserAdapter {
    cdp_port: u16,
    child: std::sync::Arc<std::sync::Mutex<Option<Child>>>,
}

impl PlaywrightBrowserAdapter {
    /// Create a new adapter for the specified CDP port
    pub fn new(cdp_port: u16) -> Self {
        Self {
            cdp_port,
            child: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Check if Chrome/Chromium is available on the system
    pub fn is_chrome_available() -> bool {
        let paths = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/usr/bin/google-chrome",
            "/usr/bin/chromium-browser",
        ];
        paths.iter().any(|p| std::path::Path::new(p).exists())
    }

    /// Check if Playwright is available
    pub fn is_playwright_available() -> bool {
        Command::new("npx")
            .args(["playwright", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if CDP port is already in use by a running browser
    fn is_browser_running(&self) -> bool {
        let url = format!("http://localhost:{}/json/version", self.cdp_port);
        match reqwest::blocking::get(&url) {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Launch browser using system Chrome (headless)
    fn launch_chrome(&self) -> anyhow::Result<Child> {
        let chrome_paths = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/usr/bin/google-chrome",
            "/usr/bin/chromium-browser",
        ];

        let chrome_path = chrome_paths
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .ok_or_else(|| anyhow::anyhow!("Chrome not found"))?;

        let chrome_args: Vec<&str> = vec![
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--disable-popup-blocking",
            "--disable-translate",
            "--headless=new",
        ];

        let mut all_args = vec![format!("--remote-debugging-port={}", self.cdp_port)];
        all_args.extend(chrome_args.into_iter().map(String::from));

        let child = Command::new(*chrome_path)
            .args(&all_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to launch Chrome: {}", e))?;

        Ok(child)
    }

    /// Launch browser via Playwright CLI
    fn launch_via_playwright(&self) -> anyhow::Result<Child> {
        let child = Command::new("npx")
            .args(["playwright", "open", "--browser", "chromium"])
            .env("PLAYWRIGHT_CHROMIUM_ARGS", format!("--remote-debugging-port={}", self.cdp_port))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to launch Playwright: {}", e))?;

        Ok(child)
    }

    /// Auto-launch browser using best available method
    pub fn launch(&self) -> anyhow::Result<()> {
        if self.is_browser_running() {
            println!("Browser already running on port {}", self.cdp_port);
            return Ok(());
        }

        println!("Launching browser on port {}...", self.cdp_port);

        // Try Chrome first
        if Self::is_chrome_available() {
            let child = self.launch_chrome()?;
            *self.child.lock().unwrap() = Some(child);
            return Ok(());
        }

        // Fall back to Playwright
        if Self::is_playwright_available() {
            let child = self.launch_via_playwright()?;
            *self.child.lock().unwrap() = Some(child);
            return Ok(());
        }

        anyhow::bail!(
            "No browser available. Please either:\n\
             1. Install Google Chrome, or\n\
             2. Run: npx playwright install chromium"
        )
    }

    /// Wait for browser to be ready
    fn wait_for_browser_internal(&self, timeout_secs: u64) -> Result<String, SandboxError> {
        let start = std::time::Instant::now();
        let interval = Duration::from_millis(500);

        while start.elapsed().as_secs() < timeout_secs {
            if self.is_browser_running() {
                let ws_url = format!("ws://localhost:{}/devtools/browser", self.cdp_port);
                return Ok(ws_url);
            }
            sleep(interval);
        }

        Err(SandboxError::BrowserNotReady("timeout".to_string()))
    }

    /// Stop the launched browser
    pub fn stop(&self) {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
            }
        }
    }
}

#[async_trait]
impl SandboxAdapter for PlaywrightBrowserAdapter {
    fn cdp_url(&self) -> Option<String> {
        Some(format!("ws://localhost:{}/devtools/browser", self.cdp_port))
    }

    async fn health(&self) -> Result<SandboxHealth, SandboxError> {
        if self.is_browser_running() {
            Ok(SandboxHealth {
                browser_ready: true,
                cdp_url: Some(format!("ws://localhost:{}/devtools/browser", self.cdp_port)),
                pid: None,
            })
        } else {
            Ok(SandboxHealth {
                browser_ready: false,
                cdp_url: None,
                pid: None,
            })
        }
    }

    async fn wait_for_browser(&self, timeout_secs: u64) -> Result<String, SandboxError> {
        // If no browser running, try to launch one
        if !self.is_browser_running() {
            self.launch().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
        }

        // Wait for browser to be ready
        let ws_url = self.wait_for_browser_internal(timeout_secs)?;
        Ok(ws_url)
    }

    async fn restart(&self) -> Result<(), SandboxError> {
        self.stop();
        self.launch().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
        Ok(())
    }
}