use qin_aegis_core::{
    protocol::{JsonRpcRequest, MidsceneProcess},
    resolve_env_var,
    storage::{CaseStatus, Storage},
    AppConfig, ArcLlmClient, Explorer, LighthouseResult, LlmClient, LlmConfig, LlmRouter,
    LocalStorage, LocalStorageInstance, LocustResult, Message, MiniMaxClient, ProviderConfig,
    SandboxConfig, TestCaseRef, TestCaseService, TestExecutor,
};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

const LOG_LIMIT: usize = 64 * 1024;
#[derive(Clone, Default)]
pub struct JobOutput {
    text: String,
    start: u64,
    secrets: Vec<String>,
    pending: String,
}
impl JobOutput {
    fn configure(&mut self, secrets: &[String]) {
        self.secrets = secrets.iter().filter(|s| !s.is_empty()).cloned().collect();
        self.secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
        // Production configures before any worker writes. Conservatively mask
        // preexisting text because a previously truncated secret is unrecoverable.
        if !self.secrets.is_empty() && !self.text.is_empty() {
            self.text = "*".repeat(self.text.len());
        }
    }
    fn push_str(&mut self, value: &str) {
        let mut incoming = std::mem::take(&mut self.pending);
        incoming.push_str(value);
        let mut offset = 0;
        while offset < incoming.len() {
            let rest = &incoming[offset..];
            if self
                .secrets
                .iter()
                .any(|s| s.len() > rest.len() && s.starts_with(rest))
            {
                self.pending = rest.into();
                break;
            } else if let Some(secret) = self.secrets.iter().find(|s| rest.starts_with(s.as_str()))
            {
                self.text.push_str(&"*".repeat(secret.len()));
                offset += secret.len();
            } else {
                let ch = rest.chars().next().unwrap();
                self.text.push(ch);
                offset += ch.len_utf8();
            }
        }
        self.trim();
    }
    fn finish(&mut self) {
        self.text.push_str(&"*".repeat(self.pending.len()));
        self.pending.clear();
        self.trim();
    }
    fn trim(&mut self) {
        if self.text.len() > LOG_LIMIT {
            let mut remove = self.text.len() - LOG_LIMIT;
            while !self.text.is_char_boundary(remove) {
                remove += 1;
            }
            self.text.drain(..remove);
            self.start += remove as u64;
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
#[derive(Clone)]
pub(crate) struct History {
    store: Arc<qin_aegis_core::jobs::JobStore>,
    serial: Arc<Mutex<()>>,
    error: Arc<Mutex<Option<String>>>,
    last: Arc<Mutex<Option<(String, serde_json::Value)>>>,
}
impl History {
    fn write(&self, id: &str, entry: &JobEntry, terminal: bool) {
        let mut record = entry.record(id);
        if !terminal {
            record.status = if entry.cancel_flag.load(Ordering::Acquire) {
                "cancelling"
            } else {
                "running"
            }
            .into();
        }
        let mut signature = serde_json::to_value(&record).expect("job record serializes");
        signature["updated_at"] = serde_json::json!(0);
        if !terminal
            && self
                .last
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|(previous_id, previous)| previous_id == id && previous == &signature)
        {
            return;
        }
        match self.store.checkpoint(record) {
            Ok(()) => *self.last.lock().unwrap() = Some((id.into(), signature)),
            Err(error) => {
                *self.error.lock().unwrap() =
                    Some(format!("Task history persistence failed: {error}"))
            }
        }
    }
}

// ============================================================================
// JobEntry — tracks async job state for cancel/status control
// ============================================================================

#[derive(Clone)]
pub struct JobEntry {
    pub job_type: String,
    pub status: String, // idle/running/completed/failed/stopped
    pub output: Arc<Mutex<JobOutput>>,
    pub created_at: u64,
    secrets: Vec<String>,
    pub(crate) history: Option<History>,
    pub cancel_flag: Arc<AtomicBool>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub snapshot: Option<serde_json::Value>,
    // Published with the active worker count after final output and history cleanup.
    worker_done: bool,
    // done_tx intentionally omitted - not needed after thread spawn
}

impl JobEntry {
    pub fn new(job_type: &str) -> Self {
        Self {
            job_type: job_type.to_string(),
            status: "running".to_string(),
            output: Arc::new(Mutex::new(JobOutput::default())),
            created_at: now(),
            secrets: Vec::new(),
            history: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            result: None,
            error: None,
            snapshot: None,
            worker_done: false,
        }
    }
    fn visible_status(&self) -> &str {
        if !self.worker_done && !matches!(self.status.as_str(), "queued" | "running" | "cancelling")
        {
            "running"
        } else {
            &self.status
        }
    }
    fn redact(&self, mut text: String) -> String {
        for secret in &self.secrets {
            if !secret.is_empty() {
                text = text.replace(secret, "[REDACTED]");
            }
        }
        text
    }
    fn record(&self, id: &str) -> qin_aegis_core::jobs::JobRecord {
        let output = self.output.lock().unwrap().clone();
        let clean = |value: &Option<serde_json::Value>| {
            value.as_ref().map(|v| {
                fn walk(v: &mut serde_json::Value, entry: &JobEntry) {
                    match v {
                        serde_json::Value::String(s) => *s = entry.redact(s.clone()),
                        serde_json::Value::Array(a) => {
                            for v in a {
                                walk(v, entry);
                            }
                        }
                        serde_json::Value::Object(o) => {
                            for (_, v) in o {
                                walk(v, entry);
                            }
                        }
                        _ => {}
                    }
                }
                let mut v = v.clone();
                walk(&mut v, self);
                v
            })
        };
        qin_aegis_core::jobs::JobRecord {
            id: id.into(),
            job_type: self.job_type.clone(),
            status: self.status.clone(),
            output: output.text,
            output_start: output.start,
            result: clean(&self.result),
            error: self.error.as_ref().map(|e| self.redact(e.clone())),
            snapshot: clean(&self.snapshot),
            created_at: self.created_at,
            updated_at: now().max(self.created_at),
        }
    }
}

/// Tracks actual worker lifetime independently of the user-visible status.
struct WorkerGuard {
    active: Arc<AtomicUsize>,
    jobs: Arc<Mutex<HashMap<String, JobEntry>>>,
    job_id: String,
    done: Arc<AtomicBool>,
}

impl WorkerGuard {
    fn new(
        active: Arc<AtomicUsize>,
        jobs: Arc<Mutex<HashMap<String, JobEntry>>>,
        job_id: String,
    ) -> Self {
        active.fetch_add(1, Ordering::AcqRel);
        let done = Arc::new(AtomicBool::new(false));
        Self {
            active,
            jobs,
            job_id,
            done,
        }
    }
    fn start(&self) {
        let jobs = &self.jobs;
        let job_id = self.job_id.clone();
        let done = &self.done;
        let history = jobs
            .lock()
            .unwrap()
            .get(&job_id)
            .and_then(|e| e.history.clone());
        if let Some(history) = history {
            let finished = done.clone();
            let entries = jobs.clone();
            let id = job_id.clone();
            thread::spawn(move || {
                while !finished.load(Ordering::Acquire) {
                    thread::sleep(std::time::Duration::from_millis(250));
                    let _serial = history.serial.lock().unwrap();
                    if finished.load(Ordering::Acquire) {
                        break;
                    }
                    let entry = entries.lock().unwrap().get(&id).cloned();
                    if let Some(entry) = entry {
                        history.write(&id, &entry, false);
                    }
                }
            });
        }
    }
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Release);
        if self
            .jobs
            .lock()
            .unwrap()
            .get(&self.job_id)
            .is_some_and(|e| e.worker_done)
        {
            self.active.fetch_sub(1, Ordering::AcqRel);
            return;
        }
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(entry) = jobs.get_mut(&self.job_id) {
                if entry.cancel_flag.load(Ordering::Acquire)
                    && matches!(entry.status.as_str(), "running" | "cancelling")
                {
                    // This is the only point that confirms the worker actually exited.
                    entry.status = "stopped".into();
                } else if matches!(entry.status.as_str(), "running" | "cancelling") {
                    entry.status = "failed".into();
                    entry
                        .error
                        .get_or_insert_with(|| "worker exited without a result".into());
                }
            }
        }
        let entry = self.jobs.lock().unwrap().get(&self.job_id).cloned();
        if let Some(entry) = entry {
            entry.output.lock().unwrap().finish();
            if let Some(history) = &entry.history {
                let _serial = history.serial.lock().unwrap();
                history.write(&self.job_id, &entry, true);
            }
        }
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(entry) = jobs.get_mut(&self.job_id) {
            entry.worker_done = true;
        }
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Create an LlmRouter from config.
/// Falls back to a single-provider ArcLlmClient if no secondary is configured.
fn create_llm_client(cfg: &AppConfig) -> ArcLlmClient {
    let primary = ProviderConfig {
        provider: cfg.llm.provider.clone(),
        base_url: resolve_env_var(&cfg.llm.base_url),
        api_key: resolve_env_var(&cfg.llm.api_key),
        model: cfg.llm.model.clone(),
    };
    let secondary = if !cfg.llm.secondary_api_key.is_empty() && !cfg.llm.secondary_model.is_empty()
    {
        Some(ProviderConfig {
            provider: cfg.llm.secondary_provider.clone(),
            base_url: resolve_env_var(&cfg.llm.secondary_base_url),
            api_key: resolve_env_var(&cfg.llm.secondary_api_key),
            model: cfg.llm.secondary_model.clone(),
        })
    } else {
        None
    };

    let primary_clone = primary.clone();
    match LlmRouter::from_configs(primary, secondary) {
        Ok(router) => ArcLlmClient::new(router),
        Err(_) => ArcLlmClient::new(MiniMaxClient::new(
            primary_clone.base_url,
            primary_clone.api_key,
            primary_clone.model,
        )),
    }
}

// ============================================================================
// AppState — holds UI state + output buffer
// ============================================================================

pub const APP_VERSION: &str = env!("APP_BUILD_VERSION");

type PendingWork = (String, Box<dyn FnOnce() + Send>);
#[derive(Default)]
pub(crate) struct TaskQueue {
    running: bool,
    pending: VecDeque<PendingWork>,
}

pub struct AppState {
    pub config: Option<AppConfig>,
    pub output: Arc<Mutex<String>>, // legacy shared output
    pub current_view: String,
    pub next_job_id: std::sync::atomic::AtomicU64,
    pub jobs: Arc<Mutex<HashMap<String, JobEntry>>>, // jobId -> JobEntry
    pub active_workers: Arc<AtomicUsize>,
    pub(crate) queue: Arc<Mutex<TaskQueue>>,
    pub(crate) history: Option<History>,
    pub(crate) session_id: String,
}

impl Default for AppState {
    fn default() -> Self {
        let config_result: Result<AppConfig, ()> = Err(());
        Self {
            config: config_result.ok(),
            output: Arc::new(Mutex::new(String::new())),
            current_view: "dashboard".to_string(),
            next_job_id: std::sync::atomic::AtomicU64::new(1),
            jobs: Arc::new(Mutex::new(HashMap::new())),
            active_workers: Arc::new(AtomicUsize::new(0)),
            queue: Arc::new(Mutex::new(TaskQueue::default())),
            history: None,
            session_id: "test".into(),
        }
    }
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let store = Arc::new(qin_aegis_core::jobs::JobStore::new(
            LocalStorage::base_path().join("jobs/history.json"),
        ));
        let records = store.load_and_recover()?;
        let mut app = Self::default();
        app.config = AppConfig::load_global().ok();
        app.session_id = crate::server::random_token()?;
        let history = History {
            store,
            serial: Arc::new(Mutex::new(())),
            error: Arc::new(Mutex::new(None)),
            last: Arc::new(Mutex::new(None)),
        };
        for r in records {
            let mut e = JobEntry::new(&r.job_type);
            e.status = r.status;
            e.output = Arc::new(Mutex::new(JobOutput {
                text: r.output,
                start: r.output_start,
                ..JobOutput::default()
            }));
            e.created_at = r.created_at;
            e.result = r.result;
            e.error = r.error;
            e.snapshot = r.snapshot;
            e.worker_done = true;
            app.jobs.lock().unwrap().insert(r.id, e);
        }
        app.history = Some(history);
        Ok(app)
    }

