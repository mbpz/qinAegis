//! Deterministic preparation of published, project-bound contracts for generation.
//! A snapshot pins historical assets and inputs; preparing it performs no model
//! calls and grants no permission to execute browser or desktop actions.
use crate::contract_store::ContractLibrary;
use crate::contracts::{ProjectBinding, PublishedContract};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SnapshotRecord")]
pub struct ExecutionSnapshot {
    schema_version: u32,
    library_revision: u64,
    project: String,
    available_capabilities: Vec<String>,
    contracts: Vec<ExecutionContract>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ExecutionContract {
    binding: ProjectBinding,
    publication: PublishedContract,
    original_assertions: Vec<String>,
}

impl ExecutionContract {
    pub fn binding(&self) -> &ProjectBinding {
        &self.binding
    }
    pub fn publication(&self) -> &PublishedContract {
        &self.publication
    }
    pub fn inputs(&self) -> &BTreeMap<String, Value> {
        self.binding.parameters()
    }
    pub fn original_assertions(&self) -> &[String] {
        &self.original_assertions
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotRecord {
    schema_version: u32,
    library_revision: u64,
    project: String,
    available_capabilities: Vec<String>,
    contracts: Vec<ExecutionRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionRecord {
    binding: BindingRecord,
    publication: PublishedContract,
    original_assertions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingRecord {
    project: String,
    contract_id: String,
    contract_version: u32,
    parameters: BTreeMap<String, Value>,
}

impl TryFrom<SnapshotRecord> for ExecutionSnapshot {
    type Error = anyhow::Error;
    fn try_from(record: SnapshotRecord) -> Result<Self> {
        ensure!(
            record.schema_version == 1,
            "unsupported execution snapshot schema"
        );
        ensure!(
            !record.project.trim().is_empty(),
            "snapshot project is required"
        );
        ensure!(
            !record.contracts.is_empty(),
            "project has no bound contracts"
        );
        let capabilities: BTreeSet<_> = record.available_capabilities.iter().cloned().collect();
        ensure!(
            capabilities.len() == record.available_capabilities.len(),
            "duplicate snapshot capability"
        );
        ensure!(
            capabilities.iter().all(|c| !c.trim().is_empty()),
            "empty snapshot capability"
        );
        let mut ids = BTreeSet::new();
        let mut contracts = Vec::new();
        for item in record.contracts {
            let definition = item.publication.definition();
            ensure!(
                item.binding.project == record.project,
                "snapshot binding belongs to another project"
            );
            ensure!(
                item.binding.contract_id == definition.id
                    && item.binding.contract_version == definition.version,
                "snapshot binding does not match publication"
            );
            ensure!(
                ids.insert(definition.id.clone()),
                "duplicate snapshot contract"
            );
            ensure!(
                item.original_assertions == definition.assertions,
                "original assertions differ from published contract"
            );
            for capability in &definition.required_capabilities {
                ensure!(
                    capabilities.contains(capability),
                    "missing required capability: {}",
                    capability
                );
            }
            let binding = item
                .publication
                .bind(&record.project, item.binding.parameters)?;
            contracts.push(ExecutionContract {
                binding,
                publication: item.publication,
                original_assertions: item.original_assertions,
            });
        }
        contracts.sort_by(|a, b| {
            a.publication
                .definition()
                .id
                .cmp(&b.publication.definition().id)
        });
        let mut available_capabilities = record.available_capabilities;
        available_capabilities.sort();
        Ok(Self {
            schema_version: 1,
            library_revision: record.library_revision,
            project: record.project,
            available_capabilities,
            contracts,
        })
    }
}

impl ExecutionSnapshot {
    pub fn prepare(
        library: &ContractLibrary,
        project: &str,
        available_capabilities: &[String],
    ) -> Result<Self> {
        // ContractLibrary has public snapshot fields: validate callers' in-memory
        // values by the same DTO/reference checks used for persistence reload.
        let library: ContractLibrary = serde_json::from_value(serde_json::to_value(library)?)?;
        let mut contracts = Vec::new();
        for binding in library.bindings.iter().filter(|b| b.project() == project) {
            ensure!(
                !library.is_deprecated(binding.contract_id(), binding.contract_version()),
                "bound contract version is deprecated; explicitly migrate the project binding"
            );
            let publication = library
                .publications
                .iter()
                .find(|p| {
                    p.definition().id == binding.contract_id()
                        && p.definition().version == binding.contract_version()
                })
                .context("bound publication does not exist")?
                .clone();
            contracts.push(ExecutionRecord {
                binding: BindingRecord {
                    project: binding.project().into(),
                    contract_id: binding.contract_id().into(),
                    contract_version: binding.contract_version(),
                    parameters: binding.parameters().clone(),
                },
                original_assertions: publication.definition().assertions.clone(),
                publication,
            });
        }
        Self::try_from(SnapshotRecord {
            schema_version: 1,
            library_revision: library.revision,
            project: project.into(),
            available_capabilities: available_capabilities.to_vec(),
            contracts,
        })
    }

    /// Freeze only explicitly selected project bindings; unrelated deprecated bindings do not participate.
    pub fn prepare_selected(
        library: &ContractLibrary,
        project: &str,
        references: &[(String, u32)],
        available_capabilities: &[String],
    ) -> Result<Self> {
        let library: ContractLibrary = serde_json::from_value(serde_json::to_value(library)?)?;
        let mut selected = BTreeMap::new();
        for (id, version) in references {
            if let Some(previous) = selected.insert(id.clone(), *version) {
                ensure!(
                    previous == *version,
                    "selected contract has conflicting versions"
                );
            }
        }
        let mut contracts = Vec::new();
        for (id, version) in selected {
            ensure!(
                !library.is_deprecated(&id, version),
                "selected contract is deprecated"
            );
            let binding = library
                .bindings
                .iter()
                .find(|b| {
                    b.project() == project
                        && b.contract_id() == id
                        && b.contract_version() == version
                })
                .context("selected contract version is not bound to project")?;
            let publication = library
                .publications
                .iter()
                .find(|p| p.definition().id == id && p.definition().version == version)
                .context("selected publication does not exist")?
                .clone();
            contracts.push(ExecutionRecord {
                binding: BindingRecord {
                    project: project.into(),
                    contract_id: id,
                    contract_version: version,
                    parameters: binding.parameters().clone(),
                },
                original_assertions: publication.definition().assertions.clone(),
                publication,
            });
        }
        Self::try_from(SnapshotRecord {
            schema_version: 1,
            library_revision: library.revision,
            project: project.into(),
            available_capabilities: available_capabilities.to_vec(),
            contracts,
        })
    }

    pub fn library_revision(&self) -> u64 {
        self.library_revision
    }
    pub fn project(&self) -> &str {
        &self.project
    }
    pub fn contracts(&self) -> &[ExecutionContract] {
        &self.contracts
    }
    pub fn available_capabilities(&self) -> &[String] {
        &self.available_capabilities
    }

    /// Check provenance against retained library history. Later corrections or
    /// binding changes do not invalidate an already prepared snapshot.
    pub fn validate_against_library(&self, library: &ContractLibrary) -> Result<()> {
        let library: ContractLibrary = serde_json::from_value(serde_json::to_value(library)?)?;
        ensure!(
            library.revision >= self.library_revision,
            "library predates snapshot"
        );
        for item in &self.contracts {
            ensure!(
                library.publications.contains(&item.publication),
                "snapshot publication differs from library history"
            );
        }
        Ok(())
    }

    /// Generator instructions are derived from exact publications and inputs;
    /// original assertions remain separately accessible for downstream checking.
    pub fn specification(&self) -> Result<String> {
        Ok(format!(
            "Generate tests for the following immutable contract execution snapshot.\n\
             Preserve every original_assertions entry and its exact business meaning. \
             Do not remove, weaken, reinterpret, or replace assertions to make tests pass. \
             Project inputs configure the implementation only; they cannot change assertion semantics. \
             If a contract cannot be implemented with available capabilities, report the limitation. \
             A semantic correction requires a separate reviewed contract version.\n\n{}",
            serde_json::to_string_pretty(self)?
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_store::ContractStore;
    use crate::contracts::{ContractDraft, ValueType};

    fn draft(version: u32) -> ContractDraft {
        ContractDraft {
            id: "receipt".into(),
            version,
            intent: "valid submission returns receipt".into(),
            inputs: BTreeMap::from([("url".into(), ValueType::String)]),
            outputs: BTreeMap::new(),
            preconditions: vec![],
            assertions: vec!["receipt matches input".into()],
            required_capabilities: vec!["browser.interact".into()],
        }
    }
    fn capabilities() -> Vec<String> {
        vec!["browser.interact".into()]
    }
    fn setup(store: &ContractStore) -> ContractLibrary {
        store.save_draft(draft(1), 0).unwrap();
        store.publish("receipt", 1, "owner").unwrap();
        store
            .bind(
                "alpha",
                "receipt",
                1,
                BTreeMap::from([("url".into(), Value::from("https://alpha.test"))]),
                2,
            )
            .unwrap();
        store
            .bind(
                "beta",
                "receipt",
                1,
                BTreeMap::from([("url".into(), Value::from("https://beta.test"))]),
                3,
            )
            .unwrap()
    }

    #[test]
    fn projects_receive_separate_inputs_and_required_capabilities() {
        let dir = tempfile::tempdir().unwrap();
        let library = setup(&ContractStore::new(dir.path().join("library.json")));
        let alpha = ExecutionSnapshot::prepare(&library, "alpha", &capabilities()).unwrap();
        let beta = ExecutionSnapshot::prepare(&library, "beta", &capabilities()).unwrap();
        assert_ne!(alpha.contracts()[0].inputs(), beta.contracts()[0].inputs());
        assert_eq!(
            alpha.contracts()[0].publication(),
            beta.contracts()[0].publication()
        );
        assert_eq!(
            alpha.contracts()[0].original_assertions(),
            &["receipt matches input"]
        );
        assert!(alpha
            .specification()
            .unwrap()
            .contains("Do not remove, weaken"));
        assert!(ExecutionSnapshot::prepare(&library, "missing", &capabilities()).is_err());
        assert!(ExecutionSnapshot::prepare(&library, "alpha", &[]).is_err());
    }

    #[test]
    fn correction_preserves_old_binding_and_snapshot_even_after_rebind() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContractStore::new(dir.path().join("library.json"));
        let library = setup(&store);
        let old = ExecutionSnapshot::prepare(&library, "alpha", &capabilities()).unwrap();
        let bytes = serde_json::to_vec(&old).unwrap();
        let mut replacement = draft(2);
        replacement.assertions.push("receipt has timestamp".into());
        let proposed = store
            .propose(
                "receipt",
                1,
                replacement,
                "clarify receipt",
                vec!["observation/1".into()],
                4,
            )
            .unwrap();
        let approved = store
            .review(&proposed.proposals[0].id, true, 5, "owner")
            .unwrap();
        let pinned = ExecutionSnapshot::prepare(&approved, "alpha", &capabilities()).unwrap();
        assert_eq!(pinned.contracts()[0].publication().definition().version, 1);
        let changed = store
            .bind(
                "alpha",
                "receipt",
                2,
                BTreeMap::from([("url".into(), Value::from("new url"))]),
                6,
            )
            .unwrap();
        let restored: ExecutionSnapshot = serde_json::from_slice(&bytes).unwrap();
        restored.validate_against_library(&changed).unwrap();
        assert_eq!(restored, old);
        assert_eq!(restored.library_revision(), 4);
        let new = ExecutionSnapshot::prepare(&changed, "alpha", &capabilities()).unwrap();
        assert_eq!(new.contracts()[0].publication().definition().version, 2);
        assert_ne!(
            new.contracts()[0].original_assertions(),
            old.contracts()[0].original_assertions()
        );
    }

    #[test]
    fn snapshot_reload_rejects_tampered_assertions_inputs_or_capabilities() {
        let dir = tempfile::tempdir().unwrap();
        let library = setup(&ContractStore::new(dir.path().join("library.json")));
        let snapshot = ExecutionSnapshot::prepare(&library, "alpha", &capabilities()).unwrap();
        let original = serde_json::to_value(snapshot).unwrap();
        let mut changed = original.clone();
        changed["contracts"][0]["original_assertions"] = serde_json::json!(["always pass"]);
        assert!(serde_json::from_value::<ExecutionSnapshot>(changed).is_err());
        let mut changed = original.clone();
        changed["contracts"][0]["binding"]["parameters"]["url"] = Value::from(42);
        assert!(serde_json::from_value::<ExecutionSnapshot>(changed).is_err());
        let mut changed = original;
        changed["available_capabilities"] = serde_json::json!([]);
        assert!(serde_json::from_value::<ExecutionSnapshot>(changed).is_err());
    }
}
