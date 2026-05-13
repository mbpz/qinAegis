use anyhow::Result;
use qin_aegis_core::{
    AppConfig, resolve_env_var, Explorer, LlmConfig, SandboxConfig,
    TestCaseService, TestExecutor, TestCaseRef,
    ArcLlmClient, MiniMaxClient, LocalStorage, LocalStorageInstance,
    storage::{CaseStatus, Storage},
};
use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread;

mod assets;

// ============================================================================
// AppState — holds UI state + output buffer
// ============================================================================

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

                let spec_markdown = match spec_path.as_ref().and_then(|sp| std::fs::read_to_string(sp).ok()) {
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
                                let refs: Vec<TestCaseRef> = cases.iter().map(|c| TestCaseRef {
                                    id: c.id.clone(),
                                    yaml_script: c.yaml_script.clone(),
                                    name: c.name.clone(),
                                    priority: c.priority.clone(),
                                    target_url: None,
                                }).collect();

                                {
                                    let mut o = output.lock().unwrap();
                                    o.push_str(&format!("[{}] Found {} test cases\n", job_id, refs.len()));
                                }

                                let results = match executor.run_parallel(refs).await {
                                    Ok(results) => results,
                                    Err(e) => {
                                        let mut o = output.lock().unwrap();
                                        o.push_str(&format!("[{}] ✗ Run failed: {}\n", job_id, e));
                                        return;
                                    }
                                };

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
                                    if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
                                        let msg = r#"{"error":"url must start with http:// or https://"}"#.to_string();
                                        return wry::http::Response::builder()
                                            .header("Content-Type", "application/json")
                                            .body(Cow::Owned(msg.into_bytes()))
                                            .unwrap();
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
                            "getOutput" => app.get_output(),
                            "clearOutput" => {
                                app.clear_output();
                                r#"{"ok":true}"#.to_string()
                            }
                            "getProjects" => app.get_projects(),
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
            if hex.len() < 2 || u8::from_str_radix(&hex, 16).is_err() {
                result.push('%');
                result.push_str(&hex);
            } else {
                result.push(u8::from_str_radix(&hex, 16).unwrap() as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}