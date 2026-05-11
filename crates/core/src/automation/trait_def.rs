// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// Shared types for browser automation
// ============================================================================

#[derive(Debug, Error)]
pub enum AutomationError {
    #[error("process died: {0}")]
    ProcessDied(String),
    #[error("CDP connection failed: {0}")]
    CdpConnectionFailed(String),
    #[error("navigation failed: {0}")]
    NavigationFailed(String),
    #[error("LLM query failed: {0}")]
    LlmQueryFailed(String),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("unsupported command: {0}")]
    UnsupportedCommand(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "args")]
pub enum AutomationCommand {
    Explore { url: String, depth: u32 },
    RunYaml { yaml_script: String, case_id: String },
    Goto { url: String },
    Screenshot,
    AiQuery(String),
    AiAct(String),
    AiAssert(String),
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreResult {
    pub pages: Vec<PageInfo>,
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub primary_nav: Vec<String>,
    #[serde(default)]
    pub main_features: Vec<String>,
    #[serde(default)]
    pub auth_required: bool,
    #[serde(default)]
    pub tech_stack: Vec<String>,
    #[serde(default)]
    pub forms: Vec<FormInfo>,
    #[serde(default)]
    pub key_elements: Vec<String>,
    /// Legacy: URL-based links (deprecated, kept for compat)
    #[serde(default)]
    pub links: Vec<String>,
    /// Visual exploration: clickable UI elements
    #[serde(default)]
    pub clickable_elements: Vec<ClickableElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickableElement {
    pub description: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FormInfo {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub fields: Vec<String>,
}

impl From<AiFormInfo> for FormInfo {
    fn from(ai: AiFormInfo) -> Self {
        // If actions is empty but we have actions in the old location, use that
        // Forms might be: [String] or [{label, ...}] or [{type, label, ...}]

        // Try to extract action from text (submit button text)
        let action = if !ai.text.is_empty() {
            ai.text.clone()
        } else if !ai.actions.is_empty() {
            ai.actions.get(0).cloned().unwrap_or_default()
        } else {
            String::new()
        };

        // Extract fields - could be strings or objects
        let fields: Vec<String> = if !ai.fields.is_empty() {
            // Check if first field is a string or object
            if let Some(first) = ai.fields.first() {
                if first.is_string() {
                    // Old format: ["账号", "密码"]
                    ai.fields.iter()
                        .filter_map(|f| f.as_str().map(String::from))
                        .collect()
                } else if first.is_object() {
                    // New format: [{label: "账号"}, {label: "密码"}]
                    ai.fields.iter()
                        .filter_map(|f| f.get("label")?.as_str().map(String::from))
                        .collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        // For the new format (type/label/text), use ui_type as method
        let method = if !ai.ui_type.is_empty() {
            ai.ui_type.clone()
        } else if !ai.text.is_empty() {
            "button".to_string()
        } else {
            String::new()
        };

        FormInfo {
            action,
            method,
            fields,
        }
    }
}

// Direct conversion from JSON value for forms that come as string arrays like ["账号", "密码"]
impl From<serde_json::Value> for FormInfo {
    fn from(value: serde_json::Value) -> Self {
        if let Some(arr) = value.as_array() {
            if let Some(first) = arr.first() {
                if first.is_string() {
                    // String array format: ["账号", "密码"]
                    let fields: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    return FormInfo {
                        action: String::new(),
                        method: String::new(),
                        fields,
                    };
                } else if first.is_object() {
                    // Object array format: [{label: "账号"}, {label: "密码"}]
                    let fields: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.get("label")?.as_str().map(String::from))
                        .collect();
                    return FormInfo {
                        action: String::new(),
                        method: String::new(),
                        fields,
                    };
                }
            }
        }
        FormInfo::default()
    }
}

// Intermediate struct to match Midscene's actual response format
// Handles two formats:
// 1. Old: { actions: [...], fields: [...] } - form with action and field list
// 2. New: { type: "input|button|checkbox", label: "...", text: "..." } - flat UI elements
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiFormInfo {
    // Old format fields
    #[serde(alias = "actions", default)]
    actions: Vec<String>,
    #[serde(alias = "submitButtonText", default)]
    submit_button_text: String,
    #[serde(alias = "checkboxes", default)]
    checkboxes: Vec<String>,
    #[serde(alias = "fields", default)]
    fields: Vec<serde_json::Value>,
    // New format fields (flat UI elements)
    #[serde(rename = "type", default)]
    ui_type: String,
    #[serde(alias = "label", default)]
    label: String,
    #[serde(alias = "text", default)]
    text: String,
    #[serde(alias = "placeholder", default)]
    placeholder: String,
}

// Same for PageInfo
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiPageInfo {
    title: String,
    #[serde(alias = "primaryNav")]
    primary_nav: Vec<String>,
    #[serde(alias = "mainFeatures")]
    main_features: Vec<String>,
    #[serde(alias = "authRequired")]
    auth_required: bool,
    #[serde(alias = "techStack")]
    tech_stack: Vec<String>,
    #[serde(alias = "forms", default)]
    forms: Vec<serde_json::Value>,
    #[serde(alias = "keyElements")]
    key_elements: Vec<String>,
    #[serde(alias = "links", default)]
    links: Vec<String>,
    #[serde(alias = "clickableElements", default)]
    clickable_elements: Vec<AiClickableElement>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AiClickableElement {
    pub description: String,
    pub reason: String,
}

impl From<AiPageInfo> for PageInfo {
    fn from(ai: AiPageInfo) -> Self {
        PageInfo {
            url: String::new(),
            title: ai.title,
            primary_nav: ai.primary_nav,
            main_features: ai.main_features,
            auth_required: ai.auth_required,
            tech_stack: ai.tech_stack,
            forms: ai.forms.into_iter().map(FormInfo::from).collect(),
            key_elements: ai.key_elements,
            links: ai.links,
            clickable_elements: ai.clickable_elements.into_iter().map(|e| ClickableElement {
                description: e.description,
                reason: e.reason,
            }).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub case_id: String,
    pub passed: bool,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

// ============================================================================
// BrowserAutomation trait
// ============================================================================

/// Unified interface for browser automation operations.
///
/// Using `dyn BrowserAutomation` allows runtime switching between implementations
/// (e.g., Midscene-based, Steel-based, or mock for testing).
#[async_trait]
pub trait BrowserAutomation: Send + Sync {
    /// Execute a command and return a response.
    async fn execute(&self, cmd: AutomationCommand) -> Result<AutomationResponse, AutomationError>;

    /// Explore a website using BFS and AI page extraction.
    async fn explore(&self, url: &str, depth: u32) -> Result<ExploreResult, AutomationError>;

    /// Run a YAML test script.
    async fn run_yaml(&self, yaml_script: &str, case_id: &str) -> Result<TestResult, AutomationError>;

    /// Take a screenshot of the current page.
    async fn screenshot(&self) -> Result<String, AutomationError>;

    /// Navigate to a URL.
    async fn goto(&self, url: &str) -> Result<(), AutomationError>;

    /// AI query on the current page.
    async fn ai_query(&self, prompt: &str) -> Result<String, AutomationError>;

    /// AI action on the current page.
    async fn ai_act(&self, action: &str) -> Result<(), AutomationError>;

    /// AI assertion on the current page.
    async fn ai_assert(&self, assertion: &str) -> Result<(), AutomationError>;

    /// Shutdown the automation session.
    async fn shutdown(&self) -> Result<(), AutomationError>;
}
