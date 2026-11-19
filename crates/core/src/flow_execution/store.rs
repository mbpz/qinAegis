//! Separate schema-2 publications and private execution evidence. v1 is untouched.
use super::{
    runtime::{ExecutionFacts, Status},
    ExecutionDefinition, Operation,
};
use crate::{asset_file::AssetFile, flow::identifier};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
const LIMIT: usize = 32 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionDraft {
    pub definition: ExecutionDefinition,
    pub model_profiles: BTreeMap<String, String>,
}
impl ExecutionDraft {
    pub fn validate(&self) -> Result<Vec<String>> {
        let order = self.definition.validate()?;
        let ids: BTreeSet<_> = self
            .definition
            .steps
            .iter()
            .filter(|s| matches!(s.operation, Operation::ExecuteCase { .. }))
            .map(|s| s.id.as_str())
            .collect();
        ensure!(
            ids == self.model_profiles.keys().map(String::as_str).collect(),
            "case steps require exactly one explicitly selected model profile"
        );
        for profile in self.model_profiles.values() {
            identifier(profile)?;
        }
        Ok(order)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publication {
    #[serde(flatten)]
    pub draft: ExecutionDraft,
    pub version: u32,
    pub reviewer: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Library {
    pub schema_version: u32,
    pub revision: u64,
    pub drafts: Vec<ExecutionDraft>,
    pub publications: Vec<Publication>,
}
impl Default for Library {
    fn default() -> Self {
        Self {
            schema_version: 2,
            revision: 0,
            drafts: vec![],
            publications: vec![],
        }
    }
}
impl Library {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 2 && self.drafts.len() <= 64 && self.publications.len() <= 64,
            "invalid execution library"
        );
        let mut ids = BTreeSet::new();
        for d in &self.drafts {
            d.validate()?;
            ensure!(ids.insert(&d.definition.id), "duplicate execution draft");
        }
        let mut versions = BTreeSet::new();
        for p in &self.publications {
            p.draft.validate()?;
            ensure!(
                p.version > 0
                    && p.reviewer == "local-owner"
                    && versions.insert((&p.draft.definition.id, p.version)),
                "invalid execution publication"
            );
        }
        Ok(())
    }
    pub fn publication(&self, id: &str, version: u32) -> Result<&Publication> {
        self.publications
            .iter()
            .find(|p| p.draft.definition.id == id && p.version == version)
            .context("execution publication missing")
    }
}
pub struct ExecutionStore {
    root: PathBuf,
}
impl ExecutionStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root: root.join("flow-executions"),
        }
    }
    fn library(&self) -> AssetFile {
        AssetFile::new(self.root.join("library.json"), LIMIT)
    }
    pub fn snapshot(&self) -> Result<Library> {
        let f = self.library();
        let g = f.lock()?;
        let lib = g
            .read()?
            .map(|b| serde_json::from_slice(&b))
            .transpose()?
            .unwrap_or_default();
        Library::validate(&lib)?;
        Ok(lib)
    }
    pub fn save(&self, draft: ExecutionDraft, expected: u64) -> Result<Library> {
        draft.validate()?;
        self.update(expected, |lib| {
            lib.drafts
                .retain(|d| d.definition.id != draft.definition.id);
            lib.drafts.push(draft);
            Ok(())
        })
    }
    pub fn publish(&self, id: &str, expected: u64) -> Result<Library> {
        identifier(id)?;
        self.update(expected, |lib| {
            let index = lib
                .drafts
                .iter()
                .position(|d| d.definition.id == id)
                .context("execution draft missing")?;
            let version = lib
                .publications
                .iter()
                .filter(|p| p.draft.definition.id == id)
                .map(|p| p.version)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .context("execution version exhausted")?;
            let draft = lib.drafts.remove(index);
            lib.publications.push(Publication {
                draft,
                version,
                reviewer: "local-owner".into(),
            });
            Ok(())
        })
    }
    fn update(
        &self,
        expected: u64,
        change: impl FnOnce(&mut Library) -> Result<()>,
    ) -> Result<Library> {
        let f = self.library();
        let g = f.lock()?;
        let mut lib: Library = g
            .read()?
            .map(|b| serde_json::from_slice(&b))
            .transpose()?
            .unwrap_or_default();
        lib.validate()?;
        ensure!(
            lib.revision == expected,
            "execution library revision conflict"
        );
        change(&mut lib)?;
        lib.revision = lib
            .revision
            .checked_add(1)
            .context("execution revision exhausted")?;
        lib.validate()?;
        g.write(&serde_json::to_vec(&lib)?)?;
        Ok(lib)
    }
    fn run_file(&self, id: &str) -> Result<AssetFile> {
        identifier(id)?;
        Ok(AssetFile::new(
            self.root.join("runs").join(format!("{id}.json")),
            LIMIT,
        ))
    }
    pub fn begin(&self, record: &FlowRun) -> Result<()> {
        record.validate()?;
        ensure!(record.status == "queued", "new execution must be queued");
        ensure!(self.list()?.len() < 100, "execution history full");
        let f = self.run_file(&record.run_id)?;
        let g = f.lock()?;
        ensure!(g.read()?.is_none(), "execution already exists");
        g.write(&serde_json::to_vec(record)?)
    }
    pub fn load(&self, id: &str) -> Result<FlowRun> {
        let bytes = self
            .run_file(id)?
            .lock()?
            .read()?
            .context("flow execution missing")?;
        let run: FlowRun = serde_json::from_slice(&bytes)?;
        run.validate()?;
        ensure!(run.run_id == id, "execution identity mismatch");
        Ok(run)
    }
    pub fn case_evidence(&self, id: &str, step: &str, evidence: Value) -> Result<()> {
        identifier(step)?;
        self.change(id, |r| {
            r.case_runs.insert(step.into(), evidence);
            Ok(())
        })
    }
    pub fn checkpoint(&self, id: &str, facts: ExecutionFacts) -> Result<()> {
        self.change(id, |r| {
            r.status = "running".into();
            r.facts = Some(facts);
            Ok(())
        })
    }
    pub fn finish(
        &self,
        id: &str,
        status: &str,
        facts: Option<ExecutionFacts>,
        error: Option<String>,
    ) -> Result<()> {
        ensure!(
            matches!(status, "completed" | "failed" | "stopped" | "interrupted"),
            "invalid execution terminal status"
        );
        self.change(id, |r| {
            r.status = status.into();
            if facts.is_some() {
                r.facts = facts;
            }
            r.error = error;
            Ok(())
        })
    }
    fn change(&self, id: &str, change: impl FnOnce(&mut FlowRun) -> Result<()>) -> Result<()> {
        let f = self.run_file(id)?;
        let g = f.lock()?;
        let mut r: FlowRun = serde_json::from_slice(&g.read()?.context("flow execution missing")?)?;
        r.validate()?;
        ensure!(
            matches!(r.status.as_str(), "queued" | "running"),
            "terminal flow execution is immutable"
        );
        change(&mut r)?;
        r.updated_at = chrono::Utc::now().to_rfc3339();
        r.validate()?;
        g.write(&serde_json::to_vec(&r)?)
    }
    pub fn list(&self) -> Result<Vec<FlowRun>> {
        let path = self.root.join("runs");
        if !path.exists() {
            return Ok(vec![]);
        }
        let meta = std::fs::symlink_metadata(&path)?;
        ensure!(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "unsafe execution history"
        );
        let mut result = vec![];
        let mut inspected = 0;
        for entry in std::fs::read_dir(path)? {
            inspected += 1;
            ensure!(inspected <= 250, "execution directory exceeds limit");
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_str().context("invalid execution filename")?;
            if let Some(id) = name.strip_suffix(".json") {
                result.push(self.load(id)?);
                ensure!(result.len() <= 100, "execution history exceeds limit");
            }
        }
        result.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.run_id.cmp(&a.run_id))
        });
        Ok(result)
    }
    fn recover_cases(&self, flow: &FlowRun) -> Result<BTreeMap<String, Value>> {
        use crate::execution_records::{RunRecord, RunStore};
        let mut links = flow.case_runs.clone();
        for (step, link) in &mut links {
            let id = link["run_id"]
                .as_str()
                .context("case run identity missing")?;
            identifier(id)?;
            identifier(&flow.project)?;
            let root = self.root.parent().context("execution data root missing")?;
            let project = root.join("projects").join(&flow.project);
            let directory = project.join("reports").join(id);
            // A recovered association never follows a redirected project/run directory.
            for path in [root.join("projects"), project.clone()] {
                let meta = std::fs::symlink_metadata(path)?;
                ensure!(
                    meta.is_dir() && !meta.file_type().is_symlink(),
                    "unsafe associated case directory"
                );
            }
            // The intent is journaled before RunStore::begin creates reports.
            let reports = project.join("reports");
            if !reports.try_exists()? {
                std::fs::create_dir(&reports)?;
            }
            let reports_meta = std::fs::symlink_metadata(&reports)?;
            ensure!(
                reports_meta.is_dir() && !reports_meta.file_type().is_symlink(),
                "unsafe associated case reports directory"
            );
            if directory.exists() {
                let meta = std::fs::symlink_metadata(&directory)?;
                ensure!(
                    meta.is_dir() && !meta.file_type().is_symlink(),
                    "unsafe associated case directory"
                );
            }
            let intent = link
                .get("record")
                .map(|v| serde_json::from_value::<RunRecord>(v.clone()))
                .transpose()?;
            if let Some(intent) = &intent {
                intent.validate()?;
                ensure!(
                    intent.status == "running",
                    "associated case intent must be frozen before execution"
                );
            }
            let cases = &flow.preparation["cases"][step];
            let check = |record: &RunRecord| -> Result<()> {
                ensure!(
                    record.run_id == id
                        && record.project == flow.project
                        && record.test_type == "functional"
                        && record.cases.len() == 1,
                    "associated case identity differs"
                );
                let case = &record.cases[0];
                ensure!(
                    serde_json::to_value(&case.case_id)? == cases["reference"]["case_id"]
                        && serde_json::to_value(&case.name)? == cases["name"]
                        && serde_json::to_value(&case.yaml_script)? == cases["yaml_script"]
                        && serde_json::to_value(&case.target_url)? == cases["target_url"]
                        && serde_json::to_value(&case.provenance)? == cases["provenance"],
                    "associated case differs from frozen reviewed content"
                );
                let model = &flow.preparation["models"][step];
                ensure!(
                    record.model["revision"] == flow.preparation["model_revision"]
                        && record.model["task"] == "execute"
                        && record.model["candidates"]
                            .as_array()
                            .is_some_and(|a| a.len() == 1)
                        && record.model["candidates"][0]["id"] == model["profile_id"]
                        && record.model["candidates"][0]["adapter"] == model["adapter"]
                        && record.model["candidates"][0]["model"] == model["model"]
                        && record.model["candidates"][0]["midscene_family"]
                            == model["midscene_family"],
                    "associated case model differs from frozen selection"
                );
                ensure!(
                    serde_json::to_value(&record.engine_version)?
                        == flow.preparation["runtime"]["version"],
                    "associated case runtime differs"
                );
                Ok(())
            };
            if let Some(intent) = &intent {
                check(intent)?;
            }
            let store = RunStore::new(directory.clone());
            if !directory.join("run.json").try_exists()? {
                store.begin(
                    intent
                        .as_ref()
                        .context("associated case evidence missing without frozen intent")?,
                )?;
            }
            let mut record = store.load()?;
            check(&record)?;
            if let Some(intent) = &intent {
                let mut original = record.clone();
                original.status = "running".into();
                original.finished_at = None;
                original.results.clear();
                original.error = None;
                ensure!(
                    serde_json::to_value(original)? == serde_json::to_value(intent)?,
                    "associated case changed frozen execution inputs"
                );
            }
            if record.status == "running" {
                record=store.finish("stopped",vec![],Some("Service interrupted this case; no replay was attempted and owned resource cleanup remains unconfirmed".into()))?;
            }
            let allure =
                crate::allure::export_run(&record, &directory.join("allure-results")).is_ok();
            link["status"] = serde_json::json!(record.status);
            link["allure_results_ready"] = serde_json::json!(allure);
        }
        Ok(links)
    }
    fn sync_recovered_links(
        &self,
        id: &str,
        expected: &BTreeMap<String, Value>,
        links: BTreeMap<String, Value>,
    ) -> Result<()> {
        let file = self.run_file(id)?;
        let guard = file.lock()?;
        let mut flow: FlowRun =
            serde_json::from_slice(&guard.read()?.context("execution missing during recovery")?)?;
        flow.validate()?;
        ensure!(
            flow.case_runs == *expected,
            "case associations changed during recovery"
        );
        // Only the derived association/report index is reconciled. Published
        // inputs and immutable execution facts are never rewritten.
        flow.case_runs = links;
        flow.validate()?;
        guard.write(&serde_json::to_vec(&flow)?)?;
        Ok(())
    }
    pub fn recover(&self) -> Result<()> {
        for r in self.list()? {
            if r.status == "running" && r.has_case_attempt() {
                self.mark_quarantine(&r.run_id)?;
            }
            if !r.case_runs.is_empty() {
                let links = self.recover_cases(&r)?;
                if links != r.case_runs {
                    self.sync_recovered_links(&r.run_id, &r.case_runs, links)?;
                }
            }
            if matches!(r.status.as_str(), "queued" | "running") {
                self.finish(
                    &r.run_id,
                    "interrupted",
                    None,
                    Some("Service exited; execution was not replayed".into()),
                )?;
            }
        }
        Ok(())
    }
    pub fn quarantine(&self) -> Result<bool> {
        Ok(
            AssetFile::new(self.root.join("resource-quarantine.json"), 4096)
                .lock()?
                .read()?
                .is_some(),
        )
    }
    pub fn mark_persistence_block(&self, id: &str) -> Result<()> {
        identifier(id)?;
        AssetFile::new(self.root.join("resource-quarantine.json"),4096).lock()?.write(&serde_json::to_vec(&serde_json::json!({"schema_version":2,"run_id":id,"reason":"Terminal evidence persistence is unconfirmed; further actions blocked"}))?)
    }
    pub fn mark_quarantine(&self, id: &str) -> Result<()> {
        identifier(id)?;
        AssetFile::new(self.root.join("resource-quarantine.json"),4096).lock()?.write(&serde_json::to_vec(&serde_json::json!({"schema_version":2,"run_id":id,"reason":"Owned resource cleanup unconfirmed; further actions blocked"}))?)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowRun {
    pub schema_version: u32,
    pub run_id: String,
    pub job_id: Option<String>,
    pub flow_id: String,
    pub version: u32,
    pub project: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub preparation: Value,
    #[serde(default)]
    pub case_runs: BTreeMap<String, Value>,
    pub facts: Option<ExecutionFacts>,
    pub error: Option<String>,
}
impl FlowRun {
    pub fn has_case_attempt(&self) -> bool {
        self.facts.as_ref().is_some_and(|facts| {
            facts.steps.iter().any(|fact| {
                !fact.attempts.is_empty()
                    && self.preparation["publication"]["definition"]["steps"]
                        .as_array()
                        .is_some_and(|steps| {
                            steps.iter().any(|step| {
                                step["id"] == fact.step_id
                                    && step["operation"]["capability"] == "case.execute.v1"
                            })
                        })
            })
        })
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 2 && self.version > 0,
            "invalid execution record schema"
        );
        for id in [&self.run_id, &self.flow_id, &self.project] {
            identifier(id)?;
        }
        if let Some(job) = &self.job_id {
            identifier(job)?;
        }
        let created = chrono::DateTime::parse_from_rfc3339(&self.created_at)?;
        let updated = chrono::DateTime::parse_from_rfc3339(&self.updated_at)?;
        ensure!(
            updated >= created
                && matches!(
                    self.status.as_str(),
                    "queued" | "running" | "completed" | "failed" | "stopped" | "interrupted"
                ),
            "invalid execution status/time"
        );
        let publication: Publication = serde_json::from_value(
            self.preparation
                .get("publication")
                .context("missing frozen publication")?
                .clone(),
        )?;
        ensure!(
            publication.version == self.version && publication.reviewer == "local-owner",
            "frozen publication approval differs"
        );
        let draft = publication.draft;
        let order = draft.validate()?;
        let definition = &draft.definition;
        ensure!(
            definition.id == self.flow_id && definition.project == self.project,
            "frozen publication identity differs"
        );
        let inputs: BTreeMap<String, Value> = serde_json::from_value(
            self.preparation
                .get("inputs")
                .context("missing frozen inputs")?
                .clone(),
        )?;
        definition.validate_inputs(&inputs)?;
        ensure!(self.case_runs.len() <= 64, "too many case evidence links");
        for (step, value) in &self.case_runs {
            ensure!(
                definition
                    .steps
                    .iter()
                    .any(|s| s.id == *step && matches!(s.operation, Operation::ExecuteCase { .. })),
                "case evidence has unknown step"
            );
            identifier(
                value["run_id"]
                    .as_str()
                    .context("case evidence identity missing")?,
            )?;
            ensure!(
                value["project"] == self.project,
                "case evidence project differs"
            );
        }
        ensure!(
            !matches!(self.status.as_str(), "completed" | "stopped") || self.facts.is_some(),
            "terminal execution lacks facts"
        );
        if let Some(facts) = &self.facts {
            ensure!(
                facts.schema_version == 2
                    && facts.flow_id == self.flow_id
                    && facts.steps.len() <= order.len(),
                "invalid execution fact schema/coverage"
            );
            let mut global_end = 0;
            let mut outputs = BTreeMap::new();
            for (index, step_fact) in facts.steps.iter().enumerate() {
                ensure!(
                    order[index] == step_fact.step_id,
                    "facts are not a stable execution prefix"
                );
                let step = definition
                    .steps
                    .iter()
                    .find(|s| s.id == step_fact.step_id)
                    .context("unknown execution step")?;
                ensure!(
                    step_fact.attempts.len() <= usize::from(step.max_attempts),
                    "attempts exceed reviewed budget"
                );
                ensure!(
                    step_fact.status == Status::Succeeded || step_fact.outputs.is_empty(),
                    "unsuccessful step published outputs"
                );
                if step_fact.status == Status::Skipped {
                    ensure!(step_fact.attempts.is_empty(), "skipped step has attempts");
                }
                for (attempt_index, attempt) in step_fact.attempts.iter().enumerate() {
                    ensure!(
                        usize::from(attempt.attempt) == attempt_index + 1
                            && attempt.started_ms >= global_end
                            && attempt.finished_ms >= attempt.started_ms,
                        "invalid attempt order/time"
                    );
                    ensure!(
                        attempt.status != Status::Skipped,
                        "attempt cannot be skipped"
                    );
                    if attempt.status == Status::Running {
                        ensure!(
                            step_fact.status == Status::Running
                                && index + 1 == facts.steps.len()
                                && attempt_index + 1 == step_fact.attempts.len(),
                            "running attempt is not the active tail"
                        );
                    }
                    global_end = attempt.finished_ms;
                    if let Some(result) = &attempt.case_result {
                        let case = match &step.operation {
                            Operation::ExecuteCase { case } => case,
                            _ => anyhow::bail!("wait attempt contains case evidence"),
                        };
                        ensure!(
                            result.case_id == case.case_id
                                && result.retry_count == 0
                                && result.healed_yaml_script.is_none(),
                            "case evidence changed identity or healing policy"
                        );
                        super::runtime::validate_case_result_budget(result)?;
                        ensure!(
                            (attempt.status == Status::Succeeded && result.passed)
                                || (attempt.status == Status::Failed && !result.passed),
                            "case result disagrees with attempt status"
                        );
                    }
                    if matches!(step.operation, Operation::ExecuteCase { .. })
                        && attempt.status == Status::Succeeded
                    {
                        ensure!(
                            attempt.case_result.is_some(),
                            "successful case lacks result evidence"
                        );
                    }
                }
                if step_fact.status == Status::Succeeded {
                    ensure!(
                        step_fact
                            .attempts
                            .last()
                            .is_some_and(|a| a.status == Status::Succeeded),
                        "successful step lacks successful attempt"
                    );
                    definition.validate_step_outputs(&step_fact.step_id, &step_fact.outputs)?;
                    outputs.insert(step_fact.step_id.clone(), step_fact.outputs.clone());
                }
            }
            if self.status == "running" {
                ensure!(
                    facts.status == Status::Running
                        && facts.outputs.is_empty()
                        && facts.cleanup_started_ms == 0
                        && facts.finished_ms == 0,
                    "running checkpoint claims terminal evidence"
                );
            }
            if matches!(self.status.as_str(), "completed" | "failed" | "stopped") {
                ensure!(
                    facts.steps.len() == order.len()
                        && facts.cleanup_started_ms >= global_end
                        && facts.finished_ms >= facts.cleanup_started_ms,
                    "terminal facts lack complete order or cleanup timing"
                );
                ensure!(
                    facts.steps.iter().all(|s| s.status != Status::Running),
                    "terminal execution retains running step"
                );
                ensure!(
                    match self.status.as_str() {
                        "completed" =>
                            facts.status == Status::Succeeded && facts.cleanup_error.is_none(),
                        "stopped" => facts.status == Status::Cancelled,
                        "failed" => matches!(facts.status, Status::Failed | Status::TimedOut),
                        _ => false,
                    },
                    "terminal execution status differs from facts"
                );
            }
            if facts.status == Status::Succeeded {
                let mut expected = BTreeMap::new();
                for (name, port) in &definition.outputs {
                    let value =
                        ExecutionDefinition::resolve_source(&port.source, &inputs, &outputs)?;
                    ensure!(port.value_type.accepts(&value), "final output type differs");
                    expected.insert(name.clone(), value);
                }
                ensure!(
                    facts.outputs == expected,
                    "final output differs from frozen typed source"
                );
            } else {
                ensure!(
                    facts.outputs.is_empty(),
                    "unsuccessful flow published outputs"
                );
            }
        }
        ensure!(
            serde_json::to_vec(self)?.len() <= LIMIT,
            "execution record exceeds limit"
        );
        Ok(())
    }
}

