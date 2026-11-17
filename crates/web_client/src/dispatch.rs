use crate::app::AppState;
use serde_json::{json, Value};

fn error(message: impl Into<String>) -> Value {
    json!({"ok":false,"error":message.into()})
}
fn text<'a>(params: &'a Value, name: &str) -> Result<&'a str, String> {
    params
        .get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("{name} is required"))
}
fn id<'a>(params: &'a Value, name: &str) -> Result<&'a str, String> {
    let value = text(params, name)?;
    if value.len() > 128
        || value.starts_with('.')
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        return Err(format!("invalid {name}"));
    }
    Ok(value)
}
fn web_url(value: &str) -> Result<(), String> {
    let url = url::Url::parse(value).map_err(|_| "invalid URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("URL must use http(s) without credentials".into());
    }
    Ok(())
}
/// Prevent an existing project or nested asset symlink from escaping its storage root.
fn project_paths(params: &Value) -> Result<(), String> {
    let project = id(params, "project")?;
    let base = qin_aegis_core::LocalStorage::projects_dir();
    let Ok(root) = base.canonicalize() else {
        return Ok(());
    };
    let mut path = base.join(project);
    // Legacy readers enumerate nested files directly. Reject symlinks before
    // delegating so a report/case leaf cannot bypass the directory checks.
    let mut pending = vec![path.clone()];
    let mut inspected = 0usize;
    while let Some(candidate) = pending.pop() {
        let metadata = match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err("unreadable project asset".into()),
        };
        inspected += 1;
        if inspected > 20_000 {
            return Err("project asset scan limit exceeded".into());
        }
        if metadata.file_type().is_symlink() {
            return Err("symlinked project assets are not supported".into());
        }
        if metadata.is_dir() {
            for entry in
                std::fs::read_dir(&candidate).map_err(|_| "unreadable project directory")?
            {
                pending.push(entry.map_err(|_| "unreadable project entry")?.path());
            }
        }
    }

    for part in [None, Some("reports"), Some("cases")] {
        let candidate = part.map(|p| path.join(p)).unwrap_or_else(|| path.clone());
        if candidate.exists()
            && !candidate
                .canonicalize()
                .map_err(|_| "unreadable project path")?
                .starts_with(&root)
        {
            return Err("project path escapes storage".into());
        }
    }
    if let Some(run) = params.get("run_id") {
        let run = run.as_str().ok_or("invalid run_id")?;
        path = path.join("reports").join(run);
        if path.exists()
            && !path
                .canonicalize()
                .map_err(|_| "unreadable run path")?
                .starts_with(&root)
        {
            return Err("run path escapes storage".into());
        }
    }
    Ok(())
}

pub(crate) fn validate_report_path(project: &str, run_id: &str) -> Result<(), String> {
    let params = json!({"project":project,"run_id":run_id});
    id(&params, "project")?;
    id(&params, "run_id")?;
    project_paths(&params)
}

