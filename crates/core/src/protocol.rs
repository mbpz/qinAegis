// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::sandbox::{SandboxAdapter, PlaywrightBrowserAdapter};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex};

// ============================================================================
// JSON-RPC types (shared between Rust CLI and TypeScript executor)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "args")]
pub enum JsonRpcRequest {
    #[serde(rename = "aiQuery")]
    AiQuery(String),
    #[serde(rename = "aiAct")]
    AiAct(String),
    #[serde(rename = "aiAssert")]
    AiAssert(String),
    #[serde(rename = "explore")]
    Explore { url: String, depth: u32 },
    #[serde(rename = "goto")]
    Goto { url: String },
    #[serde(rename = "screenshot")]
    Screenshot,
    #[serde(rename = "run_yaml")]
    RunYaml {
        yaml_script: String,
        case_id: String,
    },
    #[serde(rename = "lighthouse")]
    Lighthouse { url: String },
    #[serde(rename = "stress")]
    Stress { target_url: String, users: u32, spawn_rate: u32, duration: u32 },
    #[serde(rename = "shutdown")]
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub ok: bool,
    #[serde(rename = "id")]
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl JsonRpcResponse {
    pub fn ok(id: impl Into<String>, data: impl Serialize) -> Self {
        JsonRpcResponse {
            ok: true,
            id: id.into(),
            data: Some(serde_json::to_value(data).unwrap()),
            error: None,
        }
    }

    pub fn err(id: impl Into<String>, error: impl Into<String>) -> Self {
        JsonRpcResponse {
            ok: false,
            id: id.into(),
            data: None,
            error: Some(error.into()),
        }
    }
}

// ============================================================================
// MidsceneProcess
// ============================================================================

pub struct MidsceneProcess {
    #[allow(dead_code)]
    child: Arc<Child>,
    request_tx: mpsc::Sender<JsonRpcRequest>,
    response_rx: Arc<Mutex<mpsc::Receiver<JsonRpcResponse>>>,
}

impl Clone for MidsceneProcess {
    fn clone(&self) -> Self {
        Self {
            child: self.child.clone(),
            request_tx: self.request_tx.clone(),
            response_rx: self.response_rx.clone(),
        }
    }
}

#[derive(Clone)]
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

#[derive(Clone)]
pub struct SandboxConfig {
    pub cdp_port: u16,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self { cdp_port: 9333 }
    }
}

impl MidsceneProcess {
    /// Spawn with a concrete SandboxAdapter.
    /// The adapter is used to resolve the CDP URL and wait for browser readiness.
    pub async fn with_adapter(
        llm_config: Option<LlmConfig>,
        adapter: Arc<dyn SandboxAdapter>,
    ) -> anyhow::Result<Self> {
        // Navigate from crates/core to project root (../../)
        let sandbox_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()  // crates
            .parent().unwrap()  // project root
            .join("sandbox");
        let tsx_path = sandbox_dir.join("node_modules/.bin/tsx");

        // Wait for browser to be ready via adapter
        let cdp_url = adapter
            .wait_for_browser(30)
            .await
            .map_err(|e| anyhow::anyhow!("browser not ready: {}", e))?;

        let mut cmd = Command::new(&tsx_path);
        cmd.args(["src/executor.ts"])
            .current_dir(&sandbox_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);

        // Pass CDP WebSocket URL from adapter
        cmd.env("CDP_WS_URL", &cdp_url);

        // Pass LLM environment variables from config
        if let Some(cfg) = llm_config {
            if !cfg.api_key.is_empty() {
                cmd.env("MIDSCENE_MODEL_API_KEY", &cfg.api_key);
                cmd.env("MIDSCENE_MODEL_BASE_URL", &cfg.base_url);
                cmd.env("MIDSCENE_MODEL_NAME", &cfg.model);
            }
        }

        let mut child = cmd.spawn()?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (request_tx, request_rx) = mpsc::channel::<JsonRpcRequest>(32);
        let (resp_tx, response_rx) = mpsc::channel::<JsonRpcResponse>(32);

        // Spawn writer task
        tokio::spawn(async move {
            let mut stdin = stdin;
            let mut rx = request_rx;
            while let Some(req) = rx.recv().await {
                let line = match serde_json::to_string(&req) {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("failed to serialize request: {}", e);
                        continue;
                    }
                };
                if let Err(e) = stdin.write_all(line.as_bytes()).await {
                    tracing::error!("failed to write to stdin: {}", e);
                    break;
                }
                if let Err(e) = stdin.write_all(b"\n").await {
                    tracing::error!("failed to write newline to stdin: {}", e);
                    break;
                }
            }
        });

        // Spawn reader task
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                // Skip non-JSON lines (e.g., Midscene report output)
                let trimmed = line.trim();
                if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
                    continue;
                }
                println!("[protocol] Received JSON line: {} chars", line.len());
                if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&line) {
                    println!("[protocol] Parsed OK, sending to channel");
                    let _ = resp_tx.send(resp).await;
                } else {
                    println!("[protocol] WARN: Failed to parse JSON: {}", &line[..line.len().min(200)]);
                }
            }
            println!("[protocol] Reader loop ended");
        });

        Ok(Self {
            child: Arc::new(child),
            request_tx,
            response_rx: Arc::new(Mutex::new(response_rx)),
        })
    }

    /// Spawn with SandboxConfig (creates PlaywrightBrowserAdapter).
    pub async fn spawn(
        llm_config: Option<LlmConfig>,
        sandbox_config: Option<SandboxConfig>,
    ) -> anyhow::Result<Self> {
        let cfg = sandbox_config.unwrap_or_default();
        let adapter: Arc<dyn SandboxAdapter> = Arc::new(PlaywrightBrowserAdapter::new(cfg.cdp_port));
        Self::with_adapter(llm_config, adapter).await
    }

    pub async fn call(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcResponse> {
        let method_name = match &req {
            JsonRpcRequest::AiQuery(_) => "aiQuery",
            JsonRpcRequest::AiAct(_) => "aiAct",
            JsonRpcRequest::AiAssert(_) => "aiAssert",
            JsonRpcRequest::Explore { .. } => "explore",
            JsonRpcRequest::Goto { .. } => "goto",
            JsonRpcRequest::Screenshot => "screenshot",
            JsonRpcRequest::RunYaml { .. } => "run_yaml",
            JsonRpcRequest::Lighthouse { .. } => "lighthouse",
            JsonRpcRequest::Stress { .. } => "stress",
            JsonRpcRequest::Shutdown => "shutdown",
        };
        println!("[protocol] Sending {} request...", method_name);
        self.request_tx.send(req).await?;
        println!("[protocol] Waiting for response...");
        let resp = self.response_rx.lock().await.recv().await;
        println!("[protocol] Response received");
        resp.ok_or_else(|| anyhow::anyhow!("process died"))
    }
}
