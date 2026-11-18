//! Version 2 execution definitions. These validate intent; they neither upgrade
//! immutable v1 plans nor claim that a browser runner has been prepared.
pub mod runtime;
pub mod store;

use crate::{
    contracts::ValueType,
    flow::{identifier, ContractRef, FlowOutput, InputSource},
    scenario::{Scenario, ScenarioStep},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedCaseRef {
    pub project: String,
    pub case_id: String,
    /// Exact approved content revision, not a signature or authorization token.
    pub content_revision: String,
    pub contract: ContractRef,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "capability", deny_unknown_fields)]
pub enum Operation {
    #[serde(rename = "wait.v1")]
    Wait { milliseconds: InputSource },
    #[serde(rename = "case.execute.v1")]
    ExecuteCase { case: ReviewedCaseRef },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operator", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    Equals {
        source: InputSource,
        value_type: ValueType,
        value: Value,
    },
    NotEquals {
        source: InputSource,
        value_type: ValueType,
        value: Value,
    },
}
/// A false condition skips this step; any skipped dependency skips downstream
/// steps. Missing outputs are errors, never substituted with null or defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionStep {
    pub id: String,
    pub depends_on: Vec<String>,
    pub operation: Operation,
    pub condition: Option<Condition>,
    pub timeout_ms: u64,
    pub max_attempts: u8,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionDefinition {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub project: String,
    pub inputs: BTreeMap<String, ValueType>,
    pub outputs: BTreeMap<String, FlowOutput>,
    pub steps: Vec<ExecutionStep>,
    pub total_timeout_ms: u64,
    /// Separate bounded budget; cleanup errors must retain the initial failure.
    pub cleanup_timeout_ms: u64,
    pub required_permissions: Vec<String>,
}
impl Operation {
    fn capability(&self) -> &'static str {
        match self {
            Self::Wait { .. } => "wait.v1",
            Self::ExecuteCase { .. } => "case.execute.v1",
        }
    }
    fn output(&self) -> (&'static str, ValueType) {
        match self {
            Self::Wait { .. } => ("elapsed_ms", ValueType::Number),
            Self::ExecuteCase { .. } => ("result", ValueType::Object),
        }
    }
}
impl ExecutionDefinition {
    pub fn validate(&self) -> Result<Vec<String>> {
        ensure!(
            self.schema_version == 2,
            "execution definition requires schema 2"
        );
        identifier(&self.id)?;
        identifier(&self.project)?;
        ensure!(
            !self.title.trim().is_empty() && self.title.len() <= 512,
            "invalid execution title"
        );
        ensure!(
            !self.steps.is_empty()
                && self.steps.len() <= 64
                && self.inputs.len() <= 64
                && self.outputs.len() <= 64,
            "execution exceeds structural limits"
        );
        ensure!(
            serde_json::to_vec(self)?.len() <= 256 * 1024,
            "execution definition exceeds size limit"
        );
        ensure!(
            (1..=3_600_000).contains(&self.total_timeout_ms)
                && (1..=15_000).contains(&self.cleanup_timeout_ms),
            "invalid execution or cleanup budget"
        );
        for name in self.inputs.keys().chain(self.outputs.keys()) {
            identifier(name)?;
        }
        let permissions: BTreeSet<_> = self
            .required_permissions
            .iter()
            .map(String::as_str)
            .collect();
        ensure!(
            permissions.len() == self.required_permissions.len()
                && permissions.iter().all(|p| matches!(
                    *p,
                    "browser.read" | "browser.write" | "model.send" | "report.write"
                )),
            "unknown or duplicate execution permission"
        );
        let scenario = Scenario {
            id: self.id.clone(),
            version: 2,
            steps: self
                .steps
                .iter()
                .map(|s| ScenarioStep {
                    id: s.id.clone(),
                    capability: s.operation.capability().into(),
                    depends_on: s.depends_on.clone(),
                    idempotent: matches!(s.operation, Operation::Wait { .. }),
                    max_attempts: s.max_attempts,
                })
                .collect(),
        };
        let order: Vec<String> = scenario
            .execution_order()?
            .into_iter()
            .map(|s| s.id.clone())
            .collect();
        let mut ancestors: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for id in &order {
            let step = self.steps.iter().find(|s| &s.id == id).unwrap();
            identifier(&step.id)?;
            ensure!(
                step.depends_on.len() <= 64
                    && (1..=self.total_timeout_ms).contains(&step.timeout_ms),
                "invalid step dependency or budget"
            );
            let mut upstream = BTreeSet::new();
            for dep in &step.depends_on {
                upstream.insert(dep.clone());
                upstream.extend(ancestors[dep].iter().cloned());
            }
            match &step.operation {
                Operation::Wait { milliseconds } => {
                    self.check_source(milliseconds, ValueType::Number, &upstream)?;
                    if let InputSource::Literal { value } = milliseconds {
                        Self::wait_duration(value, step.timeout_ms)?;
                    }
                }
                Operation::ExecuteCase { case } => {
                    identifier(&case.project)?;
                    identifier(&case.case_id)?;
                    identifier(&case.contract.id)?;
                    ensure!(
                        case.project == self.project
                            && case.contract.version > 0
                            && case.content_revision.starts_with("content:")
                            && case.content_revision.len() > 8
                            && case.content_revision.len() <= 180_000,
                        "invalid reviewed case reference"
                    );
                    ensure!(
                        [
                            "browser.read",
                            "browser.write",
                            "model.send",
                            "report.write"
                        ]
                        .iter()
                        .all(|p| permissions.contains(p)),
                        "reviewed browser execution permissions missing"
                    );
                }
            }
            if let Some(condition) = &step.condition {
                let (source, kind, value) = match condition {
                    Condition::Equals {
                        source,
                        value_type,
                        value,
                    }
                    | Condition::NotEquals {
                        source,
                        value_type,
                        value,
                    } => (source, *value_type, value),
                };
                ensure!(
                    kind != ValueType::Object && kind.accepts(value),
                    "condition requires a typed scalar comparison"
                );
                self.check_source(source, kind, &upstream)?;
            }
            ancestors.insert(id.clone(), upstream);
        }
        let all = order.iter().cloned().collect();
        for output in self.outputs.values() {
            self.check_source(&output.source, output.value_type, &all)?;
        }
        Ok(order)
    }
    fn check_source(
        &self,
        source: &InputSource,
        expected: ValueType,
        upstream: &BTreeSet<String>,
    ) -> Result<()> {
        match source {
            InputSource::Literal { value } => {
                ensure!(expected.accepts(value), "literal type mismatch")
            }
            InputSource::FlowInput { name } => ensure!(
                self.inputs.get(name) == Some(&expected),
                "flow input type mismatch"
            ),
            InputSource::StepOutput { step, port } => {
                ensure!(
                    upstream.contains(step),
                    "output must reference an upstream dependency"
                );
                let operation = &self
                    .steps
                    .iter()
                    .find(|s| &s.id == step)
                    .context("missing output step")?
                    .operation;
                ensure!(
                    operation.output() == (port.as_str(), expected),
                    "missing or incompatible output port"
                );
            }
        }
        Ok(())
    }
    pub fn validate_inputs(&self, inputs: &BTreeMap<String, Value>) -> Result<()> {
        self.validate()?;
        ensure!(
            inputs.keys().eq(self.inputs.keys()) && serde_json::to_vec(inputs)?.len() <= 128 * 1024,
            "execution inputs differ or exceed limits"
        );
        for (key, value) in inputs {
            ensure!(
                self.inputs[key].accepts(value),
                "execution input type mismatch"
            );
        }
        Ok(())
    }
    /// Resolve data only from successful upstream outputs supplied by the
    /// runner. A skipped step has no entry and therefore cannot fabricate data.
    pub fn resolve_source(
        source: &InputSource,
        inputs: &BTreeMap<String, Value>,
        outputs: &BTreeMap<String, BTreeMap<String, Value>>,
    ) -> Result<Value> {
        match source {
            InputSource::Literal { value } => Ok(value.clone()),
            InputSource::FlowInput { name } => {
                inputs.get(name).cloned().context("flow input unavailable")
            }
            InputSource::StepOutput { step, port } => outputs
                .get(step)
                .and_then(|o| o.get(port))
                .cloned()
                .context("upstream output unavailable or skipped"),
        }
    }
    pub fn evaluate_condition(
        condition: &Condition,
        inputs: &BTreeMap<String, Value>,
        outputs: &BTreeMap<String, BTreeMap<String, Value>>,
    ) -> Result<bool> {
        let (source, kind, value, equal) = match condition {
            Condition::Equals {
                source,
                value_type,
                value,
            } => (source, *value_type, value, true),
            Condition::NotEquals {
                source,
                value_type,
                value,
            } => (source, *value_type, value, false),
        };
        let actual = Self::resolve_source(source, inputs, outputs)?;
        ensure!(
            kind != ValueType::Object && kind.accepts(&actual) && kind.accepts(value),
            "condition value type mismatch"
        );
        Ok((actual == *value) == equal)
    }
    pub fn wait_duration(value: &Value, step_timeout_ms: u64) -> Result<u64> {
        let ms = value
            .as_u64()
            .context("wait must be whole nonnegative milliseconds")?;
        ensure!(
            ms <= 300_000 && ms < step_timeout_ms,
            "wait exceeds step budget or five minutes"
        );
        Ok(ms)
    }
    pub fn validate_step_outputs(
        &self,
        step_id: &str,
        values: &BTreeMap<String, Value>,
    ) -> Result<()> {
        self.validate()?;
        let step = self
            .steps
            .iter()
            .find(|s| s.id == step_id)
            .context("unknown execution step")?;
        let (port, kind) = step.operation.output();
        ensure!(
            values.len() == 1
                && values.get(port).is_some_and(|v| kind.accepts(v))
                && serde_json::to_vec(values)?.len() <= 128 * 1024,
            "invalid execution output ports, types or size"
        );
        Ok(())
    }
}

