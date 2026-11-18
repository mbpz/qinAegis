//! Model suggestions remain unapproved contract proposals. These records retain
//! the selected historical input independently of bounded task history.
use crate::contracts::PublishedContract;
use crate::execution_records::{GenerationContext, RunCase, RunRecord};
use crate::models::{ModelCallAttempt, ModelCallOutcome};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const INPUT_LIMIT: usize = 192 * 1024;
pub const OUTPUT_LIMIT: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionResult {
    pub passed: bool,
    pub duration_ms: u64,
    pub error_message: Option<String>,
    pub failed_step: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionInput {
    pub project: String,
    pub contract_id: String,
    pub version: u32,
    pub library_revision: u64,
    pub publication: PublishedContract,
    pub run_id: String,
    pub case_id: String,
    pub instruction: String,
    pub case: RunCase,
    pub run_status: String,
    pub result: Option<CorrectionResult>,
    pub evidence_token: String,
}
impl PartialEq for CorrectionInput {
    fn eq(&self, other: &Self) -> bool {
        serde_json::to_value(self).ok() == serde_json::to_value(other).ok()
    }
}
fn identity(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 128
            && !value.starts_with('.')
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')),
        "invalid correction identity"
    );
    Ok(())
}
fn terminal_case(
    record: &RunRecord,
    project: &str,
    case_id: &str,
    publication: &PublishedContract,
) -> Result<RunCase> {
    record.validate()?;
    ensure!(
        record.project == project && record.status != "running",
        "correction requires matching terminal run"
    );
    let case = record
        .cases
        .iter()
        .find(|case| case.case_id == case_id)
        .context("selected case not in run")?;
    let provenance = case
        .provenance
        .as_ref()
        .context("selected case has no contract provenance")?;
    provenance.validate(project, case_id, &case.yaml_script)?;
    let snapshot = provenance
        .contracts
        .as_ref()
        .context("selected case has no contract snapshot")?;
    let contract = snapshot
        .contracts()
        .iter()
        .find(|c| c.publication().definition().id == publication.definition().id)
        .context("selected case does not reference contract")?;
    ensure!(
        contract.publication() == publication
            && contract.binding().project() == project
            && contract.original_assertions() == publication.definition().assertions,
        "selected run contract differs from requested publication"
    );
    Ok(case.clone())
}
impl CorrectionInput {
    pub fn prepare(
        record: &RunRecord,
        case_id: &str,
        publication: PublishedContract,
        revision: u64,
        instruction: &str,
    ) -> Result<Self> {
        let case = terminal_case(record, &record.project, case_id, &publication)?;
        let result = record
            .results
            .iter()
            .find(|r| r.case_id == case_id)
            .map(|r| CorrectionResult {
                passed: r.passed,
                duration_ms: r.duration_ms,
                error_message: r.error_message.clone(),
                failed_step: r.failed_step.clone(),
            });
        let value = Self {
            project: record.project.clone(),
            contract_id: publication.definition().id.clone(),
            version: publication.definition().version,
            library_revision: revision,
            publication,
            run_id: record.run_id.clone(),
            case_id: case_id.into(),
            instruction: instruction.into(),
            case,
            run_status: record.status.clone(),
            result,
            evidence_token: format!(
                "managed-run:{}:{}:{}",
                record.project, record.run_id, case_id
            ),
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        for id in [
            &self.project,
            &self.contract_id,
            &self.run_id,
            &self.case_id,
        ] {
            identity(id)?;
        }
        ensure!(
            !self.instruction.trim().is_empty() && self.instruction.len() <= 8192,
            "correction instruction must contain 1..8192 bytes"
        );
        ensure!(
            self.publication.definition().id == self.contract_id
                && self.publication.definition().version == self.version,
            "correction publication identity mismatch"
        );
        let _: PublishedContract =
            serde_json::from_value(serde_json::to_value(&self.publication)?)?;
        ensure!(
            matches!(self.run_status.as_str(), "completed" | "failed" | "stopped"),
            "correction requires terminal run"
        );
        ensure!(
            self.case.case_id == self.case_id,
            "correction case identity mismatch"
        );
        let provenance = self
            .case
            .provenance
            .as_ref()
            .context("correction source provenance missing")?;
        provenance.validate(&self.project, &self.case_id, &self.case.yaml_script)?;
        let snapshot = provenance
            .contracts
            .as_ref()
            .context("correction source contracts missing")?;
        let contract = snapshot
            .contracts()
            .iter()
            .find(|c| c.publication().definition().id == self.contract_id)
            .context("correction contract missing")?;
        ensure!(
            contract.publication() == &self.publication
                && contract.binding().project() == self.project
                && contract.original_assertions() == self.publication.definition().assertions,
            "correction source publication differs"
        );
        let url =
            reqwest::Url::parse(&self.case.target_url).context("invalid correction target")?;
        ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none(),
            "invalid correction target"
        );
        ensure!(
            self.run_status != "completed" || self.result.is_some(),
            "completed correction source lacks result"
        );
        ensure!(
            self.evidence_token
                == format!(
                    "managed-run:{}:{}:{}",
                    self.project, self.run_id, self.case_id
                ),
            "invalid correction evidence token"
        );
        ensure!(
            serde_json::to_vec(self)?.len() <= INPUT_LIMIT,
            "correction input exceeds size limit"
        );
        Ok(())
    }
    /// Deliberately excludes screenshots, other cases and unrelated contracts.
    pub fn prompt(&self) -> Result<String> {
        self.validate()?;
        let snapshot = self
            .case
            .provenance
            .as_ref()
            .unwrap()
            .contracts
            .as_ref()
            .unwrap();
        let binding = snapshot
            .contracts()
            .iter()
            .find(|c| c.publication() == &self.publication)
            .unwrap()
            .binding();
        Ok(serde_json::to_string(
            &serde_json::json!({"publication":self.publication,"binding":binding,
            "instruction":self.instruction,"script":self.case.yaml_script,"target_url":self.case.target_url,
            "run_status":self.run_status,"result":self.result,"evidence_token":self.evidence_token}),
        )?)
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionOutput {
    pub replacement: crate::contracts::ContractDraft,
    pub reason: String,
    pub limitations: Vec<String>,
    pub evidence_tokens: Vec<String>,
}
impl CorrectionOutput {
    pub fn parse(raw: &str, input: &CorrectionInput) -> Result<Self> {
        ensure!(
            raw.len() <= OUTPUT_LIMIT,
            "correction output exceeds size limit"
        );
        let value: Self = serde_json::from_str(raw)
            .map_err(|_| anyhow::anyhow!("invalid correction output document"))?;
        value.validate(input)?;
        Ok(value)
    }
    fn validate(&self, input: &CorrectionInput) -> Result<()> {
        ensure!(
            !self.reason.trim().is_empty() && self.reason.len() <= 8192,
            "invalid correction reason"
        );
        ensure!(
            !self.limitations.is_empty()
                && self.limitations.len() <= 16
                && self
                    .limitations
                    .iter()
                    .all(|s| !s.trim().is_empty() && s.len() <= 2048),
            "correction limitations required"
        );
        ensure!(
            self.evidence_tokens == vec![input.evidence_token.clone()],
            "correction must cite the selected server evidence token"
        );
        ensure!(
            self.replacement.id == input.contract_id
                && Some(self.replacement.version) == input.version.checked_add(1),
            "correction replacement must target next version"
        );
        input.publication.propose_correction(
            self.replacement.clone(),
            &self.reason,
            self.evidence_tokens.clone(),
        )?;
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCorrectionSource {
    pub schema_version: u32,
    pub input: CorrectionInput,
    pub model: Value,
    pub attempts: Vec<ModelCallAttempt>,
    pub limitations: Vec<String>,
    pub evidence_tokens: Vec<String>,
}
/// Validate the single correction call without inheriting generation/critic sequencing.
pub fn validate_attempts(
    model: &Value,
    attempts: &[ModelCallAttempt],
    complete: bool,
) -> Result<()> {
    GenerationContext {
        specification: None,
        contracts: None,
        model: model.clone(),
    }
    .validate("correction")?;
    ensure!(
        model["task"] == "correct" && model.get("source").is_none(),
        "explicit registered correct route required"
    );
    let candidates = model["candidates"]
        .as_array()
        .context("correction candidates missing")?;
    ensure!(
        attempts.len() <= candidates.len(),
        "too many correction attempts"
    );
    if complete {
        ensure!(!attempts.is_empty(), "correction model attempts missing");
    }
    for (i, attempt) in attempts.iter().enumerate() {
        ensure!(
            attempt.call == 1
                && usize::from(attempt.attempt) == i + 1
                && candidates[i]["id"] == attempt.profile_id,
            "invalid correction attempt identity"
        );
        if i + 1 < attempts.len() {
            ensure!(
                matches!(
                    attempt.outcome,
                    ModelCallOutcome::TransportFailure
                        | ModelCallOutcome::TimedOut
                        | ModelCallOutcome::RateLimited
                        | ModelCallOutcome::ServiceUnavailable
                ),
                "nonretryable correction fallback"
            );
        }
    }
    if complete {
        ensure!(
            attempts.last().unwrap().outcome == ModelCallOutcome::Succeeded,
            "correction proposal needs successful model call"
        );
    }
    Ok(())
}
impl ModelCorrectionSource {
    pub fn validate(
        &self,
        replacement: &crate::contracts::ContractDraft,
        reason: &str,
        evidence: &[String],
        original: &PublishedContract,
    ) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "unsupported model correction source"
        );
        self.input.validate()?;
        ensure!(
            &self.input.publication == original,
            "model correction original publication mismatch"
        );
        validate_attempts(&self.model, &self.attempts, true)?;
        CorrectionOutput {
            replacement: replacement.clone(),
            reason: reason.into(),
            limitations: self.limitations.clone(),
            evidence_tokens: self.evidence_tokens.clone(),
        }
        .validate(&self.input)?;
        ensure!(
            evidence == self.evidence_tokens,
            "proposal evidence differs from model source"
        );
        ensure!(
            serde_json::to_vec(self)?.len() <= INPUT_LIMIT + OUTPUT_LIMIT,
            "correction source exceeds size limit"
        );
        Ok(())
    }
}
/// Reject rather than replace known credentials, including JSON-escaped keys/values.
pub fn reject_secrets(raw: &str, secrets: &[String]) -> Result<()> {
    fn contains(value: &Value, secret: &str) -> bool {
        match value {
            Value::String(s) => s.contains(secret),
            Value::Array(a) => a.iter().any(|v| contains(v, secret)),
            Value::Object(o) => o
                .iter()
                .any(|(k, v)| k.contains(secret) || contains(v, secret)),
            _ => false,
        }
    }
    let decoded = serde_json::from_str::<Value>(raw).ok();
    ensure!(
        !secrets
            .iter()
            .filter(|s| !s.is_empty())
            .any(|s| raw.contains(s) || decoded.as_ref().is_some_and(|v| contains(v, s))),
        "correction input/output contains a known task credential"
    );
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionObservation {
    pub proposal_id: String,
    pub project: String,
    pub run_id: String,
    pub case_id: String,
    pub contract_version: u32,
    pub outcome: String,
    pub recorded_by: String,
    pub semantic_validation: String,
    pub input: CorrectionInput,
}
impl CorrectionObservation {
    pub fn prepare(
        proposal_id: &str,
        record: &RunRecord,
        case_id: &str,
        publication: &PublishedContract,
    ) -> Result<Self> {
        terminal_case(record, &record.project, case_id, publication)?;
        let result = record.results.iter().find(|r| r.case_id == case_id);
        let outcome = if record.status != "completed" {
            "incomplete"
        } else if result.is_some_and(|r| r.passed) {
            "passed"
        } else {
            "failed"
        };
        let value = Self {
            proposal_id: proposal_id.into(),
            project: record.project.clone(),
            run_id: record.run_id.clone(),
            case_id: case_id.into(),
            contract_version: publication.definition().version,
            outcome: outcome.into(),
            recorded_by: "local-owner".into(),
            semantic_validation: "not_verified".into(),
            input: CorrectionInput::prepare(
                record,
                case_id,
                publication.clone(),
                0,
                "Explicitly recorded managed run observation",
            )?,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        self.input.validate()?;
        ensure!(
            self.input.project == self.project
                && self.input.run_id == self.run_id
                && self.input.case_id == self.case_id
                && self.input.version == self.contract_version,
            "observation frozen input differs"
        );
        let observed = if self.input.run_status != "completed" {
            "incomplete"
        } else if self.input.result.as_ref().is_some_and(|r| r.passed) {
            "passed"
        } else {
            "failed"
        };
        ensure!(
            self.outcome == observed,
            "observation differs from frozen result"
        );
        for id in [
            &self.proposal_id,
            &self.project,
            &self.run_id,
            &self.case_id,
        ] {
            identity(id)?;
        }
        ensure!(
            self.contract_version > 0
                && matches!(self.outcome.as_str(), "passed" | "failed" | "incomplete")
                && self.recorded_by == "local-owner"
                && self.semantic_validation == "not_verified",
            "invalid correction observation"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_execution::ExecutionSnapshot;
    use crate::contract_store::{ContractLibrary, ContractStore, CorrectionStatus};
    use crate::contracts::ContractDraft;
    use crate::execution_records::CaseProvenance;
    use crate::executor::TestResult;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn draft(version: u32) -> ContractDraft {
        ContractDraft {
            id: "receipt".into(),
            version,
            intent: "receipt exists".into(),
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            preconditions: vec![],
            assertions: vec!["receipt matches input".into()],
            required_capabilities: vec![],
        }
    }
    fn model() -> Value {
        json!({"revision":1,"task":"correct","candidates":[{"id":"correct-primary","adapter":"openai_compatible","model":"fixture","capabilities":["text"],"midscene_family":null}]})
    }
    fn run(lib: &ContractLibrary, run_id: &str, version: u32) -> RunRecord {
        let publication = lib
            .publications
            .iter()
            .find(|p| p.definition().version == version)
            .unwrap()
            .clone();
        let mut scoped = lib.clone();
        scoped.bindings = vec![publication.bind("project", BTreeMap::new()).unwrap()];
        let snapshot = ExecutionSnapshot::prepare(&scoped, "project", &[]).unwrap();
        let script = "- aiAssert: receipt matches input".to_string();
        let provenance = CaseProvenance {
            schema_version: 1,
            project: "project".into(),
            case_id: "case".into(),
            yaml_script: script.clone(),
            created_at: "2026-11-18T00:00:00Z".into(),
            contracts: Some(snapshot),
            model: json!({"source":"legacy_config","model":"fixture"}),
            model_calls: None,
            specification: None,
        };
        let case = RunCase {
            case_id: "case".into(),
            name: "receipt".into(),
            yaml_script: script,
            target_url: "https://project.test".into(),
            provenance: Some(provenance),
        };
        let mut record = RunRecord::prepare(
            run_id.into(),
            "project".into(),
            "functional".into(),
            json!({"source":"legacy_config","model":"fixture"}),
            None,
            vec![case],
        )
        .unwrap();
        record.status = "completed".into();
        record.finished_at = Some("2026-11-18T00:00:01Z".into());
        record.results = vec![TestResult {
            case_id: "case".into(),
            passed: false,
            duration_ms: 2,
            screenshot_base64: None,
            error_message: Some("receipt absent".into()),
            retry_count: 0,
            healed_yaml_script: None,
            failed_step: Some("aiAssert".into()),
        }];
        record.validate().unwrap();
        record
    }
    fn setup() -> (
        tempfile::TempDir,
        ContractStore,
        ContractLibrary,
        CorrectionInput,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let store = ContractStore::new(dir.path().join("library.json"));
        store.save_draft(draft(1), 0).unwrap();
        let lib = store.publish("receipt", 1, "owner").unwrap();
        let record = run(&lib, "old-run", 1);
        let input = CorrectionInput::prepare(
            &record,
            "case",
            lib.publications[0].clone(),
            2,
            "Review the failed observation",
        )
        .unwrap();
        (dir, store, lib, input)
    }
    fn source(input: CorrectionInput) -> ModelCorrectionSource {
        let token = input.evidence_token.clone();
        ModelCorrectionSource {
            schema_version: 1,
            input,
            model: model(),
            attempts: vec![ModelCallAttempt {
                call: 1,
                attempt: 1,
                profile_id: "correct-primary".into(),
                outcome: ModelCallOutcome::Succeeded,
            }],
            limitations: vec!["Business meaning requires human review".into()],
            evidence_tokens: vec![token],
        }
    }
    #[test]
    fn correction_source_and_output_reject_forged_identity_evidence_and_secrets() {
        let (_dir, _store, lib, input) = setup();
        let mut record = run(&lib, "old-run", 1);
        record.status = "running".into();
        record.results.clear();
        record.finished_at = None;
        assert!(
            CorrectionInput::prepare(&record, "case", lib.publications[0].clone(), 2, "why")
                .is_err()
        );
        let mut forged = input.clone();
        forged.project = "other".into();
        assert!(forged.validate().is_err());
        forged = input.clone();
        forged.case.yaml_script = "- aiAssert: always pass".into();
        assert!(forged.validate().is_err());
        let mut output = json!({"replacement":draft(2),"reason":"Clarify expected receipt","limitations":["not semantic proof"],"evidence_tokens":[input.evidence_token]});
        assert!(CorrectionOutput::parse(&output.to_string(), &input).is_ok());
        output["evidence_tokens"] = json!(["/tmp/fake"]);
        assert!(CorrectionOutput::parse(&output.to_string(), &input).is_err());
        output["evidence_tokens"] = json!([input.evidence_token]);
        output["approved"] = json!(true);
        assert!(CorrectionOutput::parse(&output.to_string(), &input).is_err());
        assert!(reject_secrets(r#"{"reason":"sec\u0072et"}"#, &["secret".into()]).is_err());
        assert!(reject_secrets(r#"{"sec\u0072et":"value"}"#, &["secret".into()]).is_err());
        assert!(reject_secrets("ordinary observation", &["secret".into()]).is_ok());
    }
    #[test]
    fn model_proposal_is_atomic_pending_cas_and_reload_checks_actual_source() {
        let (dir, store, lib, input) = setup();
        let origin = source(input.clone());
        assert!(store
            .propose_model(draft(2), "clarify", origin.clone(), 1)
            .is_err());
        assert!(store.snapshot().unwrap().proposals.is_empty());
        let saved = store.propose_model(draft(2), "clarify", origin, 2).unwrap();
        assert_eq!(saved.publications, lib.publications);
        assert_eq!(saved.proposals[0].status, CorrectionStatus::Pending);
        assert_eq!(saved.proposals[0].reviewer, None);
        assert_eq!(store.snapshot().unwrap(), saved);
        assert!(store
            .propose_model(draft(2), "clarify", source(input), 2)
            .is_err());
        for (field, value) in [
            (
                "attempts",
                json!([{ "call":1,"attempt":1,"profile_id":"invented","outcome":"succeeded"}]),
            ),
            ("evidence_tokens", json!(["fake"])),
        ] {
            let mut forged = serde_json::to_value(&saved).unwrap();
            forged["proposals"][0]["model_source"][field] = value;
            assert!(serde_json::from_value::<ContractLibrary>(forged).is_err());
        }
        let mut forged = serde_json::to_value(&saved).unwrap();
        forged["proposals"][0]["model_source"]["input"]["case"]["yaml_script"] = json!("changed");
        std::fs::write(
            dir.path().join("library.json"),
            serde_json::to_vec(&forged).unwrap(),
        )
        .unwrap();
        assert!(store.snapshot().is_err());
    }
    #[test]
    fn approved_new_version_observations_reject_old_run_and_preserve_source_on_reload() {
        let (_dir, store, lib, input) = setup();
        let saved = store
            .propose_model(draft(2), "clarify", source(input), 2)
            .unwrap();
        let id = saved.proposals[0].id.clone();
        let old = run(&lib, "old-run", 1);
        assert!(store.observe_correction(&id, &old, "case", 3).is_err());
        let approved = store.review(&id, true, 3, "local-owner").unwrap();
        assert!(store.observe_correction(&id, &old, "case", 4).is_err());
        let fresh = run(&approved, "new-run", 2);
        let observed = store.observe_correction(&id, &fresh, "case", 4).unwrap();
        assert_eq!(observed.observations[0].outcome, "failed");
        assert_eq!(observed.observations[0].semantic_validation, "not_verified");
        assert_eq!(store.snapshot().unwrap(), observed);
        let mut forged = serde_json::to_value(&observed).unwrap();
        forged["observations"][0]["outcome"] = json!("passed");
        assert!(serde_json::from_value::<ContractLibrary>(forged).is_err());
        assert!(store.observe_correction(&id, &fresh, "case", 5).is_err());
        let mut legacy = serde_json::to_value(&lib).unwrap();
        legacy.as_object_mut().unwrap().remove("observations");
        assert!(serde_json::from_value::<ContractLibrary>(legacy)
            .unwrap()
            .observations
            .is_empty());
    }
    #[test]
    fn correction_fallback_has_one_call_and_only_retryable_prior_failures() {
        let mut model = model();
        model["candidates"].as_array_mut().unwrap().push(json!({"id":"backup","adapter":"openai_compatible","model":"fixture","capabilities":["text"],"midscene_family":null}));
        let mut attempts = vec![
            ModelCallAttempt {
                call: 1,
                attempt: 1,
                profile_id: "correct-primary".into(),
                outcome: ModelCallOutcome::RateLimited,
            },
            ModelCallAttempt {
                call: 1,
                attempt: 2,
                profile_id: "backup".into(),
                outcome: ModelCallOutcome::Succeeded,
            },
        ];
        validate_attempts(&model, &attempts, true).unwrap();
        attempts[0].outcome = ModelCallOutcome::Rejected;
        assert!(validate_attempts(&model, &attempts, true).is_err());
        attempts[0].outcome = ModelCallOutcome::RateLimited;
        attempts[1].call = 2;
        assert!(validate_attempts(&model, &attempts, true).is_err());
    }
}
