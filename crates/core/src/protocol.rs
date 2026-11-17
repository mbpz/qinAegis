// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
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
    #[serde(rename = "allure_generate")]
    GenerateAllure {
        results_dir: String,
        report_dir: String,
    },
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

/// A private directory created by this engine instance, never an inherited path.
struct EngineWorkspace(std::path::PathBuf, u64, u64, String);
impl EngineWorkspace {
    fn create() -> std::io::Result<Self> {
        use std::hash::{BuildHasher, Hasher};
        for _ in 0..16 {
            let token = std::collections::hash_map::RandomState::new()
                .build_hasher()
                .finish();
            let path = std::env::temp_dir().join(format!(
                "qinaegis-engine-{}-{token:016x}",
                std::process::id()
            ));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => {
                    let marker = format!("qinaegis-owned-workspace-{token:016x}");
                    std::fs::write(path.join(".qinaegis-owner"), &marker)?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;
                        let metadata = std::fs::metadata(&path)?;
                        return Ok(Self(path, metadata.dev(), metadata.ino(), marker));
                    }
                    #[cfg(not(unix))]
                    return Ok(Self(path, 0, 0, marker));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "cannot allocate engine workspace",
        ))
    }
}
impl Drop for EngineWorkspace {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let identity_matches = std::fs::symlink_metadata(&self.0).is_ok_and(|m| {
                m.is_dir() && !m.file_type().is_symlink() && m.dev() == self.1 && m.ino() == self.2
            });
            let marker_matches =
                std::fs::read_to_string(self.0.join(".qinaegis-owner")).is_ok_and(|s| s == self.3);
            if identity_matches && marker_matches {
                if let Err(error) = std::fs::remove_dir_all(&self.0) {
                    tracing::warn!(path = %self.0.display(), %error, "engine workspace retained for manual inspection");
                }
                return;
            }
        }
        // Windows lacks a pinned directory identity here; retain conservatively.
        tracing::warn!(path = %self.0.display(), "engine workspace retained: ownership cannot be revalidated");
    }
}

