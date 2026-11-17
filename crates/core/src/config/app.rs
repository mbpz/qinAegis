// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::asset_file::AssetFile;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_CONFIG_BYTES: usize = 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct ConfigInspection {
    pub revision: u64,
    pub backup_available: bool,
}

fn config_error() -> ConfigError {
    ConfigError::Internal("configuration storage operation failed".into())
}
fn parse_config(bytes: &[u8]) -> Result<(AppConfig, u64), ConfigError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ConfigError::Parse("invalid configuration".into()))?;
    let config =
        toml::from_str(text).map_err(|_| ConfigError::Parse("invalid configuration".into()))?;
    let value: toml::Value =
        toml::from_str(text).map_err(|_| ConfigError::Parse("invalid configuration".into()))?;
    let revision = match value.get("qinaegis_revision") {
        None => 0,
        Some(v) => v
            .as_integer()
            .filter(|v| *v >= 0)
            .ok_or_else(config_error)? as u64,
    };
    Ok((config, revision))
}

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
    /// Secondary LLM for fallback or complex vision tasks.
    /// When empty, only the primary LLM is used.
    #[serde(default)]
    pub secondary_provider: String,
    #[serde(default)]
    pub secondary_base_url: String,
    #[serde(default)]
    pub secondary_api_key: String,
    #[serde(default)]
    pub secondary_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfigSection {
    /// Port for CDP (Chrome DevTools Protocol)
    pub cdp_port: u16,
}

impl Default for SandboxConfigSection {
    fn default() -> Self {
        Self { cdp_port: 9222 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationConfigSection {
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    #[serde(default = "default_max_pages_per_seed")]
    pub max_pages_per_seed: u32,
}

fn default_max_depth() -> u32 {
    3
}
fn default_max_pages_per_seed() -> u32 {
    20
}

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
            model: "MiniMax-M2.7".to_string(),
            secondary_provider: String::new(),
            secondary_base_url: String::new(),
            secondary_api_key: String::new(),
            secondary_model: String::new(),
        }
    }
}

impl AppConfig {
    /// OS config directory by default; explicit data roots contain their own config.
    pub fn global_path() -> PathBuf {
        if std::env::var_os("QINAEGIS_DATA_DIR").is_some() {
            return crate::LocalStorage::base_path().join("config.toml");
        }
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
        let meta = std::fs::symlink_metadata(path).map_err(|_| config_error())?;
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_CONFIG_BYTES as u64
        {
            return Err(config_error());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| config_error())?
            .take(MAX_CONFIG_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| config_error())?;
        if bytes.len() > MAX_CONFIG_BYTES {
            return Err(config_error());
        }
        parse_config(&bytes).map(|(config, _)| config)
    }

    /// Load from multiple sources, with project config taking precedence.
    /// Order: global defaults → per-project overrides
    pub fn load_multi_source(project_dir: Option<&std::path::Path>) -> Result<Self, ConfigError> {
        let mut config = match Self::load_global() {
            Ok(config) => config,
            Err(ConfigError::NotFound(_)) => Self::default(),
            Err(error) => return Err(error),
        };

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
        // Secondary LLM
        if !other.llm.secondary_provider.is_empty() {
            self.llm.secondary_provider = other.llm.secondary_provider;
        }
        if !other.llm.secondary_base_url.is_empty() {
            self.llm.secondary_base_url = other.llm.secondary_base_url;
        }
        if !other.llm.secondary_api_key.is_empty() {
            self.llm.secondary_api_key = other.llm.secondary_api_key;
        }
        if !other.llm.secondary_model.is_empty() {
            self.llm.secondary_model = other.llm.secondary_model;
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
        self.llm.secondary_api_key = resolve_env_var(&self.llm.secondary_api_key);
        self.llm.secondary_base_url = resolve_env_var(&self.llm.secondary_base_url);
    }

    /// Existing callers retain compatibility; UI callers should use save_to_checked.
    pub fn save_global(&self) -> Result<(), ConfigError> {
        let path = Self::global_path();
        let revision = Self::inspect_at(&path)?.revision;
        self.save_to_checked(&path, revision).map(|_| ())
    }

    pub fn backup_path(path: &Path) -> PathBuf {
        path.with_file_name(format!(
            "{}.backup",
            path.file_name().unwrap_or_default().to_string_lossy()
        ))
    }

    /// Returns only revision/backup metadata, never credentials or configuration values.
    pub fn inspect_at(path: &Path) -> Result<ConfigInspection, ConfigError> {
        let file = AssetFile::new(path.to_owned(), MAX_CONFIG_BYTES);
        let guard = file.lock().map_err(|_| config_error())?;
        let revision = guard
            .read()
            .map_err(|_| config_error())?
            .as_deref()
            .map(parse_config)
            .transpose()?
            .map(|(_, r)| r)
            .unwrap_or(0);
        let backup = AssetFile::new(Self::backup_path(path), MAX_CONFIG_BYTES);
        let backup_guard = backup.lock().map_err(|_| config_error())?;
        let backup_available = backup_guard
            .read()
            .map_err(|_| config_error())?
            .as_deref()
            .map(parse_config)
            .transpose()?
            .is_some();
        Ok(ConfigInspection {
            revision,
            backup_available,
        })
    }

    pub fn save_to_checked(&self, path: &Path, expected_revision: u64) -> Result<u64, ConfigError> {
        let file = AssetFile::new(path.to_owned(), MAX_CONFIG_BYTES);
        let guard = file.lock().map_err(|_| config_error())?;
        let previous = guard.read().map_err(|_| config_error())?;
        let revision = previous
            .as_deref()
            .map(parse_config)
            .transpose()?
            .map(|(_, r)| r)
            .unwrap_or(0);
        if revision != expected_revision {
            return Err(ConfigError::Internal(
                "configuration revision conflict".into(),
            ));
        }
        let next = revision
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or_else(config_error)?;
        let mut value = toml::Value::try_from(self).map_err(|_| config_error())?;
        value.as_table_mut().ok_or_else(config_error)?.insert(
            "qinaegis_revision".into(),
            toml::Value::Integer(next as i64),
        );
        let bytes = toml::to_string_pretty(&value)
            .map_err(|_| config_error())?
            .into_bytes();
        if bytes.len() > MAX_CONFIG_BYTES {
            return Err(config_error());
        }
        let backup = AssetFile::new(Self::backup_path(path), MAX_CONFIG_BYTES);
        let backup_guard = backup.lock().map_err(|_| config_error())?;
        // Refuse to silently overwrite a damaged recovery copy.
        if let Some(existing) = backup_guard.read().map_err(|_| config_error())? {
            parse_config(&existing)?;
        }
        if let Some(previous) = previous {
            backup_guard.write(&previous).map_err(|_| config_error())?;
        }
        guard.write(&bytes).map_err(|_| config_error())?;
        Ok(next)
    }

    /// Explicit restore of a valid backup. Invalid primary files remain untouched:
    /// recovery from corruption requires retaining that file before operator repair.
    pub fn restore_backup_at(path: &Path, expected_revision: u64) -> Result<u64, ConfigError> {
        let backup = AssetFile::new(Self::backup_path(path), MAX_CONFIG_BYTES);
        let bytes = {
            let guard = backup.lock().map_err(|_| config_error())?;
            guard
                .read()
                .map_err(|_| config_error())?
                .ok_or_else(config_error)?
        };
        let (config, _) = parse_config(&bytes)?;
        config.save_to_checked(path, expected_revision)
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
                let mut var_name = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' {
                        var_name.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
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
            midscene_family: None,
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