/// Private immutable approved-case preparation. Execution/model authorization
/// is deliberately separate and must be checked before queue admission.
#[derive(Clone, Debug, Serialize)]
pub struct PreparedCase {
    reference: ReviewedCaseRef,
    target_url: String,
    name: String,
    yaml_script: String,
    expected_result: String,
    provenance: crate::execution_records::CaseProvenance,
    contracts: crate::contract_execution::ExecutionSnapshot,
}
impl PreparedCase {
    pub fn run_case(&self) -> crate::execution_records::RunCase {
        crate::execution_records::RunCase {
            case_id: self.reference.case_id.clone(),
            name: self.name.clone(),
            yaml_script: self.yaml_script.clone(),
            target_url: self.target_url.clone(),
            provenance: Some(self.provenance.clone()),
        }
    }
    pub fn reference(&self) -> &ReviewedCaseRef {
        &self.reference
    }
    pub fn target_url(&self) -> &str {
        &self.target_url
    }
    pub fn yaml_script(&self) -> &str {
        &self.yaml_script
    }
    pub fn expected_result(&self) -> &str {
        &self.expected_result
    }
    pub fn contracts(&self) -> &crate::contract_execution::ExecutionSnapshot {
        &self.contracts
    }
    /// Use the case store's locked, recovery-aware read rather than reading
    /// JSON files directly. Later edits cannot mutate the returned preparation.
    pub fn prepare(
        reference: &ReviewedCaseRef,
        project: &crate::storage::ProjectConfig,
        expected_target: &str,
        contracts: &crate::contract_execution::ExecutionSnapshot,
    ) -> Result<Self> {
        let asset = crate::LocalStorage::get_case_asset(&reference.project, &reference.case_id)?;
        Self::from_asset(reference, project, expected_target, contracts, asset)
    }
    fn from_asset(
        reference: &ReviewedCaseRef,
        project: &crate::storage::ProjectConfig,
        expected_target: &str,
        contracts: &crate::contract_execution::ExecutionSnapshot,
        asset: crate::storage::local::CaseAsset,
    ) -> Result<Self> {
        identifier(&reference.project)?;
        identifier(&reference.case_id)?;
        identifier(&reference.contract.id)?;
        ensure!(
            reference.project == project.name && contracts.project() == project.name,
            "reviewed case belongs to another project"
        );
        let url = reqwest::Url::parse(expected_target)?;
        ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && expected_target.len() <= 8192
                && project.url == expected_target,
            "project target changed or is invalid; prepare again"
        );
        ensure!(
            asset.case_id == reference.case_id
                && asset.status == crate::storage::CaseStatus::Approved
                && asset.digest == reference.content_revision,
            "approved case content revision changed; review and prepare again"
        );
        let validated: crate::contract_execution::ExecutionSnapshot =
            serde_json::from_value(serde_json::to_value(contracts)?)?;
        let source = asset
            .provenance
            .context("reviewed flow case requires contract provenance")?;
        source.validate(&reference.project, &reference.case_id, &asset.yaml_script)?;
        let historical = source
            .contracts
            .as_ref()
            .context("case has no original contract assertions")?;
        let matches = |c: &&crate::contract_execution::ExecutionContract| {
            c.publication().definition().id == reference.contract.id
                && c.publication().definition().version == reference.contract.version
        };
        let current = validated
            .contracts()
            .iter()
            .find(matches)
            .context("fixed contract is not currently bound")?;
        let original = historical
            .contracts()
            .iter()
            .find(matches)
            .context("case source does not contain fixed contract")?;
        ensure!(current==original,"reviewed case binding, parameters or original assertions differ; regenerate and review");
        // A generated case can implement several bound contracts. The primary
        // reference cannot authorize silently dropping another original rule.
        for original in historical.contracts() {
            let definition = original.publication().definition();
            let current = validated
                .contracts()
                .iter()
                .find(|candidate| {
                    let candidate = candidate.publication().definition();
                    candidate.id == definition.id && candidate.version == definition.version
                })
                .context("original case contract is no longer bound; regenerate and review")?;
            ensure!(
                current == original,
                "an original case contract binding or assertion changed; regenerate and review"
            );
        }