struct ProcessOwner {
    child: std::sync::Mutex<Child>,
    tree: crate::process_owner::ProcessTree,
    poisoned: AtomicBool,
    workspace: std::sync::Mutex<Option<EngineWorkspace>>,
    tasks: std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl ProcessOwner {
    fn terminate_now(&self) {
        if self.poisoned.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Ok(mut child) = self.child.lock() {
            self.tree.terminate_now(&mut child);
        }
        if let Ok(tasks) = self.tasks.lock() {
            for task in tasks.iter() {
                task.abort();
            }
        }
    }
    async fn terminate(&self) -> anyhow::Result<()> {
        self.terminate_now();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if self
                    .child
                    .lock()
                    .map_err(|_| anyhow::anyhow!("engine owner unavailable"))?
                    .try_wait()?
                    .is_some()
                    && self.tree.cleanup_complete()?
                {
                    self.workspace
                        .lock()
                        .map_err(|_| anyhow::anyhow!("workspace owner unavailable"))?
                        .take();
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("engine cleanup timed out"))?
    }
}
impl Drop for ProcessOwner {
    fn drop(&mut self) {
        self.terminate_now();
        // If process exit is still pending, keep the directory rather than
        // deleting files that an engine may still be writing.
        let reaped = self
            .child
            .lock()
            .ok()
            .and_then(|mut child| child.try_wait().ok())
            .flatten()
            .is_some();
        if !reaped || !self.tree.cleanup_complete().unwrap_or(false) {
            if let Ok(workspace) = self.workspace.get_mut() {
                if let Some(workspace) = workspace.take() {
                    tracing::warn!(path = %workspace.0.display(), "engine workspace retained: process cleanup is still pending");
                    std::mem::forget(workspace);
                }
            }
        }
    }
}

struct InFlight {
    owner: Arc<ProcessOwner>,
    completed: bool,
}
impl Drop for InFlight {
    fn drop(&mut self) {
        if !self.completed {
            self.owner.terminate_now();
        }
    }
}

#[derive(Clone)]
pub struct MidsceneProcess {
    owner: Arc<ProcessOwner>,
    request_tx: mpsc::Sender<JsonRpcRequest>,
    response_rx: Arc<Mutex<mpsc::Receiver<JsonRpcResponse>>>,
    call_lock: Arc<Mutex<()>>,
    cancellation: Arc<AtomicBool>,
    _stderr_buf: Arc<Mutex<String>>,
}

#[derive(Clone, Deserialize)]
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub midscene_family: Option<String>,
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
        _sandbox_config: Option<SandboxConfig>,
    ) -> anyhow::Result<Self> {
        let runtime = crate::runtime::RuntimeResolver::resolve()?;
        let workspace = EngineWorkspace::create()?;
        let mut cmd = Command::new(&runtime.node);
        cmd.args(&runtime.arguments)
            .current_dir(&workspace.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if let Some(browsers) = runtime.browser_directory()? {
            cmd.env("PLAYWRIGHT_BROWSERS_PATH", browsers);
        }

        // Pass LLM environment variables from config
        if let Some(cfg) = llm_config {
            if !cfg.api_key.is_empty() {
                let family = cfg.midscene_family.as_deref().unwrap_or("qwen2.5-vl");
                cmd.env("MIDSCENE_MODEL_API_KEY", &cfg.api_key);
                cmd.env("MIDSCENE_MODEL_BASE_URL", &cfg.base_url);
                cmd.env("MIDSCENE_MODEL_NAME", &cfg.model);
                cmd.env("MIDSCENE_MODEL_FAMILY", family);
                cmd.env("MIDSCENE_REPLANNING_CYCLE_LIMIT", "100");
                cmd.env("MIDSCENE_ACTION_CACHE", "off");
                // Intent-specific overrides
                cmd.env("MIDSCENE_INSIGHT_MODEL_NAME", &cfg.model);
                cmd.env("MIDSCENE_INSIGHT_MODEL_FAMILY", family);
                cmd.env("MIDSCENE_INSIGHT_MODEL_API_KEY", &cfg.api_key);
                cmd.env("MIDSCENE_INSIGHT_MODEL_BASE_URL", &cfg.base_url);
            }
        }

        cmd.env_remove("CDP_WS_URL").env_remove("CDP_PORT");
        let process = Self::launch(cmd).await?;
        *process
            .owner
            .workspace
            .lock()
            .map_err(|_| anyhow::anyhow!("workspace owner unavailable"))? = Some(workspace);
        Ok(process)
    }

    async fn launch(mut cmd: Command) -> anyhow::Result<Self> {
        let (mut child, tree) = crate::process_owner::ProcessTree::spawn(&mut cmd)?;
        let mut tasks = Vec::new();

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));

        // Spawn stderr reader task
        {
            let stderr_buf = stderr_buf.clone();
            tasks.push(tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut buf = stderr_buf.lock().await;
                    append_stderr_tail(&mut buf, &line);
                    append_stderr_tail(&mut buf, "\n");
                }
            }));
        }

        let (request_tx, request_rx) = mpsc::channel::<JsonRpcRequest>(32);
        let (resp_tx, response_rx) = mpsc::channel::<JsonRpcResponse>(32);

        // Spawn writer task
        tasks.push(tokio::spawn(async move {
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
        }));

        // Spawn reader task
        tasks.push(tokio::spawn(async move {
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
        }));

        Ok(Self {
            owner: Arc::new(ProcessOwner {
                child: std::sync::Mutex::new(child),
                tree,
                poisoned: AtomicBool::new(false),
                workspace: std::sync::Mutex::new(None),
                tasks: std::sync::Mutex::new(tasks),
            }),
            request_tx,
            response_rx: Arc::new(Mutex::new(response_rx)),
            call_lock: Arc::new(Mutex::new(())),
            cancellation: Arc::new(AtomicBool::new(false)),
            _stderr_buf: stderr_buf,
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
        self.call_cancellable(req, self.cancellation.clone()).await
    }

    /// Bind all subsequent calls (including cloned adapters) to the owning task.
    pub fn with_cancellation(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancellation = cancel;
        self
    }

    /// Stop the owned engine and wait for direct-child reaping, bounded to 5 seconds.
    pub async fn terminate(&self) -> anyhow::Result<()> {
        self.owner.terminate().await
    }

    /// A cancelled/timed-out/abandoned call permanently poisons this process.
    /// Entire send/receive is serialized because legacy engines reply with id=0.
    pub async fn call_cancellable(
        &self,
        req: JsonRpcRequest,
        cancel: Arc<AtomicBool>,
    ) -> anyhow::Result<JsonRpcResponse> {
        let budget = if matches!(&req, JsonRpcRequest::Shutdown) {
            Duration::from_secs(10)
        } else {
            Duration::from_secs(300)
        };
        self.call_with_deadline(req, cancel, budget).await
    }

    async fn call_with_deadline(
        &self,
        req: JsonRpcRequest,
        cancel: Arc<AtomicBool>,
        budget: Duration,
    ) -> anyhow::Result<JsonRpcResponse> {
        let cancelled = async {
            while !cancel.load(Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        };
        tokio::pin!(cancelled);
        let lock = tokio::select! {
            biased;
            _ = &mut cancelled => anyhow::bail!("engine request cancelled before admission"),
            lock = tokio::time::timeout(budget, self.call_lock.lock()) => lock.map_err(|_| anyhow::anyhow!("engine request admission timed out"))?,
        };
        anyhow::ensure!(
            !self.owner.poisoned.load(Ordering::Acquire),
            "engine is terminated and cannot accept requests"
        );
        let mut flight = InFlight {
            owner: self.owner.clone(),
            completed: false,
        };
        let exchange = async {
            self.request_tx
                .send(req)
                .await
                .map_err(|_| anyhow::anyhow!("engine request channel closed"))?;
            self.response_rx
                .lock()
                .await
                .recv()
                .await
                .ok_or_else(|| anyhow::anyhow!("engine exited without a response"))
        };
        let result = tokio::select! {
            biased;
            _ = &mut cancelled => Err(anyhow::anyhow!("engine request cancelled")),
            response = tokio::time::timeout(budget, exchange) => response.map_err(|_| anyhow::anyhow!("engine request timed out")).and_then(|v|v),
        };
        if result.is_err() {
            self.owner.terminate().await?;
        }
        flight.completed = true;
        drop(lock);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::{append_stderr_tail, STDERR_TAIL_LIMIT};

    #[cfg(any(unix, windows))]
    async fn blocking_process() -> (super::MidsceneProcess, u16) {
        use std::process::Stdio;
        let script = r#"
import subprocess,sys,json,time
child=subprocess.Popen([sys.executable,'-u','-c','import socket,time; s=socket.socket(); s.bind(("127.0.0.1",0)); s.listen(); print(s.getsockname()[1],flush=True); time.sleep(120)'],stdout=subprocess.PIPE,text=True)
port=int(child.stdout.readline())
for line in sys.stdin:
 req=json.loads(line)
 if req['method']=='currentUrl':
  print(json.dumps({'id':'0','ok':True,'data':{'port':port}}),flush=True)
 else:
  print('blocked',file=sys.stderr,flush=True)
  time.sleep(120)
"#;
        let mut command =
            tokio::process::Command::new(if cfg!(windows) { "python" } else { "python3" });
        command
            .args(["-u", "-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let process = super::MidsceneProcess::launch(command).await.unwrap();
        let reply = process
            .call(super::JsonRpcRequest::CurrentUrl)
            .await
            .unwrap();
        let port = reply.data.unwrap()["port"].as_u64().unwrap() as u16;
        assert!(std::net::TcpListener::bind(("127.0.0.1", port)).is_err());
        (process, port)
    }

    #[cfg(any(unix, windows))]
    async fn wait_for_blocked(process: &super::MidsceneProcess) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if process._stderr_buf.lock().await.contains("blocked") {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }

    #[cfg(any(unix, windows))]
    async fn assert_port_released(port: u16) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("owned grandchild retained its listening socket");
    }

    #[cfg(any(unix, windows))]
    #[tokio::test]
    async fn cancellation_reaps_owned_engine_group_and_poisons_reuse() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let (process, port) = blocking_process().await;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker = process.clone();
        let token = cancel.clone();
        let task = tokio::spawn(async move {
            worker
                .call_cancellable(super::JsonRpcRequest::AiAct("fixture".into()), token)
                .await
        });
        wait_for_blocked(&process).await;
        cancel.store(true, Ordering::Release);
        let error = task.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert!(process.owner.tree.cleanup_complete().unwrap());
        assert_port_released(port).await;
        assert!(process
            .owner
            .child
            .lock()
            .unwrap()
            .try_wait()
            .unwrap()
            .is_some());
        assert!(process
            .call(super::JsonRpcRequest::CurrentUrl)
            .await
            .unwrap_err()
            .to_string()
            .contains("terminated"));
    }

    #[cfg(any(unix, windows))]
    #[tokio::test]
    async fn cancelling_one_tree_preserves_other_owned_engine() {
        let (first, first_port) = blocking_process().await;
        let (second, second_port) = blocking_process().await;
        first.owner.terminate().await.unwrap();
        assert_port_released(first_port).await;
        assert!(std::net::TcpListener::bind(("127.0.0.1", second_port)).is_err());
        assert!(
            second
                .call(super::JsonRpcRequest::CurrentUrl)
                .await
                .unwrap()
                .ok
        );
        drop(second);
        assert_port_released(second_port).await;
    }

    #[cfg(any(unix, windows))]
    #[tokio::test]
    async fn last_owner_drop_releases_owned_grandchild_port() {
        let (process, port) = blocking_process().await;
        let clone = process.clone();
        drop(process);
        assert!(std::net::TcpListener::bind(("127.0.0.1", port)).is_err());
        drop(clone);
        assert_port_released(port).await;
    }

    #[cfg(any(unix, windows))]
    #[tokio::test]
    async fn concurrent_calls_serialize_wire_exchange() {
        use std::process::Stdio;
        let script = r#"
import json,sys,time,threading
lock=threading.Lock()
def reply(req):
 time.sleep(0.1 if req['args']=='one' else 0.001)
 with lock: print(json.dumps({'id':'0','ok':True,'data':req['args']}),flush=True)
for line in sys.stdin:
 threading.Thread(target=reply,args=(json.loads(line),)).start()
"#;
        let mut command =
            tokio::process::Command::new(if cfg!(windows) { "python" } else { "python3" });
        command
            .args(["-u", "-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let process = super::MidsceneProcess::launch(command).await.unwrap();
        let (one, two) = tokio::join!(
            process.call(super::JsonRpcRequest::AiQuery("one".into())),
            process.call(super::JsonRpcRequest::AiQuery("two".into()))
        );
        assert_eq!(one.unwrap().data.unwrap(), serde_json::json!("one"));
        assert_eq!(two.unwrap().data.unwrap(), serde_json::json!("two"));
    }

    #[cfg(any(unix, windows))]
    #[tokio::test]
    async fn timed_out_request_kills_owned_grandchild() {
        let (process, port) = blocking_process().await;
        let error = process
            .call_with_deadline(
                super::JsonRpcRequest::AiAct("fixture".into()),
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                std::time::Duration::from_millis(100),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert_port_released(port).await;
    }

    #[cfg(any(unix, windows))]
    #[tokio::test]
    async fn dropping_in_flight_future_kills_group_and_prevents_stale_response() {
        let (process, port) = blocking_process().await;
        let worker = process.clone();
        let task = tokio::spawn(async move {
            worker
                .call(super::JsonRpcRequest::AiAct("fixture".into()))
                .await
        });
        wait_for_blocked(&process).await;
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_port_released(port).await;
        assert!(process
            .call(super::JsonRpcRequest::CurrentUrl)
            .await
            .is_err());
    }

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

#[cfg(test)]
mod workspace_tests {
    use super::EngineWorkspace;
    #[cfg(unix)]
    #[test]
    fn replaced_workspace_directory_is_never_deleted() {
        let workspace = EngineWorkspace::create().unwrap();
        let original = workspace.0.clone();
        let moved = original.with_extension("moved");
        std::fs::rename(&original, &moved).unwrap();
        std::fs::create_dir(&original).unwrap();
        // Even a copied marker is insufficient without the original inode.
        std::fs::write(original.join(".qinaegis-owner"), &workspace.3).unwrap();
        std::fs::write(original.join("unrelated"), b"preserved").unwrap();
        drop(workspace);
        assert_eq!(
            std::fs::read(original.join("unrelated")).unwrap(),
            b"preserved"
        );
        std::fs::remove_dir_all(original).unwrap();
        std::fs::remove_dir_all(moved).unwrap();
    }
    #[test]
    fn workspaces_are_unique_private_and_only_remove_owned_files() {
        let first = EngineWorkspace::create().unwrap();
        let second = EngineWorkspace::create().unwrap();
        assert_ne!(first.0, second.0);
        std::fs::write(first.0.join("output"), b"one").unwrap();
        std::fs::write(second.0.join("output"), b"two").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&first.0).unwrap().permissions().mode() & 0o777,
                0o700
            );
            std::os::unix::fs::symlink(&second.0, first.0.join("external")).unwrap();
        }
        let path = first.0.clone();
        drop(first);
        #[cfg(unix)]
        assert!(!path.exists());
        assert_eq!(std::fs::read(second.0.join("output")).unwrap(), b"two");
    }
}
