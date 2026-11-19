//! Human edits use exact-content CAS, reset approval and discard inapplicable generated provenance.
use anyhow::{ensure, Result};
use qin_aegis_core::storage::local::AuditContext;
use qin_aegis_core::{CaseStatus, LocalStorage};
use serde::Deserialize;
use serde_json::Value;

fn context(reason: Option<String>, operation: &str) -> AuditContext {
    AuditContext {
        source: "local_web".into(),
        author: Some("local-owner".into()),
        reason: reason.unwrap_or_else(|| operation.into()),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Get {
    project: String,
    case_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    project: String,
    case_id: String,
    expected_digest: String,
    name: String,
    yaml_script: String,
    expected_result: String,
    approved: bool,
    reason: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    project: String,
    case_id: String,
    status: CaseStatus,
    expected_digest: Option<String>,
    reason: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    project: String,
    case_id: String,
    revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Restore {
    project: String,
    case_id: String,
    revision: u64,
    expected_digest: String,
    reason: String,
    approved: bool,
}
pub fn handles(method: &str) -> bool {
    matches!(
        method,
        "getCaseAsset"
            | "saveCaseAsset"
            | "updateCaseStatus"
            | "getCaseHistory"
            | "getCaseRevision"
            | "restoreCaseRevision"
    )
}
pub fn dispatch(method: &str, params: &Value) -> Result<Value> {
    if method == "updateCaseStatus" {
        let p: Status = serde_json::from_value(params.clone())?;
        LocalStorage::change_case_status_with_context(
            &p.project,
            &p.case_id,
            p.status,
            p.expected_digest.as_deref(),
            context(p.reason, "explicit case state change"),
        )?;
        return Ok(serde_json::json!({"ok": true}));
    }
    if method == "getCaseHistory" {
        let p: Get = serde_json::from_value(params.clone())?;
        return Ok(serde_json::to_value(LocalStorage::get_case_history(
            &p.project, &p.case_id,
        )?)?);
    }
    if method == "getCaseRevision" {
        let p: Revision = serde_json::from_value(params.clone())?;
        return Ok(serde_json::to_value(LocalStorage::get_case_revision(
            &p.project, &p.case_id, p.revision,
        )?)?);
    }
    if method == "restoreCaseRevision" {
        let p: Restore = serde_json::from_value(params.clone())?;
        ensure!(p.approved, "explicit restoration to a new draft required");
        return Ok(serde_json::to_value(LocalStorage::restore_case_revision(
            &p.project,
            &p.case_id,
            p.revision,
            &p.expected_digest,
            context(Some(p.reason), "restore historical case as draft"),
        )?)?);
    }
    let asset = match method {
        "getCaseAsset" => {
            let p: Get = serde_json::from_value(params.clone())?;
            LocalStorage::get_case_asset(&p.project, &p.case_id)?
        }
        "saveCaseAsset" => {
            let p: Save = serde_json::from_value(params.clone())?;
            ensure!(p.approved, "human edit confirmation required");
            LocalStorage::edit_case_asset_with_context(
                &p.project,
                &p.case_id,
                &p.expected_digest,
                &p.name,
                &p.yaml_script,
                &p.expected_result,
                context(p.reason, "explicit human case edit"),
            )?
        }
        _ => anyhow::bail!("unknown case asset operation"),
    };
    Ok(serde_json::to_value(asset)?)
}
