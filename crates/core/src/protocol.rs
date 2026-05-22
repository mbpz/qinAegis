// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

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
    #[serde(rename = "currentUrl")]
    CurrentUrl,
    #[serde(rename = "run_yaml")]
    RunYaml {
        yaml_script: String,
        case_id: String,
        target_url: Option<String>,
    },
    #[serde(rename = "lighthouse")]
    Lighthouse { url: String },
    #[serde(rename = "stress")]
    Stress { target_url: String, users: u32, spawn_rate: u32, duration: u32 },
    #[serde(rename = "zap_scan")]
    ZapScan { target_url: String },
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
    stderr_buf: Arc<Mutex<String>>,
}

impl Clone for MidsceneProcess {
    fn clone(&self) -> Self {
        Self {
            child: self.child.clone(),
            request_tx: self.request_tx.clone(),
            response_rx: self.response_rx.clone(),
            stderr_buf: self.stderr_buf.clone(),
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
    fn resolve_node() -> String {
        let home = std::env::var("HOME").unwrap_or_default();
        let candidates: Vec<String> = vec![
            "/opt/homebrew/bin/node".into(),
            "/usr/local/bin/node".into(),
            "/opt/local/bin/node".into(),
            "/usr/bin/node".into(),
            format!("{}/.n/bin/node", home),
            format!("{}/.local/bin/node", home),
        ];
        for path in &candidates {
            if std::path::Path::new(path).exists() {
                return path.clone();
            }
        }
        "node".to_string()
    }

    /// Spawn the TS executor process (browser lifecycle managed by executor).
    pub async fn with_adapter(
        llm_config: Option<LlmConfig>,
        sandbox_config: Option<SandboxConfig>,
    ) -> anyhow::Result<Self> {
        let sandbox_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .parent().unwrap()
            .join("sandbox");
        let tsx_path = sandbox_dir.join("node_modules/.bin/tsx");
        let node_path = Self::resolve_node();

        let mut cmd = Command::new(&node_path);
        cmd.args([&tsx_path.to_string_lossy().to_string(), "src/executor.ts"])
            .current_dir(&sandbox_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // Pass LLM environment variables from config
        if let Some(cfg) = llm_config {
            if !cfg.api_key.is_empty() {
                cmd.env("MIDSCENE_MODEL_API_KEY", &cfg.api_key);
                cmd.env("MIDSCENE_MODEL_BASE_URL", &cfg.base_url);
                cmd.env("MIDSCENE_MODEL_NAME", &cfg.model);
                cmd.env("MIDSCENE_MODEL_FAMILY", "qwen2.5-vl");
                cmd.env("MIDSCENE_REPLANNING_CYCLE_LIMIT", "100");
                cmd.env("MIDSCENE_ACTION_CACHE", "off");
                // Intent-specific overrides
                cmd.env("MIDSCENE_INSIGHT_MODEL_NAME", &cfg.model);
                cmd.env("MIDSCENE_INSIGHT_MODEL_FAMILY", "qwen2.5-vl");
                cmd.env("MIDSCENE_INSIGHT_MODEL_API_KEY", &cfg.api_key);
                cmd.env("MIDSCENE_INSIGHT_MODEL_BASE_URL", &cfg.base_url);
            }
        }

        // Pass CDP port so TS executor uses a free port instead of hardcoded 9222
        if let Some(cfg) = sandbox_config.as_ref() {
            cmd.env("CDP_PORT", cfg.cdp_port.to_string());
        }

        let mut child = cmd.spawn()?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));

        // Spawn stderr reader task
        {
            let stderr_buf = stderr_buf.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut buf = stderr_buf.lock().await;
                    buf.push_str(&line);
                    buf.push('\n');
                }
            });
        }

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
                let trimmed = line.trim();
                if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
                    continue;
                }
                println!("[protocol] Received JSON line: {} chars", line.len());
                match serde_json::from_str::<JsonRpcResponse>(&line) {
                    Ok(resp) => {
                        println!("[protocol] Parsed OK, sending to channel");
                        let _ = resp_tx.send(resp).await;
                    }
                    Err(e) => {
                        println!("[protocol] WARN: JsonRpcResponse parse error: {}", e);
                        println!("[protocol] WARN: Raw line (first 300): {}", &line[..line.len().min(300)]);
                    }
                }
            }
            println!("[protocol] Reader loop ended");
        });

        Ok(Self {
            child: Arc::new(child),
            request_tx,
            response_rx: Arc::new(Mutex::new(response_rx)),
            stderr_buf,
        })
    }

    /// Spawn with SandboxConfig (creates PlaywrightBrowserAdapter).
    pub async fn spawn(
        llm_config: Option<LlmConfig>,
        sandbox_config: Option<SandboxConfig>,
    ) -> anyhow::Result<Self> {
        Self::with_adapter(llm_config, sandbox_config).await
    }

    pub async fn call(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcResponse> {
        let method_name = match &req {
            JsonRpcRequest::AiQuery(_) => "aiQuery",
            JsonRpcRequest::AiAct(_) => "aiAct",
            JsonRpcRequest::AiAssert(_) => "aiAssert",
            JsonRpcRequest::Explore { .. } => "explore",
            JsonRpcRequest::Goto { .. } => "goto",
            JsonRpcRequest::Screenshot => "screenshot",
            JsonRpcRequest::CurrentUrl => "currentUrl",
            JsonRpcRequest::RunYaml { .. } => "run_yaml",
            JsonRpcRequest::Lighthouse { .. } => "lighthouse",
            JsonRpcRequest::Stress { .. } => "stress",
            JsonRpcRequest::ZapScan { .. } => "zap_scan",
            JsonRpcRequest::Shutdown => "shutdown",
        };
        println!("[protocol] Sending {} request...", method_name);
        self.request_tx.send(req).await?;
        println!("[protocol] Waiting for response...");
        let resp = self.response_rx.lock().await.recv().await;
        println!("[protocol] Response received");
        match resp {
            Some(resp) => Ok(resp),
            None => {
                let stderr = self.stderr_buf.lock().await;
                let stderr_str = if stderr.is_empty() {
                    "(empty)".to_string()
                } else {
                    let s = stderr.trim().to_string();
                    if s.is_empty() { "(empty)".to_string() } else { s }
                };
                Err(anyhow::anyhow!(
                    "executor exited or dropped the request\nstderr:\n{}",
                    stderr_str
                ))
            }
        }
    }
}
