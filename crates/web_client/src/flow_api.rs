//! Flow governance and immutable planning; this surface never starts an adapter.
use anyhow::{ensure, Context, Result};
use qin_aegis_core::{
    contract_store::ContractStore,
    flow::{capability_catalog, FlowDraft},
    flow_plan::{FlowPlanRequest, FlowPlanStore},
    flow_store::FlowStore,
    LocalStorage,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn handles(method: &str) -> bool {
    matches!(
        method,
        "getFlows"
            | "saveFlowDraft"
            | "validateFlowDraft"
            | "publishFlow"
            | "prepareFlowPlan"
            | "getFlowPlan"
    )
}

fn parameters<T: DeserializeOwned>(value: &Value) -> Result<T> {
    serde_json::from_value(value.clone()).context("invalid flow parameters")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Draft {
    draft: FlowDraft,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    draft: FlowDraft,
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
struct GetPlan {
    plan_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepare {
    id: String,
    version: u32,
    project: String,
    inputs: BTreeMap<String, Value>,
    permissions: Vec<String>,
    expected_revision: u64,
    expected_model_revision: u64,
    approved: bool,
}

pub fn dispatch(method: &str, value: &Value) -> Result<Value> {
    let root = LocalStorage::base_path();
    let flows = FlowStore::new(root.clone());
    let plans = FlowPlanStore::new(root.clone());
    match method {
        "getFlows" => {
            let _: Empty = parameters(value)?;
            Ok(json!({
                "library": flows.snapshot()?,
                "capabilities": capability_catalog(),
                "model_revision": crate::model_api::registry()?.snapshot()?.revision,
                "plan_history": plans.list()?,
            }))
        }
        "saveFlowDraft" => {
            let p: Save = parameters(value)?;
            Ok(serde_json::to_value(
                flows.save_draft(p.draft, p.expected_revision)?,
            )?)
        }
        "validateFlowDraft" => {
            let p: Draft = parameters(value)?;
            Ok(serde_json::to_value(p.draft.validate()?)?)
        }
        "publishFlow" => {
            let p: Publish = parameters(value)?;
            ensure!(p.approved, "explicit flow publication approval required");
            Ok(serde_json::to_value(flows.publish(
                &p.id,
                p.expected_revision,
                "local-owner",
            )?)?)
        }
        "prepareFlowPlan" => {
            let p: Prepare = parameters(value)?;
            ensure!(p.approved, "explicit planning approval required");
            let project = LocalStorage::checked_project_config(&p.project)?;
            let request = FlowPlanRequest {
                id: p.id,
                version: p.version,
                project: p.project,
                inputs: p.inputs,
                permissions: p.permissions,
                expected_revision: p.expected_revision,
                expected_model_revision: p.expected_model_revision,
            };
            Ok(serde_json::to_value(plans.prepare(
                &flows,
                &ContractStore::new(root.join("contracts/library.json")),
                &crate::model_api::registry()?,
                request,
                &project.url,
            )?)?)
        }
        "getFlowPlan" => {
            let p: GetPlan = parameters(value)?;
            Ok(serde_json::to_value(plans.get(&p.plan_id)?)?)
        }
        _ => anyhow::bail!("unknown flow operation"),
    }
}
