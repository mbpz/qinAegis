//! Human edits use exact-content CAS, reset approval and discard inapplicable generated provenance.
use anyhow::{ensure, Result};
use qin_aegis_core::{CaseStatus, LocalStorage};
use serde::Deserialize;
use serde_json::Value;
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
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    project: String,
    case_id: String,
    status: CaseStatus,
    expected_digest: Option<String>,
}
pub fn handles(method: &str) -> bool {
    matches!(
        method,
        "getCaseAsset" | "saveCaseAsset" | "updateCaseStatus"
    )
}
pub fn dispatch(method: &str, params: &Value) -> Result<Value> {
    if method == "updateCaseStatus" {
        let p: Status = serde_json::from_value(params.clone())?;
        LocalStorage::change_case_status_checked(
            &p.project,
            &p.case_id,
            p.status,
            p.expected_digest.as_deref(),
        )?;
        return Ok(serde_json::json!({"ok": true}));
    }
    let asset = match method {
        "getCaseAsset" => {
            let p: Get = serde_json::from_value(params.clone())?;
            LocalStorage::get_case_asset(&p.project, &p.case_id)?
        }
        "saveCaseAsset" => {
            let p: Save = serde_json::from_value(params.clone())?;
            ensure!(p.approved, "human edit confirmation required");
            LocalStorage::edit_case_asset(
                &p.project,
                &p.case_id,
                &p.expected_digest,
                &p.name,
                &p.yaml_script,
                &p.expected_result,
            )?
        }
        _ => anyhow::bail!("unknown case asset operation"),
    };
    Ok(serde_json::to_value(asset)?)
}
