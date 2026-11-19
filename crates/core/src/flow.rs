//! Typed declarative flow governance. Validation never executes an adapter.
use crate::{
    contracts::ValueType,
    scenario::{Scenario, ScenarioStep},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InputSource {
    Literal { value: Value },
    FlowInput { name: String },
    StepOutput { step: String, port: String },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractRef {
    pub id: String,
    pub version: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowOutput {
    pub value_type: ValueType,
    pub source: InputSource,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowStep {
    pub id: String,
    pub capability: String,
    pub depends_on: Vec<String>,
    pub inputs: BTreeMap<String, InputSource>,
    pub contract: Option<ContractRef>,
    pub model_role: Option<String>,
    pub max_attempts: u8,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowDraft {
    pub id: String,
    pub title: String,
    pub inputs: BTreeMap<String, ValueType>,
    pub outputs: BTreeMap<String, FlowOutput>,
    pub steps: Vec<FlowStep>,
    pub required_permissions: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct CapabilityDescriptor {
    pub id: String,
    pub inputs: BTreeMap<String, ValueType>,
    pub outputs: BTreeMap<String, ValueType>,
    pub permissions: Vec<String>,
    pub idempotent: bool,
    pub planning_supported: bool,
    pub runner_available: bool,
}
/// Immutable v1 planning contracts, not a runtime registration. Preserve these
/// descriptors when extending the catalog: historical v1 plans validate them.
pub fn capability_catalog() -> Vec<CapabilityDescriptor> {
    use ValueType::{Object, String as Text};
    [
        (
            "browser.observe",
            vec![("target", Text)],
            "observation",
            "browser.read",
            true,
        ),
        (
            "browser.interact",
            vec![("target", Text), ("instruction", Text)],
            "result",
            "browser.write",
            false,
        ),
        (
            "model.generate",
            vec![("requirement", Text)],
            "cases",
            "model.send",
            true,
        ),
        (
            "report.collect",
            vec![("result", Object)],
            "report",
            "report.write",
            true,
        ),
    ]
    .into_iter()
    .map(
        |(id, inputs, output, permission, idempotent)| CapabilityDescriptor {
            id: id.into(),
            inputs: inputs.into_iter().map(|(k, v)| (k.into(), v)).collect(),
            outputs: BTreeMap::from([(output.into(), Object)]),
            permissions: vec![permission.into()],
            idempotent,
            planning_supported: true,
            runner_available: false,
        },
    )
    .collect()
}
#[derive(Clone, Debug, Serialize)]
pub struct FlowValidation {
    pub execution_order: Vec<String>,
    pub capabilities: Vec<CapabilityDescriptor>,
}
pub(crate) fn identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 128
            && !value.starts_with('.')
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_')),
        "invalid flow identity"
    );
    Ok(())
}
impl FlowDraft {
    pub fn validate(&self) -> Result<FlowValidation> {
        identifier(&self.id)?;
        ensure!(
            !self.title.trim().is_empty() && self.title.len() <= 512,
            "invalid flow title"
        );
        ensure!(
            !self.steps.is_empty()
                && self.steps.len() <= 64
                && self.inputs.len() <= 64
                && self.outputs.len() <= 64
                && self.required_permissions.len() <= 16,
            "flow exceeds structural limits"
        );
        ensure!(
            serde_json::to_vec(self)?.len() <= 256 * 1024,
            "flow exceeds 256 KiB"
        );
        let permissions: BTreeSet<_> = self.required_permissions.iter().collect();
        ensure!(
            permissions.len() == self.required_permissions.len(),
            "duplicate flow permission"
        );
        let catalog = capability_catalog();
        let known: BTreeSet<_> = catalog.iter().flat_map(|c| c.permissions.iter()).collect();
        ensure!(permissions.is_subset(&known), "unknown flow permission");
        for name in self.inputs.keys().chain(self.outputs.keys()) {
            identifier(name)?;
        }
        let mut descriptors = BTreeMap::new();
        let mut scenario = Scenario {
            id: self.id.clone(),
            version: 1,
            steps: vec![],
        };
        for step in &self.steps {
            identifier(&step.id)?;
            ensure!(
                step.depends_on.len() <= 64 && step.inputs.len() <= 64,
                "step exceeds limits"
            );
            let desc = catalog
                .iter()
                .find(|c| c.id == step.capability)
                .context("unknown flow capability")?;
            ensure!(
                desc.permissions.iter().all(|p| permissions.contains(p)),
                "missing required flow permission"
            );
            ensure!(
                step.inputs.keys().eq(desc.inputs.keys()),
                "step input ports must exactly match capability"
            );
            let browser = step.capability.starts_with("browser.");
            ensure!(
                step.contract.is_some() == browser,
                "browser steps require a fixed contract; other steps cannot bind one"
            );
            if let Some(reference) = &step.contract {
                identifier(&reference.id)?;
                ensure!(reference.version > 0, "contract version required");
            }
            match step.capability.as_str() {
                "model.generate" => ensure!(
                    step.model_role.as_deref() == Some("generate"),
                    "model.generate requires generate role"
                ),
                "browser.observe" | "browser.interact" => ensure!(
                    step.model_role.as_deref().is_none_or(|r| r == "execute"),
                    "browser model role must be execute"
                ),
                _ => ensure!(
                    step.model_role.is_none(),
                    "capability does not use a model role"
                ),
            }
            scenario.steps.push(ScenarioStep {
                id: step.id.clone(),
                capability: step.capability.clone(),
                depends_on: step.depends_on.clone(),
                idempotent: desc.idempotent,
                max_attempts: step.max_attempts,
            });
            descriptors.insert(step.id.clone(), desc);
        }
        let order: Vec<String> = scenario
            .execution_order()?
            .iter()
            .map(|s| s.id.clone())
            .collect();
        let mut ancestors: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for id in &order {
            let step = self.steps.iter().find(|s| &s.id == id).unwrap();
            let mut upstream = BTreeSet::new();
            for dep in &step.depends_on {
                upstream.insert(dep.clone());
                upstream.extend(ancestors[dep].iter().cloned());
            }
            for (port, source) in &step.inputs {
                self.check_source(
                    source,
                    descriptors[id].inputs[port],
                    &upstream,
                    &descriptors,
                )?;
            }
            ancestors.insert(id.clone(), upstream);
        }
        let all = order.iter().cloned().collect();
        for output in self.outputs.values() {
            self.check_source(&output.source, output.value_type, &all, &descriptors)?;
        }
        Ok(FlowValidation {
            execution_order: order,
            capabilities: catalog
                .into_iter()
                .filter(|c| self.steps.iter().any(|s| s.capability == c.id))
                .collect(),
        })
    }
    fn check_source(
        &self,
        source: &InputSource,
        expected: ValueType,
        upstream: &BTreeSet<String>,
        descriptors: &BTreeMap<String, &CapabilityDescriptor>,
    ) -> Result<()> {
        match source {
            InputSource::Literal { value } => {
                ensure!(expected.accepts(value), "literal input type mismatch")
            }
            InputSource::FlowInput { name } => ensure!(
                self.inputs.get(name) == Some(&expected),
                "missing or incompatible flow input"
            ),
            InputSource::StepOutput { step, port } => {
                ensure!(
                    upstream.contains(step),
                    "step output is not an upstream dependency"
                );
                ensure!(
                    descriptors.get(step).and_then(|d| d.outputs.get(port)) == Some(&expected),
                    "missing or incompatible output port"
                );
            }
        }
        Ok(())
    }
    pub fn validate_inputs(&self, inputs: &BTreeMap<String, Value>) -> Result<()> {
        self.validate()?;
        ensure!(
            inputs.keys().eq(self.inputs.keys()),
            "flow input keys do not match declaration"
        );
        for (key, value) in inputs {
            ensure!(
                self.inputs[key].accepts(value),
                "flow input type mismatch: {}",
                key
            );
        }
        Ok(())
    }
}
