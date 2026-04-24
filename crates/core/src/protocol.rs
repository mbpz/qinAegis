use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

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

pub struct MidsceneProcess {
    child: Child,
    request_tx: mpsc::Sender<JsonRpcRequest>,
    response_rx: mpsc::Receiver<JsonRpcResponse>,
}

impl MidsceneProcess {
    pub async fn spawn() -> anyhow::Result<Self> {
        let mut child = Command::new("node")
            .args(["src/executor.ts"])
            .current_dir("sandbox")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (request_tx, request_rx) = mpsc::channel::<JsonRpcRequest>(32);
        let (resp_tx, response_rx) = mpsc::channel::<JsonRpcResponse>(32);

        // Spawn writer task
        let _writer_handle = tokio::spawn(async move {
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
        let _reader_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&line) {
                    let _ = resp_tx.send(resp).await;
                }
            }
        });

        let process = Self {
            child,
            request_tx,
            response_rx,
        };
        Ok(process)
    }

    pub async fn call(&mut self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcResponse> {
        self.request_tx.send(req).await?;
        self.response_rx
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("process died"))
    }
}