        Ok(Self {
            reference: reference.clone(),
            target_url: expected_target.into(),
            name: asset.name,
            yaml_script: asset.yaml_script,
            expected_result: asset.expected_result,
            provenance: source,
            contracts: validated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    pub(super) fn flow() -> ExecutionDefinition {
        ExecutionDefinition {
            schema_version: 2,
            id: "flow".into(),
            title: "Reviewed flow".into(),
            project: "project".into(),
            inputs: BTreeMap::from([("enabled".into(), ValueType::Boolean)]),
            outputs: BTreeMap::new(),
            steps: vec![ExecutionStep {
                id: "pause".into(),
                depends_on: vec![],
                operation: Operation::Wait {
                    milliseconds: InputSource::Literal { value: json!(10) },
                },
                condition: Some(Condition::Equals {
                    source: InputSource::FlowInput {
                        name: "enabled".into(),
                    },
                    value_type: ValueType::Boolean,
                    value: json!(true),
                }),
                timeout_ms: 100,
                max_attempts: 1,
            }],
            total_timeout_ms: 1000,
            cleanup_timeout_ms: 1000,
            required_permissions: vec![],
        }
    }
    #[test]
    fn typed_conditions_reject_coercion_and_missing_skipped_outputs() {
        let flow = flow();
        flow.validate().unwrap();
        let condition = flow.steps[0].condition.as_ref().unwrap();
        assert!(ExecutionDefinition::evaluate_condition(
            condition,
            &BTreeMap::from([("enabled".into(), json!(true))]),
            &BTreeMap::new()
        )
        .unwrap());
        assert!(!ExecutionDefinition::evaluate_condition(
            condition,
            &BTreeMap::from([("enabled".into(), json!(false))]),
            &BTreeMap::new()
        )
        .unwrap());
        assert!(ExecutionDefinition::evaluate_condition(
            condition,
            &BTreeMap::from([("enabled".into(), json!("true"))]),
            &BTreeMap::new()
        )
        .is_err());
        assert!(ExecutionDefinition::resolve_source(
            &InputSource::StepOutput {
                step: "skipped".into(),
                port: "result".into()
            },
            &BTreeMap::new(),
            &BTreeMap::new()
        )
        .is_err());
    }
    #[test]
    fn dataflow_requires_upstream_ports_and_preserves_stable_order() {
        let mut f = flow();
        let mut next = f.steps[0].clone();
        next.id = "next".into();
        next.depends_on = vec!["pause".into()];
        next.condition = None;
        next.operation = Operation::Wait {
            milliseconds: InputSource::StepOutput {
                step: "pause".into(),
                port: "elapsed_ms".into(),
            },
        };
        f.steps.insert(0, next);
        assert_eq!(f.validate().unwrap(), vec!["pause", "next"]);
        f.steps[0].depends_on.clear();
        assert!(f.validate().is_err());
    }
    #[test]
    fn rejects_unbounded_waits_budgets_versions_and_side_effect_retries() {
        for value in [
            json!(-1),
            json!(1.5),
            json!("10"),
            json!(300001),
            json!(100),
        ] {
            assert!(ExecutionDefinition::wait_duration(&value, 100).is_err());
        }
        let mut f = flow();
        f.schema_version = 1;
        assert!(f.validate().is_err());
        f.schema_version = 2;
        f.steps[0].operation = Operation::ExecuteCase {
            case: ReviewedCaseRef {
                project: "project".into(),
                case_id: "case".into(),
                content_revision: "content:exact".into(),
                contract: ContractRef {
                    id: "contract".into(),
                    version: 1,
                },
            },
        };
        f.required_permissions = [
            "browser.read",
            "browser.write",
            "model.send",
            "report.write",
        ]
        .map(String::from)
        .to_vec();
        f.steps[0].max_attempts = 2;
        assert!(f.validate().is_err());
        f.steps[0].max_attempts = 1;
        f.validate().unwrap();
        f.cleanup_timeout_ms = 15001;
        assert!(f.validate().is_err());
    }
    #[test]
    fn validates_runtime_outputs_and_strict_schema() {
        let f = flow();
        f.validate_step_outputs("pause", &BTreeMap::from([("elapsed_ms".into(), json!(10))]))
            .unwrap();
        assert!(f
            .validate_step_outputs(
                "pause",
                &BTreeMap::from([("elapsed_ms".into(), json!("10"))])
            )
            .is_err());
        let mut value = serde_json::to_value(&f).unwrap();
        value["steps"][0]["expression"] = json!("shell()");
        assert!(serde_json::from_value::<ExecutionDefinition>(value).is_err());
        let round: ExecutionDefinition =
            serde_json::from_value(serde_json::to_value(&f).unwrap()).unwrap();
        assert_eq!(round, f);
    }
    pub(super) fn approved_fixture() -> (
        ReviewedCaseRef,
        crate::storage::ProjectConfig,
        crate::contract_execution::ExecutionSnapshot,
        crate::storage::local::CaseAsset,
    ) {
        use crate::{
            contract_store::ContractLibrary, contracts::ContractDraft,
            execution_records::CaseProvenance,
        };
        let publication = ContractDraft {
            id: "receipt".into(),
            version: 1,
            intent: "receipt".into(),
            inputs: BTreeMap::from([("url".into(), ValueType::String)]),
            outputs: BTreeMap::new(),
            preconditions: vec![],
            assertions: vec!["receipt matches input".into()],
            required_capabilities: vec!["browser.interact".into()],
        }
        .publish("owner")
        .unwrap();
        let binding = publication
            .bind(
                "project",
                BTreeMap::from([("url".into(), json!("https://example.test"))]),
            )
            .unwrap();
        let library = ContractLibrary {
            revision: 1,
            publications: vec![publication],
            bindings: vec![binding],
            ..Default::default()
        };
        let snapshot = crate::contract_execution::ExecutionSnapshot::prepare(
            &library,
            "project",
            &["browser.interact".into()],
        )
        .unwrap();
        let project = crate::storage::ProjectConfig {
            name: "project".into(),
            url: "https://example.test".into(),
            tech_stack: vec![],
            created_at: "fixture".into(),
        };
        let reference = ReviewedCaseRef {
            project: "project".into(),
            case_id: "case".into(),
            content_revision: "content:exact-approved-revision".into(),
            contract: ContractRef {
                id: "receipt".into(),
                version: 1,
            },
        };
        let asset = crate::storage::local::CaseAsset {
            case_id: "case".into(),
            name: "receipt".into(),
            yaml_script: "tasks: []".into(),
            expected_result: "receipt matches input".into(),
            status: crate::storage::CaseStatus::Approved,
            digest: reference.content_revision.clone(),
            provenance: Some(CaseProvenance {
                schema_version: 1,
                project: "project".into(),
                case_id: "case".into(),
                yaml_script: "tasks: []".into(),
                created_at: "fixture".into(),
                contracts: Some(snapshot.clone()),
                model: json!({"source":"legacy_config","model":"fixture"}),
                model_calls: None,
                specification: None,
            }),
        };
        (reference, project, snapshot, asset)
    }
    #[test]
    fn preparation_freezes_reviewed_content_and_original_assertions() {
        let (reference, project, snapshot, asset) = approved_fixture();
        let prepared =
            PreparedCase::from_asset(&reference, &project, &project.url, &snapshot, asset).unwrap();
        assert_eq!(prepared.yaml_script(), "tasks: []");
        assert_eq!(prepared.target_url(), project.url);
        assert_eq!(
            prepared.contracts().contracts()[0].original_assertions(),
            &["receipt matches input"]
        );
        assert_eq!(prepared.reference(), &reference);
    }
    #[test]
    fn preparation_rejects_stale_unapproved_cross_project_target_and_source_changes() {
        for fault in 0..7 {
            let (mut reference, mut project, snapshot, mut asset) = approved_fixture();
            match fault {
                0 => asset.digest = "changed".into(),
                1 => asset.status = crate::storage::CaseStatus::Draft,
                2 => project.name = "other".into(),
                3 => project.url = "https://changed.test".into(),
                4 => asset.yaml_script = "modified".into(),
                5 => reference.contract.version = 2,
                _ => asset.provenance = None,
            }
            assert!(
                PreparedCase::from_asset(
                    &reference,
                    &project,
                    "https://example.test",
                    &snapshot,
                    asset
                )
                .is_err(),
                "fault {fault} was accepted"
            );
        }
        let (reference, project, snapshot, mut asset) = approved_fixture();
        let historical = asset
            .provenance
            .as_mut()
            .unwrap()
            .contracts
            .as_ref()
            .unwrap();
        let mut record = serde_json::to_value(historical).unwrap();
        record["contracts"][0]["binding"]["parameters"]["url"] = json!("https://other.test");
        asset.provenance.as_mut().unwrap().contracts =
            Some(serde_json::from_value(record).unwrap());
        assert!(
            PreparedCase::from_asset(&reference, &project, &project.url, &snapshot, asset).is_err()
        );
    }
    #[test]
    fn preparation_checks_every_source_contract_and_allows_unrelated_new_bindings() {
        let (reference, project, snapshot, mut asset) = approved_fixture();
        let mut expanded = serde_json::to_value(&snapshot).unwrap();
        let mut second = expanded["contracts"][0].clone();
        second["binding"]["contract_id"] = json!("secondary");
        second["publication"]["definition"]["id"] = json!("secondary");
        expanded["contracts"].as_array_mut().unwrap().push(second);
        let expanded_snapshot: crate::contract_execution::ExecutionSnapshot =
            serde_json::from_value(expanded.clone()).unwrap();
        // A new unrelated binding does not invalidate the originally approved case.
        PreparedCase::from_asset(
            &reference,
            &project,
            &project.url,
            &expanded_snapshot,
            asset,
        )
        .unwrap();
        let (_, _, _, new_asset) = approved_fixture();
        asset = new_asset;
        asset.provenance.as_mut().unwrap().contracts = Some(expanded_snapshot.clone());
        PreparedCase::from_asset(
            &reference,
            &project,
            &project.url,
            &expanded_snapshot,
            asset,
        )
        .unwrap();
        for change in 0..3 {
            let (_, _, _, mut asset) = approved_fixture();
            asset.provenance.as_mut().unwrap().contracts = Some(expanded_snapshot.clone());
            let mut current = expanded.clone();
            match change {
                0 => {
                    current["contracts"].as_array_mut().unwrap().pop();
                }
                1 => {
                    current["contracts"][1]["binding"]["parameters"]["url"] =
                        json!("https://changed.test");
                }
                _ => {
                    current["contracts"][1]["publication"]["definition"]["assertions"] =
                        json!(["new secondary rule"]);
                    current["contracts"][1]["original_assertions"] = json!(["new secondary rule"]);
                }
            }
            let current = serde_json::from_value(current).unwrap();
            assert!(
                PreparedCase::from_asset(&reference, &project, &project.url, &current, asset)
                    .is_err(),
                "secondary contract change {change} escaped validation"
            );
        }
    }
}