    fn register_job(&self, id: &str, mut entry: JobEntry) -> anyhow::Result<()> {
        if self.active_workers.load(Ordering::Acquire) >= 9 {
            anyhow::bail!("Task queue full: one execution slot and eight waiting tasks");
        }
        if self.active_workers.load(Ordering::Acquire) != 0 {
            entry.status = "queued".into();
        }
        if let Some(cfg) = &self.config {
            entry.secrets.extend([
                resolve_env_var(&cfg.llm.api_key),
                resolve_env_var(&cfg.llm.secondary_api_key),
            ]);
        }
        entry.output.lock().unwrap().configure(&entry.secrets);
        entry.history = self.history.clone();
        if let Some(history) = &self.history {
            let _serial = history.serial.lock().unwrap();
            history.store.checkpoint(entry.record(id))?;
        }
        let mut jobs = self.jobs.lock().unwrap();
        while jobs.len() >= 100 {
            let remove = jobs
                .iter()
                .filter(|(_, e)| e.worker_done)
                .min_by_key(|(_, e)| e.created_at)
                .map(|(id, _)| id.clone());
            if let Some(id) = remove {
                jobs.remove(&id);
            } else {
                anyhow::bail!("Task history full of active work");
            }
        }
        jobs.insert(id.into(), entry);
        Ok(())
    }
    pub fn get_jobs(&self) -> String {
        let positions: HashMap<String, usize> = self
            .queue
            .lock()
            .unwrap()
            .pending
            .iter()
            .enumerate()
            .map(|(i, (id, _))| (id.clone(), i + 1))
            .collect();
        let mut jobs = self.jobs.lock().unwrap().iter().map(|(id,e)|serde_json::json!({"jobId":id,"jobType":e.job_type,"status":e.visible_status(),"queue_position":positions.get(id),"wait_reason":if e.status == "queued" { Some("Waiting for the single execution slot and owned resource cleanup") } else { None },"created_at":e.created_at,"error":e.error.as_ref().map(|v|e.redact(v.clone()))})).collect::<Vec<_>>();
        jobs.sort_by(|a, b| {
            b["created_at"]
                .as_u64()
                .cmp(&a["created_at"].as_u64())
                .then_with(|| b["jobId"].as_str().cmp(&a["jobId"].as_str()))
        });
        let error = self
            .history
            .as_ref()
            .and_then(|h| h.error.lock().unwrap().clone());
        serde_json::json!({"jobs":jobs,"history_error":error}).to_string()
    }
    pub fn get_job_log(&self, id: &str, cursor: u64) -> String {
        let entry = self.jobs.lock().unwrap().get(id).cloned();
        let Some(entry) = entry else {
            return serde_json::json!({"error":"job not found"}).to_string();
        };
        let output = entry.output.lock().unwrap();
        let end = output.start + output.text.len() as u64;
        let mut offset = cursor
            .saturating_sub(output.start)
            .min(output.text.len() as u64) as usize;
        while !output.text.is_char_boundary(offset) {
            offset += 1;
        }
        serde_json::json!({"jobId":id,"output":output.text[offset..],"cursor":end,"truncated":cursor<output.start||cursor>end}).to_string()
    }

    fn job_id(&self) -> String {
        format!(
            "job-{}-{}",
            self.session_id,
            self.next_job_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        )
    }

    fn append_output(&self, msg: &str) {
        let mut o = self.output.lock().unwrap();
        o.push_str(msg);
        if o.len() > LOG_LIMIT {
            let mut start = o.len() - LOG_LIMIT;
            while !o.is_char_boundary(start) {
                start += 1;
            }
            o.drain(..start);
        }
    }

    pub fn get_job_status(&self, job_id: &str) -> String {
        let position = self
            .queue
            .lock()
            .unwrap()
            .pending
            .iter()
            .position(|(id, _)| id == job_id)
            .map(|i| i + 1);
        let entry = self.jobs.lock().unwrap().get(job_id).cloned();
        match entry {
            Some(entry) => {
                let output = entry.output.lock().unwrap().text.clone();
                serde_json::to_string(&serde_json::json!({
                    "jobId": job_id,
                    "jobType": entry.job_type,
                    "status": entry.visible_status(),
                    "queue_position": position,
                    "wait_reason": if entry.status == "queued" { Some("Waiting for the single execution slot and owned resource cleanup") } else { None },
                    "output": output,
                    "result": entry.record(job_id).result,
                    "error": entry.record(job_id).error,
                    "snapshot": entry.record(job_id).snapshot,
                    "history_error":self.history.as_ref().and_then(|h|h.error.lock().unwrap().clone()),
                }))
                .unwrap_or_default()
            }
            None => serde_json::to_string(&serde_json::json!({
                "error": "job not found"
            }))
            .unwrap_or_default(),
        }
    }

    pub fn stop_job(&mut self, job_id: &str) -> String {
        let mut queue = self.queue.lock().unwrap();
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(mut stopped) = jobs.get(job_id).filter(|e| e.status == "queued").cloned() {
            drop(jobs);
            stopped.status = "stopped".into();
            if let Some(history) = &stopped.history {
                let _serial = history.serial.lock().unwrap();
                if let Err(error) = history.store.checkpoint(stopped.record(job_id)) {
                    let message = format!("Queued cancellation not saved: {error}");
                    *history.error.lock().unwrap() = Some(message.clone());
                    // Never execute a task after a failed cancellation request.
                    // Keep its lease until dequeue/drop can checkpoint cleanup.
                    if let Some(entry) = self.jobs.lock().unwrap().get_mut(job_id) {
                        entry.cancel_flag.store(true, Ordering::Release);
                        entry.status = "cancelling".into();
                        entry.error = Some(message.clone());
                    }
                    return serde_json::json!({"error":message}).to_string();
                }
            }
            stopped.worker_done = true;
            stopped.cancel_flag.store(true, Ordering::Release);
            self.jobs.lock().unwrap().insert(job_id.into(), stopped);
            let removed = queue
                .pending
                .iter()
                .position(|(id, _)| id == job_id)
                .and_then(|i| queue.pending.remove(i));
            drop(queue);
            drop(removed);
            return serde_json::json!({"ok":true,"jobId":job_id}).to_string();
        }
        drop(queue);
        match jobs.get_mut(job_id) {
            Some(entry) if matches!(entry.status.as_str(), "running" | "cancelling") => {
                entry.cancel_flag.store(true, Ordering::Release);
                entry.status = "cancelling".to_string();
                serde_json::to_string(&serde_json::json!({
                    "ok": true,
                    "jobId": job_id
                }))
                .unwrap_or_default()
            }
            Some(_) => serde_json::to_string(&serde_json::json!({
                "ok": false,
                "error": "job is not running"
            }))
            .unwrap_or_default(),
            None => serde_json::to_string(&serde_json::json!({
                "ok": false,
                "error": "job not found"
            }))
            .unwrap_or_default(),
        }
    }

    fn select_model(
        &self,
        task: &str,
        project: Option<&str>,
    ) -> anyhow::Result<(
        AppConfig,
        LlmConfig,
        Option<qin_aegis_core::models::ResolvedModelRoute>,
    )> {
        let mut cfg = self.config.clone().unwrap_or_default();
        let registry = crate::model_api::registry()?;
        let snapshot = registry.snapshot()?;
        if !snapshot.profiles.is_empty()
            || !snapshot.routes.is_empty()
            || !snapshot.project_routes.is_empty()
        {
            let route = registry.resolve_for_project(project, task, &[])?;
            let selected = route
                .candidates()
                .first()
                .ok_or_else(|| anyhow::anyhow!("empty model route"))?;
            cfg.llm.api_key = selected.api_key.clone();
            cfg.llm.base_url = selected.base_url.clone();
            cfg.llm.model = selected.model.clone();
            cfg.llm.provider = selected.adapter.clone();
            let visual = LlmConfig {
                api_key: selected.api_key.clone(),
                base_url: selected.base_url.clone(),
                model: selected.model.clone(),
                midscene_family: selected.midscene_family.clone(),
            };
            return Ok((cfg, visual, Some(route)));
        }
        anyhow::ensure!(
            !resolve_env_var(&cfg.llm.api_key).trim().is_empty(),
            "configure a model and task route first"
        );
        let visual = LlmConfig {
            api_key: resolve_env_var(&cfg.llm.api_key),
            base_url: resolve_env_var(&cfg.llm.base_url),
            model: cfg.llm.model.clone(),
            midscene_family: None,
        };
        Ok((cfg, visual, None))
    }

    fn model_snapshot(
        route: &Option<qin_aegis_core::models::ResolvedModelRoute>,
        cfg: &AppConfig,
    ) -> serde_json::Value {
        match route {
            Some(route) => {
                serde_json::json!({"revision":route.revision(), "task":route.task(), "candidates":route.candidates().iter().map(|p| serde_json::json!({"id":p.id,"adapter":p.adapter,"model":p.model,"capabilities":p.capabilities,"midscene_family":p.midscene_family})).collect::<Vec<_>>() })
            }
            None => serde_json::json!({"source":"legacy_config", "model":cfg.llm.model}),
        }
    }

    pub fn is_configured(&self) -> bool {
        if let Ok(registry) = crate::model_api::registry() {
            if let Ok(snapshot) = registry.snapshot() {
                if !snapshot.profiles.is_empty() {
                    return ["generate", "explore", "execute"]
                        .iter()
                        .any(|task| registry.resolve(task, &[]).is_ok());
                }
            }
        }
        self.config
            .as_ref()
            .map(|c| !resolve_env_var(&c.llm.api_key).trim().is_empty())
            .unwrap_or(false)
    }

