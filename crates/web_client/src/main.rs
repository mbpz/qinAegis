use anyhow::Result;
use qin_aegis_core::{
    AppConfig, resolve_env_var, Explorer, LlmClient, Message, LlmConfig, SandboxConfig,
    TestCaseService, TestExecutor, TestCaseRef,
    ArcLlmClient, MiniMaxClient, LocalStorage, LocalStorageInstance,
    storage::{CaseStatus, Storage},
    LighthouseResult, LocustResult,
    protocol::{JsonRpcRequest, MidsceneProcess},
};
use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread;

mod assets;

// ============================================================================
// AppState — holds UI state + output buffer
// ============================================================================

pub const APP_VERSION: &str = env!("APP_BUILD_VERSION");

#[derive(Debug)]
pub struct AppState {
    pub config: Option<AppConfig>,
    pub output: Arc<Mutex<String>>,
    pub current_view: String,
    pub next_job_id: std::sync::atomic::AtomicU64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config: AppConfig::load_global().ok(),
            output: Arc::new(Mutex::new(String::new())),
            current_view: "dashboard".to_string(),
            next_job_id: std::sync::atomic::AtomicU64::new(1),
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    fn job_id(&self) -> String {
        format!("job-{}", self.next_job_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
    }

    fn append_output(&self, msg: &str) {
        let mut o = self.output.lock().unwrap();
        o.push_str(msg);
    }

    fn is_configured(&self) -> bool {
        self.config.as_ref().map(|c| !c.llm.api_key.is_empty()).unwrap_or(false)
    }

    // ── Config ──────────────────────────────────────────────────────────────

    pub fn get_config(&self) -> String {
        match &self.config {
            Some(cfg) => match serde_json::to_string(cfg) {
                Ok(s) => s,
                Err(e) => format!(r#"{{"error":"config serialization failed: {}"}}"#, e),
            },
            None => r#"null"#.to_string(),
        }
    }

    pub fn get_version(&self) -> String {
        format!(r#"{{"version":"{}"}}"#, APP_VERSION)
    }

    fn version_compare(a: &str, b: &str) -> i32 {
        let parse = |s: &str| {
            s.split('.').filter_map(|p| p.parse::<u64>().ok()).collect::<Vec<u64>>()
        };
        let a_parts = parse(a);
        let b_parts = parse(b);
        let max_len = a_parts.len().max(b_parts.len());
        for i in 0..max_len {
            let a_v = *a_parts.get(i).unwrap_or(&0);
            let b_v = *b_parts.get(i).unwrap_or(&0);
            if a_v < b_v { return -1; }
            if a_v > b_v { return 1; }
        }
        0
    }

    pub fn check_update(&self) -> String {
        let repo = "your-repo/qinAegis"; // TODO: update to actual repo
        let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
        match ureq::get(&url).set("Accept", "application/vnd.github+json").call() {
            Ok(resp) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(resp.into_string().as_deref().unwrap_or("{}")) {
                    let latest = json.get("tag_name").and_then(|v| v.as_str()).unwrap_or(APP_VERSION);
                    let latest_ver = latest.trim_start_matches('v');
                    let current_ver = APP_VERSION;
                    let up_to_date = Self::version_compare(current_ver, latest_ver) >= 0;
                    return serde_json::to_string(&serde_json::json!({
                        "current": current_ver,
                        "latest": latest_ver,
                        "upToDate": up_to_date,
                    })).unwrap_or_default();
                }
            }
            Err(e) => {
                tracing::warn!("update check failed: {}", e);
            }
        }
        // fallback: assume up to date on network error
        format!(r#"{{"current":"{}","latest":"{}","upToDate":true}}"#, APP_VERSION, APP_VERSION)
    }

    pub fn set_config(&mut self, raw: &str) -> String {
        let value: serde_json::Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(e) => return format!(r#"{{"error":"{}"}}"#, e),
        };

        let mut cfg = self.config.clone().unwrap_or_default();

        if let Some(obj) = value.as_object() {
            if let Some(llm) = obj.get("llm").and_then(|v| v.as_object()) {
                if let Some(k) = llm.get("api_key").and_then(|v| v.as_str()) {
                    cfg.llm.api_key = k.to_string();
                }
                if let Some(u) = llm.get("base_url").and_then(|v| v.as_str()) {
                    cfg.llm.base_url = u.to_string();
                }
                if let Some(m) = llm.get("model").and_then(|v| v.as_str()) {
                    cfg.llm.model = m.to_string();
                }
            }
            if let Some(sandbox) = obj.get("sandbox").and_then(|v| v.as_object()) {
                if let Some(p) = sandbox.get("cdp_port").and_then(|v| v.as_u64()) {
                    cfg.sandbox.cdp_port = p as u16;
                }
            }
        }

        match cfg.save_global() {
            Ok(_) => {
                self.config = Some(cfg);
                r#"{"ok":true}"#.to_string()
            }
            Err(e) => format!(r#"{{"error":"{}"}}"#, e),
        }
    }

    // ── Explore ──────────────────────────────────────────────────────────────

    pub fn run_explore(&mut self, url: &str, depth: u32) -> String {
        if !self.is_configured() {
            return r#"{"error":"config not set. Set LLM API key in Config panel."}"#.to_string();
        }

        let job_id = self.job_id();
        let output = self.output.clone();
        let cfg = self.config.as_ref().unwrap().clone();
        let url = url.to_string();
        let job_id_for_return = job_id.clone();

        self.append_output(&format!("[{}] explore dispatched: {} depth={}\n", job_id, url, depth));

        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime for explore");
            rt.block_on(async {
                let llm_cfg = LlmConfig {
                    api_key: resolve_env_var(&cfg.llm.api_key),
                    base_url: resolve_env_var(&cfg.llm.base_url),
                    model: cfg.llm.model.clone(),
                };
                let sandbox_cfg = SandboxConfig {
                    cdp_port: cfg.sandbox.cdp_port,
                };

                {
                    let mut o = output.lock().unwrap();
                    o.push_str(&format!("[{}] Starting explore: {} depth={}\n", job_id, url, depth));
                }

                match Explorer::new(Some(llm_cfg), Some(sandbox_cfg), None).await {
                    Ok(mut explorer) => {
                        match explorer.explore(&url, depth).await {
                            Ok(result) => {
                                let mut o = output.lock().unwrap();
                                o.push_str(&format!("[{}] ✓ Explored {} pages\n", job_id, result.pages.len()));
                                o.push_str(&format!("[{}] Output length: {} chars\n", job_id, result.markdown.len()));
                            }
                            Err(e) => {
                                let mut o = output.lock().unwrap();
                                o.push_str(&format!("[{}] ✗ Explore failed: {}\n", job_id, e));
                            }
                        }
                        let _ = explorer.shutdown().await;
                    }
                    Err(e) => {
                        let mut o = output.lock().unwrap();
                        o.push_str(&format!("[{}] ✗ Failed to start explorer: {}\n", job_id, e));
                    }
                }
            });
        });

        format!(r#"{{"ok":true,"jobId":"{}"}}"#, job_id_for_return)
    }

    // ── Generate ─────────────────────────────────────────────────────────────

    pub fn run_generate(&mut self, requirement: &str, spec_path: Option<&str>) -> String {
        if !self.is_configured() {
            return r#"{"error":"config not set. Set LLM API key in Config panel."}"#.to_string();
        }

        let job_id = self.job_id();
        let output = self.output.clone();
        let cfg = self.config.as_ref().unwrap().clone();
        let requirement = requirement.to_string();
        let spec_path = spec_path.map(|s| s.to_string());
        let job_id_for_return = job_id.clone();

        self.append_output(&format!("[{}] generate dispatched\n", job_id));

        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime for generate");
            rt.block_on(async {
                {
                    let mut o = output.lock().unwrap();
                    o.push_str(&format!("[{}] Generating test cases...\n", job_id));
                    o.push_str(&format!("[{}] Requirement: {}\n", job_id, requirement));
                }

                let spec_markdown = match spec_path.as_ref().and_then(|sp| {
                        // Reject absolute paths and path traversal attempts
                        if sp.starts_with('/') || sp.contains("..") {
                            return None;
                        }
                        std::fs::read_to_string(sp).ok()
                    }) {
                    Some(content) => content,
                    None => {
                        let mut o = output.lock().unwrap();
                        o.push_str(&format!("[{}] ⚠ spec file not readable, proceeding without it\n", job_id));
                        String::new()
                    }
                };

                let llm = ArcLlmClient::new(MiniMaxClient::new(
                    resolve_env_var(&cfg.llm.base_url),
                    resolve_env_var(&cfg.llm.api_key),
                    cfg.llm.model.clone(),
                ));

                let service = TestCaseService::new(llm, LocalStorageInstance);

                match service.generate_and_save("default", &spec_markdown, &requirement).await {
                    Ok(cases) => {
                        let mut o = output.lock().unwrap();
                        let saved: usize = cases.iter().filter(|c| c.saved).count();
                        o.push_str(&format!("[{}] ✓ Generated {} test cases ({} saved)\n", job_id, cases.len(), saved));
                    }
                    Err(e) => {
                        let mut o = output.lock().unwrap();
                        o.push_str(&format!("[{}] ✗ Generation failed: {}\n", job_id, e));
                    }
                }
            });
        });

        format!(r#"{{"ok":true,"jobId":"{}"}}"#, job_id_for_return)
    }

    // ── Run Tests ────────────────────────────────────────────────────────────

    pub fn run_tests(&mut self, project: &str, test_type: &str) -> String {
        if !self.is_configured() {
            return r#"{"error":"config not set. Set LLM API key in Config panel."}"#.to_string();
        }

        let job_id = self.job_id();
        let output = self.output.clone();
        let cfg = self.config.as_ref().unwrap().clone();
        let project = project.to_string();
        let test_type = test_type.to_string();
        let job_id_for_return = job_id.clone();

        self.append_output(&format!("[{}] run_tests dispatched: project={} type={}\n", job_id, project, test_type));

        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime for test execution");
            rt.block_on(async {
                {
                    let mut o = output.lock().unwrap();
                    o.push_str(&format!("[{}] Running tests: project={} type={}\n", job_id, project, test_type));
                }

                let llm_cfg = LlmConfig {
                    api_key: resolve_env_var(&cfg.llm.api_key),
                    base_url: resolve_env_var(&cfg.llm.base_url),
                    model: cfg.llm.model.clone(),
                };
                let sandbox_cfg = SandboxConfig {
                    cdp_port: cfg.sandbox.cdp_port,
                };

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
                                    let mut o = output.lock().unwrap();
                                    o.push_str(&format!("[{}] Found {} test cases\n", job_id, filtered.len()));
                                }

                                let results = match executor.run_parallel(filtered).await {
                                    Ok(results) => results,
                                    Err(e) => {
                                        let mut o = output.lock().unwrap();
                                        o.push_str(&format!("[{}] ✗ Run failed: {}\n", job_id, e));
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
                                    let mut o = output.lock().unwrap();
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
                                        let mut o = output.lock().unwrap();
                                        o.push_str(&format!("[{}] ✗ Failed to save summary: {}\n", job_id, e));
                                    }

                                    // For performance type, run lighthouse
                                    if test_type == "performance" {
                                        let project_url = storage.load_project(&project).await.map(|p| p.url).ok();
                                        if let Some(target_url) = project_url {
                                            let lh_path = run_dir.join("lighthouse.json");
                                            let llm_cfg = LlmConfig {
                                                api_key: resolve_env_var(&cfg.llm.api_key),
                                                base_url: resolve_env_var(&cfg.llm.base_url),
                                                model: cfg.llm.model.clone(),
                                            };
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
                                                                    let mut o = output.lock().unwrap();
                                                                    o.push_str(&format!("[{}] ✗ Failed to save lighthouse result: {}\n", job_id, e));
                                                                }
                                                            }
                                                        } else {
                                                            let mut o = output.lock().unwrap();
                                                            o.push_str(&format!("[{}] Lighthouse failed: {}\n", job_id, resp.error.unwrap_or_default()));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let mut o = output.lock().unwrap();
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
                                            let llm_cfg = LlmConfig {
                                                api_key: resolve_env_var(&cfg.llm.api_key),
                                                base_url: resolve_env_var(&cfg.llm.base_url),
                                                model: cfg.llm.model.clone(),
                                            };
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
                                                                    let mut o = output.lock().unwrap();
                                                                    o.push_str(&format!("[{}] ✗ Failed to save stress result: {}\n", job_id, e));
                                                                }
                                                            }
                                                        } else {
                                                            let mut o = output.lock().unwrap();
                                                            o.push_str(&format!("[{}] Stress test failed: {}\n", job_id, resp.error.unwrap_or_default()));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let mut o = output.lock().unwrap();
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
                                            let llm_cfg = LlmConfig {
                                                api_key: resolve_env_var(&cfg.llm.api_key),
                                                base_url: resolve_env_var(&cfg.llm.base_url),
                                                model: cfg.llm.model.clone(),
                                            };
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
                                                                    let mut o = output.lock().unwrap();
                                                                    o.push_str(&format!("[{}] ✗ Failed to save ZAP result: {}\n", job_id, e));
                                                                } else {
                                                                    let mut o = output.lock().unwrap();
                                                                    o.push_str(&format!("[{}] ZAP scan completed: {} alerts\n", job_id, data.get("alert_count").and_then(|v| v.as_u64()).unwrap_or(0)));
                                                                }
                                                            }
                                                        } else {
                                                            let mut o = output.lock().unwrap();
                                                            o.push_str(&format!("[{}] ZAP scan failed: {}\n", job_id, resp.error.unwrap_or_default()));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let mut o = output.lock().unwrap();
                                                    o.push_str(&format!("[{}] ✗ Failed to spawn midscene for ZAP: {}\n", job_id, e));
                                                }
                                            }
                                        }
                                    }
                                }

                                {
                                    let mut o = output.lock().unwrap();
                                    let passed = results.iter().filter(|r| r.passed).count();
                                    o.push_str(&format!("[{}] ✓ Passed {}/{} tests\n", job_id, passed, results.len()));
                                }
                            }
                            Err(e) => {
                                let mut o = output.lock().unwrap();
                                o.push_str(&format!("[{}] ✗ Failed to load cases: {}\n", job_id, e));
                            }
                        }
                        let _ = executor.shutdown().await;
                    }
                    Err(e) => {
                        let mut o = output.lock().unwrap();
                        o.push_str(&format!("[{}] ✗ Failed to start executor: {}\n", job_id, e));
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
            Ok(projects) => serde_json::to_string(&projects).unwrap_or_else(|_| r#"[]"#.to_string()),
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
                                if let Ok(summary) = serde_json::from_str::<serde_json::Value>(&content) {
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
            })).unwrap_or_default();
        }

        let mut latest_run: Option<(String, usize, usize)> = None;
        if let Ok(entries) = std::fs::read_dir(&reports_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(run_id) = path.file_name().and_then(|n| n.to_str()).map(String::from) {
                        if latest_run.is_none() || run_id > latest_run.as_ref().unwrap().0 {
                            let summary_path = path.join("summary.json");
                            if summary_path.exists() {
                                if let Ok(content) = std::fs::read_to_string(&summary_path) {
                                    if let Ok(summary) = serde_json::from_str::<serde_json::Value>(&content) {
                                        let total = summary.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                        let passed = summary.get("passed").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
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

                let performance_val = if let Ok(lh_content) = std::fs::read_to_string(run_dir.join("lighthouse.json")) {
                    if let Ok(lh) = serde_json::from_str::<LighthouseResult>(&lh_content) {
                        let score = (lh.metrics.performance * 100.0) as u32;
                        Some(score)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let stress_val = if let Ok(lc_content) = std::fs::read_to_string(run_dir.join("locust-summary.json")) {
                    if let Ok(lc) = serde_json::from_str::<LocustResult>(&lc_content) {
                        let rps = lc.stats.rps as u32;
                        Some(rps)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let pass_rate = if total > 0 { (passed as f64 / total as f64) * 100.0 } else { 100.0 };
                serde_json::to_string(&serde_json::json!({
                    "e2e_pass_rate": pass_rate,
                    "e2e_pass_rate_display": format!("{:.0}%", pass_rate),
                    "performance": performance_val.map(|s| format!("{:.0}%", s)),
                    "stress": stress_val.map(|r| format!("{:.0} req/s", r)),
                    "has_runs": true,
                    "last_run_id": run_id,
                    "last_run_passed": passed,
                    "last_run_total": total,
                })).unwrap_or_default()
            }
            None => serde_json::to_string(&serde_json::json!({
                "e2e_pass_rate": null,
                "performance": null,
                "stress": null,
                "has_runs": false,
            })).unwrap_or_default(),
        }
    }

    pub fn preview_test_plan(&self, project: &str, test_type: &str) -> String {
        let storage = LocalStorageInstance;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime for preview");

        let cases = rt.block_on(async {
            storage.load_cases_by_status(project, CaseStatus::Approved).await
        });

        let cases = match cases {
            Ok(c) => c,
            Err(_) => return r#"{"error":"failed to load cases"}"#.to_string(),
        };

        // Filter by test_type
        let filtered: Vec<_> = cases.into_iter().filter(|c| c.test_type == test_type).collect();

        if filtered.is_empty() {
            return serde_json::to_string(&serde_json::json!({
                "plan": format!("No {} test cases found for project '{}'. Generate test cases first.", test_type, project),
                "case_count": 0,
                "steps": [],
            })).unwrap_or_default();
        }

        // Build summary of cases
        let case_summaries: Vec<String> = filtered.iter().map(|c| {
            format!("[{}] {} ({})", c.id, c.name, c.test_type)
        }).collect();

        let plan_text = if let Some(ref cfg) = self.config {
            let llm_cfg = LlmConfig {
                api_key: resolve_env_var(&cfg.llm.api_key),
                base_url: resolve_env_var(&cfg.llm.base_url),
                model: cfg.llm.model.clone(),
            };
            let llm = ArcLlmClient::new(MiniMaxClient::new(
                llm_cfg.base_url,
                llm_cfg.api_key,
                llm_cfg.model,
            ));

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
                llm.chat(&msgs).await.unwrap_or_else(|_| "Failed to generate plan preview.".to_string())
            })
        } else {
            format!("{} test case(s) will run: {}", filtered.len(), case_summaries.join(", "))
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
        })).unwrap_or_default()
    }

    pub fn get_report_html(&self, project: &str, run_id: &str) -> String {
        let run_dir = LocalStorage::run_dir(project, run_id);
        let html_path = run_dir.join("report.html");
        if html_path.exists() {
            match std::fs::read_to_string(&html_path) {
                Ok(content) => serde_json::to_string(&serde_json::json!({
                    "ok": true,
                    "html": content,
                })).unwrap_or_default(),
                Err(e) => serde_json::to_string(&serde_json::json!({
                    "ok": false,
                    "error": format!("failed to read report: {}", e),
                })).unwrap_or_default(),
            }
        } else {
            serde_json::to_string(&serde_json::json!({
                "ok": false,
                "error": "report.html not found for this run",
            })).unwrap_or_default()
        }
    }

    pub fn export_project(&self, project: &str) -> String {
        let reports_dir = LocalStorage::reports_dir(project);
        if !reports_dir.exists() {
            return serde_json::to_string(&serde_json::json!({
                "ok": false,
                "error": "no reports found",
            })).unwrap_or_default();
        }

        let mut all_results = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&reports_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(run_id) = path.file_name().and_then(|n| n.to_str()) {
                        let summary_path = path.join("summary.json");
                        if let Ok(content) = std::fs::read_to_string(&summary_path) {
                            if let Ok(summary) = serde_json::from_str::<serde_json::Value>(&content) {
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
        })).unwrap_or_else(|_| r#"{"ok":false,"error":"serialization failed"}"#.to_string())
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
                            if let Ok(case_data) = serde_json::from_str::<serde_json::Value>(&content) {
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
            })).unwrap_or_default();
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
                        })).unwrap_or_default();
                    }
                }
                let dst = dst_dir.join(&case_file);
                if let Err(e) = std::fs::rename(&src, &dst) {
                    return serde_json::to_string(&serde_json::json!({
                        "ok": false,
                        "error": format!("failed to move case: {}", e)
                    })).unwrap_or_default();
                }
                return serde_json::to_string(&serde_json::json!({"ok": true})).unwrap_or_default();
            }
        }
        serde_json::to_string(&serde_json::json!({
            "ok": false,
            "error": "case not found"
        })).unwrap_or_default()
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() -> Result<()> {
    let event_loop = tao::event_loop::EventLoop::new();
    let window = tao::window::WindowBuilder::new()
        .with_title("QinAegis")
        .with_inner_size(tao::dpi::LogicalSize::new(1200.0, 800.0))
        .build(&event_loop)?;

    let app = Arc::new(Mutex::new(AppState::new()));
    let app_clone = app.clone();

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "ios", target_os = "android"))]
    {
        let webview = wry::WebViewBuilder::new()
            .with_custom_protocol("app".into(), move |_id, request| {
                let uri = request.uri();
                let path = uri.path().trim_start_matches('/');
                let query = uri.query().unwrap_or("");

                let mut params: HashMap<String, String> = HashMap::new();
                for pair in query.split('&') {
                    let mut it = pair.splitn(2, '=');
                    if let Some(k) = it.next() {
                        let v = it.next().unwrap_or("");
                        params.insert(k.to_string(), url_decode(v));
                    }
                }

                if path == "invoke" {
                    let method = params.get("method").map(|s| s.as_str()).unwrap_or("");
                    let params_json = params.get("params").map(|s| s.as_str()).unwrap_or("{}");

                    let result = {
                        let mut app = app_clone.lock().expect("app state mutex poisoned");
                        match method {
                            "getState" => {
                                let cfg_json = app.get_config();
                                let cfg_val: serde_json::Value = match serde_json::from_str(&cfg_json) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        return wry::http::Response::builder()
                                            .header("Content-Type", "application/json")
                                            .body(Cow::Owned(format!(r#"{{"error":"config parse failed: {}"}}"#, e).into_bytes()))
                                            .unwrap();
                                    }
                                };
                                format!(r#"{{"ok":true,"config":{},"currentView":"{}"}}"#, cfg_val, app.current_view)
                            }
                            "setConfig" => app.set_config(params_json),
                            "runExplore" => {
                                if let Ok(p) = serde_json::from_str::<serde_json::Value>(params_json) {
                                    let url = p.get("url").and_then(|v| v.as_str()).unwrap_or("");
                                    if url.is_empty() {
                                        let msg = r#"{"error":"url is required"}"#.to_string();
                                        return wry::http::Response::builder()
                                            .header("Content-Type", "application/json")
                                            .body(Cow::Owned(msg.into_bytes()))
                                            .unwrap();
                                    }
                                    // Use URL parser for proper scheme validation (not prefix-based)
                                    match url::Url::parse(url) {
                                        Ok(parsed) => {
                                            let scheme = parsed.scheme();
                                            if scheme != "http" && scheme != "https" {
                                                let msg = r#"{"error":"url must use http or https scheme"}"#.to_string();
                                                return wry::http::Response::builder()
                                                    .header("Content-Type", "application/json")
                                                    .body(Cow::Owned(msg.into_bytes()))
                                                    .unwrap();
                                            }
                                        }
                                        Err(_) => {
                                            let msg = r#"{"error":"invalid url format"}"#.to_string();
                                            return wry::http::Response::builder()
                                                .header("Content-Type", "application/json")
                                                .body(Cow::Owned(msg.into_bytes()))
                                                .unwrap();
                                        }
                                    }
                                    let depth = p.get("depth").and_then(|v| v.as_u64()).unwrap_or(3) as u32;
                                    app.run_explore(url, depth)
                                } else {
                                    r#"{"error":"invalid params"}"#.to_string()
                                }
                            }
                            "runGenerate" => {
                                if let Ok(p) = serde_json::from_str::<serde_json::Value>(params_json) {
                                    let req = p.get("requirement").and_then(|v| v.as_str()).unwrap_or("");
                                    let spec = p.get("spec").and_then(|v| v.as_str());
                                    app.run_generate(req, spec)
                                } else {
                                    r#"{"error":"invalid params"}"#.to_string()
                                }
                            }
                            "runTests" => {
                                if let Ok(p) = serde_json::from_str::<serde_json::Value>(params_json) {
                                    let project = p.get("project").and_then(|v| v.as_str()).unwrap_or("default");
                                    let test_type = p.get("type").and_then(|v| v.as_str()).unwrap_or("smoke");
                                    app.run_tests(project, test_type)
                                } else {
                                    r#"{"error":"invalid params"}"#.to_string()
                                }
                            }
                            "previewTestPlan" => {
                                if let Ok(p) = serde_json::from_str::<serde_json::Value>(params_json) {
                                    let project = p.get("project").and_then(|v| v.as_str()).unwrap_or("default");
                                    let test_type = p.get("type").and_then(|v| v.as_str()).unwrap_or("smoke");
                                    app.preview_test_plan(project, test_type)
                                } else {
                                    r#"{"error":"invalid params"}"#.to_string()
                                }
                            }
                            "getOutput" => app.get_output(),
                            "clearOutput" => {
                                app.clear_output();
                                r#"{"ok":true}"#.to_string()
                            }
                            "checkConfig" => {
                                let configured = app.is_configured();
                                serde_json::to_string(&serde_json::json!({"configured": configured})).unwrap_or_default()
                            }
                            "getProjects" => app.get_projects(),
                            "getReports" => {
                                if let Ok(p) = serde_json::from_str::<serde_json::Value>(params_json) {
                                    let project = p.get("project").and_then(|v| v.as_str()).unwrap_or("default");
                                    app.get_reports(project)
                                } else {
                                    app.get_reports("default")
                                }
                            }
                            "getGateStatus" => {
                                if let Ok(p) = serde_json::from_str::<serde_json::Value>(params_json) {
                                    let project = p.get("project").and_then(|v| v.as_str()).unwrap_or("default");
                                    app.get_gate_status(project)
                                } else {
                                    app.get_gate_status("default")
                                }
                            }
                            "createProject" => {
                                if let Ok(p) = serde_json::from_str::<serde_json::Value>(params_json) {
                                    let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                    let url = p.get("url").and_then(|v| v.as_str()).unwrap_or("");
                                    let tech_stack = p.get("tech_stack").and_then(|v| v.as_array())
                                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                        .unwrap_or_default();
                                    app.create_project(name, url, tech_stack)
                                } else {
                                    r#"{"error":"invalid params"}"#.to_string()
                                }
                            }
                            "getReportHtml" => {
                                if let Ok(p) = serde_json::from_str::<serde_json::Value>(params_json) {
                                    let project = p.get("project").and_then(|v| v.as_str()).unwrap_or("default");
                                    let run_id = p.get("run_id").and_then(|v| v.as_str()).unwrap_or("");
                                    app.get_report_html(project, run_id)
                                } else {
                                    r#"{"error":"invalid params"}"#.to_string()
                                }
                            }
                            "exportProject" => {
                                if let Ok(p) = serde_json::from_str::<serde_json::Value>(params_json) {
                                    let project = p.get("project").and_then(|v| v.as_str()).unwrap_or("default");
                                    app.export_project(project)
                                } else {
                                    r#"{"error":"invalid params"}"#.to_string()
                                }
                            }
                            "getReviewCases" => {
                                if let Ok(p) = serde_json::from_str::<serde_json::Value>(params_json) {
                                    let project = p.get("project").and_then(|v| v.as_str()).unwrap_or("default");
                                    app.get_review_cases(project)
                                } else {
                                    app.get_review_cases("default")
                                }
                            }
                            "updateCaseStatus" => {
                                if let Ok(p) = serde_json::from_str::<serde_json::Value>(params_json) {
                                    let project = p.get("project").and_then(|v| v.as_str()).unwrap_or("default");
                                    let case_id = p.get("case_id").and_then(|v| v.as_str()).unwrap_or("");
                                    let new_status = p.get("status").and_then(|v| v.as_str()).unwrap_or("");
                                    app.update_case_status(project, case_id, new_status)
                                } else {
                                    r#"{"error":"invalid params"}"#.to_string()
                                }
                            }
                            "getVersion" => app.get_version(),
                            "checkUpdate" => app.check_update(),
                            _ => format!(r#"{{"error":"unknown method: {}"}}"#, method),
                        }
                    };

                    wry::http::Response::builder()
                        .header("Content-Type", "application/json")
                        .body(Cow::Owned(result.into_bytes()))
                        .unwrap()
                } else {
                    let (contents, mime) = assets::getAsset(path);
                    wry::http::Response::builder()
                        .header("Content-Type", mime)
                        .body(Cow::Owned(contents.to_vec()))
                        .unwrap()
                }
            })
            .with_url("app://localhost/index.html")
            .build(&window)?;

        let init_script = r#"
            window.rpc = function(method, params) {
                var controller = null;
                var timeoutId = null;
                return new Promise(function(resolve, reject) {
                    var id = Date.now();
                    var paramsStr = encodeURIComponent(JSON.stringify(params));
                    var url = 'app://localhost/invoke?method=' + encodeURIComponent(method) + '&params=' + paramsStr + '&id=' + id;
                    controller = new AbortController();
                    timeoutId = setTimeout(function() {
                        controller.abort();
                        reject(new Error('timeout'));
                    }, 60000);
                    fetch(url, { signal: controller.signal }).then(function(resp) {
                        clearTimeout(timeoutId);
                        return resp.text();
                    }).then(function(text) {
                        try { resolve(JSON.parse(text)); }
                        catch(e) { reject(e); }
                    }).catch(function(e) {
                        clearTimeout(timeoutId);
                        reject(e);
                    });
                });
            };
            window.getState = function() { return window.rpc('getState', {}); };
            window.setConfig = function(c) { return window.rpc('setConfig', {config: c}); };
            window.runExplore = function(url, depth) { return window.rpc('runExplore', {url: url, depth: depth}); };
            window.runGenerate = function(req, spec) { return window.rpc('runGenerate', {requirement: req, spec: spec || null}); };
            window.runTests = function(project, type) { return window.rpc('runTests', {project: project, type: type}); };
            window.getOutput = function() { return window.rpc('getOutput', {}); };
            window.clearOutput = function() { return window.rpc('clearOutput', {}); };
            window.getProjects = function() { return window.rpc('getProjects', {}); };
            window.checkConfig = function() { return window.rpc('checkConfig', {}); };
            window.getReports = function(project) { return window.rpc('getReports', {project: project || 'default'}); };
            window.getGateStatus = function(project) { return window.rpc('getGateStatus', {project: project || 'default'}); };
            window.createProject = function(name, url, tech_stack) { return window.rpc('createProject', {name: name, url: url, tech_stack: tech_stack || []}); };
            window.getReportHtml = function(project, run_id) { return window.rpc('getReportHtml', {project: project || 'default', run_id: run_id}); };
            window.exportProject = function(project) { return window.rpc('exportProject', {project: project || 'default'}); };
            window.getReviewCases = function(project) { return window.rpc('getReviewCases', {project: project || 'default'}); };
            window.updateCaseStatus = function(project, case_id, status) { return window.rpc('updateCaseStatus', {project: project || 'default', case_id: case_id, status: status}); };
            console.log('RPC bridge ready');
        "#;

        webview.evaluate_script(init_script)?;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = tao::event_loop::ControlFlow::Wait;
            if let tao::event::Event::WindowEvent {
                event: tao::event::WindowEvent::CloseRequested,
                ..
            } = event {
                *control_flow = tao::event_loop::ControlFlow::Exit;
            }
        })
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios", target_os = "android")))]
    {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().unwrap();
        let _webview = wry::WebViewBuilder::new()
            .with_custom_protocol("app".into(), move |_id, request| {
                let path = request.uri().path().trim_start_matches('/');
                let (contents, mime) = assets::getAsset(path);
                wry::http::Response::builder()
                    .header("Content-Type", mime)
                    .body(Cow::Owned(contents.to_vec()))
                    .unwrap()
            })
            .with_url("app://localhost/index.html")
            .build_gtk(vbox)?;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = tao::event_loop::ControlFlow::Wait;
            if let tao::event::Event::WindowEvent {
                event: tao::event::WindowEvent::CloseRequested,
                ..
            } = event {
                *control_flow = tao::event_loop::ControlFlow::Exit;
            }
        })
    }
}

fn url_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() != 2 || u8::from_str_radix(&hex, 16).is_err() {
                // Invalid percent-encoding — reject the sequence entirely
                return String::new(); // fail fast rather than silently continue
            }
            result.push(u8::from_str_radix(&hex, 16).unwrap() as char);
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}