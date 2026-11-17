//! Fixed private flow library with CAS and immutable publications.
use crate::{
    asset_file::AssetFile,
    flow::{identifier, FlowDraft},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PublishedRecord")]
pub struct PublishedFlow {
    definition: FlowDraft,
    version: u32,
    reviewer: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishedRecord {
    definition: FlowDraft,
    version: u32,
    reviewer: String,
}
impl TryFrom<PublishedRecord> for PublishedFlow {
    type Error = anyhow::Error;
    fn try_from(r: PublishedRecord) -> Result<Self> {
        r.definition.validate()?;
        reviewer(&r.reviewer)?;
        ensure!(r.version > 0, "flow publication version must be positive");
        Ok(Self {
            definition: r.definition,
            version: r.version,
            reviewer: r.reviewer,
        })
    }
}
impl PublishedFlow {
    pub fn definition(&self) -> &FlowDraft {
        &self.definition
    }
    pub fn version(&self) -> u32 {
        self.version
    }
    pub fn reviewer(&self) -> &str {
        &self.reviewer
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LibraryRecord")]
pub struct FlowLibrary {
    schema_version: u32,
    revision: u64,
    drafts: Vec<FlowDraft>,
    publications: Vec<PublishedFlow>,
}
impl Default for FlowLibrary {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            drafts: vec![],
            publications: vec![],
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryRecord {
    schema_version: u32,
    revision: u64,
    drafts: Vec<FlowDraft>,
    publications: Vec<PublishedFlow>,
}
impl TryFrom<LibraryRecord> for FlowLibrary {
    type Error = anyhow::Error;
    fn try_from(r: LibraryRecord) -> Result<Self> {
        let lib = Self {
            schema_version: r.schema_version,
            revision: r.revision,
            drafts: r.drafts,
            publications: r.publications,
        };
        lib.validate()?;
        Ok(lib)
    }
}
impl FlowLibrary {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn drafts(&self) -> &[FlowDraft] {
        &self.drafts
    }
    pub fn publications(&self) -> &[PublishedFlow] {
        &self.publications
    }
    pub fn publication(&self, id: &str, version: u32) -> Result<&PublishedFlow> {
        self.publications
            .iter()
            .find(|p| p.definition.id == id && p.version == version)
            .context("published flow version does not exist")
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1 && self.drafts.len() <= 256 && self.publications.len() <= 1024,
            "unsupported or oversized flow library"
        );
        let mut ids = BTreeSet::new();
        for draft in &self.drafts {
            draft.validate()?;
            ensure!(ids.insert(&draft.id), "duplicate flow draft");
        }
        let mut versions = BTreeSet::new();
        for publication in &self.publications {
            publication.definition.validate()?;
            reviewer(&publication.reviewer)?;
            ensure!(
                publication.version > 0
                    && versions.insert((&publication.definition.id, publication.version)),
                "duplicate or invalid flow version"
            );
        }
        Ok(())
    }
}
fn reviewer(value: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= 128 && !value.chars().any(char::is_control),
        "explicit flow reviewer required"
    );
    Ok(())
}
pub struct FlowStore {
    file: AssetFile,
}
impl FlowStore {
    pub fn new(state_root: PathBuf) -> Self {
        Self {
            file: AssetFile::new(state_root.join("flows/library.json"), 8 * 1024 * 1024),
        }
    }
    pub fn snapshot(&self) -> Result<FlowLibrary> {
        self.file
            .lock()?
            .read()?
            .map(|b| serde_json::from_slice(&b).context("invalid flow library"))
            .unwrap_or_else(|| Ok(FlowLibrary::default()))
    }
    fn update(
        &self,
        expected: u64,
        change: impl FnOnce(&mut FlowLibrary) -> Result<()>,
    ) -> Result<FlowLibrary> {
        let guard = self.file.lock()?;
        let mut lib: FlowLibrary = guard
            .read()?
            .map(|b| serde_json::from_slice(&b).context("invalid flow library"))
            .unwrap_or_else(|| Ok(FlowLibrary::default()))?;
        ensure!(
            lib.revision == expected,
            "flow library revision conflict; reload before saving"
        );
        change(&mut lib)?;
        lib.revision = lib
            .revision
            .checked_add(1)
            .context("flow revision exhausted")?;
        lib.validate()?;
        guard.write(&serde_json::to_vec_pretty(&lib)?)?;
        Ok(lib)
    }
    pub fn save_draft(&self, draft: FlowDraft, expected: u64) -> Result<FlowLibrary> {
        draft.validate()?;
        self.update(expected, |lib| {
            lib.drafts.retain(|d| d.id != draft.id);
            lib.drafts.push(draft);
            Ok(())
        })
    }
    pub fn publish(&self, id: &str, expected: u64, approved_reviewer: &str) -> Result<FlowLibrary> {
        identifier(id)?;
        reviewer(approved_reviewer)?;
        self.update(expected, |lib| {
            let index = lib
                .drafts
                .iter()
                .position(|d| d.id == id)
                .context("saved flow draft not found")?;
            let version = lib
                .publications
                .iter()
                .filter(|p| p.definition.id == id)
                .map(|p| p.version)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .context("flow version exhausted")?;
            let definition = lib.drafts.remove(index);
            definition.validate()?;
            lib.publications.push(PublishedFlow {
                definition,
                version,
                reviewer: approved_reviewer.into(),
            });
            Ok(())
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{contracts::ValueType, flow::*};
    use std::collections::BTreeMap;
    fn draft() -> FlowDraft {
        FlowDraft {
            id: "receipt".into(),
            title: "Receipt".into(),
            inputs: BTreeMap::from([("target".into(), ValueType::String)]),
            outputs: BTreeMap::new(),
            required_permissions: vec!["browser.read".into()],
            steps: vec![FlowStep {
                id: "observe".into(),
                capability: "browser.observe".into(),
                depends_on: vec![],
                inputs: BTreeMap::from([(
                    "target".into(),
                    InputSource::FlowInput {
                        name: "target".into(),
                    },
                )]),
                contract: Some(ContractRef {
                    id: "receipt".into(),
                    version: 1,
                }),
                model_role: None,
                max_attempts: 1,
            }],
        }
    }
    #[test]
    fn publication_cas_and_history_immutability() {
        let dir = tempfile::tempdir().unwrap();
        let store = FlowStore::new(dir.path().into());
        assert_eq!(store.snapshot().unwrap().revision(), 0);
        let saved = store.save_draft(draft(), 0).unwrap();
        assert!(store.publish("receipt", 0, "human").is_err());
        assert!(store.publish("receipt", saved.revision(), "").is_err());
        let published = store.publish("receipt", saved.revision(), "human").unwrap();
        let old = published.publication("receipt", 1).unwrap().clone();
        let mut changed = draft();
        changed.title = "New".into();
        let saved = store.save_draft(changed, published.revision()).unwrap();
        let published = store.publish("receipt", saved.revision(), "human").unwrap();
        assert_eq!(published.publication("receipt", 1).unwrap(), &old);
        assert_eq!(
            published
                .publication("receipt", 2)
                .unwrap()
                .definition()
                .title,
            "New"
        );
        assert_eq!(store.snapshot().unwrap(), published);
        let mut value = serde_json::to_value(published).unwrap();
        value["publications"][0]["definition"]["steps"][0]["max_attempts"] = serde_json::json!(0);
        assert!(serde_json::from_value::<FlowLibrary>(value).is_err());
    }
    #[test]
    fn graph_types_permissions_and_side_effects_are_owned_by_catalog() {
        let original = draft();
        assert!(original.validate().is_ok());
        let mut d = original.clone();
        d.required_permissions.clear();
        assert!(d.validate().is_err());
        let mut d = original.clone();
        d.steps[0].inputs.insert(
            "target".into(),
            InputSource::Literal {
                value: serde_json::json!(42),
            },
        );
        assert!(d.validate().is_err());
        let mut d = original.clone();
        d.steps[0].depends_on = vec!["observe".into()];
        assert!(d.validate().is_err());
        let mut d = original.clone();
        d.steps[0].inputs.insert(
            "extra".into(),
            InputSource::Literal {
                value: serde_json::json!("x"),
            },
        );
        assert!(d.validate().is_err());
        let mut d = original.clone();
        d.steps[0].capability = "unknown".into();
        assert!(d.validate().is_err());
        let mut d = original.clone();
        d.steps[0].capability = "browser.interact".into();
        d.required_permissions = vec!["browser.write".into()];
        d.steps[0].inputs.insert(
            "instruction".into(),
            InputSource::Literal {
                value: serde_json::json!("click"),
            },
        );
        d.steps[0].max_attempts = 2;
        assert!(d.validate().is_err());
        d.steps[0].max_attempts = 1;
        assert!(d.validate().is_ok());
        assert!(original.validate_inputs(&BTreeMap::new()).is_err());
    }
    #[test]
    fn output_references_must_be_typed_and_upstream() {
        let mut d = draft();
        d.required_permissions.push("report.write".into());
        d.steps.push(FlowStep {
            id: "report".into(),
            capability: "report.collect".into(),
            depends_on: vec!["observe".into()],
            inputs: BTreeMap::from([(
                "result".into(),
                InputSource::StepOutput {
                    step: "observe".into(),
                    port: "observation".into(),
                },
            )]),
            contract: None,
            model_role: None,
            max_attempts: 1,
        });
        assert_eq!(
            d.validate().unwrap().execution_order,
            vec!["observe", "report"]
        );
        d.steps.reverse();
        assert_eq!(
            d.validate().unwrap().execution_order,
            vec!["observe", "report"]
        );
        d.steps[0].depends_on.clear();
        assert!(d.validate().is_err());
        d.steps[0].depends_on.push("observe".into());
        d.outputs.insert(
            "report".into(),
            FlowOutput {
                value_type: ValueType::String,
                source: InputSource::StepOutput {
                    step: "report".into(),
                    port: "report".into(),
                },
            },
        );
        assert!(d.validate().is_err());
        d.outputs.get_mut("report").unwrap().value_type = ValueType::Object;
        assert!(d.validate().is_ok());
    }
}
