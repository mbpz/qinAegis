// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! Playwright Browser Launcher
//!
//! Launches a Chromium browser using Playwright's CLI for local development
//! without requiring Docker. This provides a zero-dependency alternative
//! to the steel-browser Docker container.
//!
//! Usage:
//! ```bash
//! # Install browser (one-time)
//! npx playwright install chromium
//!
//! # Launch browser in debug mode
//! npx playwright chromium --remote-debugging-port=9222
//! ```

use std::process::{Child, Command, Stdio};
use std::time::Duration;
use std::thread::sleep;

/// Default CDP port for browser debugging
pub const DEFAULT_CDP_PORT: u16 = 9222;

/// Playwright browser launcher
pub struct PlaywrightLauncher {
    port: u16,
    browser_path: Option<String>,
}

/// Browser process handle
pub struct BrowserHandle {
    process: Option<Child>,
    port: u16,
}

impl PlaywrightLauncher {
    /// Create a new launcher for the specified CDP port
    pub fn new(port: u16) -> Self {
        Self {
            port,
            browser_path: None,
        }
    }

    /// Set custom browser path
    pub fn browser_path(mut self, path: &str) -> Self {
        self.browser_path = Some(path.to_string());
        self
    }

    /// Check if Playwright browser is available
    pub fn is_playwright_available() -> bool {
        Command::new("npx")
            .args(["playwright", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if Chrome/Chromium is available
    pub fn is_chrome_available() -> bool {
        // Try common macOS Chrome paths
        let chrome_paths = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/usr/bin/google-chrome",
            "/usr/bin/chromium-browser",
        ];

        for path in chrome_paths {
            if std::path::Path::new(path).exists() {
                return true;
            }
        }
        false
    }

    /// Check if CDP port is already in use
    pub fn is_port_available(&self) -> bool {
        let url = format!("http://localhost:{}/json/version", self.port);
        match reqwest::blocking::get(&url) {
            Ok(resp) => !resp.status().is_success(),
            Err(_) => true, // Port is available if connection fails
        }
    }

    /// Check if browser is running on the CDP port
    pub fn is_browser_running(&self) -> bool {
        let url = format!("http://localhost:{}/json/version", self.port);
        match reqwest::blocking::get(&url) {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Launch browser using Playwright CLI
    pub fn launch_via_playwright(&mut self) -> anyhow::Result<BrowserHandle> {
        // First ensure browser is installed
        self.ensure_playwright_installed()?;

        // Check if already running
        if !self.is_port_available() {
            println!("Browser already running on port {}", self.port);
            return self.connect_to_existing();
        }

        println!("Launching Chromium via Playwright on port {}...", self.port);

        // Launch via npx playwright open
        let child = Command::new("npx")
            .args([
                "playwright",
                "open",
                "--browser",
                "chromium",
            ])
            .env("PLAYWRIGHT_CHROMIUM_ARGS", format!("--remote-debugging-port={}", self.port))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to launch Playwright: {}", e))?;

        // Wait for browser to start
        self.wait_for_browser_startup()?;

        Ok(BrowserHandle {
            process: Some(child),
            port: self.port,
        })
    }

    /// Launch browser using system Chrome/Chromium directly
    pub fn launch_via_chrome(&mut self) -> anyhow::Result<BrowserHandle> {
        // Find Chrome
        let chrome_path = self.find_chrome_path()?;

        if !self.is_port_available() {
            println!("Browser already running on port {}", self.port);
            return self.connect_to_existing();
        }

        println!("Launching {} on port {}...", chrome_path, self.port);

        let chrome_args: Vec<&str> = vec![
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--disable-popup-blocking",
            "--disable-translate",
            "--headless=new", // Use headless mode
        ];

        let chrome_args_with_port: Vec<String> = std::iter::once(format!("--remote-debugging-port={}", self.port))
            .chain(chrome_args.into_iter().map(String::from))
            .collect();

        let child = Command::new(&chrome_path)
            .args(&chrome_args_with_port)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to launch Chrome: {}", e))?;

        self.wait_for_browser_startup()?;

        Ok(BrowserHandle {
            process: Some(child),
            port: self.port,
        })
    }

    /// Auto-detect best launch method and launch
    pub fn launch(&mut self) -> anyhow::Result<BrowserHandle> {
        // Try Chrome first if available
        if Self::is_chrome_available() {
            return self.launch_via_chrome();
        }

        // Fall back to Playwright
        if Self::is_playwright_available() {
            return self.launch_via_playwright();
        }

        anyhow::bail!(
            "No browser available. Please either:\n\
             1. Install Google Chrome, or\n\
             2. Run: npx playwright install chromium"
        )
    }

    /// Connect to an already running browser
    pub fn connect_to_existing(&self) -> anyhow::Result<BrowserHandle> {
        if !self.is_browser_running() {
            anyhow::bail!("No browser running on port {}", self.port);
        }

        // Return a handle with no child process (browser already running)
        Ok(BrowserHandle {
            process: None,
            port: self.port,
        })
    }

    fn find_chrome_path(&self) -> anyhow::Result<String> {
        let paths = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/usr/bin/google-chrome",
            "/usr/bin/chromium-browser",
        ];

        for path in &paths {
            if std::path::Path::new(path).exists() {
                return Ok(path.to_string());
            }
        }

        anyhow::bail!("Chrome not found. Please install Google Chrome.")
    }

    fn ensure_playwright_installed(&self) -> anyhow::Result<()> {
        // Check if browsers are installed
        let output = Command::new("npx")
            .args(["playwright", "install", "--dry-run", "chromium"])
            .output()?;

        if !output.status.success() {
            println!("Installing Playwright Chromium...");
            let status = Command::new("npx")
                .args(["playwright", "install", "chromium"])
                .status()?;

            if !status.success() {
                anyhow::bail!("Failed to install Playwright Chromium");
            }
        }

        Ok(())
    }

    fn wait_for_browser_startup(&self) -> anyhow::Result<()> {
        let timeout = Duration::from_secs(30);
        let interval = Duration::from_millis(500);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if self.is_browser_running() {
                return Ok(());
            }
            sleep(interval);
        }

        anyhow::bail!("Browser failed to start within {} seconds", timeout.as_secs())
    }

    /// Get CDP WebSocket URL for connecting
    pub fn cdp_ws_url(&self) -> String {
        format!("ws://localhost:{}/devtools/browser", self.port)
    }
}

impl BrowserHandle {
    /// Get the CDP WebSocket URL
    pub fn cdp_url(&self) -> String {
        format!("ws://localhost:{}/devtools/browser", self.port)
    }

    /// Get the HTTP CDP URL for JSON API
    pub fn cdp_http_url(&self) -> String {
        format!("http://localhost:{}/json", self.port)
    }

    /// Check if browser is still running
    /// Note: For spawned processes, this always returns true (assumes running until stopped)
    pub fn is_running(&self) -> bool {
        // For connected external browser, we trust it's running
        // For spawned processes, we assume it's running (check via stop())
        self.process.is_none() || true
    }

    /// Stop the browser (only if we started it)
    pub fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
        }
    }
}

impl Drop for BrowserHandle {
    fn drop(&mut self) {
        // Only kill if we started the process
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_port() {
        let launcher = PlaywrightLauncher::new(9222);
        assert_eq!(launcher.port, 9222);
    }

    #[test]
    fn test_cdp_ws_url() {
        let launcher = PlaywrightLauncher::new(9222);
        assert_eq!(launcher.cdp_ws_url(), "ws://localhost:9222/devtools/browser");
    }
}