pub fn timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow_execution::{
        runtime::{AttemptFact, StepFact},
        tests::flow,
    };
    fn draft() -> ExecutionDraft {
        ExecutionDraft {
            definition: flow(),
            model_profiles: BTreeMap::new(),
        }
    }
    fn record() -> FlowRun {
        let draft = draft();
        FlowRun {
            schema_version: 2,
            run_id: "run-1".into(),
            job_id: Some("job-1".into()),
            flow_id: draft.definition.id.clone(),
            project: draft.definition.project.clone(),
            version: 1,
            status: "queued".into(),
            created_at: timestamp(),
            updated_at: timestamp(),
            preparation: serde_json::json!({"publication":Publication{draft,version:1,reviewer:"local-owner".into()},"inputs":{"enabled":true}}),
            case_runs: BTreeMap::new(),
            facts: None,
            error: None,
        }
    }
    fn facts() -> ExecutionFacts {
        ExecutionFacts {
            schema_version: 2,
            flow_id: flow().id,
            status: Status::Running,
            steps: vec![StepFact {
                step_id: "pause".into(),
                status: Status::Running,
                reason: None,
                attempts: vec![AttemptFact {
                    attempt: 1,
                    started_ms: 1,
                    finished_ms: 1,
                    status: Status::Running,
                    error: None,
                    case_result: None,
                }],
                outputs: BTreeMap::new(),
            }],
            outputs: BTreeMap::new(),
            error: None,
            cleanup_error: None,
            cleanup_started_ms: 0,
            finished_ms: 0,
        }
    }
    #[test]
    fn cas_publications_are_immutable_and_reload_validates_flattened_schema() {
        let dir = tempfile::tempdir().unwrap();
        let s = ExecutionStore::new(dir.path().into());
        let first = s.save(draft(), 0).unwrap();
        assert!(s.save(draft(), 0).is_err());
        let published = s.publish(&first.drafts[0].definition.id, 1).unwrap();
        assert_eq!(published.revision, 2);
        assert_eq!(s.snapshot().unwrap().publications[0].draft, draft());
        let mut next = draft();
        next.definition.title = "changed".into();
        s.save(next, 2).unwrap();
        let all = s.publish(&draft().definition.id, 3).unwrap();
        assert_eq!(all.publications[0].draft, draft());
        assert_eq!(all.publications[1].version, 2);
    }
    fn case_record() -> FlowRun {
        let mut r = record();
        let mut d = draft();
        d.definition.outputs.clear();
        d.definition.steps[0].condition = None;
        d.definition.steps[0].operation = Operation::ExecuteCase {
            case: crate::flow_execution::tests::approved_fixture().0,
        };
        d.definition.required_permissions = [
            "browser.read",
            "browser.write",
            "model.send",
            "report.write",
        ]
        .map(String::from)
        .to_vec();
        d.model_profiles.insert("pause".into(), "selected".into());
        r.preparation["publication"] = serde_json::to_value(Publication {
            draft: d,
            version: 1,
            reviewer: "local-owner".into(),
        })
        .unwrap();
        r
    }
    #[test]
    fn case_attempt_recovery_is_quarantined_even_after_response_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let s = ExecutionStore::new(dir.path().into());
        s.begin(&case_record()).unwrap();
        s.checkpoint("run-1", facts()).unwrap();
        s.recover().unwrap();
        assert!(s.quarantine().unwrap());
        assert_eq!(s.load("run-1").unwrap().status, "interrupted");
    }
    fn associated_case(
        root: &std::path::Path,
        materialize: bool,
    ) -> (
        ExecutionStore,
        crate::execution_records::RunStore,
        crate::execution_records::RunRecord,
    ) {
        use crate::{
            execution_records::{RunRecord, RunStore},
            flow_execution::PreparedCase,
        };
        let (reference, project, contracts, asset) =
            crate::flow_execution::tests::approved_fixture();
        let case = PreparedCase::from_asset(&reference, &project, &project.url, &contracts, asset)
            .unwrap();
        let mut flow = case_record();
        // The general store fixture uses a different project; bind this record
        // to the actual approved fixture before either evidence file is created.
        flow.project = project.name.clone();
        flow.preparation["publication"]["definition"]["project"] = serde_json::json!(project.name);
        let model = serde_json::json!({"revision":3,"task":"execute","candidates":[{"id":"selected","adapter":"openai_compatible","model":"fixture","capabilities":["text","vision"],"midscene_family":"qwen2.5-vl"}]});
        let record = RunRecord::prepare(
            "case-owned".into(),
            project.name.clone(),
            "functional".into(),
            model,
            Some("fixture-runtime".into()),
            vec![case.run_case()],
        )
        .unwrap();
        flow.preparation["cases"] = serde_json::json!({"pause":case});
        flow.preparation["models"] = serde_json::json!({"pause":{"profile_id":"selected","adapter":"openai_compatible","model":"fixture","capabilities":["text","vision"],"midscene_family":"qwen2.5-vl"}});
        flow.preparation["model_revision"] = serde_json::json!(3);
        flow.preparation["runtime"] = serde_json::json!({"version":"fixture-runtime"});
        std::fs::create_dir_all(root.join("projects").join(&project.name)).unwrap();
        let store = ExecutionStore::new(root.into());
        store.begin(&flow).unwrap();
        store.checkpoint("run-1", facts()).unwrap();
        store.case_evidence("run-1", "pause", serde_json::json!({"run_id":record.run_id,"project":project.name,"status":"running","allure_results_ready":false,"record":record})).unwrap();
        let cases = RunStore::new(
            root.join("projects")
                .join(&project.name)
                .join("reports/case-owned"),
        );
        if materialize {
            cases.begin(&record).unwrap();
        }
        (store, cases, record)
    }
    #[test]
    fn recovery_closes_case_records_and_allure_at_both_journal_boundaries_without_replay() {
        for materialize in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let (store, cases, original) = associated_case(dir.path(), materialize);
            let before = serde_json::to_value(store.load("run-1").unwrap().facts).unwrap();
            store.recover().unwrap();
            let recovered = store.load("run-1").unwrap();
            assert_eq!(recovered.status, "interrupted");
            assert_eq!(serde_json::to_value(&recovered.facts).unwrap(), before);
            assert!(store.quarantine().unwrap());
            let stopped = cases.load().unwrap();
            assert_eq!(stopped.status, "stopped");
            assert!(stopped.error.as_ref().unwrap().contains("interrupted"));
            assert!(stopped.results.is_empty());
            assert_eq!(
                serde_json::to_value(&stopped.cases).unwrap(),
                serde_json::to_value(&original.cases).unwrap()
            );
            assert_eq!(recovered.case_runs["pause"]["status"], "stopped");
            assert_eq!(recovered.case_runs["pause"]["allure_results_ready"], true);
            let terminal = serde_json::to_value(stopped).unwrap();
            store.recover().unwrap();
            assert_eq!(
                serde_json::to_value(cases.load().unwrap()).unwrap(),
                terminal
            );
        }
    }
    #[test]
    fn recovery_preserves_terminal_case_and_rejects_foreign_frozen_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let (store, cases, _) = associated_case(dir.path(), true);
        cases
            .finish("failed", vec![], Some("original failure".into()))
            .unwrap();
        let original = serde_json::to_value(cases.load().unwrap()).unwrap();
        store.recover().unwrap();
        assert_eq!(
            serde_json::to_value(cases.load().unwrap()).unwrap(),
            original
        );
        assert_eq!(
            store.load("run-1").unwrap().case_runs["pause"]["status"],
            "failed"
        );
        for field in ["project", "case", "model", "runtime", "provenance"] {
            let dir = tempfile::tempdir().unwrap();
            let (store, cases, _) = associated_case(dir.path(), true);
            let path = dir
                .path()
                .join("projects/project/reports/case-owned/run.json");
            let mut foreign = serde_json::to_value(cases.load().unwrap()).unwrap();
            match field {
                "project" => foreign["project"] = serde_json::json!("foreign"),
                "case" => {
                    foreign["cases"][0]["yaml_script"] = serde_json::json!("tasks: [changed]")
                }
                "model" => foreign["model"]["candidates"][0]["id"] = serde_json::json!("other"),
                "runtime" => foreign["engine_version"] = serde_json::json!("other"),
                _ => foreign["cases"][0]["provenance"] = serde_json::Value::Null,
            }
            std::fs::write(&path, serde_json::to_vec(&foreign).unwrap()).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert!(store.recover().is_err(), "accepted changed {field}");
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert!(store.quarantine().unwrap());
        }
    }
    #[test]
    fn fact_validation_rejects_forged_order_ports_final_outputs_and_case_results() {
        let mut r = record();
        let mut f = facts();
        r.status = "completed".into();
        f.status = Status::Succeeded;
        f.steps[0].status = Status::Succeeded;
        f.steps[0].attempts[0].status = Status::Succeeded;
        f.steps[0]
            .outputs
            .insert("elapsed_ms".into(), serde_json::json!(10));
        f.cleanup_started_ms = 2;
        f.finished_ms = 3;
        r.facts = Some(f);
        assert!(r.validate().is_ok());
        let good = r.clone();
        r.facts.as_mut().unwrap().steps.clear();
        assert!(r.validate().is_err());
        r = good.clone();
        r.facts.as_mut().unwrap().steps[0].step_id = "other".into();
        assert!(r.validate().is_err());
        r = good.clone();
        r.facts.as_mut().unwrap().steps[0]
            .outputs
            .insert("unexpected".into(), serde_json::json!(true));
        assert!(r.validate().is_err());
        r = good.clone();
        r.facts
            .as_mut()
            .unwrap()
            .outputs
            .insert("forged".into(), serde_json::json!(true));
        assert!(r.validate().is_err());
        r = good;
        r.facts.as_mut().unwrap().finished_ms = 0;
        assert!(r.validate().is_err());
        let mut r = case_record();
        r.status = "running".into();
        let mut f = facts();
        f.steps[0].attempts[0].case_result = Some(crate::executor::TestResult {
            capture: None,
            case_id: "foreign".into(),
            passed: true,
            duration_ms: 1,
            screenshot_base64: None,
            error_message: None,
            retry_count: 0,
            healed_yaml_script: None,
            failed_step: None,
        });
        r.facts = Some(f);
        assert!(r.validate().is_err());
    }
    #[test]
    fn checkpoints_are_separate_private_and_terminal_records_cannot_be_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let s = ExecutionStore::new(dir.path().into());
        s.begin(&record()).unwrap();
        s.checkpoint("run-1", facts()).unwrap();
        assert_eq!(s.load("run-1").unwrap().status, "running");
        s.finish("run-1", "interrupted", None, None).unwrap();
        assert!(s.checkpoint("run-1", facts()).is_err());
        assert!(s.finish("run-1", "failed", None, None).is_err());
        assert!(!s.load("run-1").unwrap().facts.unwrap().steps.is_empty());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(dir.path().join("flow-executions/runs/run-1.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn waiting_recovery_never_replays_and_does_not_quarantine() {
        let dir = tempfile::tempdir().unwrap();
        let s = ExecutionStore::new(dir.path().into());
        s.begin(&record()).unwrap();
        s.checkpoint("run-1", facts()).unwrap();
        s.recover().unwrap();
        assert!(!s.quarantine().unwrap());
        assert_eq!(s.load("run-1").unwrap().status, "interrupted");
        s.recover().unwrap();
        assert!(!s.quarantine().unwrap());
    }
    #[test]
    fn failed_or_missing_facts_cannot_claim_completed_and_attempts_are_bounded() {
        let mut r = record();
        r.status = "completed".into();
        assert!(r.validate().is_err());
        r.facts = Some(facts());
        assert!(r.validate().is_err());
        r.status = "running".into();
        r.facts.as_mut().unwrap().steps[0].attempts[0].attempt = 2;
        assert!(r.validate().is_err());
    }
}
