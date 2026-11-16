//! Contract governance is independent of model configuration and execution.
use anyhow::{bail, Context, Result};
use qin_aegis_core::contract_store::ContractStore;
use serde::de::DeserializeOwned;
use serde_json::Value;

pub fn handles(method: &str) -> bool {
    matches!(
        method,
        "getContracts"
            | "saveContractDraft"
            | "publishContract"
            | "proposeContractCorrection"
            | "reviewContractCorrection"
            | "bindContract"
            | "exportContracts"
            | "importContracts"
            | "getContractExecution"
            | "deprecateContract"
            | "deriveContract"
            | "migrateContractBindings"
    )
}

pub fn dispatch(method: &str, params: &Value) -> Result<Value> {
    let path = qin_aegis_core::LocalStorage::base_path().join("contracts/library.json");
    if matches!(method, "bindContract" | "getContractExecution") {
        let project: String = field(params, "project")?;
        anyhow::ensure!(
            qin_aegis_core::LocalStorage::project_config_path(&project).is_file(),
            "project does not exist"
        );
    }
    handle(&ContractStore::new(path), method, params)
}

pub fn execution(project: &str) -> Result<qin_aegis_core::contract_execution::ExecutionSnapshot> {
    let path = qin_aegis_core::LocalStorage::base_path().join("contracts/library.json");
    prepare_execution(&ContractStore::new(path), project)
}

fn prepare_execution(
    store: &ContractStore,
    project: &str,
) -> Result<qin_aegis_core::contract_execution::ExecutionSnapshot> {
    // This is a generation specification, not an execution authorization. No
    // desktop or native application capabilities are exposed by this service.
    qin_aegis_core::contract_execution::ExecutionSnapshot::prepare(
        &store.snapshot()?,
        project,
        &[
            "browser.interact".into(),
            "browser.assert".into(),
            "browser.visual".into(),
        ],
    )
}

pub(crate) fn field<T: DeserializeOwned>(params: &Value, name: &str) -> Result<T> {
    serde_json::from_value(
        params
            .get(name)
            .with_context(|| format!("{name} is required"))?
            .clone(),
    )
    .with_context(|| format!("invalid {name}"))
}

fn confirmation(params: &Value) -> Result<()> {
    anyhow::ensure!(
        params.get("approved").and_then(Value::as_bool) == Some(true),
        "explicit human review confirmation is required"
    );
    Ok(())
}

