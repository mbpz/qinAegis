// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use thiserror::Error;

// ============================================================================
// AppConfig errors
// ============================================================================

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config file not found: {0}")]
    NotFound(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("env var not set: {0}")]
    EnvVarNotSet(String),
    #[error("internal: {0}")]
    Internal(String),
}

// ============================================================================
// AppConfig — unified configuration for CLI and core
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub llm: LlmConfigSection,
    pub sandbox: SandboxConfigSection,
    #[serde(default)]
    pub exploration: ExplorationConfigSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfigSection {
    pub provider: String,
    pub base_url: String,
    /// API key — may contain "$VAR" or "${VAR}" env var references.
    /// Resolved lazily via resolve().
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfigSection {
    /// Port for CDP (Chrome DevTools Protocol)
    pub cdp_port: u16,
}

impl Default for SandboxConfigSection {
    fn default() -> Self {
        Self {
            cdp_port: 9222,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationConfigSection {
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    #[serde(default = "default_max_pages_per_seed")]
    pub max_pages_per_seed: u32,
}

fn default_max_depth() -> u32 { 3 }
fn default_max_pages_per_seed() -> u32 { 20 }

impl Default for ExplorationConfigSection {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_pages_per_seed: 20,
        }
    }
}

impl Default for LlmConfigSection {
    fn default() -> Self {
        Self {
            provider: "minimax".to_string(),
            base_url: "https://api.minimax.chat/v1".to_string(),
            api_key: String::new(),
            model: "MiniMax-VL-01".to_string(),
        }
    }
}

impl AppConfig {
    /// Path to the global config file: ~/.config/qinAegis/config.toml
    pub fn global_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("qinAegis")
            .join("config.toml")
    }

    /// Path to a per-project config file: <project_dir>/qinAegis.toml
    pub fn project_path(project_dir: &std::path::Path) -> PathBuf {
        project_dir.join("qinAegis.toml")
    }

    /// Load config from global path (~/.config/qinAegis/config.toml).
    pub fn load_global() -> Result<Self, ConfigError> {
        let path = Self::global_path();
        if !path.exists() {
            return Err(ConfigError::NotFound(path.display().to_string()));
        }
        Self::load_from(&path)
    }

    /// Load config from a specific path.
    pub fn load_from(path: &std::path::Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Internal(e.to_string()))?;
        toml::from_str(&content)
            .map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Load from multiple sources, with project config taking precedence.
    /// Order: global defaults → per-project overrides
    pub fn load_multi_source(project_dir: Option<&std::path::Path>) -> Result<Self, ConfigError> {
        let mut config = Self::load_global().unwrap_or_default();

        if let Some(dir) = project_dir {
            let project_path = Self::project_path(dir);
            if project_path.exists() {
                let project_config = Self::load_from(&project_path)?;
                config.merge(project_config);
            }
        }

        Ok(config)
    }

    /// Merge another config into self (other takes precedence).
    pub fn merge(&mut self, other: Self) {
        // LLM
        if !other.llm.provider.is_empty() {
            self.llm.provider = other.llm.provider;
        }
        if !other.llm.base_url.is_empty() {
            self.llm.base_url = other.llm.base_url;
        }
        if !other.llm.api_key.is_empty() {
            self.llm.api_key = other.llm.api_key;
        }
        if !other.llm.model.is_empty() {
            self.llm.model = other.llm.model;
        }
        // Sandbox
        if other.sandbox.cdp_port != 0 {
            self.sandbox.cdp_port = other.sandbox.cdp_port;
        }
        // Exploration
        if other.exploration.max_depth != 0 {
            self.exploration.max_depth = other.exploration.max_depth;
        }
        if other.exploration.max_pages_per_seed != 0 {
            self.exploration.max_pages_per_seed = other.exploration.max_pages_per_seed;
        }
    }

    /// Resolve all environment variable references in string values.
    /// Supports $VAR and ${VAR} syntax.
    pub fn resolve_env(&mut self) {
        self.llm.api_key = resolve_env_var(&self.llm.api_key);
        self.llm.base_url = resolve_env_var(&self.llm.base_url);
    }

    /// Save to global config path.
    pub fn save_global(&self) -> Result<(), ConfigError> {
        let path = Self::global_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ConfigError::Internal(e.to_string()))?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::Parse(e.to_string()))?;
        std::fs::write(&path, content)
            .map_err(|e| ConfigError::Internal(e.to_string()))?;
        Ok(())
    }
}

/// Resolve $VAR and ${VAR} patterns in a string.
/// If a variable is not set, leave it as-is.
pub fn resolve_env_var(s: &str) -> String {
    let result = s.to_string();

    // Handle ${VAR} patterns
    let mut resolved = String::new();
    let mut chars = result.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            if chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                let var_name: String = chars.by_ref().take_while(|&c| c != '}').collect();
                let replacement = env::var(&var_name).unwrap_or_default();
                resolved.push_str(&replacement);
            } else {
                let var_name: String = chars.by_ref().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                let replacement = env::var(&var_name).unwrap_or_default();
                resolved.push_str(&replacement);
            }
        } else {
            resolved.push(c);
        }
    }

    resolved
}

/// Convert AppConfig to protocol LlmConfig (for MidsceneAutomation)
impl From<&AppConfig> for crate::protocol::LlmConfig {
    fn from(app: &AppConfig) -> Self {
        crate::protocol::LlmConfig {
            api_key: resolve_env_var(&app.llm.api_key),
            base_url: resolve_env_var(&app.llm.base_url),
            model: app.llm.model.clone(),
        }
    }
}

/// Convert AppConfig to protocol SandboxConfig
impl From<&AppConfig> for crate::protocol::SandboxConfig {
    fn from(app: &AppConfig) -> Self {
        crate::protocol::SandboxConfig {
            cdp_port: app.sandbox.cdp_port,
        }
    }
}