pub fn dispatch(app: &mut AppState, method: &str, params: Value) -> Value {
    match dispatch_inner(app, method, &params) {
        Ok(value) => value,
        Err(message) => error(message),
    }
}
fn dispatch_inner(app: &mut AppState, method: &str, p: &Value) -> Result<Value, String> {
    if !p.is_object() {
        return Err("params must be an object".into());
    }
    if p.get("project").is_some() {
        id(p, "project")?;
    }
    if p.get("run_id").is_some() {
        id(p, "run_id")?;
    }
    if p.get("case_id").is_some() {
        id(p, "case_id")?;
    }
    if p.get("project").is_some() {
        project_paths(p)?;
    }
    if crate::report_maintenance::handles(method) {
        return crate::report_maintenance::dispatch(
            method,
            p,
            app.active_workers
                .load(std::sync::atomic::Ordering::Acquire),
        )
        .map_err(|error| error.to_string());
    }
    if crate::config_migration_api::handles(method) {
        if method == "applyConfigMigration"
            && app
                .active_workers
                .load(std::sync::atomic::Ordering::Acquire)
                != 0
        {
            return Err(
                "Wait for all active or cancelling tasks before configuration migration".into(),
            );
        }
        let result =
            crate::config_migration_api::dispatch(method, p).map_err(|error| error.to_string())?;
        if method == "applyConfigMigration" {
            app.config =
                Some(qin_aegis_core::AppConfig::load_global().map_err(|error| error.to_string())?);
        }
        return Ok(result);
    }
    if crate::case_asset_api::handles(method) {
        return crate::case_asset_api::dispatch(method, p).map_err(|e| e.to_string());
    }
    if crate::contract_api::handles(method) {
        return crate::contract_api::dispatch(method, p).map_err(|e| e.to_string());
    }
    if crate::model_api::handles(method) {
        return crate::model_api::dispatch(method, p).map_err(|e| e.to_string());
    }
    // Browser tasks share ports and resources. Keep one worker until the
    // persistent per-resource scheduler replaces this conservative admission rule.
    if matches!(
        method,
        "runExplore" | "runGenerate" | "runTests" | "generateAllureReport"
    ) && app
        .active_workers
        .load(std::sync::atomic::Ordering::Acquire)
        != 0
    {
        return Err("Another task is running or cancelling; wait for it to finish".into());
    }
    let raw = match method {
        "getState" => {
            let metadata =
                qin_aegis_core::AppConfig::inspect_at(&qin_aegis_core::AppConfig::global_path())
                    .map_err(|e| e.to_string())?;
            app.config = match qin_aegis_core::AppConfig::load_global() {
                Ok(config) => Some(config),
                Err(qin_aegis_core::config::app::ConfigError::NotFound(_)) => None,
                Err(error) => return Err(error.to_string()),
            };
            return Ok(
                json!({"ok":true,"config":serde_json::from_str::<Value>(&app.get_config()).map_err(|_|"config serialization failed")?,"config_revision":metadata.revision,"config_backup_available":metadata.backup_available,"currentView":app.current_view}),
            );
        }
        "restoreConfigBackup" => {
            if p.get("approved").and_then(Value::as_bool) != Some(true) {
                return Err("explicit restore approval required".into());
            }
            let revision = p
                .get("expected_revision")
                .and_then(Value::as_u64)
                .ok_or("expected_revision is required")?;
            let next = qin_aegis_core::AppConfig::restore_backup_at(
                &qin_aegis_core::AppConfig::global_path(),
                revision,
            )
            .map_err(|e| e.to_string())?;
            app.config = Some(qin_aegis_core::AppConfig::load_global().map_err(|e| e.to_string())?);
            return Ok(json!({"ok":true,"revision":next}));
        }
        "probeModel" => {
            let probe = crate::model_api::registry()
                .map_err(|e| e.to_string())?
                .prepare_probe(id(p, "profile_id")?)
                .map_err(|e| e.to_string())?;
            app.start_maintenance("modelProbe", move |cancel| {
                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
                let result = rt.block_on(async {
                    tokio::select! {
                        result = probe.run() => Ok(serde_json::to_value(result)?),
                        _ = crate::environment::cancelled(cancel) => Err(anyhow::anyhow!("Model probe cancelled")),
                    }
                });
                result
            })
        }
        "setConfig" => app.set_config(&p.to_string()),
        "diagnoseRuntime" => app.start_maintenance("runtimeCheck", crate::environment::diagnose),
        "installRuntimeBrowser" => {
            if p.get("approved").and_then(Value::as_bool) != Some(true) {
                return Err("explicit browser download approval required".into());
            }
            app.start_maintenance("browserInstall", crate::environment::install_browser)
        }
        "checkConfig" => return Ok(json!({"configured":app.is_configured()})),
        "getVersion" => app.get_version(),
        "checkUpdate" => app.check_update(),
        "getProjects" => app.get_projects(),
        "createProject" => {
            let name = id(p, "name")?;
            let target = text(p, "url")?;
            web_url(target)?;
            if qin_aegis_core::LocalStorage::project_dir(name).exists() {
                return Err("project already exists".into());
            }
            let stack = match p.get("tech_stack") {
                None => Vec::new(),
                Some(Value::Array(items)) => items
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .ok_or("tech_stack must contain strings".to_owned())
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                Some(_) => return Err("tech_stack must be an array".into()),
            };
            app.create_project(name, target, stack)
        }
        "getOutput" => app.get_output(),
        "clearOutput" => {
            app.clear_output();
            return Ok(json!({"ok":true}));
        }
        "runExplore" => {
            let target = text(p, "url")?;
            web_url(target)?;
            let depth = p
                .get("depth")
                .map(|v| {
                    v.as_u64()
                        .filter(|n| *n > 0 && *n <= 10)
                        .ok_or("depth must be 1..10")
                })
                .transpose()?
                .unwrap_or(3);
            app.run_explore(target, depth as u32)
        }
        "runGenerate" => {
            // External filesystem paths are not accepted over HTTP; project-bound specs follow in asset APIs.
            if p.get("spec").is_some_and(|v| !v.is_null()) {
                return Err("spec paths are not accepted; use project assets".into());
            }
            let project = id(p, "project")?;
            if !qin_aegis_core::LocalStorage::project_config_path(project).is_file() {
                return Err("project does not exist".into());
            }
            let use_contracts = p
                .get("use_contracts")
                .map(|v| v.as_bool().ok_or("use_contracts must be a boolean"))
                .transpose()?
                .unwrap_or(true);
            let snapshot = if use_contracts {
                Some(crate::contract_api::execution(project).map_err(|e| e.to_string())?)
            } else {
                None
            };
            app.run_generate(text(p, "requirement")?, project, snapshot)
        }
        "runTests" | "previewTestPlan" => {
            let project = id(p, "project")?;
            let kind = text(p, "type")?;
            if !matches!(kind, "smoke" | "functional" | "performance" | "stress") {
                return Err("unknown test type".into());
            }
            if method == "runTests" {
                app.run_tests(project, kind)
            } else {
                app.preview_test_plan(project, kind)
            }
        }
        "getCaseProvenance" => app.get_case_provenance(id(p, "project")?, id(p, "case_id")?),
        "getRunEvidence" => app.get_run_evidence(id(p, "project")?, id(p, "run_id")?),
        "getReports" => app.get_reports(id(p, "project")?),
        "getGateStatus" => app.get_gate_status(id(p, "project")?),
        "getReportHtml" => app.get_report_html(id(p, "project")?, id(p, "run_id")?),
        "generateAllureReport" => app.generate_allure_report(id(p, "project")?, id(p, "run_id")?),
        "getAllureReportHtml" => app.get_allure_report_html(id(p, "project")?, id(p, "run_id")?),
        "exportProject" => app.export_project(id(p, "project")?),
        "getReviewCases" => app.get_review_cases(id(p, "project")?),
        "getJobs" => app.get_jobs(),
        "getJobLog" => app.get_job_log(
            id(p, "jobId")?,
            p.get("cursor")
                .and_then(Value::as_u64)
                .ok_or("cursor must be non-negative integer")?,
        ),
        "getJobStatus" => app.get_job_status(id(p, "jobId")?),
        "stopJob" => app.stop_job(id(p, "jobId")?),
        _ => return Err(format!("unknown method: {method}")),
    };
    let mut value: Value =
        serde_json::from_str(&raw).map_err(|_| "invalid application response")?;
    // A terminal job's error is status data, allowing the browser to render its
    // failure. Errors without a job status are failed RPC operations.
    if !(matches!(method, "getJobStatus" | "getRunEvidence") && value.get("status").is_some())
        && value.get("error").is_some_and(|v| !v.is_null())
    {
        value["ok"] = json!(false);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_traversal_and_non_web_urls() {
        for name in [
            "..",
            "../secret",
            "a/b",
            "/tmp/x",
            "a\\b",
            "%2e%2e",
            ".hidden",
        ] {
            assert!(id(&json!({"name":name}), "name").is_err());
        }
        assert_eq!(
            id(&json!({"name":"my-project_2"}), "name").unwrap(),
            "my-project_2"
        );
        assert!(web_url("file:///tmp/a").is_err());
        assert!(web_url("https://key:secret@example.com").is_err());
        assert!(web_url("https://example.com").is_ok());
    }
    #[test]
    fn busy_workers_reject_new_tasks_before_starting_another() {
        let mut app = AppState::default();
        app.active_workers
            .store(1, std::sync::atomic::Ordering::Release);
        let response = dispatch(
            &mut app,
            "runExplore",
            json!({"url":"https://example.com","depth":1}),
        );
        assert_eq!(response["ok"], false);
        assert!(response["error"].as_str().unwrap().contains("Another task"));
        assert!(app.jobs.lock().unwrap().is_empty());
    }
    #[test]
    fn missing_values_and_types_are_not_silent_defaults() {
        assert!(text(&json!({}), "project").is_err());
        assert!(text(&json!({"project":22}), "project").is_err());
        assert!(text(&json!({"project":"   "}), "project").is_err());
    }
    #[test]
    fn job_polling_returns_failure_details_as_status_data() {
        let mut app = AppState::default();
        let mut job = crate::app::JobEntry::new("generate");
        app.jobs.lock().unwrap().insert("job-1".into(), job.clone());
        let running = dispatch(&mut app, "getJobStatus", json!({"jobId":"job-1"}));
        assert_eq!(running["status"], "running");
        assert!(running["error"].is_null());
        job.status = "failed".into();
        job.error = Some("model unavailable".into());
        app.jobs.lock().unwrap().insert("job-1".into(), job);
        let failed = dispatch(&mut app, "getJobStatus", json!({"jobId":"job-1"}));
        assert_eq!(failed["error"], "model unavailable");
        assert_ne!(failed["ok"], false);
        assert_eq!(
            dispatch(&mut app, "getJobStatus", json!({"jobId":"missing"}))["ok"],
            false
        );
    }
}
