//! Private historical inputs and execution evidence. Provenance is not a claim
//! that generated code implements every business assertion.
use crate::{asset_file::AssetFile, contract_execution::ExecutionSnapshot, executor::TestResult};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeSet, path::PathBuf};

const MAX_BYTES: usize = 32 * 1024 * 1024;

fn identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 128
            && !value.starts_with('.')
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_')),
        "invalid record identity"
    );
    Ok(())
}
fn nonempty(value: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "required record field is empty");
    Ok(())
}

/// Only metadata actually emitted by the application's redacted snapshot is
/// accepted. Unknown fields (including keys or endpoints) fail closed.
fn validate_model(model: &Value) -> Result<()> {
    let object = model.as_object().context("invalid model snapshot")?;
    if object.get("source").is_some() {
        ensure!(
            object.len() == 2
                && object.get("source").and_then(Value::as_str) == Some("legacy_config"),
            "invalid legacy model snapshot"
        );
        nonempty(
            object
                .get("model")
                .and_then(Value::as_str)
                .context("model name missing")?,
        )?;
    } else {
        ensure!(
            object.len() == 3 && object.get("revision").and_then(Value::as_u64).is_some(),
            "invalid model snapshot revision"
        );
        ensure!(
            matches!(
                object.get("task").and_then(Value::as_str),
                Some("generate" | "explore" | "execute" | "correct")
            ),
            "invalid model task"
        );
        let candidates = object
            .get("candidates")
            .and_then(Value::as_array)
            .context("model candidates missing")?;
        ensure!(
            !candidates.is_empty() && candidates.len() <= 4,
            "invalid model candidates"
        );
        let mut ids = BTreeSet::new();
        for candidate in candidates {
            let c = candidate.as_object().context("invalid model candidate")?;
            ensure!(
                c.len() == 5
                    && c.keys().all(|k| matches!(
                        k.as_str(),
                        "id" | "adapter" | "model" | "capabilities" | "midscene_family"
                    )),
                "unexpected model candidate field"
            );
            for key in ["id", "adapter", "model"] {
                nonempty(
                    c.get(key)
                        .and_then(Value::as_str)
                        .context("model metadata missing")?,
                )?;
            }
            ensure!(
                ids.insert(c["id"].as_str().unwrap()),
                "duplicate model candidate"
            );
            ensure!(
                c["capabilities"].as_array().is_some_and(|a| a
                    .iter()
                    .all(|v| matches!(v.as_str(), Some("text" | "vision" | "json" | "tools")))),
                "invalid capabilities"
            );
            ensure!(
                c["midscene_family"].is_null()
                    || c["midscene_family"]
                        .as_str()
                        .is_some_and(|s| !s.trim().is_empty()),
                "invalid model family"
            );
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct GenerationContext {
    pub contracts: Option<ExecutionSnapshot>,
    pub model: Value,
}
impl GenerationContext {
    pub fn validate(&self, project: &str) -> Result<()> {
        identifier(project)?;
        validate_model(&self.model)?;
        if let Some(snapshot) = &self.contracts {
            let _: ExecutionSnapshot = serde_json::from_value(serde_json::to_value(snapshot)?)?;
            ensure!(
                snapshot.project() == project,
                "contract snapshot belongs to another project"
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseProvenance {
    pub schema_version: u32,
    pub project: String,
    pub case_id: String,
    pub yaml_script: String,
    pub created_at: String,
    pub contracts: Option<ExecutionSnapshot>,
    pub model: Value,
}
impl CaseProvenance {
    pub fn validate(&self, project: &str, case_id: &str, script: &str) -> Result<()> {
        ensure!(self.schema_version == 1, "unsupported provenance schema");
        identifier(case_id)?;
        identifier(&self.case_id)?;
        ensure!(
            self.project == project && self.case_id == case_id && self.yaml_script == script,
            "case provenance differs from project, identity or script; regenerate and review"
        );
        nonempty(script)?;
        nonempty(&self.created_at)?;
        GenerationContext {
            contracts: self.contracts.clone(),
            model: self.model.clone(),
        }
        .validate(project)
    }
}
pub struct CaseProvenanceStore {
    directory: PathBuf,
}
impl CaseProvenanceStore {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
    fn file(&self, id: &str) -> Result<AssetFile> {
        identifier(id)?;
        Ok(AssetFile::new(
            self.directory.join(format!("{id}.json")),
            MAX_BYTES,
        ))
    }
    pub fn save(&self, provenance: &CaseProvenance) -> Result<()> {
        provenance.validate(
            &provenance.project,
            &provenance.case_id,
            &provenance.yaml_script,
        )?;
        let file = self.file(&provenance.case_id)?;
        let guard = file.lock()?;
        ensure!(guard.read()?.is_none(), "case provenance already exists");
        guard.write(&serde_json::to_vec_pretty(provenance)?)
    }
    pub fn load(
        &self,
        project: &str,
        case_id: &str,
        script: &str,
    ) -> Result<Option<CaseProvenance>> {
        identifier(project)?;
        let file = self.file(case_id)?;
        let guard = file.lock()?;
        let Some(bytes) = guard.read()? else {
            return Ok(None);
        };
        let record: CaseProvenance =
            serde_json::from_slice(&bytes).context("invalid case provenance")?;
        record.validate(project, case_id, script)?;
        Ok(Some(record))
    }
}

/// Read embedded source first; old sidecars remain compatible but are never newly generated.
pub fn case_provenance(
    project: &str,
    case: &crate::storage::TestCase,
) -> Result<Option<CaseProvenance>> {
    if let Some(source) = &case.provenance {
        source.validate(project, &case.id, &case.yaml_script)?;
        return Ok(Some(source.clone()));
    }
    CaseProvenanceStore::new(crate::storage::LocalStorage::project_dir(project).join("provenance"))
        .load(project, &case.id, &case.yaml_script)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunCase {
    pub case_id: String,
    pub name: String,
    pub yaml_script: String,
    pub target_url: String,
    pub provenance: Option<CaseProvenance>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRecord {
    pub schema_version: u32,
    pub run_id: String,
    pub project: String,
    pub test_type: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub model: Value,
    pub engine_version: Option<String>,
    pub cases: Vec<RunCase>,
    pub results: Vec<TestResult>,
    pub error: Option<String>,
    pub semantic_validation: String,
}
impl RunRecord {
    pub fn prepare(
        run_id: String,
        project: String,
        test_type: String,
        model: Value,
        engine_version: Option<String>,
        cases: Vec<RunCase>,
    ) -> Result<Self> {
        let record = Self {
            schema_version: 1,
            run_id,
            project,
            test_type,
            status: "running".into(),
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            model,
            engine_version,
            cases,
            results: vec![],
            error: None,
            semantic_validation: "not_verified".into(),
        };
        record.validate()?;
        Ok(record)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1 && self.semantic_validation == "not_verified",
            "unsupported run schema or semantic validation claim"
        );
        identifier(&self.project)?;
        identifier(&self.run_id)?;
        nonempty(&self.started_at)?;
        ensure!(
            matches!(
                self.test_type.as_str(),
                "smoke" | "functional" | "performance" | "stress" | "security"
            ),
            "invalid test type"
        );
        validate_model(&self.model)?;
        ensure!(
            !self.cases.is_empty() && self.cases.len() <= 200,
            "run must contain 1..200 cases"
        );
        let mut ids = BTreeSet::new();
        for case in &self.cases {
            identifier(&case.case_id)?;
            nonempty(&case.name)?;
            nonempty(&case.yaml_script)?;
            let url = reqwest::Url::parse(&case.target_url)?;
            ensure!(
                matches!(url.scheme(), "http" | "https")
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none(),
                "invalid case target URL"
            );
            ensure!(ids.insert(case.case_id.as_str()), "duplicate run case");
            if let Some(p) = &case.provenance {
                p.validate(&self.project, &case.case_id, &case.yaml_script)?;
            }
        }
        let mut results = BTreeSet::new();
        for result in &self.results {
            ensure!(
                ids.contains(result.case_id.as_str()) && results.insert(result.case_id.as_str()),
                "foreign or duplicate result identity"
            );
        }
        match self.status.as_str() {
            "running" => ensure!(
                self.finished_at.is_none() && self.results.is_empty() && self.error.is_none(),
                "invalid active run"
            ),
            "completed" => ensure!(
                self.finished_at
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty())
                    && self.error.is_none()
                    && results == ids,
                "completed run needs exact result coverage"
            ),
            "failed" | "stopped" => ensure!(
                self.finished_at
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty()),
                "terminal run needs finish time"
            ),
            _ => anyhow::bail!("invalid run status"),
        }
        Ok(())
    }
}
pub struct RunStore {
    file: AssetFile,
}
impl RunStore {
    pub fn new(run_dir: PathBuf) -> Self {
        Self {
            file: AssetFile::new(run_dir.join("run.json"), MAX_BYTES),
        }
    }
    pub fn begin(&self, record: &RunRecord) -> Result<()> {
        record.validate()?;
        ensure!(record.status == "running", "new run must be running");
        let guard = self.file.lock()?;
        ensure!(guard.read()?.is_none(), "run already exists");
        guard.write(&serde_json::to_vec_pretty(record)?)
    }
    pub fn load(&self) -> Result<RunRecord> {
        let bytes = self
            .file
            .lock()?
            .read()?
            .context("run evidence not found")?;
        let record: RunRecord = serde_json::from_slice(&bytes)?;
        record.validate()?;
        Ok(record)
    }
    pub fn finish(
        &self,
        status: &str,
        results: Vec<TestResult>,
        error: Option<String>,
    ) -> Result<RunRecord> {
        ensure!(
            matches!(status, "completed" | "failed" | "stopped"),
            "finish requires terminal status"
        );
        let guard = self.file.lock()?;
        let mut record: RunRecord =
            serde_json::from_slice(&guard.read()?.context("run evidence not found")?)?;
        record.validate()?;
        ensure!(record.status == "running", "terminal evidence is immutable");
        record.status = status.into();
        record.results = results;
        record.error = error;
        record.finished_at = Some(chrono::Utc::now().to_rfc3339());
        record.validate()?;
        guard.write(&serde_json::to_vec_pretty(&record)?)?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn provenance() -> CaseProvenance {
        CaseProvenance {
            schema_version: 1,
            project: "alpha".into(),
            case_id: "receipt".into(),
            yaml_script: "- aiAssert: receipt".into(),
            created_at: "2026-11-16T00:00:00Z".into(),
            contracts: None,
            model: json!({"source":"legacy_config","model":"fixture"}),
        }
    }
    fn run() -> RunRecord {
        let p = provenance();
        RunRecord {
            schema_version: 1,
            run_id: "run-1".into(),
            project: "alpha".into(),
            test_type: "smoke".into(),
            status: "running".into(),
            started_at: p.created_at.clone(),
            finished_at: None,
            model: p.model.clone(),
            engine_version: Some("fixture".into()),
            cases: vec![RunCase {
                case_id: p.case_id.clone(),
                name: "Receipt".into(),
                yaml_script: p.yaml_script.clone(),
                target_url: "http://127.0.0.1:1234".into(),
                provenance: Some(p),
            }],
            results: vec![],
            error: None,
            semantic_validation: "not_verified".into(),
        }
    }
    fn result(id: &str) -> TestResult {
        TestResult {
            case_id: id.into(),
            passed: false,
            duration_ms: 1,
            screenshot_base64: None,
            error_message: Some("business assertion failed".into()),
            retry_count: 0,
            healed_yaml_script: None,
            failed_step: Some("aiAssert".into()),
        }
    }
    #[test]
    fn provenance_is_private_immutable_and_bound_to_exact_script() {
        let dir = tempfile::tempdir().unwrap();
        let store = CaseProvenanceStore::new(dir.path().join("provenance"));
        let p = provenance();
        assert!(store.load("alpha", "receipt", "old").unwrap().is_none());
        store.save(&p).unwrap();
        assert!(store.save(&p).is_err());
        assert!(store.load("beta", "receipt", &p.yaml_script).is_err());
        assert!(store.load("alpha", "receipt", "weakened").is_err());
        assert!(store.load("alpha", "../escape", &p.yaml_script).is_err());
        assert_eq!(
            store
                .load("alpha", "receipt", &p.yaml_script)
                .unwrap()
                .unwrap()
                .yaml_script,
            p.yaml_script
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(dir.path().join("provenance/receipt.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn no_model_secrets_or_unknown_candidate_fields() {
        let mut p = provenance();
        p.model["api_key"] = json!("secret");
        assert!(p.validate("alpha", "receipt", &p.yaml_script).is_err());
        p.model = json!({"revision":1,"task":"generate","candidates":[{"id":"one","adapter":"openai_compatible","model":"m","capabilities":["text"],"midscene_family":null}]});
        p.validate("alpha", "receipt", &p.yaml_script).unwrap();
        p.model["candidates"][0]["api_key"] = json!("secret");
        assert!(p.validate("alpha", "receipt", &p.yaml_script).is_err());
    }
    #[test]
    fn run_freezes_inputs_rejects_duplicate_and_incomplete_results() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunStore::new(dir.path().join("run"));
        let original = run();
        store.begin(&original).unwrap();
        assert!(store.begin(&original).is_err());
        assert!(store.finish("completed", vec![], None).is_err());
        assert!(store
            .finish(
                "completed",
                vec![result("receipt"), result("receipt")],
                None
            )
            .is_err());
        assert!(store
            .finish("failed", vec![result("foreign")], Some("engine".into()))
            .is_err());
        assert_eq!(store.load().unwrap().status, "running");
        let finished = store
            .finish("completed", vec![result("receipt")], None)
            .unwrap();
        assert!(!finished.results[0].passed);
        assert_eq!(finished.cases[0].yaml_script, original.cases[0].yaml_script);
        assert_eq!(finished.semantic_validation, "not_verified");
        assert!(store
            .finish("completed", vec![result("receipt")], None)
            .is_err());
        assert_eq!(
            RunStore::new(dir.path().join("run")).load().unwrap().status,
            "completed"
        );
    }
    #[test]
    fn failure_and_stopping_allow_partial_evidence_and_writes_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let store = RunStore::new(dir.path().join("run"));
        store.begin(&run()).unwrap();
        assert_eq!(
            store
                .finish("failed", vec![], Some("engine failed".into()))
                .unwrap()
                .results
                .len(),
            0
        );
        let blocked = dir.path().join("blocked");
        std::fs::write(&blocked, "file").unwrap();
        assert!(RunStore::new(blocked).begin(&run()).is_err());
        let mut empty = run();
        empty.cases.clear();
        assert!(empty.validate().is_err());
        let mut altered = run();
        altered.cases[0].yaml_script = "skip assertion".into();
        assert!(altered.validate().is_err());
    }
    #[test]
    fn prepared_run_and_stopped_evidence_preserve_original_file() {
        let original = run();
        let prepared = RunRecord::prepare(
            original.run_id.clone(),
            original.project.clone(),
            original.test_type.clone(),
            original.model.clone(),
            original.engine_version.clone(),
            original.cases.clone(),
        )
        .unwrap();
        assert_eq!(prepared.status, "running");
        assert!(chrono::DateTime::parse_from_rfc3339(&prepared.started_at).is_ok());
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path().join("run");
        let store = RunStore::new(run_dir.clone());
        store.begin(&prepared).unwrap();
        let bytes = std::fs::read(run_dir.join("run.json")).unwrap();
        let mut replacement = prepared.clone();
        replacement.cases[0].target_url = "https://changed.test".into();
        assert!(store.begin(&replacement).is_err());
        assert_eq!(std::fs::read(run_dir.join("run.json")).unwrap(), bytes);
        let stopped = store
            .finish("stopped", vec![], Some("cancelled".into()))
            .unwrap();
        assert_eq!(stopped.cases[0].target_url, prepared.cases[0].target_url);
        assert_eq!(store.load().unwrap().status, "stopped");
        assert!(store
            .finish("failed", vec![], Some("rewrite".into()))
            .is_err());
        let mut invalid_model = prepared.model.clone();
        invalid_model["endpoint"] = json!("https://private.test");
        assert!(RunRecord::prepare(
            "other".into(),
            "alpha".into(),
            "smoke".into(),
            invalid_model,
            None,
            prepared.cases
        )
        .is_err());
    }
}
