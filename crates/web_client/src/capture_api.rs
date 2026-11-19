//! Opt-in policy and evidence access. No client-selected filesystem paths.
use crate::app::AppState;
use anyhow::{ensure, Context, Result};
use qin_aegis_core::{
    capture::{self, CapturePolicy, CapturePolicyStore},
    execution_records::{RunRecord, RunStore},
    protocol::{JsonRpcRequest, MidsceneProcess},
    LocalStorage,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

const TTL: u64 = 900;
pub(crate) struct Viewer {
    id: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    done: mpsc::Receiver<std::result::Result<(), String>>,
    process: Arc<Mutex<Option<MidsceneProcess>>>,
    journal: Arc<capture::ViewerOwnerStore>,
    blocked: Arc<AtomicBool>,
    deadline: Instant,
}
impl Drop for Viewer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
fn owners() -> capture::ViewerOwnerStore {
    capture::ViewerOwnerStore::new(LocalStorage::base_path())
}
/// Outstanding owners from a previous service instance are never inferred clean.
pub(crate) fn recover() -> Result<bool> {
    Ok(!owners().owners()?.is_empty())
}
/// Reuse the execution safety gate for a failed viewer or a journaled orphan.
pub(crate) fn ensure_execution_safe(app: &AppState) -> Result<()> {
    let untracked = owners()
        .owners()?
        .iter()
        .any(|id| !app.trace_viewers.contains_key(id));
    if untracked {
        app.resource_blocked.store(true, Ordering::Release);
    }
    ensure!(
        !app.resource_blocked.load(Ordering::Acquire),
        "Viewer or execution resource cleanup is unconfirmed; further engine actions are blocked"
    );
    Ok(())
}
pub(crate) fn ensure_runtime_idle(app: &AppState) -> Result<()> {
    ensure_execution_safe(app)?;
    ensure!(
        app.trace_viewers.is_empty(),
        "Close all Trace viewers before runtime maintenance"
    );
    Ok(())
}
impl Viewer {
    fn close(&mut self) -> Result<()> {
        self.close_with_timeout(Duration::from_secs(7))
    }
    fn close_with_timeout(&mut self, timeout: Duration) -> Result<()> {
        let result = (|| -> Result<()> {
            if let Some(stop) = self.stop.take() {
                let _ = stop.send(());
            }
            let finished = self.done.recv_timeout(timeout);
            match finished {
                Ok(Ok(())) => {}
                Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // Preserve the actual process owner after a failed first cleanup
                    // and allow an explicit close to confirm it on a subsequent try.
                    let process = self
                        .process
                        .lock()
                        .map_err(|_| anyhow::anyhow!("viewer owner unavailable"))?
                        .clone();
                    let process =
                        process.context("Trace viewer startup/cleanup could not be confirmed")?;
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?;
                    rt.block_on(process.terminate())?;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    self.blocked.store(true, Ordering::Release);
                    anyhow::bail!("Trace viewer cleanup not confirmed");
                }
            }
            // Release the durable intent only after the owned process tree and its
            // workspace have really terminated. A failed journal write stays blocked.
            if let Err(error) = self.journal.release(&self.id) {
                self.blocked.store(true, Ordering::Release);
                return Err(error);
            }
            self.process
                .lock()
                .map_err(|_| anyhow::anyhow!("viewer owner unavailable"))?
                .take();
            Ok(())
        })();
        if result.is_err() {
            self.blocked.store(true, Ordering::Release);
        }
        result
    }
}
pub(crate) type Viewers = HashMap<String, Viewer>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    expected_revision: u64,
    policy: CapturePolicy,
    approved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Run {
    project: String,
    run_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    project: String,
    run_id: String,
    case_id: String,
    artifact_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Open {
    project: String,
    run_id: String,
    case_id: String,
    artifact_id: String,
    approved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Close {
    viewer_id: String,
}
fn parameters<T: DeserializeOwned>(value: &Value) -> Result<T> {
    serde_json::from_value(value.clone()).context("invalid process evidence parameters")
}
fn record(project: &str, run: &str) -> Result<RunRecord> {
    crate::dispatch::validate_report_path(project, run).map_err(anyhow::Error::msg)?;
    let value = RunStore::new(LocalStorage::run_dir(project, run)).load()?;
    ensure!(
        value.project == project && value.run_id == run,
        "process evidence identity mismatch"
    );
    Ok(value)
}
fn artifact(p: &Artifact) -> Result<(capture::CaptureArtifact, Vec<u8>)> {
    let record = record(&p.project, &p.run_id)?;
    ensure!(
        record.status != "running",
        "wait for a terminal run before opening attachments"
    );
    ensure!(
        record.cases.iter().any(|c| c.case_id == p.case_id),
        "case is not part of this run"
    );
    let receipt = record
        .results
        .iter()
        .find(|r| r.case_id == p.case_id)
        .and_then(|r| r.capture.as_ref())
        .context("no final capture receipt for this case")?;
    let meta = receipt
        .artifacts
        .iter()
        .find(|a| a.id == p.artifact_id)
        .context("artifact is not committed")?
        .clone();
    let bytes = capture::read_committed_artifact(
        &LocalStorage::run_dir(&p.project, &p.run_id),
        &p.case_id,
        receipt,
        &p.artifact_id,
    )?;
    Ok((meta, bytes))
}
pub(crate) fn handles(method: &str) -> bool {
    matches!(
        method,
        "getCapturePolicy"
            | "saveCapturePolicy"
            | "readProcessEvidence"
            | "readProcessArtifact"
            | "startTraceViewer"
            | "stopTraceViewer"
    )
}
pub(crate) fn dispatch(app: &mut AppState, method: &str, value: &Value) -> Result<Value> {
    match method {
        "getCapturePolicy" => {
            let _: Empty = parameters(value)?;
            Ok(serde_json::to_value(
                CapturePolicyStore::new(LocalStorage::base_path()).snapshot()?,
            )?)
        }
        "saveCapturePolicy" => {
            let p: Save = parameters(value)?;
            ensure!(p.approved, "explicit capture policy confirmation required");
            Ok(serde_json::to_value(
                CapturePolicyStore::new(LocalStorage::base_path())
                    .save(p.expected_revision, p.policy)?,
            )?)
        }
        "readProcessEvidence" => {
            let p: Run = parameters(value)?;
            let r = record(&p.project, &p.run_id)?;
            let cases: Vec<_> = r.cases.iter().map(|case| {
                let receipt = r.results.iter().find(|result| result.case_id == case.case_id).and_then(|result| result.capture.as_ref());
                let artifacts: Vec<_> = receipt.map(|receipt| receipt.artifacts.iter().map(|a| {
                    match capture::read_committed_artifact(&LocalStorage::run_dir(&p.project, &p.run_id), &case.case_id, receipt, &a.id) {
                        Ok(_) => json!({"id":a.id,"available":true}),
                        Err(_) => json!({"id":a.id,"available":false,"reason":"Attachment missing, changed, unsafe or unsupported"}),
                    }
                }).collect()).unwrap_or_default();
                json!({"case_id":case.case_id,"name":case.name,"receipt":receipt,"artifacts":artifacts})
            }).collect();
            Ok(
                json!({"schema_version":1,"project":r.project,"run_id":r.run_id,"status":r.status,"policy":r.capture_policy,"cases":cases}),
            )
        }
        "readProcessArtifact" => {
            let p: Artifact = parameters(value)?;
            let (a, bytes) = artifact(&p)?;
            Ok(
                json!({"kind":a.kind,"bytes":a.bytes,"sha256":a.sha256,"file_name":a.file_name,"base64":capture::encode_base64(&bytes)}),
            )
        }
        "startTraceViewer" => {
            let p: Open = parameters(value)?;
            ensure!(
                p.approved,
                "explicit sensitive Trace viewing confirmation required"
            );
            let (meta, bytes) = artifact(&Artifact {
                project: p.project,
                run_id: p.run_id,
                case_id: p.case_id,
                artifact_id: p.artifact_id,
            })?;
            ensure!(
                meta.kind == "trace",
                "only committed Playwright Trace archives can be opened"
            );
            let expired: Vec<_> = app
                .trace_viewers
                .iter()
                .filter(|(_, viewer)| viewer.deadline <= Instant::now())
                .map(|(id, _)| id.clone())
                .collect();
            for id in expired {
                close_viewer(app, &id)?;
            }
            ensure_execution_safe(app)?;
            ensure!(
                !app.runtime_maintenance_active(),
                "Runtime maintenance owns the execution slot"
            );
            ensure!(
                app.trace_viewers.len() < 2,
                "close an active Trace viewer before opening another"
            );
            let id = format!("viewer-{}", crate::server::random_token()?);
            let (viewer, ready) = start_viewer(
                &id,
                capture::encode_base64(&bytes),
                app.resource_blocked.clone(),
            )?;
            app.trace_viewers.insert(id.clone(), viewer);
            match ready {
                Ok(mut ready) => {
                    ready["viewer_id"] = json!(id);
                    Ok(ready)
                }
                Err(error) => {
                    if close_viewer(app, &id).is_err() {
                        app.resource_blocked.store(true, Ordering::Release);
                    }
                    Err(error)
                }
            }
        }
        "stopTraceViewer" => {
            let p: Close = parameters(value)?;
            close_viewer(app, &p.viewer_id)?;
            Ok(json!({"ok":true}))
        }
        _ => anyhow::bail!("unknown process evidence operation"),
    }
}
fn close_viewer(app: &mut AppState, id: &str) -> Result<()> {
    if let Some(viewer) = app.trace_viewers.get_mut(id) {
        if let Err(error) = viewer.close() {
            app.resource_blocked.store(true, Ordering::Release);
            return Err(error);
        }
        app.trace_viewers.remove(id);
    }
    Ok(())
}
fn start_viewer(
    id: &str,
    trace_base64: String,
    blocked: Arc<AtomicBool>,
) -> Result<(Viewer, Result<Value>)> {
    // Journal before creating the engine: a forced service exit never loses
    // ownership evidence, even during startup before a URL was returned.
    let journal = Arc::new(owners());
    journal.begin(id)?;
    let (stop, mut stopped) = tokio::sync::oneshot::channel();
    let (ready_tx, ready) = mpsc::sync_channel(1);
    let (done_tx, done) = mpsc::sync_channel(1);
    let process = Arc::new(Mutex::new(None));
    let retained = process.clone();
    let failure_gate = blocked.clone();
    let owner_id = id.to_string();
    let worker_journal = journal.clone();
    let worker = std::thread::Builder::new().name("qinaegis-trace-viewer".into()).spawn(move || {
        let result = (|| -> Result<()> {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
            runtime.block_on(async move {
                let process = MidsceneProcess::spawn(None, None).await?;
                *retained.lock().map_err(|_| anyhow::anyhow!("viewer owner unavailable"))? = Some(process.clone());
                let opened = tokio::select! {
                    response = process.call(JsonRpcRequest::StartTraceViewer { trace_base64 }) => response,
                    _ = &mut stopped => Err(anyhow::anyhow!("Trace viewer startup stopped")),
                };
                let opened = opened.and_then(|response| {
                    ensure!(response.ok, "offline Trace viewer could not start; verify the installed runtime version");
                    let data = response.data.context("Trace viewer response missing")?;
                    let url = reqwest::Url::parse(data["url"].as_str().context("Trace viewer URL missing")?)?;
                    ensure!(url.scheme() == "http" && url.host_str() == Some("127.0.0.1") && url.port().is_some() && url.username().is_empty() && url.password().is_none() && data["expires_in_seconds"] == TTL && data["version"] == "1.59.1", "unexpected Trace viewer endpoint");
                    let prefix = url.path().strip_suffix("index.html").context("unexpected Trace viewer path")?;
                    let token = prefix.strip_prefix('/').and_then(|p| p.strip_suffix('/')).context("unexpected Trace viewer token")?;
                    ensure!(token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()) && url.fragment().is_none() && url.query_pairs().count() == 1 && url.query_pairs().any(|(k,v)| k == "trace" && v == format!("{}{}trace.zip", url.origin().ascii_serialization(), prefix)), "unexpected Trace viewer archive mapping");
                    Ok(data)
                });
                match opened {
                    Ok(data) => { if ready_tx.send(Ok(data)).is_ok() { tokio::select! { _ = stopped => {}, _ = tokio::time::sleep(Duration::from_secs(TTL)) => {} } } }
                    Err(_) => { let _ = ready_tx.send(Err(anyhow::anyhow!("Trace viewer startup failed or timed out"))); }
                }
                process.terminate().await
            })
        })();
        let result = result.and_then(|()| worker_journal.release(&owner_id));
        if result.is_err() { failure_gate.store(true, Ordering::Release); }
        let _ = done_tx.send(result.map_err(|_| "Trace viewer resource cleanup unconfirmed".to_string()));
    });
    if let Err(error) = worker {
        if journal.release(id).is_err() {
            blocked.store(true, Ordering::Release);
        }
        return Err(error.into());
    }
    let viewer = Viewer {
        id: id.into(),
        stop: Some(stop),
        done,
        process,
        journal,
        blocked,
        deadline: Instant::now() + Duration::from_secs(TTL),
    };
    let data = ready
        .recv_timeout(Duration::from_secs(20))
        .context("Trace viewer startup timed out")
        .and_then(|result| result);
    Ok((viewer, data))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TestDirectory(std::path::PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "qinaegis-viewer-{}",
                crate::server::random_token().unwrap()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn viewer(
        directory: &std::path::Path,
    ) -> (Viewer, mpsc::SyncSender<std::result::Result<(), String>>) {
        let journal = Arc::new(capture::ViewerOwnerStore::new(directory.into()));
        journal.begin("viewer-fixture").unwrap();
        let (stop, _receiver) = tokio::sync::oneshot::channel();
        let (sender, done) = mpsc::sync_channel(1);
        (
            Viewer {
                id: "viewer-fixture".into(),
                stop: Some(stop),
                done,
                process: Arc::new(Mutex::new(None)),
                journal,
                blocked: Arc::new(AtomicBool::new(false)),
                deadline: Instant::now(),
            },
            sender,
        )
    }
    #[test]
    fn unconfirmed_viewer_cleanup_never_discards_its_durable_owner() {
        for outcome in [None, Some(Err("untrusted child cleanup body".into()))] {
            let directory = TestDirectory::new();
            let (mut viewer, sender) = viewer(directory.path());
            if let Some(outcome) = outcome {
                sender.send(outcome).unwrap();
            }
            assert!(viewer.close_with_timeout(Duration::from_millis(1)).is_err());
            assert!(viewer.blocked.load(Ordering::Acquire));
            assert!(viewer.journal.owners().unwrap().contains("viewer-fixture"));
            drop(viewer);
            assert!(capture::ViewerOwnerStore::new(directory.path().into())
                .owners()
                .unwrap()
                .contains("viewer-fixture"));
        }
    }
    #[test]
    fn confirmed_viewer_cleanup_releases_only_its_journaled_owner() {
        let directory = TestDirectory::new();
        let (mut viewer, sender) = viewer(directory.path());
        viewer.journal.begin("viewer-other").unwrap();
        sender.send(Ok(())).unwrap();
        viewer.close_with_timeout(Duration::from_millis(1)).unwrap();
        assert_eq!(
            viewer
                .journal
                .owners()
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["viewer-other"]
        );
        assert!(!viewer.blocked.load(Ordering::Acquire));
    }
    #[test]
    fn confirmed_process_exit_does_not_release_safety_if_owner_journal_cannot_commit() {
        let directory = TestDirectory::new();
        let (mut viewer, sender) = viewer(directory.path());
        let journal = directory.path().join("capture/viewer-owners.json");
        std::fs::remove_file(&journal).unwrap();
        std::fs::create_dir(&journal).unwrap();
        sender.send(Ok(())).unwrap();
        assert!(viewer.close_with_timeout(Duration::from_millis(1)).is_err());
        assert!(viewer.blocked.load(Ordering::Acquire));
        assert!(journal.is_dir());
    }
}
