// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex};

const STDERR_TAIL_LIMIT: usize = 64 * 1024;

/// Keep the newest complete UTF-8 characters without growing past the byte limit.
fn append_stderr_tail(buffer: &mut String, text: &str) {
    if text.len() >= STDERR_TAIL_LIMIT {
        let mut start = text.len() - STDERR_TAIL_LIMIT;
        while !text.is_char_boundary(start) {
            start += 1;
        }
        buffer.clear();
        buffer.push_str(&text[start..]);
    } else {
        let excess = (buffer.len() + text.len()).saturating_sub(STDERR_TAIL_LIMIT);
        if excess > 0 {
            let mut start = excess;
            while !buffer.is_char_boundary(start) {
                start += 1;
            }
            buffer.drain(..start);
        }
        buffer.push_str(text);
    }
}

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
    Stress {
        target_url: String,
        users: u32,
        spawn_rate: u32,
        duration: u32,
    },
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
    /// Spawn the installed engine; developer sandbox use must be explicitly selected.
    pub async fn with_adapter(
        llm_config: Option<LlmConfig>,
        sandbox_config: Option<SandboxConfig>,
    ) -> anyhow::Result<Self> {
        let runtime = crate::runtime::RuntimeResolver::resolve()?;
        let mut cmd = Command::new(&runtime.node);
        cmd.args(&runtime.arguments)
            .current_dir(&runtime.directory)
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
                    append_stderr_tail(&mut buf, &line);
                    append_stderr_tail(&mut buf, "\n");
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
                        tracing::warn!(category = ?e.classify(), "invalid executor response");
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
                    if s.is_empty() {
                        "(empty)".to_string()
                    } else {
                        s
                    }
                };
                Err(anyhow::anyhow!(
                    "executor exited or dropped the request\nstderr:\n{}",
                    stderr_str
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{append_stderr_tail, STDERR_TAIL_LIMIT};

    #[test]
    fn stderr_tail_keeps_latest_bytes_across_writes() {
        let mut buffer = String::new();
        append_stderr_tail(&mut buffer, &"a".repeat(STDERR_TAIL_LIMIT));
        assert_eq!(buffer.len(), STDERR_TAIL_LIMIT);
        append_stderr_tail(&mut buffer, "latest\n");
        assert_eq!(buffer.len(), STDERR_TAIL_LIMIT);
        assert!(buffer.ends_with("latest\n"));
        assert_eq!(
            buffer,
            format!("{}latest\n", "a".repeat(STDERR_TAIL_LIMIT - 7))
        );
        append_stderr_tail(&mut buffer, "");
        assert_eq!(buffer.len(), STDERR_TAIL_LIMIT);
    }

    #[test]
    fn oversized_stderr_replaces_old_content_with_complete_characters() {
        let text = format!("{}tail", "界".repeat(STDERR_TAIL_LIMIT));
        let mut buffer = String::from("old content");
        append_stderr_tail(&mut buffer, &text);
        assert!(buffer.len() <= STDERR_TAIL_LIMIT);
        assert!(buffer.len() >= STDERR_TAIL_LIMIT - 3);
        assert!(text.ends_with(&buffer));
        assert!(buffer.ends_with("tail"));
        assert!(!buffer.contains("old content"));
    }

    #[test]
    fn appending_stderr_trims_at_multibyte_boundary() {
        let mut buffer = format!("界{}", "a".repeat(STDERR_TAIL_LIMIT - 3));
        append_stderr_tail(&mut buffer, "🙂");
        assert_eq!(buffer.len(), STDERR_TAIL_LIMIT);
        assert_eq!(buffer, format!("{}🙂", "a".repeat(STDERR_TAIL_LIMIT - 4)));

        let mut buffer = "界".repeat(STDERR_TAIL_LIMIT / 3);
        append_stderr_tail(&mut buffer, "ab");
        assert!(buffer.len() <= STDERR_TAIL_LIMIT);
        assert!(buffer.starts_with('界'));
        assert!(buffer.ends_with("ab"));
    }
}