    /// Secrets are write-only over the browser boundary.
    pub fn get_config(&self) -> String {
        match &self.config {
            Some(cfg) => {
                let mut value = serde_json::to_value(cfg).expect("config serializes");
                value["llm"]["api_key"] = serde_json::json!("");
                value["llm"]["secondary_api_key"] = serde_json::json!("");
                value["llm"]["api_key_configured"] = serde_json::json!(!cfg.llm.api_key.is_empty());
                value["llm"]["secondary_api_key_configured"] =
                    serde_json::json!(!cfg.llm.secondary_api_key.is_empty());
                value.to_string()
            }
            None => "null".to_owned(),
        }
    }

    pub fn get_version(&self) -> String {
        format!(r#"{{"version":"{}"}}"#, APP_VERSION)
    }

    fn version_compare(a: &str, b: &str) -> i32 {
        let parse = |s: &str| {
            s.split('.')
                .filter_map(|p| p.parse::<u64>().ok())
                .collect::<Vec<u64>>()
        };
        let a_parts = parse(a);
        let b_parts = parse(b);
        let max_len = a_parts.len().max(b_parts.len());
        for i in 0..max_len {
            let a_v = *a_parts.get(i).unwrap_or(&0);
            let b_v = *b_parts.get(i).unwrap_or(&0);
            if a_v < b_v {
                return -1;
            }
            if a_v > b_v {
                return 1;
            }
        }
        0
    }

    pub fn check_update(&self) -> String {
        let repo = "mbpz/qinAegis";
        let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
        match ureq::get(&url)
            .set("Accept", "application/vnd.github+json")
            .set("User-Agent", "QinAegis")
            .timeout(std::time::Duration::from_secs(10))
            .call()
        {
            Ok(resp) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(
                    resp.into_string().as_deref().unwrap_or("{}"),
                ) {
                    let latest = json
                        .get("tag_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(APP_VERSION);
                    let latest_ver = latest.trim_start_matches('v');
                    let current_ver = APP_VERSION;
                    let up_to_date = Self::version_compare(current_ver, latest_ver) >= 0;
                    return serde_json::to_string(&serde_json::json!({
                        "current": current_ver,
                        "latest": latest_ver,
                        "upToDate": up_to_date,
                    }))
                    .unwrap_or_default();
                }
            }
            Err(e) => {
                tracing::warn!("update check failed: {}", e);
            }
        }
        serde_json::json!({"ok":false,"error":"update check unavailable"}).to_string()
    }

    pub fn set_config(&mut self, raw: &str) -> String {
        let value: serde_json::Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => return serde_json::json!({"ok":false,"error":"invalid JSON"}).to_string(),
        };
        let cfg = match patched_config(self.config.as_ref(), &value) {
            Ok(cfg) => cfg,
            Err(error) => return serde_json::json!({"ok":false,"error":error}).to_string(),
        };
        let Some(expected) = value
            .get("expected_revision")
            .and_then(serde_json::Value::as_u64)
        else {
            return serde_json::json!({"ok":false,"error":"expected_revision is required; reload settings"}).to_string();
        };
        match cfg.save_to_checked(&AppConfig::global_path(), expected) {
            Ok(revision) => {
                self.config = Some(cfg);
                serde_json::json!({"ok":true,"revision":revision}).to_string()
            }
            Err(error) => serde_json::json!({"ok":false,"error":error.to_string()}).to_string(),
        }
    }

    fn job_response(&self, id: &str) -> String {
        let state: serde_json::Value =
            serde_json::from_str(&self.get_job_status(id)).expect("job status serializes");
        serde_json::json!({"ok":true,"jobId":id,"status":state["status"],"queue_position":state["queue_position"],"wait_reason":state["wait_reason"]}).to_string()
    }

    fn schedule(&self, id: String, work: impl FnOnce() + Send + 'static) {
        let mut queue = self.queue.lock().unwrap();
        queue.pending.push_back((id, Box::new(work)));
        if queue.running {
            return;
        }
        queue.running = true;
        let state = self.queue.clone();
        let jobs = self.jobs.clone();
        thread::spawn(move || loop {
            let (item, entry) = {
                let mut queue = state.lock().unwrap();
                let item = match queue.pending.pop_front() {
                    Some(item) => item,
                    None => {
                        queue.running = false;
                        return;
                    }
                };
                let entry = {
                    let mut entries = jobs.lock().unwrap();
                    let e = entries.get_mut(&item.0).expect("queued entry retained");
                    if !e.cancel_flag.load(Ordering::Acquire) {
                        e.status = "running".into();
                    }
                    e.clone()
                };
                (item, entry)
            };
            let mut cancelled = entry.cancel_flag.load(Ordering::Acquire);
            if !cancelled {
                if let Some(history) = &entry.history {
                    let _serial = history.serial.lock().unwrap();
                    history.write(&item.0, &entry, false);
                    if history.error.lock().unwrap().is_some() {
                        let mut entries = jobs.lock().unwrap();
                        let e = entries.get_mut(&item.0).unwrap();
                        e.status = "failed".into();
                        e.error =
                            Some("Task not started because history persistence failed".into());
                        cancelled = true;
                    }
                }
            }
            if cancelled {
                drop(item);
                continue;
            }
            // A panic drops the captured guard and cannot strand subsequent work.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(item.1));
        });
    }

    pub fn start_maintenance<F>(&mut self, job_type: &str, work: F) -> String
    where
        F: FnOnce(Arc<AtomicBool>) -> anyhow::Result<serde_json::Value> + Send + 'static,
    {
        let id = self.job_id();
        let entry = JobEntry::new(job_type);
        let cancel = entry.cancel_flag.clone();
        if let Err(error) = self.register_job(&id, entry) {
            return serde_json::json!({"error":error.to_string()}).to_string();
        }
        let jobs = self.jobs.clone();
        let guard = WorkerGuard::new(self.active_workers.clone(), jobs.clone(), id.clone());
        let job_id = id.clone();
        self.schedule(id.clone(), move || {
            let _guard = guard;
            _guard.start();
            let outcome = work(cancel.clone());
            if let Ok(mut entries) = jobs.lock() {
                if let Some(entry) = entries.get_mut(&job_id) {
                    if cancel.load(Ordering::Acquire) {
                        entry.status = "stopped".into();
                        entry.error = Some("Task cancelled".into());
                    } else {
                        match outcome {
                            Ok(result) => {
                                entry.status = "completed".into();
                                entry.result = Some(result);
                            }
                            Err(error) => {
                                entry.status = "failed".into();
                                entry.error = Some(error.to_string());
                            }
                        }
                    }
                }
            }
        });
        self.job_response(&id)
    }

    // ── Explore ──────────────────────────────────────────────────────────────

    pub fn run_explore(&mut self, url: &str, depth: u32) -> String {
        let (cfg, frozen_visual, model_route) = match self.select_model("explore", None) {
            Ok(selection) => selection,
            Err(e) => return serde_json::json!({"error":e.to_string()}).to_string(),
        };
        let model_snapshot = Self::model_snapshot(&model_route, &cfg);
        let job_id = self.job_id();
        let url = url.to_string();
        let jobs = self.jobs.clone();
        let job_id_for_return = job_id.clone();

        // Create JobEntry BEFORE spawning thread
        let mut entry = JobEntry::new("explore");
        entry.snapshot = Some(serde_json::json!({"model":model_snapshot}));
        let cancel_flag = entry.cancel_flag.clone();
        let output_arc = entry.output.clone();

        if let Some(route) = &model_route {
            entry
                .secrets
                .extend(route.candidates().iter().map(|p| p.api_key.clone()));
        }
        if let Err(error) = self.register_job(&job_id, entry) {
            return serde_json::json!({"error":error.to_string()}).to_string();
        }

        self.append_output(&format!(
            "[{}] explore dispatched: {} depth={}\n",
            job_id, url, depth
        ));

        let worker = WorkerGuard::new(self.active_workers.clone(), jobs.clone(), job_id.clone());
        self.schedule(job_id.clone(), move || {
            let _worker = worker;
            _worker.start();
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime for explore");
            rt.block_on(async {
                // Check cancel flag before starting
                if cancel_flag.load(Ordering::Relaxed) {
                    Self::set_job_status_internal(&jobs, &job_id, "stopped");
                    return;
                }

                let llm_cfg = frozen_visual.clone();
                let sandbox_cfg = SandboxConfig {
                    cdp_port: cfg.sandbox.cdp_port,
                };

                {
                    let mut o = output_arc.lock().unwrap();
                    o.push_str(&format!(
                        "[{}] Starting explore: {} depth={}\n",
                        job_id, url, depth
                    ));
                }

                // Check cancel flag before Explorer::new
                if cancel_flag.load(Ordering::Relaxed) {
                    Self::set_job_status_internal(&jobs, &job_id, "stopped");
                    return;
                }

                match Explorer::new_cancellable(
                    Some(llm_cfg),
                    Some(sandbox_cfg),
                    None,
                    cancel_flag.clone(),
                )
                .await
                {
                    Ok(mut explorer) => {
                        // Check cancel flag before explore()
                        if cancel_flag.load(Ordering::Relaxed) {
                            let _ = explorer.shutdown().await;
                            Self::set_job_status_internal(&jobs, &job_id, "stopped");
                            return;
                        }

                        match explorer.explore(&url, depth).await {
                            Ok(result) => {
                                let mut o = output_arc.lock().unwrap();
                                o.push_str(&format!(
                                    "[{}] ✓ Explored {} pages\n",
                                    job_id,
                                    result.pages.len()
                                ));
                                o.push_str(&format!(
                                    "[{}] Output length: {} chars\n",
                                    job_id,
                                    result.markdown.len()
                                ));
                                o.push_str("\n======== EXPLORE RESULT ========\n");
                                o.push_str(&result.markdown);
                                o.push_str("\n================================\n");
                                Self::set_job_status_internal(&jobs, &job_id, "completed");
                            }
                            Err(e) => {
                                let mut o = output_arc.lock().unwrap();
                                o.push_str(&format!("[{}] ✗ Explore failed: {}\n", job_id, e));
                                Self::set_job_status_internal(&jobs, &job_id, "failed");
                                Self::set_job_error_internal(&jobs, &job_id, &e.to_string());
                            }
                        }
                        let _ = explorer.shutdown().await;
                    }
                    Err(e) => {
                        let mut o = output_arc.lock().unwrap();
                        o.push_str(&format!("[{}] ✗ Failed to start explorer: {}\n", job_id, e));
                        Self::set_job_status_internal(&jobs, &job_id, "failed");
                        Self::set_job_error_internal(&jobs, &job_id, &e.to_string());
                    }
                }
            });
        });

        self.job_response(&job_id_for_return)
    }

    fn set_job_status_internal(
        jobs: &Arc<Mutex<HashMap<String, JobEntry>>>,
        job_id: &str,
        status: &str,
    ) {
        if let Ok(mut j) = jobs.lock() {
            if let Some(entry) = j.get_mut(job_id) {
                if !matches!(entry.status.as_str(), "running" | "cancelling") {
                    return;
                }
                if entry.cancel_flag.load(Ordering::Acquire) {
                    entry.status = "cancelling".into();
                } else if entry.status == "running" {
                    entry.status = status.to_string();
                }
            }
        }
    }

