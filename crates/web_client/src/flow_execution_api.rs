//! Explicit schema-2 admission. Frozen credentials never have a serialization path.
use crate::app::{AppState, JobReporter};
use anyhow::{ensure, Context, Result};
use qin_aegis_core::{
    contract_execution::ExecutionSnapshot,
    contract_store::ContractStore,
    execution_records::{RunRecord, RunStore},
    executor::{TestExecutor, TestResult},
    flow_execution::{
        runtime::{self, CaseRunner, ExecutionFacts, ExecutionObserver, Status},
        store::{timestamp, ExecutionDraft, ExecutionStore, FlowRun, Publication},
        Operation, PreparedCase,
    },
    protocol::LlmConfig,
    runtime::RuntimeResolver,
    LocalStorage,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub(crate) struct Prepared {
    publication: Publication,
    inputs: BTreeMap<String, Value>,
    cases: BTreeMap<String, PreparedCase>,
    models: BTreeMap<String, LlmConfig>,
    model_evidence: BTreeMap<String, Value>,
    secrets: Vec<String>,
    library_revision: u64,
    model_revision: u64,
    config_revision: u64,
    contract_revision: u64,
    runtime: Option<Value>,
    expires: Instant,
    metadata: Value,
    frozen: Value,
}
fn store() -> ExecutionStore {
    ExecutionStore::new(LocalStorage::base_path())
}
fn contracts() -> ContractStore {
    ContractStore::new(LocalStorage::base_path().join("contracts/library.json"))
}
fn parameters<T: DeserializeOwned>(v: &Value) -> Result<T> {
    serde_json::from_value(v.clone()).map_err(|_| anyhow::anyhow!("invalid execution parameters"))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Draft {
    draft: ExecutionDraft,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    draft: ExecutionDraft,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Publish {
    id: String,
    expected_revision: u64,
    approved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepare {
    id: String,
    version: u32,
    inputs: BTreeMap<String, Value>,
    permissions: Vec<String>,
    expected_revision: u64,
    expected_model_revision: u64,
    approved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Run {
    preparation_id: String,
    approved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Get {
    run_id: String,
}
pub fn handles(m: &str) -> bool {
    matches!(
        m,
        "getExecutionFlows"
            | "saveExecutionFlowDraft"
            | "validateExecutionFlowDraft"
            | "publishExecutionFlow"
            | "prepareFlowExecution"
            | "runFlowExecution"
            | "getFlowExecution"
    )
}

fn contains_secret(v: &Value, secrets: &[String]) -> bool {
    match v {
        Value::String(s) => secrets.iter().any(|key| !key.is_empty() && s.contains(key)),
        Value::Array(a) => a.iter().any(|v| contains_secret(v, secrets)),
        Value::Object(o) => o.iter().any(|(k, v)| {
            secrets.iter().any(|key| !key.is_empty() && k.contains(key))
                || contains_secret(v, secrets)
        }),
        _ => false,
    }
}
fn redact(v: &mut Value, secrets: &[String]) {
    match v {
        Value::String(s) => {
            for key in secrets {
                if !key.is_empty() {
                    *s = s.replace(key, "[redacted]");
                }
            }
        }
        Value::Array(a) => {
            for v in a {
                redact(v, secrets)
            }
        }
        Value::Object(o) => {
            let old = std::mem::take(o);
            for (mut k, mut v) in old {
                for key in secrets {
                    if !key.is_empty() {
                        k = k.replace(key, "[redacted]");
                    }
                }
                redact(&mut v, secrets);
                o.insert(k, v);
            }
        }
        _ => {}
    }
}
fn clean<T: serde::Serialize + DeserializeOwned>(value: &T, secrets: &[String]) -> Result<T> {
    let mut v = serde_json::to_value(value)?;
    redact(&mut v, secrets);
    Ok(serde_json::from_value(v)?)
}
fn runtime_identity() -> Result<Value> {
    let r = RuntimeResolver::resolve()?;
    Ok(json!({"directory":r.directory,"node":r.node,"arguments":r.arguments,"version":r.version}))
}
fn live_preparations(app: &mut AppState) {
    app.flow_preparations
        .retain(|_, p| p.expires > Instant::now());
}

pub fn dispatch(app: &mut AppState, method: &str, value: &Value) -> Result<Value> {
    live_preparations(app);
    match method {
        "getExecutionFlows" => {
            let _: Empty = parameters(value)?;
            let runs=store().list()?.into_iter().map(|r|json!({"run_id":r.run_id,"job_id":r.job_id,"flow_id":r.flow_id,"version":r.version,"project":r.project,"status":r.status,"created_at":r.created_at})).collect::<Vec<_>>();
            Ok(
                json!({"library":store().snapshot()?,"model_revision":crate::model_api::registry()?.snapshot()?.revision,"run_history":runs,"preparations":app.flow_preparations.values().map(|p|{let mut metadata=p.metadata.clone(); metadata["expires_in_seconds"]=json!(p.expires.saturating_duration_since(Instant::now()).as_secs()); metadata}).collect::<Vec<_>>()}),
            )
        }
        "saveExecutionFlowDraft" => {
            let p: Save = parameters(value)?;
            Ok(serde_json::to_value(
                store().save(p.draft, p.expected_revision)?,
            )?)
        }
        "validateExecutionFlowDraft" => {
            let p: Draft = parameters(value)?;
            Ok(json!({"execution_order":p.draft.validate()?}))
        }
        "publishExecutionFlow" => {
            let p: Publish = parameters(value)?;
            ensure!(p.approved, "explicit publication approval required");
            Ok(serde_json::to_value(
                store().publish(&p.id, p.expected_revision)?,
            )?)
        }
        "prepareFlowExecution" => prepare(app, parameters(value)?),
        "runFlowExecution" => start(app, parameters(value)?),
        "getFlowExecution" => {
            let p: Get = parameters(value)?;
            Ok(serde_json::to_value(store().load(&p.run_id)?)?)
        }
        _ => anyhow::bail!("unknown execution operation"),
    }
}
fn prepare(app: &mut AppState, request: Prepare) -> Result<Value> {
    ensure!(
        request.approved,
        "explicit execution preparation approval required"
    );
    ensure!(
        !app.resource_blocked.load(Ordering::Acquire),
        "Owned resource cleanup is unconfirmed"
    );
    ensure!(
        app.flow_preparations.len() < 16,
        "too many active preparations"
    );
    let library = store().snapshot()?;
    ensure!(
        library.revision == request.expected_revision,
        "execution library revision conflict"
    );
    let publication = library.publication(&request.id, request.version)?.clone();
    let d = &publication.draft.definition;
    let order = publication.draft.validate()?;
    d.validate_inputs(&request.inputs)?;
    let mut supplied = request.permissions.clone();
    supplied.sort();
    let mut expected = d.required_permissions.clone();
    expected.sort();
    ensure!(
        supplied == expected,
        "exact declared data and execution permissions must be authorized"
    );
    let project = LocalStorage::checked_project_config(&d.project)?;
    let registry = crate::model_api::registry()?;
    ensure!(
        registry.snapshot()?.revision == request.expected_model_revision,
        "model registry revision conflict"
    );
    let config_revision =
        qin_aegis_core::AppConfig::inspect_at(&qin_aegis_core::AppConfig::global_path())?.revision;
    let contract_library = contracts().snapshot()?;
    let has_cases = !publication.draft.model_profiles.is_empty();
    let runtime = if has_cases {
        Some(runtime_identity()?)
    } else {
        None
    };
    let snapshot = if has_cases {
        Some(ExecutionSnapshot::prepare(
            &contract_library,
            &d.project,
            &[
                "browser.interact".into(),
                "browser.assert".into(),
                "browser.visual".into(),
            ],
        )?)
    } else {
        None
    };
    let mut cases = BTreeMap::new();
    let mut models = BTreeMap::new();
    let mut model_evidence = BTreeMap::new();
    let mut public_models = BTreeMap::new();
    let mut secrets = vec![];
    if let Some(config) = &app.config {
        for key in [&config.llm.api_key, &config.llm.secondary_api_key] {
            let resolved = qin_aegis_core::config::app::resolve_env_var(key);
            if !resolved.is_empty() {
                secrets.push(resolved);
            }
        }
    }
    for step in &d.steps {
        if let Operation::ExecuteCase { case } = &step.operation {
            let prepared = PreparedCase::prepare(
                case,
                &project,
                &project.url,
                snapshot.as_ref().context("missing contract snapshot")?,
            )?;
            let profile_id = &publication.draft.model_profiles[&step.id];
            let route =
                registry.resolve_execution_profile(profile_id, request.expected_model_revision)?;
            let profile = route
                .candidates()
                .first()
                .context("empty model selection")?;
            secrets.push(profile.api_key.clone());
            models.insert(
                step.id.clone(),
                LlmConfig {
                    api_key: profile.api_key.clone(),
                    base_url: profile.base_url.clone(),
                    model: profile.model.clone(),
                    midscene_family: profile.midscene_family.clone(),
                },
            );
            public_models.insert(step.id.clone(),json!({"profile_id":profile.id,"adapter":profile.adapter,"model":profile.model,"base_url":profile.base_url,"midscene_family":profile.midscene_family}));
            model_evidence.insert(step.id.clone(),json!({"revision":route.revision(),"task":"execute","candidates":[{"id":profile.id,"adapter":profile.adapter,"model":profile.model,"capabilities":profile.capabilities,"midscene_family":profile.midscene_family}]}));
            cases.insert(step.id.clone(), prepared);
        }
    }
    secrets.retain(|s| !s.is_empty());
    secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
    secrets.dedup();
    let id = format!("prep-{}", crate::server::random_token()?);
    let mut metadata = json!({"preparation_id":id,"flow_id":d.id,"version":publication.version,"project":d.project,"target_url":project.url,"execution_order":order,"models":public_models,"expires_in_seconds":600,"permissions":request.permissions});
    let frozen = json!({"publication":publication,"inputs":request.inputs,"cases":cases,"models":public_models,"permissions":request.permissions,"target_url":project.url,"library_revision":library.revision,"model_revision":request.expected_model_revision,"contract_revision":contract_library.revision,"runtime":runtime});
    // Reject collisions rather than editing approved assertions, targets or input semantics.
    ensure!(!contains_secret(&frozen,&secrets)&&!contains_secret(&metadata,&secrets),"reviewed content contains a configured credential; replace it with non-secret test data before preparation");
    metadata["snapshot"] = frozen.clone();
    let prepared = Prepared {
        publication,
        inputs: request.inputs,
        cases,
        models,
        model_evidence,
        secrets,
        library_revision: library.revision,
        model_revision: request.expected_model_revision,
        config_revision,
        contract_revision: contract_library.revision,
        runtime,
        expires: Instant::now() + Duration::from_secs(600),
        metadata: metadata.clone(),
        frozen,
    };
    revalidate(&prepared)?;
    app.flow_preparations.insert(id, prepared);
    Ok(metadata)
}
fn revalidate(p: &Prepared) -> Result<()> {
    ensure!(
        p.expires > Instant::now(),
        "execution preparation expired; prepare again"
    );
    ensure!(
        store().snapshot()?.revision == p.library_revision,
        "execution library changed; prepare again"
    );
    ensure!(
        crate::model_api::registry()?.snapshot()?.revision == p.model_revision,
        "model registry changed; prepare again"
    );
    ensure!(
        qin_aegis_core::AppConfig::inspect_at(&qin_aegis_core::AppConfig::global_path())?.revision
            == p.config_revision,
        "application configuration changed; prepare again"
    );
    let library = contracts().snapshot()?;
    ensure!(
        library.revision == p.contract_revision,
        "contract library changed; prepare again"
    );
    let project = LocalStorage::checked_project_config(&p.publication.draft.definition.project)?;
    ensure!(
        p.frozen["target_url"] == project.url,
        "project target changed; prepare again"
    );
    if let Some(runtime) = &p.runtime {
        ensure!(
            *runtime == runtime_identity()?,
            "runtime changed; prepare again"
        );
    }
    for case in p.cases.values() {
        let current =
            PreparedCase::prepare(case.reference(), &project, &project.url, case.contracts())?;
        ensure!(
            serde_json::to_value(&current)? == serde_json::to_value(case)?,
            "reviewed case changed; prepare again"
        );
    }
    Ok(())
}
struct PendingRun {
    id: String,
    blocked: Arc<AtomicBool>,
    armed: bool,
    cleanup_confirmed: bool,
}
impl Drop for PendingRun {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let store = store();
        let fallback = (|| -> Result<()> {
            let record = store.load(&self.id)?;
            if record.status == "running" && record.has_case_attempt() && !self.cleanup_confirmed {
                self.blocked.store(true, Ordering::Release);
                store.mark_quarantine(&self.id)?;
            }
            if matches!(record.status.as_str(), "queued" | "running") {
                store.finish(
                    &self.id,
                    "interrupted",
                    None,
                    Some(
                        "Worker did not commit a terminal result; execution was not replayed"
                            .into(),
                    ),
                )?;
            }
            Ok(())
        })();
        if fallback.is_err() {
            // The absence of a durable terminal fact is independent of actual
            // resource exit. Never release subsequent side effects on uncertainty.
            self.blocked.store(true, Ordering::Release);
            let _ = store.mark_persistence_block(&self.id);
        }
    }
}

struct Observer {
    id: String,
    secrets: Vec<String>,
}
impl ExecutionObserver for Observer {
    fn checkpoint(&self, facts: &ExecutionFacts) -> Result<()> {
        store().checkpoint(&self.id, clean(facts, &self.secrets)?)
    }
}
fn start(app: &mut AppState, request: Run) -> Result<Value> {
    ensure!(request.approved, "explicit execution approval required");
    let prepared = app
        .flow_preparations
        .get(&request.preparation_id)
        .context("execution preparation missing or consumed")?;
    revalidate(prepared)?;
    let id = format!("flow-{}", crate::server::random_token()?);
    let job = app.job_id();
    let now = timestamp();
    let record = FlowRun {
        schema_version: 2,
        run_id: id.clone(),
        job_id: Some(job.clone()),
        flow_id: prepared.publication.draft.definition.id.clone(),
        version: prepared.publication.version,
        project: prepared.publication.draft.definition.project.clone(),
        status: "queued".into(),
        created_at: now.clone(),
        updated_at: now,
        preparation: prepared.frozen.clone(),
        case_runs: BTreeMap::new(),
        facts: None,
        error: None,
    };
    store().begin(&record)?;
    let pending = PendingRun {
        id: id.clone(),
        blocked: app.resource_blocked.clone(),
        armed: true,
        cleanup_confirmed: false,
    };
    let prepared = app
        .flow_preparations
        .remove(&request.preparation_id)
        .context("execution preparation missing")?;
    let blocked = app.resource_blocked.clone();
    let run_id = id.clone();
    let secrets = prepared.secrets.clone();
    let response: Value = serde_json::from_str(&app.start_private_work(
        job,
        "flowExecution",
        secrets,
        Some(json!({"run_id":id,"flow_id":record.flow_id,"version":record.version})),
        move |cancel, report| {
            let mut pending = pending;
            let observer = Observer {
                id: run_id.clone(),
                secrets: prepared.secrets.clone(),
            };
            let mut runner = FrozenRunner::new(&prepared, &run_id, cancel.clone(), report);
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let result = rt.block_on(runtime::run_observed(
                &prepared.publication.draft.definition,
                prepared.inputs.clone(),
                &prepared.cases,
                &mut runner,
                cancel,
                &observer,
            ));
            let facts = match result {
                Ok(f) => clean(&f, &prepared.secrets)?,
                Err(_) => {
                    store().finish(
                        &run_id,
                        "failed",
                        None,
                        Some("Execution kernel rejected frozen inputs".into()),
                    )?;
                    pending.armed = false;
                    anyhow::bail!("Execution kernel rejected frozen inputs")
                }
            };
            if facts.cleanup_error.is_some() {
                blocked.store(true, Ordering::Release);
                store().mark_quarantine(&run_id)?;
            }
            pending.cleanup_confirmed = facts.cleanup_error.is_none();
            let status = match facts.status {
                Status::Succeeded => "completed",
                Status::Cancelled => "stopped",
                _ => "failed",
            };
            let error = facts.error.clone().or_else(|| facts.cleanup_error.clone());
            // Actual case evidence IDs are reported separately from kernel outputs.
            let case_runs = runner.case_runs.clone();
            store().finish(&run_id, status, Some(facts), error.clone())?;
            pending.armed = false;
            ensure!(
                status == "completed",
                "Flow execution {status}; inspect private execution facts"
            );
            Ok(json!({"run_id":run_id,"status":status,"case_runs":case_runs}))
        },
    ))?;
    if let Some(error) = response.get("error").and_then(Value::as_str) {
        anyhow::bail!("{error}");
    }
    Ok(json!({"jobId":response["jobId"],"run_id":id}))
}

struct FrozenRunner {
    models: BTreeMap<String, LlmConfig>,
    model_evidence: BTreeMap<String, Value>,
    secrets: Vec<String>,
    project: String,
    flow_run: String,
    cancel: Arc<AtomicBool>,
    report: JobReporter,
    executor: Option<TestExecutor>,
    active_record: Option<(RunStore, RunRecord, std::path::PathBuf)>,
    case_runs: BTreeMap<String, Value>,
}
impl FrozenRunner {
    fn new(p: &Prepared, id: &str, cancel: Arc<AtomicBool>, report: JobReporter) -> Self {
        Self {
            models: p.models.clone(),
            model_evidence: p.model_evidence.clone(),
            secrets: p.secrets.clone(),
            project: p.publication.draft.definition.project.clone(),
            flow_run: id.into(),
            cancel,
            report,
            executor: None,
            active_record: None,
            case_runs: BTreeMap::new(),
        }
    }
    fn finish_case(
        &mut self,
        status: &str,
        results: Vec<TestResult>,
        error: Option<String>,
    ) -> Result<()> {
        if let Some((store, record, dir)) = self.active_record.as_ref() {
            let results = clean(&results, &self.secrets)?;
            let error = clean(&error, &self.secrets)?;
            let committed = store
                .finish(status, results, error)
                .context("cannot commit case evidence")?;
            let allure =
                qin_aegis_core::allure::export_run(&committed, &dir.join("allure-results")).is_ok();
            (self.report)(&format!(
                "Case evidence {} saved; Allure results ready: {allure}\n",
                record.run_id
            ));
            for value in self.case_runs.values_mut() {
                if value["run_id"] == record.run_id {
                    value["status"] = json!(status);
                    value["allure_results_ready"] = json!(allure);
                }
            }
            for (step, value) in &self.case_runs {
                ExecutionStore::new(LocalStorage::base_path()).case_evidence(
                    &self.flow_run,
                    step,
                    value.clone(),
                )?;
            }
            self.active_record = None;
        }
        Ok(())
    }
}
#[runtime::async_trait]
impl CaseRunner for FrozenRunner {
    async fn run_case(&mut self, _: &PreparedCase) -> Result<TestResult> {
        anyhow::bail!("explicit step selection required")
    }
    async fn run_step(&mut self, step: &str, case: &PreparedCase) -> Result<TestResult> {
        ensure!(
            self.executor.is_none() && self.active_record.is_none(),
            "prior case resources were not released"
        );
        let run_id = format!("case-{}", crate::server::random_token()?);
        let dir = LocalStorage::run_dir(&self.project, &run_id);
        let case_store = RunStore::new(dir.clone());
        let engine_version = RuntimeResolver::resolve()?.version;
        let record = RunRecord::prepare(
            run_id.clone(),
            self.project.clone(),
            "functional".into(),
            self.model_evidence[step].clone(),
            Some(engine_version),
            vec![case.run_case()],
        )?;
        // Journal the complete frozen case intent before creating its separate
        // evidence file, so crashes at either boundary have a recoverable owner.
        self.case_runs.insert(step.into(),json!({"run_id":run_id,"project":self.project,"status":"running","allure_results_ready":false,"record":record}));
        store().case_evidence(&self.flow_run, step, self.case_runs[step].clone())?;
        case_store.begin(&record)?;
        self.active_record = Some((case_store, record, dir));
        self.executor = Some(
            TestExecutor::new_cancellable(
                1,
                Some(self.models[step].clone()),
                None,
                self.cancel.clone(),
            )
            .await?,
        );
        let result =
            CaseRunner::run_case(self.executor.as_mut().context("engine unavailable")?, case).await;
        if self.executor.as_ref().unwrap().terminate().await.is_err() {
            let mut retained = vec![];
            let mut business_failed = false;
            if let Ok(output) = &result {
                runtime::validate_case_result_budget(output)?;
                ensure!(
                    output.case_id == case.reference().case_id
                        && output.retry_count == 0
                        && output.healed_yaml_script.is_none(),
                    "engine changed reviewed case identity or healing policy"
                );
                business_failed = !output.passed;
                retained.push(output.clone());
            }
            self.finish_case(
                "failed",
                retained,
                Some("Owned engine cleanup is unconfirmed".into()),
            )?;
            if business_failed {
                anyhow::bail!(
                    "Reviewed case business assertions failed; owned engine exit is unconfirmed"
                );
            }
            anyhow::bail!("Owned engine exit is unconfirmed; inspect case evidence");
        }
        self.executor = None;
        match result {
            Ok(result) => {
                runtime::validate_case_result_budget(&result)?;
                ensure!(
                    result.case_id == case.reference().case_id
                        && result.retry_count == 0
                        && result.healed_yaml_script.is_none(),
                    "engine changed reviewed case identity or healing policy"
                );
                self.finish_case("completed", vec![result.clone()], None)?;
                Ok(result)
            }
            Err(_) => {
                self.finish_case(
                    "failed",
                    vec![],
                    Some("Case engine failed; no automatic retry was attempted".into()),
                )?;
                anyhow::bail!("Case engine failed; inspect case evidence")
            }
        }
    }
    async fn cleanup(&mut self) -> Result<()> {
        if let Some(executor) = self.executor.as_ref() {
            if executor.terminate().await.is_err() {
                self.finish_case(
                    "failed",
                    vec![],
                    Some("Owned engine cleanup is unconfirmed".into()),
                )?;
                anyhow::bail!("Owned engine cleanup is unconfirmed");
            }
            self.executor = None;
        }
        self.finish_case(
            "stopped",
            vec![],
            Some("Case did not produce a committed result".into()),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redaction_covers_all_frozen_keys_and_keys_in_dynamic_objects() {
        let keys = vec!["first-private-key".into(), "second-private-key".into()];
        let mut value =
            json!({"first-private-key":{"nested":["second-private-key","first-private-key"]}});
        assert!(contains_secret(&value, &keys));
        redact(&mut value, &keys);
        assert!(!contains_secret(&value, &keys));
        assert_eq!(value["[redacted]"]["nested"][0], "[redacted]");
    }
    #[test]
    fn unconfirmed_cleanup_blocks_queued_work_and_runtime_maintenance() {
        let mut app = AppState::default();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let first: Value = serde_json::from_str(&app.start_maintenance("fixture", move |_| {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3))?;
            Ok(json!({}))
        }))
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let captured = calls.clone();
        let second: Value = serde_json::from_str(&app.start_maintenance("fixture", move |_| {
            captured.fetch_add(1, Ordering::AcqRel);
            Ok(json!({}))
        }))
        .unwrap();
        app.resource_blocked.store(true, Ordering::Release);
        release_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.active_workers.load(Ordering::Acquire) != 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(app.active_workers.load(Ordering::Acquire), 0);
        assert_eq!(calls.load(Ordering::Acquire), 0);
        assert_eq!(
            app.jobs.lock().unwrap()[second["jobId"].as_str().unwrap()].status,
            "failed"
        );
        assert!(app
            .jobs
            .lock()
            .unwrap()
            .contains_key(first["jobId"].as_str().unwrap()));
        assert!(app.start_runtime_maintenance(|_, _| Ok(json!({}))).is_err());
        let rejected: Value =
            serde_json::from_str(&app.start_maintenance("fixture", |_| Ok(json!({})))).unwrap();
        assert!(rejected["error"].as_str().unwrap().contains("cleanup"));
    }
}
