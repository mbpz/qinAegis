//! Registry APIs return redacted profiles; credentials are write-only.
use crate::contract_api::field;
use anyhow::{bail, Context, Result};
use qin_aegis_core::models::ModelRegistry;
use serde_json::Value;

pub fn registry() -> Result<ModelRegistry> {
    Ok(ModelRegistry::new(
        dirs::home_dir()
            .context("home directory is unavailable")?
            .join(".qinAegis/models/registry.json"),
    ))
}

pub fn handles(method: &str) -> bool {
    matches!(
        method,
        "getModels" | "saveModel" | "setModelRoute" | "resolveModelRoute"
    )
}

pub fn dispatch(method: &str, p: &Value) -> Result<Value> {
    let store = registry()?;
    let snapshot = match method {
        "getModels" => store.snapshot()?,
        "saveModel" => {
            let mut profile: Value = field(p, "profile")?;
            if let Some(object) = profile.as_object_mut() { object.remove("api_key_configured"); }
            store.upsert(field(p, "expected_revision")?, serde_json::from_value(profile)
                .map_err(|_| anyhow::anyhow!("invalid model profile"))?,
                p.get("clear_api_key").and_then(Value::as_bool).unwrap_or(false))?
        },
        "setModelRoute" => store.set_route(
            field(p, "expected_revision")?,
            &field::<String>(p, "task")?,
            field(p, "route")?,
        )?,
        "resolveModelRoute" => {
            let route = store.resolve(&field::<String>(p, "task")?, &[])?;
            let candidates: Vec<Value> = route.candidates().iter().map(|profile| serde_json::json!({
                "id":profile.id,"model":profile.model,"adapter":profile.adapter,"capabilities":profile.capabilities,
                "midscene_family":profile.midscene_family,
            })).collect();
            return Ok(
                serde_json::json!({"revision":route.revision(),"task":route.task(),"candidates":candidates}),
            );
        }
        _ => bail!("unknown model operation"),
    };
    Ok(serde_json::to_value(snapshot)?)
}