    fn set_job_error_internal(
        jobs: &Arc<Mutex<HashMap<String, JobEntry>>>,
        job_id: &str,
        error: &str,
    ) {
        if let Ok(mut j) = jobs.lock() {
            if let Some(entry) = j.get_mut(job_id) {
                entry.error = Some(error.to_string());
            }
        }
    }

    // ── Generate ─────────────────────────────────────────────────────────────

    pub fn run_generate(
        &mut self,
        requirement: &str,
        project: &str,
        contracts: Option<qin_aegis_core::contract_execution::ExecutionSnapshot>,
        specification: Option<qin_aegis_core::storage::local::ProjectSpec>,
    ) -> String {
        let (cfg, _, route) = match self.select_model("generate", Some(project)) {
            Ok(selection) => selection,
            Err(e) => return serde_json::json!({"error":e.to_string()}).to_string(),
        };
        let mut spec_markdown = match contracts.as_ref().map(|s| s.specification()).transpose() {
            Ok(value) => value.unwrap_or_default(),
            Err(e) => return serde_json::json!({"error":e.to_string()}).to_string(),
        };
        if let Some(spec) = &specification {
            spec_markdown = format!(
                "# Saved project specification\n{}\n\n# Published contract assertions\n{}",
                spec.markdown, spec_markdown
            );
        }
        let model_snapshot = Self::model_snapshot(&route, &cfg);
        let generation_context = qin_aegis_core::execution_records::GenerationContext {
            specification: specification.clone(),
            contracts: contracts.clone(),
            model: model_snapshot.clone(),
        };
        let job_id = self.job_id();
        let requirement = requirement.to_string();
        let project = project.to_string();
        let jobs = self.jobs.clone();
        let job_id_for_return = job_id.clone();

        // Create JobEntry BEFORE spawning thread
        let mut entry = JobEntry::new("generate");
        entry.snapshot = Some(
            serde_json::json!({"model":model_snapshot,"contracts":contracts,"specification":specification}),
        );
        let cancel_flag = entry.cancel_flag.clone();
        let output_arc = entry.output.clone();

        if let Some(route) = &route {
            entry
                .secrets
                .extend(route.candidates().iter().map(|p| p.api_key.clone()));
        }
        if let Err(error) = self.register_job(&job_id, entry) {
            return serde_json::json!({"error":error.to_string()}).to_string();
        }

        self.append_output(&format!("[{}] generate dispatched\n", job_id));

        let worker = WorkerGuard::new(self.active_workers.clone(), jobs.clone(), job_id.clone());
        self.schedule(job_id.clone(), move || {
            let _worker = worker;
            _worker.start();
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime for generate");
            rt.block_on(async {
                // Check cancel flag
                if cancel_flag.load(Ordering::Relaxed) {
                    Self::set_job_status_internal(&jobs, &job_id, "stopped");
                    return;
                }

                {
                    let mut o = output_arc.lock().unwrap();
                    o.push_str(&format!("[{}] Generating test cases...\n", job_id));
                    o.push_str(&format!("[{}] Requirement: {}\n", job_id, requirement));
                }

                let (llm, trace) = match route.as_ref() {
                    Some(route) => match qin_aegis_core::models::build_client_with_trace(route) {
                        Ok((client, trace)) => (client, Some(trace)),
                        Err(e) => {
                            Self::set_job_status_internal(&jobs, &job_id, "failed");
                            Self::set_job_error_internal(&jobs, &job_id, &e.to_string());
                            return;
                        }
                    },
                    None => (create_llm_client(&cfg), None),
                };

                let service = TestCaseService::new(llm, LocalStorageInstance);
                let service = if let Some(trace) = trace {
                    service.with_model_trace(trace)
                } else {
                    service
                };

                // Check cancel flag before generate
                if cancel_flag.load(Ordering::Relaxed) {
                    Self::set_job_status_internal(&jobs, &job_id, "stopped");
                    return;
                }

                let generated = tokio::select! {
                    result = service.generate_and_save_with_context(
                        &project, &spec_markdown, &requirement, Some(generation_context),
                    ) => result,
                    _ = crate::environment::cancelled(cancel_flag.clone()) => Err(anyhow::anyhow!("Generation cancelled")),
                };
                match generated {
                    Ok(cases) => {
                        let mut o = output_arc.lock().unwrap();
                        let saved: usize = cases.iter().filter(|c| c.saved).count();
                        o.push_str(&format!(
                            "[{}] ✓ Generated {} test cases ({} saved)\n",
                            job_id,
                            cases.len(),
                            saved
                        ));
                        Self::set_job_status_internal(&jobs, &job_id, "completed");
                    }
                    Err(e) => {
                        let mut o = output_arc.lock().unwrap();
                        o.push_str(&format!("[{}] ✗ Generation failed: {}\n", job_id, e));
                        Self::set_job_status_internal(&jobs, &job_id, "failed");
                        Self::set_job_error_internal(&jobs, &job_id, &e.to_string());
                    }
                }
            });
        });

        self.job_response(&job_id_for_return)
    }

    // ── Run Tests ────────────────────────────────────────────────────────────

