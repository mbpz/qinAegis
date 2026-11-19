//! One explicitly previewed source; secret snapshots stay in bounded process memory.
use anyhow::{ensure, Context, Result};
use qin_aegis_core::config::migration::{self, MigrationPlan, SecretPolicy};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const PREVIEW_TTL: Duration = Duration::from_secs(600);
struct Pending {
    token: String,
    created: Instant,
    plan: MigrationPlan,
}
#[derive(Default)]
struct Session {
    pending: Option<Pending>,
}
impl Session {
    fn apply(&self, token: &str, policy: SecretPolicy) -> Result<Value> {
        let preview = self
            .pending
            .as_ref()
            .filter(|p| p.token == token && p.created.elapsed() < PREVIEW_TTL)
            .context("Migration preview expired or replaced; preview again")?;
        Ok(serde_json::to_value(preview.plan.apply(policy)?)?)
    }
}
static SESSION: OnceLock<Mutex<Session>> = OnceLock::new();
fn session() -> &'static Mutex<Session> {
    SESSION.get_or_init(|| Mutex::new(Session::default()))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    project: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preview {
    source_id: String,
    project: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Apply {
    preview_token: String,
    secret_policy: SecretPolicy,
    approved: bool,
}
pub fn handles(method: &str) -> bool {
    matches!(
        method,
        "listConfigMigrationSources" | "previewConfigMigration" | "applyConfigMigration"
    )
}
pub fn dispatch(method: &str, params: &Value) -> Result<Value> {
    match method {
        "listConfigMigrationSources" => {
            let request: List = serde_json::from_value(params.clone())?;
            Ok(
                json!({"sources":migration::sources(request.project.as_deref())?.into_iter().map(|s|s.info).collect::<Vec<_>>()}),
            )
        }
        "previewConfigMigration" => {
            let request: Preview = serde_json::from_value(params.clone())?;
            let source = migration::sources(request.project.as_deref())?
                .into_iter()
                .find(|source| source.info.id == request.source_id && source.info.status == "ready")
                .context("Select an available known configuration source")?;
            let plan = source.prepare()?;
            let mut preview = plan.preview();
            let token = crate::server::random_token()?;
            preview["preview_token"] = json!(token);
            preview["source_id"] = json!(source.info.id);
            preview["source_label"] = json!(source.info.label);
            preview["expires_in_seconds"] = json!(PREVIEW_TTL.as_secs());
            session().lock().unwrap().pending = Some(Pending {
                token,
                created: Instant::now(),
                plan,
            });
            Ok(preview)
        }
        "applyConfigMigration" => {
            let request: Apply = serde_json::from_value(params.clone())?;
            ensure!(
                request.approved,
                "Explicit configuration migration approval required"
            );
            session()
                .lock()
                .unwrap()
                .apply(&request.preview_token, request.secret_policy)
        }
        _ => anyhow::bail!("Unknown configuration migration operation"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_cannot_submit_paths_implicit_secret_policy_or_missing_approval() {
        assert!(dispatch("listConfigMigrationSources", &json!({"path":"/etc/passwd"})).is_err());
        assert!(dispatch(
            "previewConfigMigration",
            &json!({"source_id":"current","path":"/etc/passwd"})
        )
        .is_err());
        assert!(dispatch(
            "applyConfigMigration",
            &json!({"preview_token":"x","approved":true})
        )
        .is_err());
        assert!(dispatch(
            "applyConfigMigration",
            &json!({"preview_token":"x","secret_policy":"use_source","approved":false})
        )
        .is_err());
        assert!(Session::default()
            .apply("x", SecretPolicy::UseSource)
            .is_err());
    }
}
