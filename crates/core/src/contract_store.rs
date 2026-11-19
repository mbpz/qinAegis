//! Local reusable contract library. Every mutation validates a complete snapshot
//! under a process lock and commits it with compare-and-swap revision semantics.
use crate::asset_file::AssetFile;
use crate::contract_correction::{CorrectionObservation, ModelCorrectionSource};
use crate::contracts::{ContractDraft, ProjectBinding, PublishedContract, ReviewerKind};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractAuditEvent {
    pub revision: u64,
    pub parent_revision: u64,
    pub created_at: String,
    pub context: crate::storage::local::AuditContext,
    pub before: BTreeMap<String, Value>,
    pub after: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredCorrection {
    pub id: String,
    pub original_version: u32,
    pub replacement: ContractDraft,
    pub reason: String,
    pub evidence: Vec<String>,
    pub status: CorrectionStatus,
    pub reviewer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_source: Option<ModelCorrectionSource>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deprecation {
    pub contract_id: String,
    pub version: u32,
    pub reason: String,
    pub reviewer: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Derivation {
    pub contract_id: String,
    pub source_id: String,
    pub source_version: u32,
    pub reason: String,
    pub reviewer: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Migration {
    pub contract_id: String,
    pub from_version: u32,
    pub to_version: u32,
    pub projects: Vec<String>,
    pub reason: String,
    pub reviewer: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationBinding {
    pub project: String,
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LibraryRecord")]
pub struct ContractLibrary {
    pub schema_version: u32,
    pub audit: Vec<ContractAuditEvent>,
    pub history_complete: bool,
    pub revision: u64,
    pub drafts: Vec<ContractDraft>,
    pub publications: Vec<PublishedContract>,
    pub proposals: Vec<StoredCorrection>,
    pub bindings: Vec<ProjectBinding>,
    pub deprecations: Vec<Deprecation>,
    pub derivations: Vec<Derivation>,
    pub migrations: Vec<Migration>,
    pub observations: Vec<CorrectionObservation>,
}

impl Default for ContractLibrary {
    fn default() -> Self {
        Self {
            schema_version: 1,
            audit: vec![],
            history_complete: true,
            revision: 0,
            drafts: vec![],
            publications: vec![],
            proposals: vec![],
            bindings: vec![],
            deprecations: vec![],
            derivations: vec![],
            migrations: vec![],
            observations: vec![],
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryRecord {
    #[serde(default)]
    audit: Vec<ContractAuditEvent>,
    #[serde(default)]
    history_complete: bool,
    schema_version: u32,
    revision: u64,
    drafts: Vec<ContractDraft>,
    publications: Vec<PublishedContract>,
    proposals: Vec<StoredCorrection>,
    bindings: Vec<BindingRecord>,
    #[serde(default)]
    deprecations: Vec<Deprecation>,
    #[serde(default)]
    derivations: Vec<Derivation>,
    #[serde(default)]
    migrations: Vec<Migration>,
    #[serde(default)]
    observations: Vec<CorrectionObservation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingRecord {
    project: String,
    contract_id: String,
    contract_version: u32,
    parameters: BTreeMap<String, Value>,
}

impl TryFrom<LibraryRecord> for ContractLibrary {
    type Error = anyhow::Error;
    fn try_from(record: LibraryRecord) -> Result<Self> {
        let mut library = Self {
            audit: record.audit,
            history_complete: record.history_complete,
            schema_version: record.schema_version,
            revision: record.revision,
            drafts: record.drafts,
            publications: record.publications,
            proposals: record.proposals,
            bindings: vec![],
            deprecations: record.deprecations,
            derivations: record.derivations,
            migrations: record.migrations,
            observations: record.observations,
        };
        for binding in record.bindings {
            library.bindings.push(
                library
                    .publication(&binding.contract_id, binding.contract_version)?
                    .bind(&binding.project, binding.parameters)?,
            );
        }
        library.validate()?;
        Ok(library)
    }
}

impl ContractLibrary {
    pub fn is_deprecated(&self, id: &str, version: u32) -> bool {
        self.deprecations
            .iter()
            .any(|d| d.contract_id == id && d.version == version)
    }

    fn latest(&self, id: &str) -> u32 {
        self.publications
            .iter()
            .filter(|p| p.definition().id == id)
            .map(|p| p.definition().version)
            .max()
            .unwrap_or(0)
    }
    fn next(&self, id: &str) -> Result<u32> {
        self.latest(id)
            .checked_add(1)
            .context("contract version exhausted")
    }
    fn publication(&self, id: &str, version: u32) -> Result<&PublishedContract> {
        self.publications
            .iter()
            .find(|p| p.definition().id == id && p.definition().version == version)
            .context("published contract version does not exist")
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "unsupported contract library schema"
        );
        let mut observed: BTreeMap<&str, &Value> = BTreeMap::new();
        let mut previous = None;
        for event in &self.audit {
            event.context.validate()?;
            ensure!(
                event.revision
                    == event
                        .parent_revision
                        .checked_add(1)
                        .context("audit revision exhausted")?
                    && event.revision <= self.revision
                    && previous.is_none_or(|r| r == event.parent_revision)
                    && !event.created_at.is_empty(),
                "invalid contract audit chain"
            );
            ensure!(
                event.before.keys().eq(event.after.keys())
                    && !event
                        .before
                        .keys()
                        .any(|k| matches!(k.as_str(), "audit" | "history_complete" | "revision")),
                "invalid contract audit fields"
            );
            for (key, value) in &event.before {
                if let Some(prior) = observed.get(key.as_str()) {
                    ensure!(
                        *prior == value,
                        "audit before snapshot differs from prior change"
                    );
                }
                observed.insert(key, &event.after[key]);
            }
            previous = Some(event.revision);
        }
        if !self.audit.is_empty() {
            ensure!(
                previous == Some(self.revision),
                "audit does not reach current revision"
            );
        }
        if self.history_complete && self.revision > 0 {
            ensure!(
                self.audit.first().is_some_and(|e| e.parent_revision == 0),
                "complete audit lacks baseline"
            );
        }
        let current = serde_json::to_value(self)?;
        for (key, value) in observed {
            ensure!(
                current.get(key) == Some(value),
                "audit differs from current contract asset"
            );
        }
        let mut versions: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
        for p in &self.publications {
            // Revalidate even snapshots assembled in memory through public fields.
            let _: PublishedContract = serde_json::from_value(serde_json::to_value(p)?)?;
            let d = p.definition();
            ensure!(
                versions.entry(&d.id).or_default().insert(d.version),
                "duplicate publication"
            );
            if let Some(correction) = p.correction() {
                ensure!(
                    correction.original_version().checked_add(1) == Some(d.version),
                    "published correction must inherit the immediately preceding version"
                );
                self.publication(&d.id, correction.original_version())?;
            }
        }
        for values in versions.values() {
            for (i, value) in values.iter().enumerate() {
                ensure!(
                    *value as usize == i + 1,
                    "publication versions must be contiguous from one"
                );
            }
        }
        let mut ids = BTreeSet::new();
        for d in &self.drafts {
            d.validate()?;
            ensure!(ids.insert(&d.id), "duplicate draft identity");
            ensure!(
                d.version == self.next(&d.id)?,
                "draft must target next publication version"
            );
        }
        ids.clear();
        for proposal in &self.proposals {
            ensure!(
                !proposal.id.trim().is_empty() && ids.insert(&proposal.id),
                "invalid or duplicate proposal identity"
            );
            let original = self.publication(&proposal.replacement.id, proposal.original_version)?;
            let domain = original.propose_correction(
                proposal.replacement.clone(),
                &proposal.reason,
                proposal.evidence.clone(),
            )?;
            ensure!(
                proposal.original_version.checked_add(1) == Some(proposal.replacement.version),
                "correction must target the next version"
            );
            if let Some(source) = &proposal.model_source {
                source.validate(
                    &proposal.replacement,
                    &proposal.reason,
                    &proposal.evidence,
                    original,
                )?;
                ensure!(
                    source.input.library_revision < self.revision,
                    "model correction source revision is invalid"
                );
            }
            match proposal.status {
                CorrectionStatus::Pending => ensure!(
                    proposal.reviewer.is_none(),
                    "pending correction has a reviewer"
                ),
                CorrectionStatus::Rejected => ensure!(
                    proposal
                        .reviewer
                        .as_ref()
                        .is_some_and(|r| !r.trim().is_empty()),
                    "rejected correction needs reviewer"
                ),
                CorrectionStatus::Approved => {
                    let reviewer = proposal
                        .reviewer
                        .as_deref()
                        .context("approved correction needs reviewer")?;
                    ensure!(
                        self.publication(&proposal.replacement.id, proposal.replacement.version)?
                            == &domain.approve(reviewer, ReviewerKind::Human)?,
                        "approved proposal does not match publication"
                    );
                }
            }
        }
        let mut observation_ids = BTreeSet::new();
        for observation in &self.observations {
            observation.validate()?;
            ensure!(
                observation_ids.insert((
                    &observation.proposal_id,
                    &observation.project,
                    &observation.run_id,
                    &observation.case_id
                )),
                "duplicate correction observation"
            );
            let proposal = self
                .proposals
                .iter()
                .find(|p| p.id == observation.proposal_id)
                .context("observation proposal missing")?;
            ensure!(
                proposal.status == CorrectionStatus::Approved
                    && observation.contract_version == proposal.replacement.version,
                "observation requires approved replacement"
            );
            ensure!(
                &observation.input.publication
                    == self.publication(&proposal.replacement.id, proposal.replacement.version)?,
                "observation publication differs"
            );
        }
        let mut deprecated = BTreeSet::new();
        for d in &self.deprecations {
            self.publication(&d.contract_id, d.version)?;
            reviewed(&d.reason, &d.reviewer)?;
            ensure!(
                deprecated.insert((&d.contract_id, d.version)),
                "duplicate deprecation"
            );
        }
        let mut derived = BTreeSet::new();
        for d in &self.derivations {
            self.publication(&d.source_id, d.source_version)?;
            reviewed(&d.reason, &d.reviewer)?;
            ensure!(
                d.contract_id != d.source_id && derived.insert(&d.contract_id),
                "invalid derivation identity"
            );
            ensure!(
                self.publications
                    .iter()
                    .any(|p| p.definition().id == d.contract_id)
                    || self.drafts.iter().any(|p| p.id == d.contract_id),
                "derived asset missing"
            );
        }
        for m in &self.migrations {
            self.publication(&m.contract_id, m.from_version)?;
            self.publication(&m.contract_id, m.to_version)?;
            reviewed(&m.reason, &m.reviewer)?;
            let unique: BTreeSet<_> = m.projects.iter().collect();
            ensure!(
                m.from_version != m.to_version
                    && !m.projects.is_empty()
                    && unique.len() == m.projects.len()
                    && m.projects.iter().all(|p| !p.trim().is_empty()),
                "invalid migration record"
            );
        }
        let mut bindings = BTreeSet::new();
        for b in &self.bindings {
            ensure!(
                bindings.insert((b.project(), b.contract_id())),
                "duplicate project binding"
            );
            ensure!(
                &self
                    .publication(b.contract_id(), b.contract_version())?
                    .bind(b.project(), b.parameters().clone())?
                    == b,
                "invalid binding"
            );
        }
        Ok(())
    }
}

fn reviewed(reason: &str, reviewer: &str) -> Result<()> {
    ensure!(
        !reason.trim().is_empty() && !reviewer.trim().is_empty(),
        "reason and human reviewer are required"
    );
    Ok(())
}

// Lifecycle decisions are local policy, not part of the publication-only bundle.
// Imports never clear local deprecations or migrate existing bindings.
/// Exports contain immutable definitions and review provenance only. Bindings
/// and project parameters are deliberately outside the interchange format.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractBundle {
    pub schema_version: u32,
    pub contracts: Vec<PublishedContract>,
}

pub struct ContractStore {
    context: crate::storage::local::AuditContext,
    file: AssetFile,
}

impl ContractStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            context: Default::default(),
            file: AssetFile::new(path, MAX_BYTES),
        }
    }

    pub fn with_audit_context(
        mut self,
        context: crate::storage::local::AuditContext,
    ) -> Result<Self> {
        context.validate()?;
        self.context = context;
        Ok(self)
    }
    pub fn audit(&self) -> Result<Vec<ContractAuditEvent>> {
        Ok(self.snapshot()?.audit)
    }
    pub fn snapshot(&self) -> Result<ContractLibrary> {
        match self.file.lock()?.read()? {
            Some(bytes) => serde_json::from_slice(&bytes).context("invalid contract library"),
            None => Ok(ContractLibrary::default()),
        }
    }

    fn update(
        &self,
        expected: u64,
        change: impl FnOnce(&mut ContractLibrary) -> Result<()>,
    ) -> Result<ContractLibrary> {
        self.update_checked(expected, change, || Ok(()))
    }
    fn update_checked(
        &self,
        expected: u64,
        change: impl FnOnce(&mut ContractLibrary) -> Result<()>,
        before_commit: impl Fn() -> Result<()>,
    ) -> Result<ContractLibrary> {
        let guard = self.file.lock()?;
        before_commit()?;
        let mut library: ContractLibrary = match guard.read()? {
            Some(bytes) => serde_json::from_slice(&bytes).context("invalid contract library")?,
            None => ContractLibrary::default(),
        };
        ensure!(
            library.revision == expected,
            "stale contract library revision: expected {}, current {}",
            expected,
            library.revision
        );
        self.context.validate()?;
        let original = serde_json::to_value(&library)?;
        change(&mut library)?;
        library.revision = library
            .revision
            .checked_add(1)
            .context("library revision exhausted")?;
        let updated = serde_json::to_value(&library)?;
        let mut before = BTreeMap::new();
        let mut after = BTreeMap::new();
        for (k, v) in original.as_object().unwrap() {
            if !matches!(k.as_str(), "audit" | "history_complete" | "revision")
                && updated.get(k) != Some(v)
            {
                before.insert(k.clone(), v.clone());
                after.insert(k.clone(), updated[k].clone());
            }
        }
        library.audit.push(ContractAuditEvent {
            revision: library.revision,
            parent_revision: expected,
            created_at: chrono::Utc::now().to_rfc3339(),
            context: self.context.clone(),
            before,
            after,
        });
        library.validate()?;
        let bytes = serde_json::to_vec_pretty(&library)?;
        // This is the atomic commit boundary. Cancellation accepted before it
        // cannot persist a proposal; an already begun write is not retractable.
        before_commit()?;
        guard.write(&bytes)?;
        Ok(library)
    }

    pub fn save_draft(&self, draft: ContractDraft, expected: u64) -> Result<ContractLibrary> {
        self.update(expected, |lib| {
            draft.validate()?;
            ensure!(
                draft.version == lib.next(&draft.id)?,
                "draft must target next publication version"
            );
            lib.drafts.retain(|d| d.id != draft.id);
            lib.drafts.push(draft);
            Ok(())
        })
    }

    pub fn publish(&self, id: &str, expected: u64, reviewer: &str) -> Result<ContractLibrary> {
        self.update(expected, |lib| {
            let index = lib
                .drafts
                .iter()
                .position(|d| d.id == id)
                .context("draft not found")?;
            let draft = lib.drafts.remove(index);
            ensure!(
                draft.version == lib.next(id)?,
                "publication must use next version"
            );
            lib.publications.push(draft.publish(reviewer)?);
            Ok(())
        })
    }

    pub fn propose(
        &self,
        contract_id: &str,
        version: u32,
        replacement: ContractDraft,
        reason: &str,
        evidence: Vec<String>,
        expected: u64,
    ) -> Result<ContractLibrary> {
        self.propose_with_source(
            contract_id,
            version,
            replacement,
            reason,
            evidence,
            expected,
            None,
            None,
        )
    }

    pub fn propose_model(
        &self,
        replacement: ContractDraft,
        reason: &str,
        source: ModelCorrectionSource,
        expected: u64,
    ) -> Result<ContractLibrary> {
        ensure!(
            source.input.library_revision == expected,
            "model correction source revision mismatch"
        );
        self.propose_with_source(
            &source.input.contract_id.clone(),
            source.input.version,
            replacement,
            reason,
            source.evidence_tokens.clone(),
            expected,
            Some(source),
            None,
        )
    }

    pub fn propose_model_cancellable(
        &self,
        replacement: ContractDraft,
        reason: &str,
        source: ModelCorrectionSource,
        expected: u64,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<ContractLibrary> {
        ensure!(
            source.input.library_revision == expected,
            "model correction source revision mismatch"
        );
        self.propose_with_source(
            &source.input.contract_id.clone(),
            source.input.version,
            replacement,
            reason,
            source.evidence_tokens.clone(),
            expected,
            Some(source),
            Some(cancel),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn propose_with_source(
        &self,
        contract_id: &str,
        version: u32,
        replacement: ContractDraft,
        reason: &str,
        evidence: Vec<String>,
        expected: u64,
        model_source: Option<ModelCorrectionSource>,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<ContractLibrary> {
        self.update_checked(
            expected,
            |lib| {
                ensure!(
                    version == lib.latest(contract_id),
                    "correction base is stale"
                );
                ensure!(
                    replacement.version == lib.next(contract_id)?,
                    "correction must target next version"
                );
                lib.publication(contract_id, version)?.propose_correction(
                    replacement.clone(),
                    reason,
                    evidence.clone(),
                )?;
                lib.proposals.push(StoredCorrection {
                    id: format!(
                        "correction-{}",
                        expected.checked_add(1).context("revision exhausted")?
                    ),
                    original_version: version,
                    replacement,
                    reason: reason.into(),
                    evidence,
                    status: CorrectionStatus::Pending,
                    reviewer: None,
                    model_source,
                });
                Ok(())
            },
            || {
                ensure!(
                    !cancel.is_some_and(|c| c.load(std::sync::atomic::Ordering::Acquire)),
                    "Contract correction cancelled before commit"
                );
                Ok(())
            },
        )
    }

    pub fn review(
        &self,
        proposal_id: &str,
        approve: bool,
        expected: u64,
        reviewer: &str,
    ) -> Result<ContractLibrary> {
        self.update(expected, |lib| {
            ensure!(!reviewer.trim().is_empty(), "reviewer is required");
            let index = lib
                .proposals
                .iter()
                .position(|p| p.id == proposal_id)
                .context("proposal not found")?;
            let proposal = lib.proposals[index].clone();
            ensure!(
                proposal.status == CorrectionStatus::Pending,
                "proposal already reviewed"
            );
            if approve {
                ensure!(
                    lib.latest(&proposal.replacement.id) == proposal.original_version,
                    "correction base is stale"
                );
                let publication = lib
                    .publication(&proposal.replacement.id, proposal.original_version)?
                    .propose_correction(
                        proposal.replacement.clone(),
                        &proposal.reason,
                        proposal.evidence.clone(),
                    )?
                    .approve(reviewer, ReviewerKind::Human)?;
                lib.publications.push(publication);
                lib.drafts.retain(|d| d.id != proposal.replacement.id);
            }
            lib.proposals[index].status = if approve {
                CorrectionStatus::Approved
            } else {
                CorrectionStatus::Rejected
            };
            lib.proposals[index].reviewer = Some(reviewer.into());
            Ok(())
        })
    }

    pub fn observe_correction(
        &self,
        proposal_id: &str,
        record: &crate::execution_records::RunRecord,
        case_id: &str,
        expected: u64,
    ) -> Result<ContractLibrary> {
        self.update(expected, |lib| {
            let proposal = lib
                .proposals
                .iter()
                .find(|p| p.id == proposal_id)
                .context("correction proposal not found")?;
            ensure!(
                proposal.status == CorrectionStatus::Approved,
                "correction must be approved before observation"
            );
            let publication =
                lib.publication(&proposal.replacement.id, proposal.replacement.version)?;
            let observation =
                CorrectionObservation::prepare(proposal_id, record, case_id, publication)?;
            ensure!(
                !lib.observations.iter().any(|o| o.proposal_id == proposal_id
                    && o.project == record.project
                    && o.run_id == record.run_id
                    && o.case_id == case_id),
                "observation already recorded"
            );
            lib.observations.push(observation);
            Ok(())
        })
    }

    pub fn bind(
        &self,
        project: &str,
        contract_id: &str,
        version: u32,
        parameters: BTreeMap<String, Value>,
        expected: u64,
    ) -> Result<ContractLibrary> {
        self.update(expected, |lib| {
            ensure!(
                !lib.is_deprecated(contract_id, version),
                "contract version is deprecated; select an active version"
            );
            let binding = lib
                .publication(contract_id, version)?
                .bind(project, parameters)?;
            lib.bindings
                .retain(|b| b.project() != project || b.contract_id() != contract_id);
            lib.bindings.push(binding);
            Ok(())
        })
    }

    pub fn deprecate(
        &self,
        id: &str,
        version: u32,
        reason: &str,
        expected: u64,
        reviewer: &str,
    ) -> Result<ContractLibrary> {
        self.update(expected, |lib| {
            reviewed(reason, reviewer)?;
            lib.publication(id, version)?;
            ensure!(
                !lib.is_deprecated(id, version),
                "contract version already deprecated"
            );
            lib.deprecations.push(Deprecation {
                contract_id: id.into(),
                version,
                reason: reason.into(),
                reviewer: reviewer.into(),
            });
            Ok(())
        })
    }

    pub fn derive(
        &self,
        source_id: &str,
        source_version: u32,
        new_id: &str,
        reason: &str,
        expected: u64,
        reviewer: &str,
    ) -> Result<ContractLibrary> {
        self.update(expected, |lib| {
            reviewed(reason, reviewer)?;
            ensure!(
                new_id != source_id
                    && !new_id.trim().is_empty()
                    && !lib.drafts.iter().any(|d| d.id == new_id)
                    && lib.latest(new_id) == 0,
                "derivation needs a fresh contract identity"
            );
            let mut draft = lib
                .publication(source_id, source_version)?
                .definition()
                .clone();
            draft.id = new_id.into();
            draft.version = 1;
            lib.drafts.push(draft);
            lib.derivations.push(Derivation {
                contract_id: new_id.into(),
                source_id: source_id.into(),
                source_version,
                reason: reason.into(),
                reviewer: reviewer.into(),
            });
            Ok(())
        })
    }

    pub fn migrate(
        &self,
        id: &str,
        from: u32,
        to: u32,
        bindings: Vec<MigrationBinding>,
        reason: &str,
        expected: u64,
        reviewer: &str,
    ) -> Result<ContractLibrary> {
        self.update(expected, |lib| {
            reviewed(reason, reviewer)?;
            lib.publication(id, from)?;
            ensure!(
                from != to && !bindings.is_empty(),
                "migration requires distinct versions and selected projects"
            );
            ensure!(!lib.is_deprecated(id, to), "migration target is deprecated");
            let publication = lib.publication(id, to)?;
            let mut selected = BTreeSet::new();
            let mut replacements = Vec::new();
            for b in bindings {
                ensure!(
                    selected.insert(b.project.clone()),
                    "duplicate migration project"
                );
                ensure!(
                    lib.bindings.iter().any(|old| old.project() == b.project
                        && old.contract_id() == id
                        && old.contract_version() == from),
                    "migration source binding changed or missing"
                );
                replacements.push(publication.bind(&b.project, b.parameters)?);
            }
            lib.bindings
                .retain(|b| !(b.contract_id() == id && selected.contains(b.project())));
            lib.bindings.extend(replacements);
            lib.migrations.push(Migration {
                contract_id: id.into(),
                from_version: from,
                to_version: to,
                projects: selected.into_iter().collect(),
                reason: reason.into(),
                reviewer: reviewer.into(),
            });
            Ok(())
        })
    }

    pub fn export(&self) -> Result<ContractBundle> {
        Ok(ContractBundle {
            schema_version: 1,
            contracts: self.snapshot()?.publications,
        })
    }

    pub fn import(&self, bundle: ContractBundle, expected: u64) -> Result<ContractLibrary> {
        ensure!(bundle.schema_version == 1, "unsupported bundle schema");
        self.update(expected, |lib| {
            let mut seen = BTreeSet::new();
            for publication in bundle.contracts {
                let d = publication.definition();
                ensure!(
                    seen.insert((d.id.clone(), d.version)),
                    "duplicate publication in bundle"
                );
                match lib
                    .publications
                    .iter()
                    .find(|p| p.definition().id == d.id && p.definition().version == d.version)
                {
                    Some(existing) if existing != &publication => {
                        bail!("conflicting immutable publication")
                    }
                    Some(_) => {}
                    None => lib.publications.push(publication),
                }
            }
            // Drafts are never silently replaced by imported assets.
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::ValueType;

    fn draft(id: &str, version: u32) -> ContractDraft {
        ContractDraft {
            id: id.into(),
            version,
            intent: "receipt is returned".into(),
            inputs: BTreeMap::from([("target".into(), ValueType::String)]),
            outputs: BTreeMap::new(),
            preconditions: vec![],
            assertions: vec!["receipt matches input".into()],
            required_capabilities: vec![],
        }
    }
    fn parameters(target: &str) -> BTreeMap<String, Value> {
        BTreeMap::from([("target".into(), Value::from(target))])
    }
    fn published(store: &ContractStore) -> ContractLibrary {
        store.save_draft(draft("receipt", 1), 0).unwrap();
        store.publish("receipt", 1, "owner").unwrap()
    }

    #[test]
    fn legacy_library_defaults_lifecycle_and_derivation_requires_fresh_identity() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContractStore::new(dir.path().join("library.json"));
        let original = published(&store);
        let mut legacy = serde_json::to_value(&original).unwrap();
        for key in ["deprecations", "derivations", "migrations"] {
            legacy.as_object_mut().unwrap().remove(key);
        }
        let restored: ContractLibrary = serde_json::from_value(legacy).unwrap();
        assert_eq!(restored, original);
        assert!(store
            .derive("receipt", 1, "receipt", "reuse", 2, "owner")
            .is_err());
        assert!(store.derive("receipt", 1, "new", "", 2, "owner").is_err());
        let derived = store
            .derive("receipt", 1, "new", "different domain", 2, "owner")
            .unwrap();
        assert_eq!(
            derived.drafts[0].assertions,
            original.publications[0].definition().assertions
        );
        assert_eq!(derived.derivations[0].source_version, 1);
        assert!(store
            .derive("receipt", 1, "new", "reuse", 3, "owner")
            .is_err());
        let saved = store.publish("new", 3, "owner").unwrap();
        assert_eq!(saved.derivations.len(), 1);
        assert_eq!(saved.publications.len(), 2);
    }

    #[test]
    fn deprecated_versions_preserve_history_but_block_new_preparations() {
        use crate::contract_execution::ExecutionSnapshot;
        let dir = tempfile::tempdir().unwrap();
        let store = ContractStore::new(dir.path().join("library.json"));
        published(&store);
        let bound = store
            .bind("alpha", "receipt", 1, parameters("https://alpha.test"), 2)
            .unwrap();
        let historical = ExecutionSnapshot::prepare(&bound, "alpha", &[]).unwrap();
        let deprecated = store
            .deprecate("receipt", 1, "use corrected rule", 3, "owner")
            .unwrap();
        assert_eq!(deprecated.bindings, bound.bindings);
        historical.validate_against_library(&deprecated).unwrap();
        assert!(ExecutionSnapshot::prepare(&deprecated, "alpha", &[]).is_err());
        assert!(store
            .bind("beta", "receipt", 1, parameters("https://beta.test"), 4)
            .is_err());
        assert!(store.deprecate("receipt", 1, "again", 4, "owner").is_err());
        // Deprecation is local policy; publication export cannot erase it on import.
        store.import(store.export().unwrap(), 4).unwrap();
        assert!(store.snapshot().unwrap().is_deprecated("receipt", 1));
    }

    #[test]
    fn migrations_validate_selected_bindings_atomically_and_keep_unselected_pins() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContractStore::new(dir.path().join("library.json"));
        published(&store);
        store
            .bind("alpha", "receipt", 1, parameters("a"), 2)
            .unwrap();
        store
            .bind("beta", "receipt", 1, parameters("b"), 3)
            .unwrap();
        store.save_draft(draft("receipt", 2), 4).unwrap();
        let before = store.publish("receipt", 5, "owner").unwrap();
        let binding = |project: &str| MigrationBinding {
            project: project.into(),
            parameters: parameters(project),
        };
        assert!(store
            .migrate(
                "receipt",
                1,
                2,
                vec![
                    binding("alpha"),
                    MigrationBinding {
                        project: "beta".into(),
                        parameters: BTreeMap::new()
                    }
                ],
                "upgrade",
                6,
                "owner"
            )
            .is_err());
        assert_eq!(store.snapshot().unwrap(), before);
        assert!(store
            .migrate(
                "receipt",
                1,
                2,
                vec![binding("alpha"), binding("alpha")],
                "upgrade",
                6,
                "owner"
            )
            .is_err());
        assert!(store
            .migrate(
                "receipt",
                1,
                2,
                vec![binding("alpha")],
                "upgrade",
                5,
                "owner"
            )
            .is_err());
        let migrated = store
            .migrate(
                "receipt",
                1,
                2,
                vec![binding("alpha")],
                "reviewed target",
                6,
                "owner",
            )
            .unwrap();
        assert_eq!(
            migrated
                .bindings
                .iter()
                .find(|b| b.project() == "alpha")
                .unwrap()
                .contract_version(),
            2
        );
        assert_eq!(
            migrated
                .bindings
                .iter()
                .find(|b| b.project() == "beta")
                .unwrap()
                .contract_version(),
            1
        );
        assert_eq!(migrated.migrations[0].projects, vec!["alpha"]);
        assert!(store
            .migrate(
                "receipt",
                1,
                2,
                vec![binding("alpha")],
                "stale pin",
                7,
                "owner"
            )
            .is_err());
        store
            .deprecate("receipt", 2, "invalid", 7, "owner")
            .unwrap();
        assert!(store
            .migrate(
                "receipt",
                1,
                2,
                vec![binding("beta")],
                "deprecated target",
                8,
                "owner"
            )
            .is_err());
    }

    #[test]
    fn persists_two_project_bindings_and_cas() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let store = ContractStore::new(path.clone());
        published(&store);
        store
            .bind("alpha", "receipt", 1, parameters("https://alpha.test"), 2)
            .unwrap();
        let saved = store
            .bind("beta", "receipt", 1, parameters("https://beta.test"), 3)
            .unwrap();
        assert_eq!(ContractStore::new(path).snapshot().unwrap(), saved);
        assert_ne!(
            saved.bindings[0].parameters(),
            saved.bindings[1].parameters()
        );
        assert!(store
            .bind("alpha", "receipt", 1, parameters("changed"), 3)
            .is_err());
        assert_eq!(store.snapshot().unwrap(), saved);
        assert!(store
            .bind("alpha", "receipt", 1, BTreeMap::new(), 4)
            .is_err());
        let changed = store
            .bind("alpha", "receipt", 1, parameters("changed"), 4)
            .unwrap();
        assert_eq!(changed.bindings.len(), 2);
        assert!(!serde_json::to_string(&store.export().unwrap())
            .unwrap()
            .contains("alpha.test"));
    }

    #[test]
    fn correction_review_preserves_original_and_rejects_stale_base() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContractStore::new(dir.path().join("library.json"));
        let original = published(&store).publications[0].clone();
        let mut replacement = draft("receipt", 2);
        replacement.assertions.push("receipt has timestamp".into());
        let proposed = store
            .propose(
                "receipt",
                1,
                replacement.clone(),
                "clarify receipt",
                vec!["run/1".into()],
                2,
            )
            .unwrap();
        let first_id = proposed.proposals[0].id.clone();
        let proposed = store
            .propose(
                "receipt",
                1,
                replacement,
                "other observation",
                vec!["run/2".into()],
                3,
            )
            .unwrap();
        let second_id = proposed.proposals[1].id.clone();
        let approved = store.review(&first_id, true, 4, "owner").unwrap();
        assert_eq!(approved.publications[0], original);
        assert_eq!(
            approved.publications[1]
                .correction()
                .unwrap()
                .original_version(),
            1
        );
        assert!(store.review(&second_id, true, 5, "owner").is_err());
        let rejected = store.review(&second_id, false, 5, "owner").unwrap();
        assert_eq!(rejected.proposals[1].status, CorrectionStatus::Rejected);
        assert_eq!(store.snapshot().unwrap(), rejected);
    }

    #[test]
    fn imports_validate_all_before_atomic_commit() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContractStore::new(dir.path().join("library.json"));
        let before = published(&store);
        let mut conflict = draft("receipt", 1);
        conflict.intent = "different intent".into();
        let bundle = ContractBundle {
            schema_version: 1,
            contracts: vec![
                draft("other", 1).publish("owner").unwrap(),
                conflict.publish("owner").unwrap(),
            ],
        };
        assert!(store.import(bundle, 2).is_err());
        assert_eq!(store.snapshot().unwrap(), before);
        let target = ContractStore::new(dir.path().join("target.json"));
        let imported = target.import(store.export().unwrap(), 0).unwrap();
        assert_eq!(imported.publications, before.publications);
        assert!(target
            .import(
                ContractBundle {
                    schema_version: 1,
                    contracts: vec![draft("gap", 2).publish("owner").unwrap()]
                },
                1
            )
            .is_err());
        let mut exported = serde_json::to_value(store.export().unwrap()).unwrap();
        exported["bindings"] = serde_json::json!([]);
        assert!(serde_json::from_value::<ContractBundle>(exported).is_err());
    }

    #[test]
    fn imported_and_reloaded_corrections_reject_stale_ancestry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let store = ContractStore::new(path.clone());
        published(&store);
        store.save_draft(draft("receipt", 2), 2).unwrap();
        let before = store.publish("receipt", 3, "owner").unwrap();
        let disk_before = std::fs::read(&path).unwrap();
        let mut value =
            serde_json::to_value(draft("receipt", 3).publish("external-author").unwrap()).unwrap();
        value["correction"] = serde_json::json!({"original_version":1,"reason":"stale baseline","evidence":["external/run-1"]});
        let stale: PublishedContract = serde_json::from_value(value.clone()).unwrap();
        assert!(store
            .import(
                ContractBundle {
                    schema_version: 1,
                    contracts: vec![stale]
                },
                4
            )
            .is_err());
        assert_eq!(store.snapshot().unwrap(), before);
        assert_eq!(std::fs::read(&path).unwrap(), disk_before);

        value["correction"]["original_version"] = Value::from(2);
        let valid: PublishedContract = serde_json::from_value(value).unwrap();
        let imported = store
            .import(
                ContractBundle {
                    schema_version: 1,
                    contracts: vec![valid],
                },
                4,
            )
            .unwrap();
        assert_eq!(imported.publications[2].reviewer(), "external-author");
        let mut corrupt = serde_json::to_value(imported).unwrap();
        corrupt["publications"][2]["correction"]["original_version"] = Value::from(1);
        let corrupt = serde_json::to_vec(&corrupt).unwrap();
        std::fs::write(&path, &corrupt).unwrap();
        assert!(store.snapshot().is_err());
        assert!(store.save_draft(draft("other", 1), 5).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), corrupt);
    }

    #[test]
    fn corrupt_and_malformed_bindings_are_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let store = ContractStore::new(path.clone());
        published(&store);
        let mut value = serde_json::to_value(store.snapshot().unwrap()).unwrap();
        value["bindings"] = serde_json::json!([{"project":"p", "contract_id":"receipt", "contract_version":1, "parameters":{"target":42}}]);
        let bad = serde_json::to_vec(&value).unwrap();
        std::fs::write(&path, &bad).unwrap();
        assert!(store.snapshot().is_err());
        assert!(store.save_draft(draft("new", 1), 2).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bad);
        std::fs::write(&path, b"not json").unwrap();
        assert!(store.snapshot().is_err());
    }

    #[test]
    fn lock_contention_and_size_limit_fail_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let store = ContractStore::new(path.clone());
        let file = AssetFile::new(path.clone(), MAX_BYTES);
        let _held = file.lock().unwrap();
        assert!(store.save_draft(draft("new", 1), 0).is_err());
        drop(_held);
        std::fs::write(&path, vec![b' '; MAX_BYTES + 1]).unwrap();
        assert!(store.snapshot().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_and_uses_private_permissions() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let outside = dir.path().join("outside.json");
        std::fs::write(&outside, b"unchanged").unwrap();
        symlink(&outside, &path).unwrap();
        let store = ContractStore::new(path.clone());
        assert!(store.snapshot().is_err());
        assert!(store.save_draft(draft("new", 1), 0).is_err());
        assert_eq!(std::fs::read(&outside).unwrap(), b"unchanged");
        std::fs::remove_file(&path).unwrap();
        published(&store);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let alias = dir.path().join("alias");
        symlink(dir.path(), &alias).unwrap();
        assert!(ContractStore::new(alias.join("library.json"))
            .snapshot()
            .is_err());
    }
    #[test]
    fn cancellation_after_lock_and_before_commit_preserves_exact_library() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let dir = tempfile::tempdir().unwrap();
        let store = ContractStore::new(dir.path().join("library.json"));
        let original = published(&store);
        let before = std::fs::read(dir.path().join("library.json")).unwrap();
        let cancel = AtomicBool::new(false);
        let result = store.update_checked(
            original.revision,
            |lib| {
                lib.drafts.push(draft("receipt", 2));
                // Deterministic cancellation after admission under the lock, before
                // serialization/commit. A preceding async select cannot catch it.
                cancel.store(true, Ordering::Release);
                Ok(())
            },
            || {
                ensure!(!cancel.load(Ordering::Acquire), "cancelled before commit");
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(
            std::fs::read(dir.path().join("library.json")).unwrap(),
            before
        );
        assert_eq!(store.snapshot().unwrap(), original);
        // Contention is explicitly fail-fast; it never queues a late write.
        let guard = store.file.lock().unwrap();
        assert!(store
            .update_checked(original.revision, |_| Ok(()), || Ok(()))
            .is_err());
        cancel.store(true, Ordering::Release);
        drop(guard);
        assert_eq!(store.snapshot().unwrap(), original);
    }
    #[test]
    fn audit_is_atomic_with_cas_and_context_and_rejects_forged_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("contracts.json");
        let context = crate::storage::local::AuditContext {
            source: "local_web".into(),
            author: Some("local-owner".into()),
            reason: "Review fixture contract".into(),
        };
        let store = ContractStore::new(path.clone())
            .with_audit_context(context.clone())
            .unwrap();
        let first = store.save_draft(draft("audit", 1), 0).unwrap();
        assert_eq!(first.audit.len(), 1);
        assert_eq!(first.audit[0].context, context);
        assert_eq!(first.audit[0].parent_revision, 0);
        let bytes = std::fs::read(&path).unwrap();
        assert!(store.save_draft(draft("other", 1), 0).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let published = store.publish("audit", 1, "local-owner").unwrap();
        assert_eq!(published.audit.len(), 2);
        let mut forged = serde_json::to_value(&published).unwrap();
        forged["audit"][1]["after"]["drafts"] = serde_json::json!(["forged"]);
        assert!(serde_json::from_value::<ContractLibrary>(forged).is_err());
        let mut legacy = serde_json::to_value(&published).unwrap();
        legacy.as_object_mut().unwrap().remove("audit");
        legacy.as_object_mut().unwrap().remove("history_complete");
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert!(!store.snapshot().unwrap().history_complete);
        let next = store.save_draft(draft("other", 1), 2).unwrap();
        assert!(!next.history_complete);
        assert_eq!(next.audit[0].parent_revision, 2);
    }
}
