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
    #[serde(default)]
    pub links: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormInfo {
    pub method: String,
    pub action: String,
    #[serde(default)]
    pub fields: Vec<String>,
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