fn handle(store: &ContractStore, method: &str, p: &Value) -> Result<Value> {
    if method == "getContractExecution" {
        return Ok(serde_json::to_value(prepare_execution(
            store,
            &field::<String>(p, "project")?,
        )?)?);
    }
    if method == "getContracts" {
        return Ok(serde_json::to_value(store.snapshot()?)?);
    }
    if method == "exportContracts" {
        return Ok(serde_json::to_value(store.export()?)?);
    }
    let expected = field(p, "expected_revision")?;
    let snapshot = match method {
        "deprecateContract" => {
            confirmation(p)?;
            store.deprecate(
                &field::<String>(p, "contract_id")?,
                field(p, "version")?,
                &field::<String>(p, "reason")?,
                expected,
                "local-owner",
            )?
        }
        "deriveContract" => {
            confirmation(p)?;
            store.derive(
                &field::<String>(p, "source_id")?,
                field(p, "source_version")?,
                &field::<String>(p, "new_id")?,
                &field::<String>(p, "reason")?,
                expected,
                "local-owner",
            )?
        }
        "migrateContractBindings" => {
            confirmation(p)?;
            let bindings: Vec<qin_aegis_core::contract_store::MigrationBinding> =
                field(p, "bindings")?;
            for b in &bindings {
                anyhow::ensure!(
                    !b.project.is_empty()
                        && b.project.len() <= 128
                        && !b.project.starts_with('.')
                        && b.project
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.')),
                    "invalid migration project"
                );
                anyhow::ensure!(
                    qin_aegis_core::LocalStorage::project_config_path(&b.project).is_file(),
                    "project does not exist"
                );
            }
            store.migrate(
                &field::<String>(p, "contract_id")?,
                field(p, "from_version")?,
                field(p, "to_version")?,
                bindings,
                &field::<String>(p, "reason")?,
                expected,
                "local-owner",
            )?
        }
        "saveContractDraft" => store.save_draft(field(p, "draft")?, expected)?,
        "publishContract" => {
            confirmation(p)?;
            store.publish(&field::<String>(p, "id")?, expected, "local-owner")?
        }
        "proposeContractCorrection" => store.propose(
            &field::<String>(p, "contract_id")?,
            field(p, "version")?,
            field(p, "replacement")?,
            &field::<String>(p, "reason")?,
            field(p, "evidence")?,
            expected,
        )?,
        "reviewContractCorrection" => {
            confirmation(p)?;
            let approve = match field::<String>(p, "decision")?.as_str() {
                "approve" => true,
                "reject" => false,
                _ => bail!("invalid review decision"),
            };
            store.review(
                &field::<String>(p, "proposal_id")?,
                approve,
                expected,
                "local-owner",
            )?
        }
        "bindContract" => store.bind(
            &field::<String>(p, "project")?,
            &field::<String>(p, "contract_id")?,
            field(p, "version")?,
            field(p, "parameters")?,
            expected,
        )?,
        "importContracts" => {
            confirmation(p)?;
            // Confirmation accepts this external asset locally. Its reviewer
            // remains source provenance, not an authenticated local review.
            store.import(field(p, "bundle")?, expected)?
        }
        _ => bail!("unknown contract operation"),
    };
    Ok(serde_json::to_value(snapshot)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn publication_requires_human_confirmation_and_revision() {
        let dir = std::env::temp_dir().join(format!(
            "qinaegis-contract-api-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ContractStore::new(dir.join("library.json"));
        let draft = json!({"id":"login","version":1,"intent":"login succeeds","inputs":{},"outputs":{},"preconditions":[],"assertions":["account visible"],"required_capabilities":[]});
        handle(
            &store,
            "saveContractDraft",
            &json!({"draft":draft,"expected_revision":0}),
        )
        .unwrap();
        assert!(handle(
            &store,
            "publishContract",
            &json!({"id":"login","expected_revision":1})
        )
        .is_err());
        assert!(handle(
            &store,
            "publishContract",
            &json!({"id":"login","expected_revision":0,"approved":true})
        )
        .is_err());
        let result = handle(
            &store,
            "publishContract",
            &json!({"id":"login","expected_revision":1,"approved":true}),
        )
        .unwrap();
        assert_eq!(result["publications"][0]["reviewer"], "local-owner");
        assert_eq!(result["revision"], 2);
        for (method, params) in [
            (
                "deprecateContract",
                json!({"contract_id":"login","version":1,"reason":"retire","expected_revision":2}),
            ),
            (
                "deriveContract",
                json!({"source_id":"login","source_version":1,"new_id":"copy","reason":"reuse","expected_revision":2}),
            ),
            (
                "migrateContractBindings",
                json!({"contract_id":"login","from_version":1,"to_version":2,"bindings":[],"reason":"upgrade","expected_revision":2}),
            ),
        ] {
            let before = store.snapshot().unwrap();
            assert!(handles(method));
            assert!(handle(&store, method, &params).is_err());
            let mut denied = params.clone();
            denied["approved"] = json!(false);
            assert!(handle(&store, method, &denied).is_err());
            assert_eq!(store.snapshot().unwrap(), before);
        }

        let mut bundle = serde_json::to_value(store.export().unwrap()).unwrap();
        bundle["contracts"][0]["definition"]["id"] = json!("imported-login");
        bundle["contracts"][0]["reviewer"] = json!("external-source-author");
        let before = store.snapshot().unwrap();
        for approval in [Value::Null, json!(false), json!("true")] {
            assert!(handle(
                &store,
                "importContracts",
                &json!({"bundle":bundle,"expected_revision":2,"approved":approval})
            )
            .is_err());
            assert_eq!(store.snapshot().unwrap(), before);
        }
        assert!(handle(
            &store,
            "importContracts",
            &json!({"bundle":bundle,"expected_revision":2})
        )
        .is_err());
        let result = handle(
            &store,
            "importContracts",
            &json!({"bundle":bundle,"expected_revision":2,"approved":true}),
        )
        .unwrap();
        assert_eq!(result["revision"], 3);
        assert_eq!(
            result["publications"][1]["reviewer"],
            "external-source-author"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
