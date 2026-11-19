//! Immutable planning evidence. Preparing or reading a plan never executes an action.
use crate::{
    asset_file::AssetFile,
    contract_execution::ExecutionSnapshot,
    contract_store::ContractStore,
    flow::{identifier, FlowDraft},
    flow_store::{FlowStore, PublishedFlow},
    models::ModelRegistry,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FlowPlanRequest {
    pub id: String,
    pub version: u32,
    pub project: String,
    pub inputs: BTreeMap<String, Value>,
    pub permissions: Vec<String>,
    pub expected_revision: u64,
    pub expected_model_revision: u64,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedModel {
    pub id: String,
    pub adapter: String,
    pub base_url: String,
    pub model: String,
    pub capabilities: Vec<String>,
    pub midscene_family: Option<String>,
    pub allow_remote: bool,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedRoute {
    pub revision: u64,
    pub task: String,
    pub candidates: Vec<PlannedModel>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FlowPlan {
    pub schema_version: u32,
    pub plan_id: String,
    pub project: String,
    pub project_target: String,
    pub flow_id: String,
    pub flow_version: u32,
    pub publication: PublishedFlow,
    pub inputs: BTreeMap<String, Value>,
    pub permissions: Vec<String>,
    pub execution_order: Vec<String>,
    pub capabilities: Vec<Value>,
    pub contracts: Option<ExecutionSnapshot>,
    pub models: BTreeMap<String, PlannedRoute>,
    pub flow_revision: u64,
    pub contract_revision: u64,
    pub model_revision: u64,
    pub executable: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FlowPlanSummary {
    pub plan_id: String,
    pub project: String,
    pub flow_id: String,
    pub flow_version: u32,
    pub executable: bool,
}
#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Plans {
    revision: u64,
    plans: Vec<FlowPlan>,
}
fn inputs(definition: &FlowDraft, values: &BTreeMap<String, Value>) -> Result<()> {
    ensure!(
        definition.inputs.keys().eq(values.keys()),
        "flow inputs must exactly match declared ports"
    );
    for (name, kind) in &definition.inputs {
        ensure!(
            kind.accepts(&values[name]),
            "flow input type mismatch: {name}"
        );
    }
    ensure!(
        serde_json::to_vec(values)?.len() <= 128 * 1024,
        "flow input values exceed limit"
    );
    Ok(())
}
fn permissions(definition: &FlowDraft, provided: &[String]) -> Result<()> {
    let a: BTreeSet<_> = provided.iter().collect();
    let b: BTreeSet<_> = definition.required_permissions.iter().collect();
    ensure!(
        a.len() == provided.len() && a == b,
        "explicit planning permissions must exactly match flow requirements"
    );
    Ok(())
}
fn target(value: &str) -> Result<()> {
    let url = reqwest::Url::parse(value).context("invalid project target")?;
    ensure!(
        value.len() <= 8192
            && matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none(),
        "invalid project target"
    );
    Ok(())
}
fn references(definition: &FlowDraft) -> Vec<(String, u32)> {
    definition
        .steps
        .iter()
        .filter_map(|s| s.contract.as_ref().map(|r| (r.id.clone(), r.version)))
        .collect()
}
fn contract_caps(capability: &str) -> Vec<String> {
    match capability {
        "browser.observe" => vec!["browser.visual".into(), "browser.assert".into()],
        "browser.interact" => vec![
            "browser.visual".into(),
            "browser.assert".into(),
            "browser.interact".into(),
        ],
        _ => vec![],
    }
}
impl FlowPlan {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1 && !self.executable && self.flow_revision > 0,
            "unsupported or executable flow plan"
        );
        identifier(&self.plan_id)?;
        identifier(&self.project)?;
        target(&self.project_target)?;
        let d = self.publication.definition();
        let validation = d.validate()?;
        ensure!(
            self.flow_id == d.id
                && self.flow_version == self.publication.version()
                && self.flow_version > 0
                && !self.publication.reviewer().trim().is_empty(),
            "flow publication identity mismatch"
        );
        inputs(d, &self.inputs)?;
        permissions(d, &self.permissions)?;
        // Browser target is project context. Contract business parameters remain
        // separate frozen bindings and never silently override these ports.
        for step in &d.steps {
            if step.capability.starts_with("browser.") {
                let source = step
                    .inputs
                    .get("target")
                    .context("browser target port missing")?;
                let value = match source {
                    crate::flow::InputSource::Literal { value } => Some(value),
                    crate::flow::InputSource::FlowInput { name } => self.inputs.get(name),
                    crate::flow::InputSource::StepOutput { .. } => None,
                }
                .and_then(Value::as_str)
                .context("browser target must be a resolved static string")?;
                ensure!(
                    value == self.project_target,
                    "browser step target differs from frozen project target"
                );
            }
        }

        ensure!(
            self.execution_order == validation.execution_order
                && self.capabilities
                    == validation
                        .capabilities
                        .iter()
                        .map(serde_json::to_value)
                        .collect::<std::result::Result<Vec<_>, _>>()?,
            "flow plan capability or order mismatch"
        );
        let refs: BTreeSet<_> = references(d).into_iter().collect();
        match &self.contracts {
            Some(c) => {
                ensure!(
                    c.project() == self.project && c.library_revision() == self.contract_revision,
                    "plan contract project/revision mismatch"
                );
                let actual: BTreeSet<_> = c
                    .contracts()
                    .iter()
                    .map(|x| {
                        (
                            x.publication().definition().id.clone(),
                            x.publication().definition().version,
                        )
                    })
                    .collect();
                ensure!(
                    actual == refs,
                    "plan contracts differ from selected references"
                );
                for s in &d.steps {
                    if let Some(r) = &s.contract {
                        let item = c
                            .contracts()
                            .iter()
                            .find(|x| {
                                x.publication().definition().id == r.id
                                    && x.publication().definition().version == r.version
                            })
                            .context("plan contract missing")?;
                        let allowed = contract_caps(&s.capability);
                        ensure!(
                            item.publication()
                                .definition()
                                .required_capabilities
                                .iter()
                                .all(|x| allowed.contains(x)),
                            "contract requires unavailable step capability"
                        );
                    }
                }
            }
            None => ensure!(refs.is_empty(), "plan contracts missing"),
        }
        let roles: BTreeMap<_, _> = d
            .steps
            .iter()
            .filter_map(|s| s.model_role.as_ref().map(|r| (s.id.as_str(), r.as_str())))
            .collect();
        ensure!(
            roles.len() == self.models.len(),
            "plan model routes differ from steps"
        );
        for (step, route) in &self.models {
            ensure!(
                roles.get(step.as_str()) == Some(&route.task.as_str())
                    && route.revision == self.model_revision
                    && !route.candidates.is_empty()
                    && route.candidates.len() <= 4,
                "invalid plan model route"
            );
            let mut ids = BTreeSet::new();
            for c in &route.candidates {
                identifier(&c.id)?;
                let endpoint =
                    reqwest::Url::parse(&c.base_url).context("invalid planned model endpoint")?;
                ensure!(
                    c.base_url.len() <= 8192
                        && matches!(endpoint.scheme(), "http" | "https")
                        && endpoint.host_str().is_some()
                        && endpoint.username().is_empty()
                        && endpoint.password().is_none()
                        && endpoint.query().is_none()
                        && endpoint.fragment().is_none(),
                    "unsafe planned model endpoint"
                );
                let local = matches!(
                    endpoint.host_str(),
                    Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
                );
                ensure!(
                    local || c.allow_remote,
                    "planned remote model lacks authorization"
                );
                ensure!(
                    ids.insert(&c.id)
                        && !c.adapter.trim().is_empty()
                        && !c.model.trim().is_empty()
                        && c.model.len() <= 1024
                        && c.capabilities.iter().any(|x| x == "text")
                        && c.capabilities
                            .iter()
                            .all(|x| matches!(x.as_str(), "text" | "vision" | "json" | "tools")),
                    "invalid plan model candidate"
                );
                if route.task == "execute" || route.task == "explore" {
                    ensure!(
                        route.candidates.len() == 1
                            && c.capabilities.iter().any(|x| x == "vision")
                            && c.midscene_family.is_some(),
                        "invalid visual plan model"
                    );
                }
            }
        }
        Ok(())
    }
}
pub struct FlowPlanStore {
    file: AssetFile,
}
impl FlowPlanStore {
    pub fn new(state_root: PathBuf) -> Self {
        Self {
            file: AssetFile::new(state_root.join("flows/plans.json"), 8 * 1024 * 1024),
        }
    }
    fn read(guard: &crate::asset_file::AssetGuard<'_>) -> Result<Plans> {
        let plans: Plans = guard
            .read()?
            .map(|b| serde_json::from_slice(&b))
            .transpose()
            .context("invalid flow plans")?
            .unwrap_or_default();
        ensure!(plans.plans.len() <= 100, "flow plan history limit exceeded");
        let mut ids = BTreeSet::new();
        for p in &plans.plans {
            p.validate()?;
            ensure!(ids.insert(&p.plan_id), "duplicate flow plan identity");
            let seq = p
                .plan_id
                .strip_prefix("plan-")
                .and_then(|s| s.parse::<u64>().ok())
                .context("invalid flow plan sequence")?;
            ensure!(
                seq > 0 && seq <= plans.revision,
                "invalid flow plan revision"
            );
        }
        Ok(plans)
    }
    pub fn list(&self) -> Result<Vec<FlowPlanSummary>> {
        let guard = self.file.lock()?;
        Ok(Self::read(&guard)?
            .plans
            .into_iter()
            .map(|p| FlowPlanSummary {
                plan_id: p.plan_id,
                project: p.project,
                flow_id: p.flow_id,
                flow_version: p.flow_version,
                executable: false,
            })
            .collect())
    }
    pub fn get(&self, id: &str) -> Result<FlowPlan> {
        identifier(id)?;
        let guard = self.file.lock()?;
        Self::read(&guard)?
            .plans
            .into_iter()
            .find(|p| p.plan_id == id)
            .context("flow plan does not exist")
    }
    pub fn prepare(
        &self,
        flows: &FlowStore,
        contracts: &ContractStore,
        models: &ModelRegistry,
        request: FlowPlanRequest,
        project_target: &str,
    ) -> Result<FlowPlan> {
        identifier(&request.project)?;
        target(project_target)?;
        let library = flows.snapshot()?;
        ensure!(
            library.revision() == request.expected_revision,
            "flow library revision conflict"
        );
        let publication = library.publication(&request.id, request.version)?.clone();
        let d = publication.definition();
        let validation = d.validate()?;
        inputs(d, &request.inputs)?;
        permissions(d, &request.permissions)?;
        let contract_library = contracts.snapshot()?;
        let refs = references(d);
        let available: BTreeSet<_> = d
            .steps
            .iter()
            .flat_map(|s| contract_caps(&s.capability))
            .collect();
        let frozen_contracts = if refs.is_empty() {
            None
        } else {
            Some(ExecutionSnapshot::prepare_selected(
                &contract_library,
                &request.project,
                &refs,
                &available.into_iter().collect::<Vec<_>>(),
            )?)
        };
        ensure!(
            models.snapshot()?.revision == request.expected_model_revision,
            "model registry revision conflict"
        );
        let mut frozen_models = BTreeMap::new();
        let mut secrets = Vec::new();
        for step in &d.steps {
            if let Some(role) = &step.model_role {
                let required = if step.capability.starts_with("browser.") {
                    vec!["text", "vision"]
                } else {
                    vec!["text"]
                };
                let route = models.resolve_for_project(Some(&request.project), role, &required)?;
                ensure!(
                    route.revision() == request.expected_model_revision,
                    "model registry changed during preparation"
                );
                secrets.extend(
                    route
                        .candidates()
                        .iter()
                        .map(|c| c.api_key.clone())
                        .filter(|s| !s.is_empty()),
                );
                let candidates = route
                    .candidates()
                    .iter()
                    .map(|c| PlannedModel {
                        id: c.id.clone(),
                        adapter: c.adapter.clone(),
                        base_url: c.base_url.clone(),
                        model: c.model.clone(),
                        capabilities: c.capabilities.clone(),
                        midscene_family: c.midscene_family.clone(),
                        allow_remote: c.allow_remote,
                    })
                    .collect();
                frozen_models.insert(
                    step.id.clone(),
                    PlannedRoute {
                        revision: route.revision(),
                        task: route.task().into(),
                        candidates,
                    },
                );
            }
        }
        ensure!(
            models.snapshot()?.revision == request.expected_model_revision
                && flows.snapshot()?.revision() == library.revision()
                && contracts.snapshot()?.revision == contract_library.revision,
            "plan sources changed during preparation"
        );
        let guard = self.file.lock()?;
        let mut history = Self::read(&guard)?;
        ensure!(
            history.plans.len() < 100,
            "flow plan history full; existing plans preserved"
        );
        let next = history
            .revision
            .checked_add(1)
            .context("flow plan sequence exhausted")?;
        let plan = FlowPlan {
            schema_version: 1,
            plan_id: format!("plan-{next}"),
            project: request.project,
            project_target: project_target.into(),
            flow_id: request.id,
            flow_version: request.version,
            publication,
            inputs: request.inputs,
            permissions: request.permissions,
            execution_order: validation.execution_order,
            capabilities: validation
                .capabilities
                .iter()
                .map(serde_json::to_value)
                .collect::<std::result::Result<Vec<_>, _>>()?,
            contracts: frozen_contracts,
            models: frozen_models,
            flow_revision: library.revision(),
            contract_revision: contract_library.revision,
            model_revision: request.expected_model_revision,
            executable: false,
        };
        plan.validate()?;
        fn no_secrets(value: &Value, secrets: &[String]) -> bool {
            match value {
                Value::String(s) => !secrets.iter().any(|key| s.contains(key)),
                Value::Array(a) => a.iter().all(|v| no_secrets(v, secrets)),
                Value::Object(o) => o.iter().all(|(k, v)| {
                    !secrets.iter().any(|key| k.contains(key)) && no_secrets(v, secrets)
                }),
                _ => true,
            }
        }
        ensure!(
            no_secrets(&serde_json::to_value(&plan)?, &secrets),
            "plan metadata contains credential material"
        );
        history.revision = next;
        history.plans.push(plan.clone());
        guard.write(&serde_json::to_vec_pretty(&history)?)?;
        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contracts::{ContractDraft, ValueType},
        flow::{ContractRef, FlowStep, InputSource},
        models::{ModelProfile, TaskModelRoute},
    };
    use serde_json::json;
    struct Fixture {
        dir: tempfile::TempDir,
        flows: FlowStore,
        contracts: ContractStore,
        models: ModelRegistry,
        plans: FlowPlanStore,
    }
    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().to_path_buf();
            Self {
                flows: FlowStore::new(root.clone()),
                contracts: ContractStore::new(root.join("contracts/library.json")),
                models: ModelRegistry::new(root.join("models/registry.json")),
                plans: FlowPlanStore::new(root),
                dir,
            }
        }
        fn draft(&self, browser: bool) -> FlowDraft {
            FlowDraft {
                id: "receipt".into(),
                title: "Receipt".into(),
                inputs: BTreeMap::from([("value".into(), ValueType::String)]),
                outputs: BTreeMap::new(),
                required_permissions: vec![if browser {
                    "browser.read"
                } else {
                    "model.send"
                }
                .into()],
                steps: vec![FlowStep {
                    id: "step".into(),
                    capability: if browser {
                        "browser.observe"
                    } else {
                        "model.generate"
                    }
                    .into(),
                    depends_on: vec![],
                    inputs: BTreeMap::from([(
                        if browser { "target" } else { "requirement" }.into(),
                        InputSource::FlowInput {
                            name: "value".into(),
                        },
                    )]),
                    contract: browser.then(|| ContractRef {
                        id: "used".into(),
                        version: 1,
                    }),
                    model_role: (!browser).then(|| "generate".into()),
                    max_attempts: 1,
                }],
            }
        }
        fn publish(&self, d: FlowDraft) {
            let r = self.flows.snapshot().unwrap().revision();
            self.flows.save_draft(d, r).unwrap();
            self.flows.publish("receipt", r + 1, "human").unwrap();
        }
        fn request(&self) -> FlowPlanRequest {
            FlowPlanRequest {
                id: "receipt".into(),
                version: 1,
                project: "alpha".into(),
                inputs: BTreeMap::from([("value".into(), json!("https://alpha.invalid"))]),
                permissions: self
                    .flows
                    .snapshot()
                    .unwrap()
                    .publication("receipt", 1)
                    .unwrap()
                    .definition()
                    .required_permissions
                    .clone(),
                expected_revision: self.flows.snapshot().unwrap().revision(),
                expected_model_revision: self.models.snapshot().unwrap().revision,
            }
        }
        fn model(&self, allow_remote: bool) -> Result<()> {
            self.model_at(allow_remote, "https://example.invalid/v1")
        }
        fn model_at(&self, allow_remote: bool, endpoint: &str) -> Result<()> {
            let r = self.models.snapshot()?.revision;
            self.models.upsert(
                r,
                ModelProfile {
                    id: "m".into(),
                    label: "private-label".into(),
                    adapter: "openai_compatible".into(),
                    base_url: endpoint.into(),
                    model: "model-m".into(),
                    api_key: "secret-never-serialize".into(),
                    capabilities: vec!["text".into()],
                    enabled: true,
                    allow_remote,
                    midscene_family: None,
                },
                false,
            )?;
            Ok(())
        }
        fn route(&self) -> Result<()> {
            self.models.set_route(
                self.models.snapshot()?.revision,
                "generate",
                TaskModelRoute {
                    primary: "m".into(),
                    fallbacks: vec![],
                },
            )?;
            Ok(())
        }
        fn contract(&self, id: &str) {
            let draft = ContractDraft {
                id: id.into(),
                version: 1,
                intent: "Observe receipt".into(),
                inputs: BTreeMap::from([("target".into(), ValueType::String)]),
                outputs: BTreeMap::new(),
                preconditions: vec![],
                assertions: vec!["original receipt assertion".into()],
                required_capabilities: vec!["browser.visual".into()],
            };
            let r = self.contracts.snapshot().unwrap().revision;
            self.contracts.save_draft(draft, r).unwrap();
            self.contracts.publish(id, r + 1, "human").unwrap();
            self.contracts
                .bind(
                    "alpha",
                    id,
                    1,
                    BTreeMap::from([("target".into(), json!("https://alpha.invalid"))]),
                    r + 2,
                )
                .unwrap();
        }
        fn prepare(&self, r: FlowPlanRequest) -> Result<FlowPlan> {
            self.plans.prepare(
                &self.flows,
                &self.contracts,
                &self.models,
                r,
                "https://alpha.invalid",
            )
        }
    }
    #[test]
    fn model_plans_freeze_and_reload_without_calls_or_secrets() {
        let f = Fixture::new();
        f.model(true).unwrap();
        f.route().unwrap();
        f.publish(f.draft(false));
        let first = f.prepare(f.request()).unwrap();
        assert!(!first.executable);
        let bytes = std::fs::read_to_string(f.dir.path().join("flows/plans.json")).unwrap();
        for secret in ["secret-never-serialize", "private-label", "api_key"] {
            assert!(!bytes.contains(secret));
        }
        f.model_at(true, "https://replacement.invalid/v1").unwrap();
        let loaded = f.plans.get(&first.plan_id).unwrap();
        assert_eq!(loaded.models, first.models);
        assert_eq!(loaded.model_revision, 2);
        let next = f.prepare(f.request()).unwrap();
        assert_eq!(
            loaded.models["step"].candidates[0].base_url,
            "https://example.invalid/v1"
        );
        assert_eq!(
            next.models["step"].candidates[0].base_url,
            "https://replacement.invalid/v1"
        );
        assert_eq!(next.plan_id, "plan-2");
        assert_eq!(f.plans.list().unwrap().len(), 2);
    }
    #[test]
    fn historical_model_endpoints_reject_credentials_query_fragment_and_missing_consent() {
        let f = Fixture::new();
        f.model(true).unwrap();
        f.route().unwrap();
        f.publish(f.draft(false));
        let first = f.prepare(f.request()).unwrap();
        let path = f.dir.path().join("flows/plans.json");
        let data: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        for endpoint in [
            "https://user:secret@example.invalid/v1",
            "https://example.invalid/v1?key=secret",
            "https://example.invalid/v1#secret",
            "file:///tmp/model",
            "not-url",
        ] {
            let mut bad = data.clone();
            bad["plans"][0]["models"]["step"]["candidates"][0]["base_url"] = json!(endpoint);
            std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
            assert!(f.plans.get(&first.plan_id).is_err());
        }
        let mut bad = data;
        bad["plans"][0]["models"]["step"]["candidates"][0]["allow_remote"] = json!(false);
        std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(f.plans.get(&first.plan_id).is_err());
    }
    #[test]
    fn selected_contracts_ignore_unrelated_deprecated_binding_and_remain_historical() {
        let f = Fixture::new();
        f.contract("used");
        f.contract("unrelated");
        let r = f.contracts.snapshot().unwrap().revision;
        f.contracts
            .deprecate("unrelated", 1, "obsolete", r, "human")
            .unwrap();
        f.publish(f.draft(true));
        let p = f.prepare(f.request()).unwrap();
        assert_eq!(p.contracts.as_ref().unwrap().contracts().len(), 1);
        let r = f.contracts.snapshot().unwrap().revision;
        f.contracts
            .deprecate("used", 1, "obsolete", r, "human")
            .unwrap();
        assert!(f.prepare(f.request()).is_err());
        let old = f.plans.get(&p.plan_id).unwrap();
        assert_eq!(
            old.contracts.unwrap().contracts()[0].original_assertions(),
            ["original receipt assertion"]
        );
    }
    #[test]
    fn invalid_inputs_permissions_routes_and_revisions_create_no_plan() {
        let f = Fixture::new();
        f.publish(f.draft(false));
        assert!(f.prepare(f.request()).is_err());
        f.model(false).unwrap();
        assert!(f.route().is_err());
        f.model(true).unwrap();
        f.route().unwrap();
        let mut r = f.request();
        r.inputs.insert("value".into(), json!(4));
        assert!(f.prepare(r).is_err());
        let mut r = f.request();
        r.permissions.clear();
        assert!(f.prepare(r).is_err());
        let mut r = f.request();
        r.expected_revision = 0;
        assert!(f.prepare(r).is_err());
        let mut r = f.request();
        r.expected_model_revision = 0;
        assert!(f.prepare(r).is_err());
        assert!(f.plans.list().unwrap().is_empty());
    }
    #[test]
    fn damaged_history_and_executable_flags_fail_closed_and_preserve_bytes() {
        let f = Fixture::new();
        f.publish(f.draft(true));
        f.contract("used");
        let p = f.prepare(f.request()).unwrap();
        let path = f.dir.path().join("flows/plans.json");
        let mut data: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        data["plans"][0]["executable"] = json!(true);
        let bad = serde_json::to_vec(&data).unwrap();
        std::fs::write(&path, &bad).unwrap();
        assert!(f.plans.get(&p.plan_id).is_err());
        assert!(f.prepare(f.request()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bad);
    }
    #[test]
    fn browser_targets_cannot_escape_frozen_project_context() {
        let f = Fixture::new();
        f.contract("used");
        let mut draft = f.draft(true);
        draft.steps[0].inputs.insert(
            "target".into(),
            InputSource::Literal {
                value: json!("https://other.invalid"),
            },
        );
        f.publish(draft);
        assert!(f.prepare(f.request()).is_err());
        assert!(f.plans.list().unwrap().is_empty());
        let f = Fixture::new();
        f.contract("used");
        f.publish(f.draft(true));
        let mut r = f.request();
        r.inputs
            .insert("value".into(), json!("https://other.invalid"));
        assert!(f.prepare(r).is_err());
        let p = f.prepare(f.request()).unwrap();
        let path = f.dir.path().join("flows/plans.json");
        let mut data: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        data["plans"][0]["project_target"] = json!("https://other.invalid");
        std::fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
        assert!(f.plans.get(&p.plan_id).is_err());
    }
    #[test]
    fn disk_failure_creates_no_successful_plan() {
        let f = Fixture::new();
        f.publish(f.draft(true));
        f.contract("used");
        let path = f.dir.path().join("flows/plans.json");
        std::fs::create_dir(&path).unwrap();
        assert!(f.prepare(f.request()).is_err());
        assert!(path.is_dir());
    }
}