    pub fn run_tests(&mut self, project: &str, test_type: &str) -> String {
        use qin_aegis_core::execution_records::{RunCase, RunRecord, RunStore};
        // Freeze approved inputs before admitting a worker. Never reconstruct
        // historical provenance from today's contract library or bindings.
        let prepared = (|| -> anyhow::Result<_> {
            let project_config = LocalStorage::load_project(project)?;
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let mut cases: Vec<_> = rt
                .block_on(LocalStorageInstance.load_cases_by_status(project, CaseStatus::Approved))?
                .into_iter()
                .filter(|case| case.test_type == test_type)
                .collect();
            cases.sort_by(|a, b| a.id.cmp(&b.id));
            anyhow::ensure!(!cases.is_empty(), "no approved cases for this test type");
            let mut frozen = Vec::new();
            for case in cases {
                frozen.push(RunCase {
                    provenance: qin_aegis_core::execution_records::case_provenance(project, &case)?,
                    case_id: case.id,
                    name: case.name,
                    yaml_script: case.yaml_script,
                    // A full YAML target may override this fallback, as in the
                    // existing engine protocol. This records the supplied URL.
                    target_url: project_config.url.clone(),
                });
            }
            Ok(frozen)
        })();
        let cases = match prepared {
            Ok(cases) => cases,
            Err(e) => return serde_json::json!({"error":e.to_string()}).to_string(),
        };
        let (cfg, frozen_visual, model_route) = match self.select_model("execute", Some(project)) {
            Ok(selection) => selection,
            Err(e) => return serde_json::json!({"error":e.to_string()}).to_string(),
        };
        let model_snapshot = Self::model_snapshot(&model_route, &cfg);
        let job_id = self.job_id();
        let run_id = format!(
            "run-{}-{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            std::process::id(),
            job_id
        );
        let run_dir = LocalStorage::run_dir(project, &run_id);
        let store = RunStore::new(run_dir.clone());
        let runtime = qin_aegis_core::runtime::RuntimeResolver::resolve();
        let record = match RunRecord::prepare(
            run_id.clone(),
            project.into(),
            test_type.into(),
            model_snapshot.clone(),
            runtime.as_ref().ok().map(|r| r.version.clone()),
            cases,
        ) {
            Ok(record) => record,
            Err(e) => return serde_json::json!({"error":e.to_string()}).to_string(),
        };
        let mut entry = JobEntry::new("testExec");
        entry.snapshot =
            Some(serde_json::json!({"model":model_snapshot,"run_id":run_id,"cases":record.cases}));
        let cancel_flag = entry.cancel_flag.clone();
        let output = entry.output.clone();
        let jobs = self.jobs.clone();
        if let Some(route) = &model_route {
            entry
                .secrets
                .extend(route.candidates().iter().map(|p| p.api_key.clone()));
        }
        if let Err(error) = self.register_job(&job_id, entry) {
            return serde_json::json!({"error":error.to_string()}).to_string();
        }
        let job_id_for_return = job_id.clone();
        let worker = WorkerGuard::new(self.active_workers.clone(), jobs.clone(), job_id.clone());
        let test_type = test_type.to_string();
        self.schedule(job_id.clone(), move || {
            let _worker = worker;
            _worker.start();
            if let Err(error) = store.begin(&record) {
                Self::set_job_error_internal(&jobs, &job_id, &format!("cannot save execution inputs: {error}"));
                return;
            }
            let mut results = Vec::new();
            let outcome = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|rt| {
                    rt.block_on(async {
                        if cancel_flag.load(Ordering::Acquire) {
                            return Ok(());
                        }
                        runtime?;
                        output.lock().unwrap().push_str(&format!(
                            "[{job_id}] Executing {} frozen approved cases\n",
                            record.cases.len()
                        ));
                        let executor = TestExecutor::new_cancellable(
                            3,
                            Some(frozen_visual.clone()),
                            Some(SandboxConfig {
                                cdp_port: cfg.sandbox.cdp_port,
                            }),
                            cancel_flag.clone(),
                        )
                        .await?;
                        let selected = record
                            .cases
                            .iter()
                            .map(|case| TestCaseRef {
                                id: case.case_id.clone(),
                                yaml_script: case.yaml_script.clone(),
                                name: case.name.clone(),
                                priority: String::new(),
                                target_url: Some(case.target_url.clone()),
                            })
                            .collect();
                        let executed = executor.run_parallel(selected).await;
                        let shutdown = executor.shutdown().await;
                        results = executed?;
                        shutdown?;
                        if !cancel_flag.load(Ordering::Acquire) {
                            Self::run_additional_checks(
                                &cfg,
                                &frozen_visual,
                                &record.cases[0].target_url,
                                &test_type,
                                &run_dir,
                                cancel_flag.clone(),
                            )
                            .await?;
                        }
                        Ok(())
                    })
                });
            // Engine messages are untrusted and may echo supplied credentials.
            for result in &mut results {
                result.error_message = result
                    .error_message
                    .take()
                    .map(|s| Self::redact_run_error(&s, &cfg));
                result.failed_step = result
                    .failed_step
                    .take()
                    .map(|s| Self::redact_run_error(&s, &cfg));
            }
            let error = outcome
                .err()
                .map(|e| Self::redact_run_error(&e.to_string(), &cfg));
            let status = if cancel_flag.load(Ordering::Acquire) {
                "stopped"
            } else if error.is_some() {
                "failed"
            } else {
                "completed"
            };
            // A successful engine response is insufficient: durable result
            // coverage must validate and commit before the job completes.
            let persisted = store.finish(status, results.clone(), error);
            let persisted = match persisted {
                Ok(record) => Ok(record),
                Err(e) => {
                    let message = format!("execution evidence could not be committed: {e}");
                    // Invalid result identities must not make it into history.
                    // A storage failure may also prevent this best-effort failure record.
                    let _ = store.finish("failed", vec![], Some(message.clone()));
                    Err(message)
                }
            };
            let (status, error) = match persisted {
                Ok(record) => {
                    // Reporting is a separate adapter: a rendering/export failure
                    // must not rewrite immutable business execution evidence.
                    let allure_error = qin_aegis_core::allure::export_run(
                        &record,
                        &run_dir.join("allure-results"),
                    )
                    .err()
                    .map(|e| Self::redact_run_error(&e.to_string(), &cfg));
                    if let Some(error) = &allure_error {
                        output
                            .lock()
                            .unwrap()
                            .push_str(&format!("[{job_id}] Allure results unavailable: {error}\n"));
                    }
                    let passed = record.results.iter().filter(|r| r.passed).count();
                    output.lock().unwrap().push_str(&format!(
                        "[{job_id}] Execution {}: {passed}/{} passed; evidence {run_id}\n",
                        record.status,
                        record.cases.len()
                    ));
                    if let Ok(mut entries) = jobs.lock() {
                        if let Some(entry) = entries.get_mut(&job_id) {
                            entry.result = Some(
                                serde_json::json!({"run_id":run_id,"status":record.status,"passed":passed,"total":record.cases.len(),
                                    "allure":{"results_ready":allure_error.is_none(),"error":allure_error}}),
                            );
                        }
                    }
                    (record.status, record.error)
                }
                Err(message) => ("failed".into(), Some(message)),
            };
            if let Some(message) = &error {
                output
                    .lock()
                    .unwrap()
                    .push_str(&format!("[{job_id}] {message}\n"));
            }
            // Commit the visible terminal state together; cancellation after a
            // durable terminal commit cannot rewrite that historical outcome.
            if let Ok(mut entries) = jobs.lock() {
                if let Some(entry) = entries.get_mut(&job_id) {
                    entry.status = status;
                    entry.error = error;
                }
            }
        });
        self.job_response(&job_id_for_return)
    }

    fn redact_run_error(message: &str, cfg: &AppConfig) -> String {
        let mut message = message.to_string();
        for secret in [&cfg.llm.api_key, &cfg.llm.secondary_api_key] {
            let secret = resolve_env_var(secret);
            if !secret.is_empty() {
                message = message.replace(&secret, "[redacted]");
            }
        }
        message
    }

    async fn run_additional_checks(
        cfg: &AppConfig,
        visual: &LlmConfig,
        target_url: &str,
        test_type: &str,
        run_dir: &std::path::Path,
        cancel: Arc<AtomicBool>,
    ) -> anyhow::Result<()> {
        let (request, filename) = match test_type {
            "performance" => (
                JsonRpcRequest::Lighthouse {
                    url: target_url.to_string(),
                },
                "lighthouse.json",
            ),
            "stress" => (
                JsonRpcRequest::Stress {
                    target_url: target_url.to_string(),
                    users: 10,
                    spawn_rate: 2,
                    duration: 30,
                },
                "locust-summary.json",
            ),
            _ => return Ok(()),
        };
        let process = MidsceneProcess::spawn(
            Some(visual.clone()),
            Some(SandboxConfig {
                cdp_port: cfg.sandbox.cdp_port,
            }),
        )
        .await?
        .with_cancellation(cancel);
        let result = process.call(request).await;
        let shutdown = process.call(JsonRpcRequest::Shutdown).await;
        let cleanup = process.terminate().await;
        let response = result?;
        let shutdown = shutdown?;
        cleanup?;
        anyhow::ensure!(
            shutdown.ok,
            "additional check shutdown failed: {}",
            shutdown.error.unwrap_or_default()
        );
        anyhow::ensure!(
            response.ok,
            "additional check failed: {}",
            response.error.unwrap_or_default()
        );
        let data = response
            .data
            .ok_or_else(|| anyhow::anyhow!("additional check returned no data"))?;
        std::fs::write(run_dir.join(filename), serde_json::to_vec(&data)?)?;
        Ok(())
    }

    pub fn get_case_provenance(&self, project: &str, case_id: &str) -> String {
        let result = (|| -> anyhow::Result<_> {
            let case = LocalStorage::load_cases(project)?
                .into_iter()
                .find(|c| c.id == case_id)
                .ok_or_else(|| anyhow::anyhow!("case not found"))?;
            qin_aegis_core::execution_records::case_provenance(project, &case)
        })();
        match result {
            Ok(record) => serde_json::to_string(&record).unwrap_or_default(),
            Err(e) => serde_json::json!({"error":e.to_string()}).to_string(),
        }
    }

    pub fn get_run_evidence(&self, project: &str, run_id: &str) -> String {
        let result = qin_aegis_core::execution_records::RunStore::new(LocalStorage::run_dir(
            project, run_id,
        ))
        .load();
        match result {
            Ok(record) if record.project == project && record.run_id == run_id => {
                serde_json::to_string(&record).unwrap_or_default()
            }
            Ok(_) => serde_json::json!({"error":"run evidence identity mismatch"}).to_string(),
            Err(e) => serde_json::json!({"error":e.to_string()}).to_string(),
        }
    }

    fn load_record(
        project: &str,
        run_id: &str,
    ) -> anyhow::Result<qin_aegis_core::execution_records::RunRecord> {
        let record = qin_aegis_core::execution_records::RunStore::new(LocalStorage::run_dir(
            project, run_id,
        ))
        .load()?;
        anyhow::ensure!(
            record.project == project && record.run_id == run_id,
            "run evidence identity mismatch"
        );
        anyhow::ensure!(record.status != "running", "Allure requires a terminal run");
        Ok(record)
    }

    pub(crate) fn allure_html(project: &str, run_id: &str) -> anyhow::Result<(String, String)> {
        Self::load_record(project, run_id)?;
        let root = LocalStorage::run_dir(project, run_id).canonicalize()?;
        let directory = root.join("allure-report");
        let metadata = std::fs::symlink_metadata(&directory)?;
        anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "unsafe Allure report directory"
        );
        let read = |name: &str, limit: u64| -> anyhow::Result<String> {
            let path = directory.join(name);
            let metadata = std::fs::symlink_metadata(&path)?;
            anyhow::ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() <= limit,
                "invalid or oversized Allure report asset"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                anyhow::ensure!(metadata.nlink() == 1, "unsafe Allure report asset");
            }
            anyhow::ensure!(
                path.canonicalize()?.starts_with(&root),
                "Allure report escapes its run directory"
            );
            Ok(std::fs::read_to_string(path)?)
        };
        let reporter: serde_json::Value =
            serde_json::from_str(&read("qinaegis-reporter.json", 4096)?)?;
        anyhow::ensure!(
            reporter["schema_version"] == 1
                && reporter["reporter"] == "allure"
                && reporter["mode"] == "awesome-single-file",
            "invalid Allure reporter metadata"
        );
        let version = reporter["version"]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 64)
            .ok_or_else(|| anyhow::anyhow!("missing Allure reporter version"))?;
        Ok((read("index.html", 64 * 1024 * 1024)?, version.to_owned()))
    }

    pub fn get_allure_report_html(&self, project: &str, run_id: &str) -> String {
        match Self::allure_html(project, run_id) {
            Ok((html, version)) => {
                serde_json::json!({"ok":true,"html":html,"version":version}).to_string()
            }
            Err(error) => {
                serde_json::json!({"error":format!("Allure report unavailable: {error}")})
                    .to_string()
            }
        }
    }

    pub fn generate_allure_report(&mut self, project: &str, run_id: &str) -> String {
        let record = match Self::load_record(project, run_id) {
            Ok(record) => record,
            Err(error) => return serde_json::json!({"error":error.to_string()}).to_string(),
        };
        let job_id = self.job_id();
        let job_return = job_id.clone();
        let entry = JobEntry::new("allureReport");
        let output = entry.output.clone();
        let cancel = entry.cancel_flag.clone();
        if let Err(error) = self.register_job(&job_id, entry) {
            return serde_json::json!({"error":error.to_string()}).to_string();
        }
        let jobs = self.jobs.clone();
        let guard = WorkerGuard::new(self.active_workers.clone(), jobs.clone(), job_id.clone());
        self.schedule(job_id.clone(), move || {
            let _guard = guard;
            _guard.start();
            let outcome = (|| -> anyhow::Result<serde_json::Value> {
                anyhow::ensure!(
                    !cancel.load(Ordering::Acquire),
                    "Allure generation cancelled"
                );
                let root = LocalStorage::run_dir(&record.project, &record.run_id).canonicalize()?;
                qin_aegis_core::allure::export_run(&record, &root.join("allure-results"))?;
                if Self::allure_html(&record.project, &record.run_id).is_ok() {
                    return Ok(
                        serde_json::json!({"run_id":record.run_id,"report_file":"index.html"}),
                    );
                }
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                let data = rt.block_on(async {
                    tokio::time::timeout(std::time::Duration::from_secs(120), async {
                        let process = MidsceneProcess::spawn(None, None)
                            .await?
                            .with_cancellation(cancel.clone());
                        let response = process
                            .call(JsonRpcRequest::GenerateAllure {
                                results_dir: root
                                    .join("allure-results")
                                    .to_string_lossy()
                                    .into_owned(),
                                report_dir: root
                                    .join("allure-report")
                                    .to_string_lossy()
                                    .into_owned(),
                            })
                            .await;
                        let shutdown = process.call(JsonRpcRequest::Shutdown).await;
                        let cleanup = process.terminate().await;
                        let response = response?;
                        let shutdown = shutdown?;
                        cleanup?;
                        anyhow::ensure!(shutdown.ok, "Allure engine shutdown failed");
                        anyhow::ensure!(
                            response.ok,
                            "{}",
                            response
                                .error
                                .unwrap_or_else(|| "Allure generation failed".into())
                        );
                        Ok::<_, anyhow::Error>(response.data.unwrap_or(serde_json::Value::Null))
                    })
                    .await
                    .map_err(|_| anyhow::anyhow!("Allure generation timed out"))?
                })?;
                Self::allure_html(&record.project, &record.run_id)?;
                Ok(data)
            })();
            if let Ok(mut entries) = jobs.lock() {
                if let Some(entry) = entries.get_mut(&job_id) {
                    if cancel.load(Ordering::Acquire) {
                        entry.status = "stopped".into();
                        entry.error = Some("Allure generation cancelled".into());
                    } else {
                        match outcome {
                            Ok(data) => {
                                entry.status = "completed".into();
                                entry.result = Some(data);
                            }
                            Err(error) => {
                                entry.status = "failed".into();
                                entry.error = Some(error.to_string());
                            }
                        }
                    }
                    output
                        .lock()
                        .unwrap()
                        .push_str(&format!("Allure report generation {}.\n", entry.status));
                }
            }
        });
        self.job_response(&job_return)
    }

    // ── Output ────────────────────────────────────────────────────────────────

    pub fn get_output(&self) -> String {
        let out = self.output.lock().unwrap().clone();
        let escaped = out.replace('\\', "\\\\").replace('"', "\\\"");
        format!(r#"{{"output":"{}"}}"#, escaped)
    }

    pub fn clear_output(&mut self) {
        self.output.lock().unwrap().clear();
    }

    // ── Projects ─────────────────────────────────────────────────────────────

    pub fn get_projects(&self) -> String {
        match LocalStorage::list_projects() {
            Ok(projects) => {
                serde_json::to_string(&projects).unwrap_or_else(|_| r#"[]"#.to_string())
            }
            Err(e) => format!(r#"{{"error":"list_projects failed: {}"}}"#, e),
        }
    }

    pub fn create_project(&mut self, name: &str, url: &str, tech_stack: Vec<String>) -> String {
        match LocalStorage::init_project(name, url, tech_stack) {
            Ok(cfg) => serde_json::to_string(&cfg).unwrap_or_else(|_| r#"{"ok":true}"#.to_string()),
            Err(e) => format!(r#"{{"error":"init_project failed: {}"}}"#, e),
        }
    }

    fn report_summary(path: &std::path::Path) -> Option<serde_json::Value> {
        if path.join("run.json").exists() {
            let record = qin_aegis_core::execution_records::RunStore::new(path.to_path_buf())
                .load()
                .ok()?;
            if path.file_name()?.to_str()? != record.run_id {
                return None;
            }
            let passed = record.results.iter().filter(|r| r.passed).count();
            Some(
                serde_json::json!({"total":record.cases.len(),"passed":passed,"failed":record.cases.len()-passed,
                "executed":record.results.len(),"status":record.status,"test_type":record.test_type,"has_evidence":true}),
            )
        } else {
            serde_json::from_slice(&std::fs::read(path.join("summary.json")).ok()?).ok()
        }
    }

    pub fn get_reports(&self, project: &str) -> String {
        let mut reports = Vec::new();
        if let Ok(entries) = std::fs::read_dir(LocalStorage::reports_dir(project)) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if let Some(mut summary) = Self::report_summary(&path) {
                    summary["run_id"] = serde_json::json!(entry.file_name().to_string_lossy());
                    reports.push(summary);
                }
            }
        }
        reports.sort_by(|a, b| b["run_id"].as_str().cmp(&a["run_id"].as_str()));
        serde_json::to_string(&reports).unwrap_or_else(|_| "[]".into())
    }

    pub fn get_gate_status(&self, project: &str) -> String {
        let reports_dir = LocalStorage::reports_dir(project);
        if !reports_dir.exists() {
            return serde_json::to_string(&serde_json::json!({
                "e2e_pass_rate": null,
                "performance": null,
                "stress": null,
                "has_runs": false,
            }))
            .unwrap_or_default();
        }

        let mut latest_run: Option<(String, usize, usize, Option<String>)> = None;
        if let Ok(entries) = std::fs::read_dir(&reports_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(run_id) =
                        path.file_name().and_then(|n| n.to_str()).map(String::from)
                    {
                        if latest_run.is_none() || run_id > latest_run.as_ref().unwrap().0 {
                            if let Some(summary) = Self::report_summary(&path) {
                                let total = summary["total"].as_u64().unwrap_or(0) as usize;
                                let passed = summary["passed"].as_u64().unwrap_or(0) as usize;
                                if total > 0 {
                                    latest_run = Some((
                                        run_id,
                                        total,
                                        passed,
                                        summary["status"].as_str().map(String::from),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        match latest_run {
            Some((run_id, total, passed, execution_status)) => {
                // A complete set of passing assertions cannot override a failed
                // executor shutdown, additional check, cancellation or active run.
                if execution_status
                    .as_deref()
                    .is_some_and(|status| status != "completed")
                {
                    return serde_json::json!({
                        "e2e_pass_rate": null,
                        "e2e_pass_rate_display": if execution_status.as_deref() == Some("running") { "Running" } else { "Blocked" },
                        "execution_status": execution_status,
                        "execution_blocked": true,
                        "performance": null,
                        "stress": null,
                        "has_runs": true,
                        "last_run_id": run_id,
                        "last_run_passed": passed,
                        "last_run_total": total,
                    }).to_string();
                }
                let reports_dir = LocalStorage::reports_dir(project);
                let run_dir = reports_dir.join(&run_id);

                let performance_val = if let Ok(lh_content) =
                    std::fs::read_to_string(run_dir.join("lighthouse.json"))
                {
                    if let Ok(lh) = serde_json::from_str::<LighthouseResult>(&lh_content) {
                        let score = (lh.metrics.performance * 100.0) as u32;
                        Some(score)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let stress_val = if let Ok(lc_content) =
                    std::fs::read_to_string(run_dir.join("locust-summary.json"))
                {
                    if let Ok(lc) = serde_json::from_str::<LocustResult>(&lc_content) {
                        let rps = lc.stats.rps as u32;
                        Some(rps)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let pass_rate = if total > 0 {
                    (passed as f64 / total as f64) * 100.0
                } else {
                    100.0
                };
                serde_json::to_string(&serde_json::json!({
                    "e2e_pass_rate": pass_rate,
                    "execution_status": execution_status,
                    "execution_blocked": false,
                    "e2e_pass_rate_display": format!("{:.0}%", pass_rate),
                    "performance": performance_val.map(|s| format!("{:.0}%", s)),
                    "stress": stress_val.map(|r| format!("{:.0} req/s", r)),
                    "has_runs": true,
                    "last_run_id": run_id,
                    "last_run_passed": passed,
                    "last_run_total": total,
                }))
                .unwrap_or_default()
            }
            None => serde_json::to_string(&serde_json::json!({
                "e2e_pass_rate": null,
                "performance": null,
                "stress": null,
                "has_runs": false,
            }))
            .unwrap_or_default(),
        }
    }

    pub fn preview_test_plan(&self, project: &str, test_type: &str) -> String {
        let storage = LocalStorageInstance;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime for preview");

        let cases = rt.block_on(async {
            storage
                .load_cases_by_status(project, CaseStatus::Approved)
                .await
        });

        let cases = match cases {
            Ok(c) => c,
            Err(_) => return r#"{"error":"failed to load cases"}"#.to_string(),
        };

        // Filter by test_type
        let filtered: Vec<_> = cases
            .into_iter()
            .filter(|c| c.test_type == test_type)
            .collect();

        if filtered.is_empty() {
            return serde_json::to_string(&serde_json::json!({
                "plan": format!("No {} test cases found for project '{}'. Generate test cases first.", test_type, project),
                "case_count": 0,
                "steps": [],
            })).unwrap_or_default();
        }

        // Build summary of cases
        let case_summaries: Vec<String> = filtered
            .iter()
            .map(|c| format!("[{}] {} ({})", c.id, c.name, c.test_type))
            .collect();

        let plan_text = if let Ok((cfg, _, route)) = self.select_model("generate", Some(project)) {
            let llm = match route
                .as_ref()
                .map(qin_aegis_core::models::build_client)
                .transpose()
            {
                Ok(Some(client)) => client,
                Ok(None) => create_llm_client(&cfg),
                Err(e) => return serde_json::json!({"error":e.to_string()}).to_string(),
            };

            let summary = case_summaries.join("\n");
            let prompt = format!(
                "You are a QA engineer explaining a test execution plan.\n\
                Project: '{}', Test Type: '{}'\n\
                Test cases to execute:\n{}\n\n\
                Provide a brief natural language summary of what will happen \
                when these tests run. Focus on what user flows are being tested. \
                Reply in 2-3 sentences.",
                project, test_type, summary
            );

            let rt2 = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime for LLM");
            rt2.block_on(async {
                let msgs: Vec<Message> = vec![Message::user(prompt)];
                llm.chat(&msgs)
                    .await
                    .unwrap_or_else(|_| "Failed to generate plan preview.".to_string())
            })
        } else {
            format!(
                "{} test case(s) will run: {}",
                filtered.len(),
                case_summaries.join(", ")
            )
        };

        serde_json::to_string(&serde_json::json!({
            "plan": plan_text,
            "case_count": filtered.len(),
            "steps": filtered.iter().map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "name": c.name,
                    "priority": c.priority,
                    "type": c.test_type,
                })
            }).collect::<Vec<_>>(),
        }))
        .unwrap_or_default()
    }

    pub fn get_report_html(&self, project: &str, run_id: &str) -> String {
        let run_dir = LocalStorage::run_dir(project, run_id);
        let html_path = run_dir.join("report.html");
        if html_path.exists() {
            match std::fs::read_to_string(&html_path) {
                Ok(content) => serde_json::to_string(&serde_json::json!({
                    "ok": true,
                    "html": content,
                }))
                .unwrap_or_default(),
                Err(e) => serde_json::to_string(&serde_json::json!({
                    "ok": false,
                    "error": format!("failed to read report: {}", e),
                }))
                .unwrap_or_default(),
            }
        } else {
            serde_json::to_string(&serde_json::json!({
                "ok": false,
                "error": "report.html not found for this run",
            }))
            .unwrap_or_default()
        }
    }

    pub fn export_project(&self, project: &str) -> String {
        let reports_dir = LocalStorage::reports_dir(project);
        if !reports_dir.exists() {
            return serde_json::to_string(&serde_json::json!({
                "ok": false,
                "error": "no reports found",
            }))
            .unwrap_or_default();
        }

        let mut all_results = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&reports_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(run_id) = path.file_name().and_then(|n| n.to_str()) {
                        if let Some(summary) = Self::report_summary(&path) {
                            all_results
                                .push(serde_json::json!({"run_id":run_id,"summary":summary}));
                        }
                    }
                }
            }
        }

        serde_json::to_string(&serde_json::json!({
            "ok": true,
            "project": project,
            "reports": all_results,
        }))
        .unwrap_or_else(|_| r#"{"ok":false,"error":"serialization failed"}"#.to_string())
    }

    // ── Review ──────────────────────────────────────────────────────────────

    pub fn get_review_cases(&self, project: &str) -> String {
        let cases_dir = LocalStorage::cases_dir(project);
        if !cases_dir.exists() {
            return r#"[]"#.to_string();
        }
        let mut cases = Vec::new();
        // Load all cases from all status subdirs
        for status in ["draft", "reviewed", "approved", "flaky", "archived"] {
            let status_dir = cases_dir.join(status);
            if !status_dir.exists() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(&status_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(case_data) =
                                serde_json::from_str::<serde_json::Value>(&content)
                            {
                                cases.push(serde_json::json!({
                                    "id": path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown"),
                                    "name": case_data.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                                    "status": status,
                                    "priority": case_data.get("priority").and_then(|v| v.as_str()).unwrap_or("P2"),
                                    "type": case_data.get("test_type").and_then(|v| v.as_str()).unwrap_or("functional"),
                                    "requirement_id": case_data.get("requirement_id"),
                                }));
                            }
                        }
                    }
                }
            }
        }
        serde_json::to_string(&cases).unwrap_or_else(|_| r#"[]"#.to_string())
    }
}

/// Apply a browser patch without serializing secrets back to the caller.
fn patched_config(
    current: Option<&AppConfig>,
    value: &serde_json::Value,
) -> Result<AppConfig, String> {
    let obj = value.get("config").unwrap_or(value);
    if !obj.is_object() {
        return Err("config must be an object".into());
    }
    let mut cfg = current.cloned().unwrap_or_default();
    if let Some(llm) = obj.get("llm") {
        if !llm.is_object() {
            return Err("llm must be an object".into());
        }
        for (name, target) in [
            ("provider", &mut cfg.llm.provider),
            ("base_url", &mut cfg.llm.base_url),
            ("model", &mut cfg.llm.model),
            ("secondary_provider", &mut cfg.llm.secondary_provider),
            ("secondary_base_url", &mut cfg.llm.secondary_base_url),
            ("secondary_model", &mut cfg.llm.secondary_model),
        ] {
            if let Some(value) = llm.get(name) {
                *target = value
                    .as_str()
                    .ok_or_else(|| format!("{name} must be a string"))?
                    .to_owned();
            }
        }
        for (name, clear, target) in [
            ("api_key", "clear_api_key", &mut cfg.llm.api_key),
            (
                "secondary_api_key",
                "clear_secondary_api_key",
                &mut cfg.llm.secondary_api_key,
            ),
        ] {
            if let Some(flag) = llm.get(clear) {
                if !flag.is_boolean() {
                    return Err(format!("{clear} must be boolean"));
                }
            }
            if llm.get(clear).and_then(|v| v.as_bool()) == Some(true) {
                target.clear();
            } else if let Some(value) = llm.get(name) {
                let key = value
                    .as_str()
                    .ok_or_else(|| format!("{name} must be a string"))?;
                if !key.is_empty() {
                    *target = key.to_owned();
                }
            }
        }
    }
    if let Some(sandbox) = obj.get("sandbox") {
        if !sandbox.is_object() {
            return Err("sandbox must be an object".into());
        }
        if let Some(port) = sandbox.get("cdp_port") {
            cfg.sandbox.cdp_port = port
                .as_u64()
                .filter(|p| *p > 0 && *p <= 65535)
                .ok_or("cdp_port must be 1..65535")? as u16;
        }
    }
    if let Some(exploration) = obj.get("exploration") {
        if !exploration.is_object() {
            return Err("exploration must be an object".into());
        }
        for (name, max, target) in [
            ("max_depth", 10, &mut cfg.exploration.max_depth),
            (
                "max_pages_per_seed",
                1000,
                &mut cfg.exploration.max_pages_per_seed,
            ),
        ] {
            if let Some(v) = exploration.get(name) {
                *target = v
                    .as_u64()
                    .filter(|n| *n > 0 && *n <= max)
                    .ok_or_else(|| format!("invalid {name}"))? as u32;
            }
        }
    }
    validate_endpoints(&cfg, resolve_env_var)?;
    Ok(cfg)
}

/// Validate the resolved destination while preserving environment references on disk.
fn validate_endpoints(cfg: &AppConfig, resolve: impl Fn(&str) -> String) -> Result<(), String> {
    for endpoint in [&cfg.llm.base_url, &cfg.llm.secondary_base_url] {
        if !endpoint.is_empty() {
            let resolved = resolve(endpoint);
            let parsed =
                url::Url::parse(&resolved).map_err(|_| "invalid or unresolved model endpoint")?;
            if !matches!(parsed.scheme(), "http" | "https")
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
            {
                return Err("model endpoint must use http(s) without user credentials".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn empty_secret_patch_preserves_and_explicit_clear_removes() {
        let mut cfg = AppConfig::default();
        cfg.llm.api_key = "private-secret".into();
        let patched = patched_config(
            Some(&cfg),
            &json!({"config":{"llm":{"api_key":"", "model":"new"}}}),
        )
        .unwrap();
        assert_eq!(patched.llm.api_key, "private-secret");
        assert_eq!(patched.llm.model, "new");
        let cleared =
            patched_config(Some(&patched), &json!({"llm":{"clear_api_key":true}})).unwrap();
        assert!(cleared.llm.api_key.is_empty());
    }
    #[test]
    fn ports_and_depth_do_not_truncate_or_accept_wrong_types() {
        for p in [json!(0), json!(65536), json!(-1), json!("9222")] {
            assert!(patched_config(None, &json!({"sandbox":{"cdp_port":p}})).is_err());
        }
        assert!(patched_config(None, &json!({"exploration":{"max_depth":0}})).is_err());
        assert!(patched_config(None, &json!({"llm":{"base_url":"file:///tmp/key"}})).is_err());
    }
    #[test]
    fn config_response_never_contains_secret() {
        let mut cfg = AppConfig::default();
        cfg.llm.api_key = "secret-primary".into();
        cfg.llm.secondary_api_key = "secret-secondary".into();
        let app = AppState {
            config: Some(cfg),
            output: Arc::new(Mutex::new(String::new())),
            current_view: "dashboard".into(),
            next_job_id: std::sync::atomic::AtomicU64::new(1),
            jobs: Arc::new(Mutex::new(HashMap::new())),
            active_workers: Arc::new(AtomicUsize::new(0)),
            queue: Arc::new(Mutex::new(TaskQueue::default())),
            history: None,
            session_id: "test".into(),
        };
        let value: serde_json::Value = serde_json::from_str(&app.get_config()).unwrap();
        assert_eq!(value["llm"]["api_key"], "");
        assert_eq!(value["llm"]["api_key_configured"], true);
        assert!(!value.to_string().contains("secret-"));
    }
    fn empty_app() -> AppState {
        AppState {
            config: None,
            output: Arc::new(Mutex::new(String::new())),
            current_view: "dashboard".into(),
            next_job_id: std::sync::atomic::AtomicU64::new(1),
            jobs: Arc::new(Mutex::new(HashMap::new())),
            active_workers: Arc::new(AtomicUsize::new(0)),
            queue: Arc::new(Mutex::new(TaskQueue::default())),
            history: None,
            session_id: "test".into(),
        }
    }

    #[test]
    fn cancellation_does_not_complete_until_worker_exits() {
        let mut app = empty_app();
        app.jobs
            .lock()
            .unwrap()
            .insert("job-1".into(), JobEntry::new("test"));
        let guard = WorkerGuard::new(app.active_workers.clone(), app.jobs.clone(), "job-1".into());
        assert_eq!(app.active_workers.load(Ordering::Acquire), 1);
        assert!(app.stop_job("job-1").contains("true"));
        assert_eq!(app.jobs.lock().unwrap()["job-1"].status, "cancelling");
        AppState::set_job_status_internal(&app.jobs, "job-1", "completed");
        assert_eq!(app.jobs.lock().unwrap()["job-1"].status, "cancelling");
        assert_eq!(app.active_workers.load(Ordering::Acquire), 1);
        drop(guard);
        assert_eq!(app.active_workers.load(Ordering::Acquire), 0);
        assert_eq!(app.jobs.lock().unwrap()["job-1"].status, "stopped");
        AppState::set_job_status_internal(&app.jobs, "job-1", "completed");
        assert_eq!(app.jobs.lock().unwrap()["job-1"].status, "stopped");
    }

    #[test]
    fn completion_does_not_mean_cleanup_has_finished_and_terminal_is_preserved() {
        let app = empty_app();
        app.jobs
            .lock()
            .unwrap()
            .insert("job-1".into(), JobEntry::new("test"));
        let guard = WorkerGuard::new(app.active_workers.clone(), app.jobs.clone(), "job-1".into());
        AppState::set_job_status_internal(&app.jobs, "job-1", "completed");
        AppState::set_job_status_internal(&app.jobs, "job-1", "failed");
        assert_eq!(app.active_workers.load(Ordering::Acquire), 1);
        assert_eq!(app.jobs.lock().unwrap()["job-1"].status, "completed");
        let status: serde_json::Value = serde_json::from_str(&app.get_job_status("job-1")).unwrap();
        assert_eq!(status["status"], "running");
        let jobs: serde_json::Value = serde_json::from_str(&app.get_jobs()).unwrap();
        assert_eq!(jobs["jobs"][0]["status"], "running");
        drop(guard);
        assert_eq!(app.active_workers.load(Ordering::Acquire), 0);
        assert_eq!(app.jobs.lock().unwrap()["job-1"].status, "completed");
        let status: serde_json::Value = serde_json::from_str(&app.get_job_status("job-1")).unwrap();
        assert_eq!(status["status"], "completed");
    }

    #[test]
    fn terminal_visibility_waits_for_guard_history_flush_and_admission_release() {
        let dir = TaskTemp::new();
        let path = dir.path().join("history.json");
        let mut app = empty_app();
        let history = History {
            store: Arc::new(qin_aegis_core::jobs::JobStore::new(&path)),
            serial: Arc::new(Mutex::new(())),
            error: Arc::new(Mutex::new(None)),
            last: Arc::new(Mutex::new(None)),
        };
        app.history = Some(history.clone());
        app.register_job("job-1", JobEntry::new("test")).unwrap();
        let guard = WorkerGuard::new(app.active_workers.clone(), app.jobs.clone(), "job-1".into());
        AppState::set_job_status_internal(&app.jobs, "job-1", "completed");
        let serial = history.serial.lock().unwrap();
        let dropping = guard.done.clone();
        let cleanup = thread::spawn(move || drop(guard));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !dropping.load(Ordering::Acquire) {
            assert!(
                std::time::Instant::now() < deadline,
                "guard failed to start cleanup"
            );
            thread::yield_now();
        }
        let status: serde_json::Value = serde_json::from_str(&app.get_job_status("job-1")).unwrap();
        assert_eq!(status["status"], "running");
        let jobs: serde_json::Value = serde_json::from_str(&app.get_jobs()).unwrap();
        assert_eq!(jobs["jobs"][0]["status"], "running");
        assert_eq!(app.active_workers.load(Ordering::Acquire), 1);
        // This test directly owns a raw guard; scheduler admission is exercised
        // separately using submitted work, not an artificial active counter.
        // A late cancellation must not rewrite the already committed business result.
        assert!(app.stop_job("job-1").contains("not running"));
        drop(serial);
        cleanup.join().unwrap();
        let status: serde_json::Value = serde_json::from_str(&app.get_job_status("job-1")).unwrap();
        assert_eq!(status["status"], "completed");
        assert_eq!(app.active_workers.load(Ordering::Acquire), 0);
        let disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert!(disk.to_string().contains("completed"));
        let next: serde_json::Value =
            serde_json::from_str(&app.start_maintenance("next", |_| Ok(json!({})))).unwrap();
        assert_eq!(next["ok"], true);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.active_workers.load(Ordering::Acquire) != 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "next worker did not finish"
            );
            thread::yield_now();
        }
    }

    #[test]
    fn worker_unwind_releases_counter_and_marks_failure() {
        let app = empty_app();
        app.jobs
            .lock()
            .unwrap()
            .insert("job-1".into(), JobEntry::new("test"));
        let guard = WorkerGuard::new(app.active_workers.clone(), app.jobs.clone(), "job-1".into());
        let _ = std::panic::catch_unwind(move || {
            let _guard = guard;
            panic!("test worker failure");
        });
        assert_eq!(app.active_workers.load(Ordering::Acquire), 0);
        assert_eq!(app.jobs.lock().unwrap()["job-1"].status, "failed");
    }

    #[test]
    fn environment_endpoint_is_validated_after_resolution_but_reference_is_retained() {
        let mut cfg = AppConfig::default();
        cfg.llm.base_url = "${MODEL_URL}".into();
        validate_endpoints(&cfg, |value| {
            if value == "${MODEL_URL}" {
                "https://models.example/v1".into()
            } else {
                value.into()
            }
        })
        .unwrap();
        assert_eq!(cfg.llm.base_url, "${MODEL_URL}");
        assert!(validate_endpoints(&cfg, |_| "file:///etc/passwd".into()).is_err());
        assert!(validate_endpoints(&cfg, |_| String::new()).is_err());
        assert!(validate_endpoints(&cfg, str::to_owned).is_err());
    }
    struct TaskTemp(std::path::PathBuf);
    impl TaskTemp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "qinaegis-task-test-{}",
                crate::server::random_token().unwrap()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for TaskTemp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn task_output_cursor_and_history_redaction() {
        let dir = TaskTemp::new();
        let path = dir.path().join("history.json");
        let mut app = empty_app();
        app.history = Some(History {
            store: Arc::new(qin_aegis_core::jobs::JobStore::new(&path)),
            serial: Arc::new(Mutex::new(())),
            error: Arc::new(Mutex::new(None)),
            last: Arc::new(Mutex::new(None)),
        });
        let mut e = JobEntry::new("generate");
        e.secrets.push("private-test-key".into());
        e.output.lock().unwrap().push_str(&"中".repeat(40000));
        e.output.lock().unwrap().push_str("private-test-key");
        e.result = Some(json!({"echo":"private-test-key"}));
        app.register_job("job-safe", e).unwrap();
        let log: serde_json::Value = serde_json::from_str(&app.get_job_log("job-safe", 0)).unwrap();
        assert_eq!(log["truncated"], true);
        assert!(!log.to_string().contains("private-test-key"));
        let cursor = log["cursor"].as_u64().unwrap();
        let empty: serde_json::Value =
            serde_json::from_str(&app.get_job_log("job-safe", cursor)).unwrap();
        assert_eq!(empty["output"], "");
        let disk = std::fs::read_to_string(path).unwrap();
        assert!(!disk.contains("private-test-key"));
        assert!(disk.contains("REDACTED"));
    }
    #[test]
    fn task_initial_persistence_failure_starts_no_worker() {
        let dir = TaskTemp::new();
        let path = dir.path().join("history.json");
        std::fs::write(&path, "corrupt").unwrap();
        let mut app = empty_app();
        app.history = Some(History {
            store: Arc::new(qin_aegis_core::jobs::JobStore::new(path)),
            serial: Arc::new(Mutex::new(())),
            error: Arc::new(Mutex::new(None)),
            last: Arc::new(Mutex::new(None)),
        });
        let ran = Arc::new(AtomicBool::new(false));
        let flag = ran.clone();
        let result = app.start_maintenance("probe", move |_| {
            flag.store(true, Ordering::Release);
            Ok(json!({}))
        });
        assert!(result.contains("error"));
        assert!(!ran.load(Ordering::Acquire));
        assert!(app.jobs.lock().unwrap().is_empty());
        assert_eq!(app.active_workers.load(Ordering::Acquire), 0);
    }
    #[test]
    fn streaming_redaction_precedes_truncation_and_cursor_slicing() {
        let secret = "private-key-fixture";
        let mut output = JobOutput::default();
        output.configure(&[secret.into()]);
        output.push_str("private-key-");
        assert!(output.text.is_empty());
        output.push_str("fixture");
        assert_eq!(output.text, "*".repeat(secret.len()));
        output.push_str(&"x".repeat(LOG_LIMIT - 5));
        assert!(output.text.starts_with("*****"));
        assert!(!output.text.contains("xture"));
        let mut entry = JobEntry::new("test");
        entry.secrets.push(secret.into());
        entry.output.lock().unwrap().configure(&entry.secrets);
        entry
            .output
            .lock()
            .unwrap()
            .push_str(&format!("before {secret} after"));
        let app = empty_app();
        app.jobs.lock().unwrap().insert("safe".into(), entry);
        let log: serde_json::Value = serde_json::from_str(&app.get_job_log("safe", 10)).unwrap();
        assert_eq!(
            log["output"],
            format!("{} after", "*".repeat(secret.len() - 3))
        );
        assert_eq!(log["cursor"], (7 + secret.len() + 6) as u64);
        let mut pending = JobOutput::default();
        pending.configure(&["abc".into(), "abcdef".into()]);
        pending.push_str("abc");
        assert!(pending.text.is_empty());
        pending.push_str("def");
        assert_eq!(pending.text, "******");
        pending.push_str("abcde");
        pending.finish();
        assert_eq!(pending.text, "***********");
    }
    #[test]
    fn unchanged_checkpoints_skip_storage_and_failed_changes_retry() {
        let dir = TaskTemp::new();
        let path = dir.path().join("history.json");
        let history = History {
            store: Arc::new(qin_aegis_core::jobs::JobStore::new(&path)),
            serial: Arc::new(Mutex::new(())),
            error: Arc::new(Mutex::new(None)),
            last: Arc::new(Mutex::new(None)),
        };
        let mut entry = JobEntry::new("test");
        history.write("one", &entry, false);
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))
            .unwrap();
        lock.try_lock().unwrap();
        // Holding the lock proves that a no-change checkpoint never accesses storage.
        history.write("one", &entry, false);
        assert!(history.error.lock().unwrap().is_none());
        entry.output.lock().unwrap().push_str("changed");
        history.write("one", &entry, false);
        assert!(history.error.lock().unwrap().is_some());
        drop(lock);
        history.write("one", &entry, false);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()).unwrap()
                ["records"][0]["output"],
            "changed"
        );
        entry.status = "completed".into();
        history.write("one", &entry, true);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap()
                ["records"][0]["status"],
            "completed"
        );
    }
    #[test]
    fn bounded_fifo_cancellation_and_snapshot_have_no_waiting_side_effects() {
        let mut app = empty_app();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let first: serde_json::Value =
            serde_json::from_str(&app.start_maintenance("first", move |_| {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(json!({}))
            }))
            .unwrap();
        started_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut waiting = Vec::new();
        for value in 0..8 {
            let captured = order.clone();
            let response: serde_json::Value =
                serde_json::from_str(&app.start_maintenance("queued", move |_| {
                    captured.lock().unwrap().push(value);
                    Ok(json!({"frozen":value}))
                }))
                .unwrap();
            let id = response["jobId"].as_str().unwrap().to_string();
            let state: serde_json::Value = serde_json::from_str(&app.get_job_status(&id)).unwrap();
            assert_eq!(state["status"], "queued");
            assert_eq!(state["queue_position"], value + 1);
            waiting.push(id);
        }
        assert!(app
            .start_maintenance("overflow", |_| panic!("must not execute"))
            .contains("queue full"));
        assert!(order.lock().unwrap().is_empty());
        assert!(app.stop_job(&waiting[3]).contains("true"));
        assert_eq!(app.active_workers.load(Ordering::Acquire), 8);
        release_tx.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.active_workers.load(Ordering::Acquire) != 0 {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(*order.lock().unwrap(), vec![0, 1, 2, 4, 5, 6, 7]);
        let state: serde_json::Value =
            serde_json::from_str(&app.get_job_status(&waiting[3])).unwrap();
        assert_eq!(state["status"], "stopped");
        assert!(first["jobId"].is_string());
    }
    #[test]
    fn panic_releases_slot_and_queued_cancel_failure_keeps_work_unstarted() {
        let mut app = empty_app();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (start_tx, start_rx) = std::sync::mpsc::channel();
        app.start_maintenance("panic", move |_| {
            start_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            panic!("deliberate worker panic");
        });
        start_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let dir = TaskTemp::new();
        let path = dir.path().join("history.json");
        let history = History {
            store: Arc::new(qin_aegis_core::jobs::JobStore::new(&path)),
            serial: Arc::new(Mutex::new(())),
            error: Arc::new(Mutex::new(None)),
            last: Arc::new(Mutex::new(None)),
        };
        app.history = Some(history);
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let response: serde_json::Value =
            serde_json::from_str(&app.start_maintenance("waiting", move |_| {
                counted.fetch_add(1, Ordering::AcqRel);
                Ok(json!({}))
            }))
            .unwrap();
        let id = response["jobId"].as_str().unwrap();
        let saved = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"bad history").unwrap();
        assert!(app.stop_job(id).contains("not saved"));
        assert_eq!(calls.load(Ordering::Acquire), 0);
        assert_eq!(app.jobs.lock().unwrap()[id].status, "cancelling");
        std::fs::write(&path, saved).unwrap();
        release_tx.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.active_workers.load(Ordering::Acquire) != 0 {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(calls.load(Ordering::Acquire), 0);
        assert_eq!(app.jobs.lock().unwrap()[id].status, "stopped");
    }
    #[test]
    fn next_work_waits_for_final_checkpoint_and_failure_blocks_actions() {
        let mut app = empty_app();
        let dir = TaskTemp::new();
        let path = dir.path().join("history.json");
        let history = History {
            store: Arc::new(qin_aegis_core::jobs::JobStore::new(&path)),
            serial: Arc::new(Mutex::new(())),
            error: Arc::new(Mutex::new(None)),
            last: Arc::new(Mutex::new(None)),
        };
        app.history = Some(history.clone());
        let (start_tx, start_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let first: serde_json::Value =
            serde_json::from_str(&app.start_maintenance("first", move |_| {
                start_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(json!({}))
            }))
            .unwrap();
        start_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let waiting: serde_json::Value =
            serde_json::from_str(&app.start_maintenance("next", move |_| {
                counted.fetch_add(1, Ordering::AcqRel);
                Ok(json!({}))
            }))
            .unwrap();
        let serial = history.serial.lock().unwrap();
        release_tx.send(()).unwrap();
        // This lock models a slow final checkpoint. No next actions may start.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.jobs.lock().unwrap()[first["jobId"].as_str().unwrap()].status != "completed" {
            assert!(std::time::Instant::now() < deadline);
            thread::yield_now();
        }
        assert_eq!(calls.load(Ordering::Acquire), 0);
        assert_eq!(app.active_workers.load(Ordering::Acquire), 2);
        std::fs::write(&path, b"damaged-history").unwrap();
        drop(serial);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.active_workers.load(Ordering::Acquire) != 0 {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(calls.load(Ordering::Acquire), 0);
        assert_eq!(
            app.jobs.lock().unwrap()[waiting["jobId"].as_str().unwrap()].status,
            "failed"
        );
        assert!(history.error.lock().unwrap().is_some());
    }
    #[test]
    fn worker_panic_drops_lease_before_next_closure_runs() {
        let mut app = empty_app();
        let (start_tx, start_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let first: serde_json::Value =
            serde_json::from_str(&app.start_maintenance("panic", move |_| {
                start_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                panic!("owned work panic")
            }))
            .unwrap();
        start_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        app.start_maintenance("next", move |_| {
            counted.fetch_add(1, Ordering::AcqRel);
            Ok(json!({}))
        });
        release_tx.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.active_workers.load(Ordering::Acquire) != 0 {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(calls.load(Ordering::Acquire), 1);
        assert_eq!(
            app.jobs.lock().unwrap()[first["jobId"].as_str().unwrap()].status,
            "failed"
        );
    }
}
