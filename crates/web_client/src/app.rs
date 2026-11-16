use qin_aegis_core::{
    protocol::{JsonRpcRequest, MidsceneProcess},
    resolve_env_var,
    storage::{CaseStatus, Storage},
    AppConfig, ArcLlmClient, Explorer, LighthouseResult, LlmClient, LlmConfig, LlmRouter,
    LocalStorage, LocalStorageInstance, LocustResult, Message, MiniMaxClient, ProviderConfig,
    SandboxConfig, TestCaseRef, TestCaseService, TestExecutor,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// JobEntry — tracks async job state for cancel/status control
// ============================================================================

#[derive(Clone)]
pub struct JobEntry {
    pub job_type: String,
    pub status: String, // idle/running/completed/failed/stopped
    pub output: Arc<Mutex<String>>,
    pub cancel_flag: Arc<AtomicBool>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub snapshot: Option<serde_json::Value>,
    // done_tx intentionally omitted - not needed after thread spawn
}

impl JobEntry {
    pub fn new(job_type: &str) -> Self {
        Self {
            job_type: job_type.to_string(),
            status: "running".to_string(),
            output: Arc::new(Mutex::new(String::new())),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            result: None,
            error: None,
            snapshot: None,
        }
    }
}

/// Tracks actual worker lifetime independently of the user-visible status.
struct WorkerGuard {
    active: Arc<AtomicUsize>,
    jobs: Arc<Mutex<HashMap<String, JobEntry>>>,
    job_id: String,
}

impl WorkerGuard {
    fn new(
        active: Arc<AtomicUsize>,
        jobs: Arc<Mutex<HashMap<String, JobEntry>>>,
        job_id: String,
    ) -> Self {
        active.fetch_add(1, Ordering::AcqRel);
        Self {
            active,
            jobs,
            job_id,
        }
    }
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(entry) = jobs.get_mut(&self.job_id) {
                if entry.cancel_flag.load(Ordering::Acquire) {
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

pub struct AppState {
    pub config: Option<AppConfig>,
    pub output: Arc<Mutex<String>>, // legacy shared output
    pub current_view: String,
    pub next_job_id: std::sync::atomic::AtomicU64,
    pub jobs: Arc<Mutex<HashMap<String, JobEntry>>>, // jobId -> JobEntry
    pub active_workers: Arc<AtomicUsize>,
}

impl Default for AppState {
    fn default() -> Self {
        let config_result = AppConfig::load_global();
        Self {
            config: config_result.ok(),
            output: Arc::new(Mutex::new(String::new())),
            current_view: "dashboard".to_string(),
            next_job_id: std::sync::atomic::AtomicU64::new(1),
            jobs: Arc::new(Mutex::new(HashMap::new())),
            active_workers: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    fn job_id(&self) -> String {
        format!(
            "job-{}",
            self.next_job_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        )
    }

    fn append_output(&self, msg: &str) {
        let mut o = self.output.lock().unwrap();
        o.push_str(msg);
    }

    pub fn get_job_status(&self, job_id: &str) -> String {
        let entry = self.jobs.lock().unwrap().get(job_id).cloned();
        match entry {
            Some(entry) => {
                let output = entry.output.lock().unwrap().clone();
                serde_json::to_string(&serde_json::json!({
                    "jobId": job_id,
                    "jobType": entry.job_type,
                    "status": entry.status,
                    "output": output,
                    "result": entry.result,
                    "error": entry.error,
                    "snapshot": entry.snapshot,
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
        let mut jobs = self.jobs.lock().unwrap();
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
    ) -> anyhow::Result<(
        AppConfig,
        LlmConfig,
        Option<qin_aegis_core::models::ResolvedModelRoute>,
    )> {
        let mut cfg = self.config.clone().unwrap_or_default();
        let registry = crate::model_api::registry()?;
        let snapshot = registry.snapshot()?;
        if !snapshot.profiles.is_empty() || !snapshot.routes.is_empty() {
            let route = registry.resolve(task, &[])?;
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
        match cfg.save_global() {
            Ok(()) => {
                self.config = Some(cfg);
                serde_json::json!({"ok":true}).to_string()
            }
            Err(_) => {
                serde_json::json!({"ok":false,"error":"could not save configuration"}).to_string()
            }
        }
    }

    // ── Explore ──────────────────────────────────────────────────────────────

    pub fn run_explore(&mut self, url: &str, depth: u32) -> String {
        let (cfg, frozen_visual, model_route) = match self.select_model("explore") {
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

        {
            let mut jobs_guard = jobs.lock().unwrap();
            jobs_guard.insert(job_id.clone(), entry);
        }

        self.append_output(&format!(
            "[{}] explore dispatched: {} depth={}\n",
            job_id, url, depth
        ));

        let worker = WorkerGuard::new(self.active_workers.clone(), jobs.clone(), job_id.clone());
        thread::spawn(move || {
            let _worker = worker;
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

                match Explorer::new(Some(llm_cfg), Some(sandbox_cfg), None).await {
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

        format!(r#"{{"ok":true,"jobId":"{}"}}"#, job_id_for_return)
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
    ) -> String {
        let (cfg, _, route) = match self.select_model("generate") {
            Ok(selection) => selection,
            Err(e) => return serde_json::json!({"error":e.to_string()}).to_string(),
        };
        let spec_markdown = match contracts.as_ref().map(|s| s.specification()).transpose() {
            Ok(value) => value.unwrap_or_default(),
            Err(e) => return serde_json::json!({"error":e.to_string()}).to_string(),
        };
        let model_snapshot = Self::model_snapshot(&route, &cfg);
        let job_id = self.job_id();
        let requirement = requirement.to_string();
        let project = project.to_string();
        let jobs = self.jobs.clone();
        let job_id_for_return = job_id.clone();

        // Create JobEntry BEFORE spawning thread
        let mut entry = JobEntry::new("generate");
        entry.snapshot = Some(serde_json::json!({"model":model_snapshot,"contracts":contracts}));
        let cancel_flag = entry.cancel_flag.clone();
        let output_arc = entry.output.clone();

        {
            let mut jobs_guard = jobs.lock().unwrap();
            jobs_guard.insert(job_id.clone(), entry);
        }

        self.append_output(&format!("[{}] generate dispatched\n", job_id));

        let worker = WorkerGuard::new(self.active_workers.clone(), jobs.clone(), job_id.clone());
        thread::spawn(move || {
            let _worker = worker;
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

                let llm = match route.as_ref() {
                    Some(route) => match qin_aegis_core::models::build_client(route) {
                        Ok(client) => client,
                        Err(e) => {
                            Self::set_job_status_internal(&jobs, &job_id, "failed");
                            Self::set_job_error_internal(&jobs, &job_id, &e.to_string());
                            return;
                        }
                    },
                    None => create_llm_client(&cfg),
                };

                let service = TestCaseService::new(llm, LocalStorageInstance);

                // Check cancel flag before generate
                if cancel_flag.load(Ordering::Relaxed) {
                    Self::set_job_status_internal(&jobs, &job_id, "stopped");
                    return;
                }

                match service
                    .generate_and_save(&project, &spec_markdown, &requirement)
                    .await
                {
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

        format!(r#"{{"ok":true,"jobId":"{}"}}"#, job_id_for_return)
    }

    // ── Run Tests ────────────────────────────────────────────────────────────

    pub fn run_tests(&mut self, project: &str, test_type: &str) -> String {
        let (cfg, frozen_visual, model_route) = match self.select_model("execute") {
            Ok(selection) => selection,
            Err(e) => return serde_json::json!({"error":e.to_string()}).to_string(),
        };
        let model_snapshot = Self::model_snapshot(&model_route, &cfg);
        let job_id = self.job_id();
        let project = project.to_string();
        let test_type = test_type.to_string();
        let jobs = self.jobs.clone();
        let job_id_for_return = job_id.clone();

        // Create JobEntry BEFORE spawning thread
        let mut entry = JobEntry::new("testExec");
        entry.snapshot = Some(serde_json::json!({"model":model_snapshot}));
        let cancel_flag = entry.cancel_flag.clone();
        let output_arc = entry.output.clone();

        {
            let mut jobs_guard = jobs.lock().unwrap();
            jobs_guard.insert(job_id.clone(), entry);
        }

        self.append_output(&format!(
            "[{}] run_tests dispatched: project={} type={}\n",
            job_id, project, test_type
        ));

        let worker = WorkerGuard::new(self.active_workers.clone(), jobs.clone(), job_id.clone());
        thread::spawn(move || {
            let _worker = worker;
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime for test execution");
            rt.block_on(async {
                // Check cancel flag
                if cancel_flag.load(Ordering::Relaxed) {
                    Self::set_job_status_internal(&jobs, &job_id, "stopped");
                    return;
                }

                {
                    let mut o = output_arc.lock().unwrap();
                    o.push_str(&format!("[{}] Running tests: project={} type={}\n", job_id, project, test_type));
                }

                let llm_cfg = frozen_visual.clone();
                let sandbox_cfg = SandboxConfig {
                    cdp_port: cfg.sandbox.cdp_port,
                };

                // Check cancel flag before TestExecutor::new
                if cancel_flag.load(Ordering::Relaxed) {
                    Self::set_job_status_internal(&jobs, &job_id, "stopped");
                    return;
                }

                match TestExecutor::new(3, Some(llm_cfg), Some(sandbox_cfg)).await {
                    Ok(executor) => {
                        let storage = LocalStorageInstance;
                        match storage.load_cases_by_status(&project, CaseStatus::Approved).await {
                            Ok(cases) => {
                                // Filter cases by test_type (smoke/functional/performance/stress)
                                let filtered: Vec<TestCaseRef> = cases
                                    .iter()
                                    .filter(|c| c.test_type == test_type)
                                    .map(|c| TestCaseRef {
                                        id: c.id.clone(),
                                        yaml_script: c.yaml_script.clone(),
                                        name: c.name.clone(),
                                        priority: c.priority.clone(),
                                        target_url: None,
                                    })
                                    .collect();

                                {
                                    let mut o = output_arc.lock().unwrap();
                                    o.push_str(&format!("[{}] Found {} test cases\n", job_id, filtered.len()));
                                }

                                // Check cancel flag before run_parallel
                                if cancel_flag.load(Ordering::Relaxed) {
                                    let _ = executor.shutdown().await;
                                    Self::set_job_status_internal(&jobs, &job_id, "stopped");
                                    return;
                                }

                                let results = match executor.run_parallel(filtered).await {
                                    Ok(results) => results,
                                    Err(e) => {
                                        let mut o = output_arc.lock().unwrap();
                                        o.push_str(&format!("[{}] ✗ Run failed: {}\n", job_id, e));
                                        Self::set_job_status_internal(&jobs, &job_id, "failed");
                                        Self::set_job_error_internal(&jobs, &job_id, &e.to_string());
                                        return;
                                    }
                                };

                                // Save run results to storage
                                let run_id = {
                                    use std::time::{SystemTime, UNIX_EPOCH};
                                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
                                    format!("run-{}", now.as_secs())
                                };
                                let run_dir = LocalStorage::run_dir(&project, &run_id);
                                if let Err(e) = std::fs::create_dir_all(&run_dir) {
                                    let mut o = output_arc.lock().unwrap();
                                    o.push_str(&format!("[{}] ✗ Failed to create run dir: {}\n", job_id, e));
                                } else {
                                    let passed = results.iter().filter(|r| r.passed).count();
                                    let total = results.len();
                                    if let Err(e) = std::fs::write(
                                        run_dir.join("summary.json"),
                                        serde_json::to_string(&serde_json::json!({
                                            "passed": passed,
                                            "failed": total - passed,
                                            "total": total,
                                            "test_type": test_type,
                                        })).unwrap(),
                                    ) {
                                        let mut o = output_arc.lock().unwrap();
                                        o.push_str(&format!("[{}] ✗ Failed to save summary: {}\n", job_id, e));
                                    }

                                    // For performance type, run lighthouse
                                    if test_type == "performance" {
                                        let project_url = storage.load_project(&project).await.map(|p| p.url).ok();
                                        if let Some(target_url) = project_url {
                                            let lh_path = run_dir.join("lighthouse.json");
                                            let llm_cfg = frozen_visual.clone();
                                            let sandbox_cfg = SandboxConfig {
                                                cdp_port: cfg.sandbox.cdp_port,
                                            };
                                            match MidsceneProcess::spawn(Some(llm_cfg), Some(sandbox_cfg)).await {
                                                Ok(process) => {
                                                    let lh_result = process.call(JsonRpcRequest::Lighthouse { url: target_url }).await;
                                                    drop(process);
                                                    if let Ok(resp) = lh_result {
                                                        if resp.ok {
                                                            if let Some(data) = resp.data {
                                                                if let Err(e) = std::fs::write(&lh_path, serde_json::to_string(&data).unwrap()) {
                                                                    let mut o = output_arc.lock().unwrap();
                                                                    o.push_str(&format!("[{}] ✗ Failed to save lighthouse result: {}\n", job_id, e));
                                                                }
                                                            }
                                                        } else {
                                                            let mut o = output_arc.lock().unwrap();
                                                            o.push_str(&format!("[{}] Lighthouse failed: {}\n", job_id, resp.error.unwrap_or_default()));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let mut o = output_arc.lock().unwrap();
                                                    o.push_str(&format!("[{}] ✗ Failed to spawn midscene for lighthouse: {}\n", job_id, e));
                                                }
                                            }
                                        }
                                    }

                                    // For stress type, run locust stress test
                                    if test_type == "stress" {
                                        let project_url = storage.load_project(&project).await.map(|p| p.url).ok();
                                        if let Some(target_url) = project_url {
                                            let lc_path = run_dir.join("locust-summary.json");
                                            let llm_cfg = frozen_visual.clone();
                                            let sandbox_cfg = SandboxConfig {
                                                cdp_port: cfg.sandbox.cdp_port,
                                            };
                                            match MidsceneProcess::spawn(Some(llm_cfg), Some(sandbox_cfg)).await {
                                                Ok(process) => {
                                                    let lc_result = process.call(JsonRpcRequest::Stress { target_url, users: 10, spawn_rate: 2, duration: 30 }).await;
                                                    drop(process);
                                                    if let Ok(resp) = lc_result {
                                                        if resp.ok {
                                                            if let Some(data) = resp.data {
                                                                if let Err(e) = std::fs::write(&lc_path, serde_json::to_string(&data).unwrap()) {
                                                                    let mut o = output_arc.lock().unwrap();
                                                                    o.push_str(&format!("[{}] ✗ Failed to save stress result: {}\n", job_id, e));
                                                                }
                                                            }
                                                        } else {
                                                            let mut o = output_arc.lock().unwrap();
                                                            o.push_str(&format!("[{}] Stress test failed: {}\n", job_id, resp.error.unwrap_or_default()));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let mut o = output_arc.lock().unwrap();
                                                    o.push_str(&format!("[{}] ✗ Failed to spawn midscene for stress: {}\n", job_id, e));
                                                }
                                            }
                                        }
                                    }

                                    // For security type, run OWASP ZAP scan
                                    if test_type == "security" {
                                        let project_url = storage.load_project(&project).await.map(|p| p.url).ok();
                                        if let Some(target_url) = project_url {
                                            let zap_path = run_dir.join("zap-report.json");
                                            let llm_cfg = frozen_visual.clone();
                                            let sandbox_cfg = SandboxConfig {
                                                cdp_port: cfg.sandbox.cdp_port,
                                            };
                                            match MidsceneProcess::spawn(Some(llm_cfg), Some(sandbox_cfg)).await {
                                                Ok(process) => {
                                                    let zap_result = process.call(JsonRpcRequest::ZapScan { target_url }).await;
                                                    drop(process);
                                                    if let Ok(resp) = zap_result {
                                                        if resp.ok {
                                                            if let Some(data) = resp.data {
                                                                if let Err(e) = std::fs::write(&zap_path, serde_json::to_string(&data).unwrap()) {
                                                                    let mut o = output_arc.lock().unwrap();
                                                                    o.push_str(&format!("[{}] ✗ Failed to save ZAP result: {}\n", job_id, e));
                                                                } else {
                                                                    let mut o = output_arc.lock().unwrap();
                                                                    o.push_str(&format!("[{}] ZAP scan completed: {} alerts\n", job_id, data.get("alert_count").and_then(|v| v.as_u64()).unwrap_or(0)));
                                                                }
                                                            }
                                                        } else {
                                                            let mut o = output_arc.lock().unwrap();
                                                            o.push_str(&format!("[{}] ZAP scan failed: {}\n", job_id, resp.error.unwrap_or_default()));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let mut o = output_arc.lock().unwrap();
                                                    o.push_str(&format!("[{}] ✗ Failed to spawn midscene for ZAP: {}\n", job_id, e));
                                                }
                                            }
                                        }
                                    }
                                }

                                {
                                    let mut o = output_arc.lock().unwrap();
                                    let passed = results.iter().filter(|r| r.passed).count();
                                    o.push_str(&format!("[{}] ✓ Passed {}/{} tests\n", job_id, passed, results.len()));
                                    Self::set_job_status_internal(&jobs, &job_id, "completed");
                                }
                            }
                            Err(e) => {
                                let mut o = output_arc.lock().unwrap();
                                o.push_str(&format!("[{}] ✗ Failed to load cases: {}\n", job_id, e));
                                Self::set_job_status_internal(&jobs, &job_id, "failed");
                                Self::set_job_error_internal(&jobs, &job_id, &e.to_string());
                            }
                        }
                        let _ = executor.shutdown().await;
                    }
                    Err(e) => {
                        let mut o = output_arc.lock().unwrap();
                        o.push_str(&format!("[{}] ✗ Failed to start executor: {}\n", job_id, e));
                        Self::set_job_status_internal(&jobs, &job_id, "failed");
                        Self::set_job_error_internal(&jobs, &job_id, &e.to_string());
                    }
                }
            });
        });

        format!(r#"{{"ok":true,"jobId":"{}"}}"#, job_id_for_return)
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

    pub fn get_reports(&self, project: &str) -> String {
        let reports_dir = LocalStorage::reports_dir(project);
        if !reports_dir.exists() {
            return r#"[]"#.to_string();
        }
        let mut reports = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&reports_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(run_id) = path.file_name().and_then(|n| n.to_str()) {
                        let summary_path = path.join("summary.json");
                        if summary_path.exists() {
                            if let Ok(content) = std::fs::read_to_string(&summary_path) {
                                if let Ok(summary) =
                                    serde_json::from_str::<serde_json::Value>(&content)
                                {
                                    reports.push(serde_json::json!({
                                        "run_id": run_id,
                                        "total": summary.get("total").unwrap_or(&serde_json::Value::Null),
                                        "passed": summary.get("passed").unwrap_or(&serde_json::Value::Null),
                                        "failed": summary.get("failed").unwrap_or(&serde_json::Value::Null),
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }
        reports.sort_by(|a, b| {
            let a_ts = b.get("run_id").and_then(|v| v.as_str()).unwrap_or("");
            let b_ts = a.get("run_id").and_then(|v| v.as_str()).unwrap_or("");
            b_ts.cmp(a_ts)
        });
        serde_json::to_string(&reports).unwrap_or_else(|_| r#"[]"#.to_string())
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

        let mut latest_run: Option<(String, usize, usize)> = None;
        if let Ok(entries) = std::fs::read_dir(&reports_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(run_id) =
                        path.file_name().and_then(|n| n.to_str()).map(String::from)
                    {
                        if latest_run.is_none() || run_id > latest_run.as_ref().unwrap().0 {
                            let summary_path = path.join("summary.json");
                            if summary_path.exists() {
                                if let Ok(content) = std::fs::read_to_string(&summary_path) {
                                    if let Ok(summary) =
                                        serde_json::from_str::<serde_json::Value>(&content)
                                    {
                                        let total = summary
                                            .get("total")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0)
                                            as usize;
                                        let passed = summary
                                            .get("passed")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0)
                                            as usize;
                                        latest_run = Some((run_id, total, passed));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        match latest_run {
            Some((run_id, total, passed)) => {
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

        let plan_text = if let Ok((cfg, _, route)) = self.select_model("generate") {
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
                        let summary_path = path.join("summary.json");
                        if let Ok(content) = std::fs::read_to_string(&summary_path) {
                            if let Ok(summary) = serde_json::from_str::<serde_json::Value>(&content)
                            {
                                all_results.push(serde_json::json!({
                                    "run_id": run_id,
                                    "summary": summary,
                                }));
                            }
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
                                    "type": case_data.get("type").and_then(|v| v.as_str()).unwrap_or("functional"),
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

    pub fn update_case_status(&self, project: &str, case_id: &str, new_status: &str) -> String {
        let cases_dir = LocalStorage::cases_dir(project);
        let valid_statuses = ["draft", "reviewed", "approved", "flaky", "archived"];
        if !valid_statuses.contains(&new_status) {
            return serde_json::to_string(&serde_json::json!({
                "ok": false,
                "error": "invalid status"
            }))
            .unwrap_or_default();
        }
        // Find the case file in any status subdir
        let case_file = format!("{}.json", case_id);
        for status in &valid_statuses {
            let src = cases_dir.join(status).join(&case_file);
            if src.exists() {
                let dst_dir = cases_dir.join(new_status);
                if !dst_dir.exists() {
                    if let Err(e) = std::fs::create_dir_all(&dst_dir) {
                        return serde_json::to_string(&serde_json::json!({
                            "ok": false,
                            "error": format!("failed to create dir: {}", e)
                        }))
                        .unwrap_or_default();
                    }
                }
                let dst = dst_dir.join(&case_file);
                if let Err(e) = std::fs::rename(&src, &dst) {
                    return serde_json::to_string(&serde_json::json!({
                        "ok": false,
                        "error": format!("failed to move case: {}", e)
                    }))
                    .unwrap_or_default();
                }
                return serde_json::to_string(&serde_json::json!({"ok": true})).unwrap_or_default();
            }
        }
        serde_json::to_string(&serde_json::json!({
            "ok": false,
            "error": "case not found"
        }))
        .unwrap_or_default()
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
        drop(guard);
        assert_eq!(app.active_workers.load(Ordering::Acquire), 0);
        assert_eq!(app.jobs.lock().unwrap()["job-1"].status, "completed");
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
}
