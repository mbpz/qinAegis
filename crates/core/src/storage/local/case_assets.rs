//! Recoverable case transitions and exact-content optimistic editing.
use super::case_history::{self, AuditContext, CaseHistory, HistoryMutation};
use crate::asset_file::{AssetFile, AssetGuard};
use crate::storage::{CaseStatus, TestCase};
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
const MAX_CASE: usize = 128 * 1024;
const MAX_JOURNAL: usize = 2 * 1024 * 1024;
const STATUSES: [CaseStatus; 5] = [
    CaseStatus::Draft,
    CaseStatus::Reviewed,
    CaseStatus::Approved,
    CaseStatus::Flaky,
    CaseStatus::Archived,
];
#[derive(Serialize)]
pub struct CaseAsset {
    pub revision: u64,
    pub case_id: String,
    pub name: String,
    pub yaml_script: String,
    pub expected_result: String,
    pub status: CaseStatus,
    pub provenance: Option<crate::execution_records::CaseProvenance>,
    /// Collision-free exact-content revision; not a hash, signature or authorization token.
    pub digest: String,
}
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum TransitionKind {
    Status,
    Edit,
    Save,
    Delete,
    Restore,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Transition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    history: Option<HistoryMutation>,
    schema: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<TransitionKind>,
    id: String,
    from: Option<CaseStatus>,
    to: CaseStatus,
    before: Vec<u8>,
    after: Vec<u8>,
}
pub(super) struct CaseAssets {
    context: AuditContext,
    root: PathBuf,
    project: String,
}
fn identity(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 128
            && !value.starts_with('.')
            && value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.')),
        "invalid case asset identity"
    );
    Ok(())
}
impl CaseAssets {
    pub(super) fn new(root: PathBuf, project: &str) -> Result<Self> {
        identity(project)?;
        ensure!(root.is_absolute(), "case root must be absolute");
        let parent_path = root.parent().context("case root needs project directory")?;
        // Resolve platform aliases only after checking the managed project/data ancestors.
        for managed in parent_path.ancestors().take(3) {
            let meta = std::fs::symlink_metadata(managed)?;
            ensure!(
                meta.is_dir() && !meta.file_type().is_symlink(),
                "unsafe managed case ancestor"
            );
        }
        let parent = parent_path.canonicalize()?;
        let root = parent.join("cases");
        for component in parent.ancestors() {
            let meta = std::fs::symlink_metadata(component)?;
            ensure!(
                meta.is_dir() && !meta.file_type().is_symlink(),
                "unsafe case ancestor"
            );
        }
        Ok(Self {
            context: Default::default(),
            root,
            project: project.into(),
        })
    }
    pub(super) fn with_context(mut self, context: AuditContext) -> Result<Self> {
        context.validate()?;
        self.context = context;
        Ok(self)
    }
    fn path(&self, id: &str, status: Option<CaseStatus>) -> Result<PathBuf> {
        identity(id)?;
        Ok(status
            .map(|s| self.root.join(s.dir_name()))
            .unwrap_or_else(|| self.root.clone())
            .join(format!("{id}.json")))
    }
    fn read(&self, path: &Path) -> Result<Option<Vec<u8>>> {
        AssetFile::new(path.to_path_buf(), MAX_CASE).lock()?.read()
    }
    fn parse(&self, bytes: &[u8], id: &str, status: Option<CaseStatus>) -> Result<TestCase> {
        ensure!(bytes.len() <= MAX_CASE, "case asset exceeds editor limit");
        let case: TestCase = serde_json::from_slice(bytes).context("invalid case asset")?;
        ensure!(
            case.id == id && status.map(|s| s == case.status).unwrap_or(true),
            "case asset identity or status mismatch"
        );
        if let Some(p) = &case.provenance {
            p.validate(&self.project, id, &case.yaml_script)?;
        }
        Ok(case)
    }
    fn validate_transition(&self, t: &Transition) -> Result<()> {
        identity(&t.id)?;
        ensure!(t.schema == 1, "unsupported case transition journal");
        if matches!(t.kind, Some(TransitionKind::Save)) {
            self.parse(&t.after, &t.id, Some(t.to))?;
            if !t.before.is_empty() {
                let before = self.parse(&t.before, &t.id, t.from)?;
                let after = self.parse(&t.after, &t.id, Some(t.to))?;
                ensure!(
                    before.status == after.status && after.status == CaseStatus::Draft,
                    "typed content changes require Draft"
                );
            }
            return Ok(());
        }
        if matches!(t.kind, Some(TransitionKind::Delete)) {
            self.parse(&t.before, &t.id, t.from)?;
            ensure!(t.after.is_empty(), "delete contains content");
            return Ok(());
        }
        if matches!(t.kind, Some(TransitionKind::Restore)) {
            self.parse(&t.before, &t.id, t.from)?;
            let after = self.parse(&t.after, &t.id, Some(t.to))?;
            ensure!(
                after.status == CaseStatus::Draft && after.provenance.is_none(),
                "restore must create unattributed draft"
            );
            let actual: Value = serde_json::from_slice(&t.after)?;
            let h = case_history::load(&self.root, &t.id)?;
            ensure!(
                h.revisions
                    .iter()
                    .filter(|r| t
                        .history
                        .as_ref()
                        .is_none_or(|m| r.revision < m.entry.revision))
                    .filter_map(|r| r.snapshot.clone())
                    .any(|mut v| {
                        v["status"] = serde_json::json!("draft");
                        v.as_object_mut().unwrap().remove("provenance");
                        v == actual
                    }),
                "restore lacks historical source snapshot"
            );
            return Ok(());
        }
        let before_case = self.parse(&t.before, &t.id, t.from)?;
        let after_case = self.parse(&t.after, &t.id, Some(t.to))?;
        let mut before_value: Value = serde_json::from_slice(&t.before)?;
        let mut after_value: Value = serde_json::from_slice(&t.after)?;
        before_value
            .as_object_mut()
            .context("invalid before asset")?
            .remove("status");
        after_value
            .as_object_mut()
            .context("invalid after asset")?
            .remove("status");
        // Legacy journals omitted kind. Prefer a valid pure status transition;
        // otherwise recover only a fully validated manual reset to Draft.
        let kind = t.kind.unwrap_or_else(|| {
            if before_value == after_value && before_case.status.can_transition_to(t.to) {
                TransitionKind::Status
            } else {
                TransitionKind::Edit
            }
        });
        if matches!(kind, TransitionKind::Status) {
            ensure!(
                before_value == after_value,
                "status journal modified case content"
            );
            ensure!(
                before_case.status.can_transition_to(t.to),
                "invalid journal status transition"
            );
        } else {
            ensure!(
                t.to == CaseStatus::Draft && after_case.provenance.is_none(),
                "edited case journal must reset to unattributed draft"
            );
            ensure!(
                !after_case.name.trim().is_empty()
                    && after_case.name.len() <= 500
                    && after_case.yaml_script.len() <= 64 * 1024
                    && after_value
                        .get("expected_result")
                        .is_none_or(|v| v.as_str().is_some_and(|v| v.len() <= 16 * 1024)),
                "invalid edited case content"
            );
            validate_script(&after_case.yaml_script)?;
            for key in ["name", "yaml_script", "expected_result", "provenance"] {
                before_value.as_object_mut().unwrap().remove(key);
                after_value.as_object_mut().unwrap().remove(key);
            }
            ensure!(
                before_value == after_value,
                "journal modified immutable case metadata"
            );
        }
        Ok(())
    }
    fn recover(&self, guard: &AssetGuard<'_>) -> Result<()> {
        let Some(bytes) = guard.read()? else {
            return Ok(());
        };
        let pending: Option<Transition> =
            serde_json::from_slice(&bytes).context("invalid case transition journal")?;
        let Some(t) = pending else { return Ok(()) };
        self.validate_transition(&t)?;
        if let Some(h) = &t.history {
            let before = if t.before.is_empty() {
                None
            } else {
                Some(t.before.as_slice())
            };
            let after = if t.after.is_empty() {
                None
            } else {
                Some(t.after.as_slice())
            };
            ensure!(
                h.before_digest == before.map(|b| format!("content:{}", STANDARD.encode(b))),
                "journal history before mismatch"
            );
            ensure!(
                h.entry.digest == after.map(|b| format!("content:{}", STANDARD.encode(b))),
                "journal history after mismatch"
            );
            if let Some(base) = &h.baseline {
                ensure!(
                    base.digest == before.map(|b| format!("content:{}", STANDARD.encode(b))),
                    "journal history baseline mismatch"
                );
            }
        }
        let src = self.path(&t.id, t.from)?;
        let dst = self.path(&t.id, Some(t.to))?;
        let current_src = self.read(&src)?;
        let current_dst = self.read(&dst)?;
        if matches!(t.kind, Some(TransitionKind::Save)) && t.before.is_empty() {
            ensure!(
                current_dst.is_none() || current_dst.as_deref() == Some(t.after.as_slice()),
                "creation recovery conflict"
            );
            self.record_history(&t)?;
            AssetFile::new(dst, MAX_CASE).lock()?.write(&t.after)?;
            guard.write(b"null")?;
            return Ok(());
        }
        if matches!(t.kind, Some(TransitionKind::Delete)) {
            ensure!(
                current_src.is_none() || current_src.as_deref() == Some(t.before.as_slice()),
                "delete recovery conflict"
            );
            self.record_history(&t)?;
            if current_src.is_some() {
                std::fs::remove_file(&src)?;
                #[cfg(unix)]
                std::fs::File::open(src.parent().unwrap())?.sync_all()?;
            }
            guard.write(b"null")?;
            return Ok(());
        }
        if src == dst {
            ensure!(
                current_src.as_deref() == Some(t.before.as_slice())
                    || current_src.as_deref() == Some(t.after.as_slice()),
                "case changed during recovery"
            );
        } else {
            ensure!(
                current_src.as_deref() == Some(t.before.as_slice()) || current_src.is_none(),
                "source changed during recovery"
            );
            ensure!(
                current_dst.is_none() || current_dst.as_deref() == Some(t.after.as_slice()),
                "destination changed during recovery"
            );
            ensure!(
                current_src.is_some() || current_dst.is_some(),
                "case transition lost both assets"
            );
        }
        self.record_history(&t)?;
        AssetFile::new(dst.clone(), MAX_CASE)
            .lock()?
            .write(&t.after)?;
        if src != dst && current_src.is_some() {
            ensure!(
                self.read(&src)?.as_deref() == Some(t.before.as_slice()),
                "source changed before cleanup"
            );
            std::fs::remove_file(&src)?;
            #[cfg(unix)]
            std::fs::File::open(src.parent().unwrap())?.sync_all()?;
        }
        guard.write(b"null")?;
        Ok(())
    }
    fn record_history(&self, t: &Transition) -> Result<()> {
        let mutation = match &t.history {
            Some(h) => h.clone(),
            None => {
                let h = case_history::load(&self.root, &t.id)?;
                if h.revisions.last().is_some_and(|r| {
                    r.digest == Some(format!("content:{}", STANDARD.encode(&t.after)))
                }) {
                    return Ok(());
                }
                case_history::prepare(
                    &self.root,
                    &t.id,
                    Some(&t.before),
                    Some(&t.after),
                    "legacy_recovered_transition",
                    AuditContext::default(),
                )?
            }
        };
        case_history::apply(&self.root, &t.id, &mutation)
    }
    pub(super) fn history(&self, id: &str) -> Result<CaseHistory> {
        identity(id)?;
        self.with_lock(|_| {
            let mut h = case_history::load(&self.root, id)?;
            if h.revisions.is_empty() {
                let (_, bytes) = self.locate(id)?;
                case_history::observe(&self.root, id, &bytes)?;
                h = case_history::load(&self.root, id)?;
            }
            Ok(h)
        })
    }
    pub(super) fn restore(
        &self,
        id: &str,
        revision: u64,
        expected: &str,
        context: AuditContext,
    ) -> Result<CaseAsset> {
        context.validate()?;
        self.with_lock(|guard| {
            let (status, before) = self.locate(id)?;
            ensure!(
                format!("content:{}", STANDARD.encode(&before)) == expected,
                "case content revision conflict"
            );
            let h = case_history::load(&self.root, id)?;
            let old = h
                .revisions
                .iter()
                .find(|r| r.revision == revision)
                .context("case revision not found")?;
            let mut value = old
                .snapshot
                .clone()
                .context("deleted revision cannot be restored")?;
            value["status"] = serde_json::to_value(CaseStatus::Draft)?;
            value
                .as_object_mut()
                .context("invalid snapshot")?
                .remove("provenance");
            let after = serde_json::to_vec_pretty(&value)?;
            let history = case_history::prepare(
                &self.root,
                id,
                Some(&before),
                Some(&after),
                "restore",
                context,
            )?;
            let t = Transition {
                history: Some(history),
                schema: 1,
                kind: Some(TransitionKind::Restore),
                id: id.into(),
                from: status,
                to: CaseStatus::Draft,
                before,
                after,
            };
            self.validate_transition(&t)?;
            guard.write(&serde_json::to_vec(&Some(t))?)?;
            self.recover(guard)?;
            let (status, b) = self.locate(id)?;
            self.view(id, status, &b)
        })
    }
    fn with_lock<T>(&self, work: impl FnOnce(&AssetGuard<'_>) -> Result<T>) -> Result<T> {
        let journal = AssetFile::new(self.root.join(".transition.json"), MAX_JOURNAL);
        let guard = journal.lock()?;
        self.recover(&guard)?;
        work(&guard)
    }
    fn locate(&self, id: &str) -> Result<(Option<CaseStatus>, Vec<u8>)> {
        let mut found = None;
        for status in STATUSES.into_iter().map(Some).chain(std::iter::once(None)) {
            if let Some(bytes) = self.read(&self.path(id, status)?)? {
                self.parse(&bytes, id, status)?;
                ensure!(
                    found.is_none(),
                    "duplicate case asset; manual recovery required"
                );
                found = Some((status, bytes));
            }
        }
        found.context("case asset not found")
    }
    pub(super) fn locate_path(&self, id: &str) -> Result<PathBuf> {
        self.with_lock(|_| {
            let (status, _) = self.locate(id)?;
            self.path(id, status)
        })
    }
    pub(super) fn list(&self) -> Result<Vec<TestCase>> {
        self.with_lock(|_| {
            let mut cases = Vec::new();
            let mut seen = std::collections::BTreeSet::new();
            for status in STATUSES.into_iter().map(Some).chain(std::iter::once(None)) {
                let dir = status
                    .map(|s| self.root.join(s.dir_name()))
                    .unwrap_or_else(|| self.root.clone());
                if !dir.exists() {
                    continue;
                }
                ensure!(
                    !std::fs::symlink_metadata(&dir)?.file_type().is_symlink(),
                    "unsafe case directory"
                );
                for entry in std::fs::read_dir(&dir)? {
                    let path = entry?.path();
                    if path.extension().and_then(|s| s.to_str()) != Some("json")
                        || path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .is_some_and(|s| s.starts_with('.'))
                    {
                        continue;
                    }
                    let id = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .context("invalid case filename")?;
                    identity(id)?;
                    ensure!(seen.insert(id.to_owned()), "duplicate case asset");
                    ensure!(seen.len() <= 20_000, "case listing limit exceeded");
                    let bytes = self.read(&path)?.context("case disappeared")?;
                    cases.push(self.parse(&bytes, id, status)?);
                }
            }
            Ok(cases)
        })
    }
    pub(super) fn save(&self, case: &TestCase) -> Result<()> {
        self.with_lock(|guard| {
            let mut before = None;
            let mut location = None;
            let _path = self.path(&case.id, Some(case.status))?;
            let mut value = serde_json::to_value(case)?;
            // Typed callers must not discard manual expected-result metadata.
            if let Ok((status, existing)) = self.locate(&case.id) {
                ensure!(
                    status == Some(case.status),
                    "case status must change through a transition"
                );
                before = Some(existing.clone());
                location = status;
                let previous: Value = serde_json::from_slice(&existing)?;
                if let Some(expected) = previous.get("expected_result") {
                    value["expected_result"] = expected.clone();
                }
            } else {
                // Distinguish absent identity from unreadable/duplicate/corrupt files.
                for status in STATUSES.into_iter().map(Some).chain(std::iter::once(None)) {
                    ensure!(
                        self.read(&self.path(&case.id, status)?)?.is_none(),
                        "existing case asset cannot be overwritten"
                    );
                }
            }
            let bytes = serde_json::to_vec_pretty(&value)?;
            self.parse(&bytes, &case.id, Some(case.status))?;
            if before.as_ref() == Some(&bytes) {
                return Ok(());
            }
            let history = case_history::prepare(
                &self.root,
                &case.id,
                before.as_deref(),
                Some(&bytes),
                "save",
                self.context.clone(),
            )?;
            let t = Transition {
                history: Some(history),
                schema: 1,
                kind: Some(TransitionKind::Save),
                id: case.id.clone(),
                from: location,
                to: case.status,
                before: before.unwrap_or_default(),
                after: bytes,
            };
            self.validate_transition(&t)?;
            guard.write(&serde_json::to_vec(&Some(t))?)?;
            self.recover(guard)
        })
    }
    fn transition(
        &self,
        guard: &AssetGuard<'_>,
        id: &str,
        from: Option<CaseStatus>,
        to: CaseStatus,
        before: Vec<u8>,
        after: Vec<u8>,
        kind: TransitionKind,
    ) -> Result<()> {
        if from != Some(to) {
            ensure!(
                self.read(&self.path(id, Some(to))?)?.is_none(),
                "transition destination already exists"
            );
        }
        let history = case_history::prepare(
            &self.root,
            id,
            Some(&before),
            Some(&after),
            if matches!(kind, TransitionKind::Status) {
                "status"
            } else {
                "edit"
            },
            self.context.clone(),
        )?;
        let pending = Transition {
            history: Some(history),
            schema: 1,
            kind: Some(kind),
            id: id.into(),
            from,
            to,
            before,
            after,
        };
        // Reject malformed operations before persisting a journal that blocks all reads.
        self.validate_transition(&pending)?;
        guard.write(&serde_json::to_vec(&Some(pending))?)?;
        self.recover(guard)
    }
    pub(super) fn delete(&self, id: &str) -> Result<()> {
        self.with_lock(|guard| {
            let (status, before) = self.locate(id)?;
            let parsed = self.parse(&before, id, status)?;
            let history = case_history::prepare(
                &self.root,
                id,
                Some(&before),
                None,
                "delete",
                self.context.clone(),
            )?;
            let t = Transition {
                history: Some(history),
                schema: 1,
                kind: Some(TransitionKind::Delete),
                id: id.into(),
                from: status,
                to: parsed.status,
                before,
                after: vec![],
            };
            guard.write(&serde_json::to_vec(&Some(t))?)?;
            self.recover(guard)
        })
    }
    pub(super) fn move_status(&self, id: &str, from: CaseStatus, to: CaseStatus) -> Result<()> {
        ensure!(from.can_transition_to(to), "invalid case status transition");
        self.with_lock(|guard| {
            let (location, bytes) = self.locate(id)?;
            let case = self.parse(&bytes, id, location)?;
            ensure!(case.status == from, "case status conflict");
            let mut value: Value = serde_json::from_slice(&bytes)?;
            value["status"] = serde_json::to_value(to)?;
            self.transition(
                guard,
                id,
                location,
                to,
                bytes,
                serde_json::to_vec_pretty(&value)?,
                TransitionKind::Status,
            )
        })
    }
    /// HTTP human review must refer to the exact asset that was inspected.
    pub(super) fn move_status_checked(
        &self,
        id: &str,
        to: CaseStatus,
        expected: Option<&str>,
    ) -> Result<()> {
        self.with_lock(|guard| {
            let (location, bytes) = self.locate(id)?;
            let case = self.parse(&bytes, id, location)?;
            ensure!(
                case.status.can_transition_to(to),
                "invalid case status transition"
            );
            if matches!(to, CaseStatus::Reviewed | CaseStatus::Approved) {
                ensure!(
                    expected.is_some(),
                    "inspected content revision required for review or approval"
                );
            }
            if let Some(expected) = expected {
                ensure!(
                    format!("content:{}", STANDARD.encode(&bytes)) == expected,
                    "case content revision conflict; inspect the current case again"
                );
            }
            let mut value: Value = serde_json::from_slice(&bytes)?;
            value["status"] = serde_json::to_value(to)?;
            self.transition(
                guard,
                id,
                location,
                to,
                bytes,
                serde_json::to_vec_pretty(&value)?,
                TransitionKind::Status,
            )
        })
    }
    pub(super) fn asset(&self, id: &str) -> Result<CaseAsset> {
        self.with_lock(|_| {
            let (status, bytes) = self.locate(id)?;
            self.view(id, status, &bytes)
        })
    }
    fn view(&self, id: &str, status: Option<CaseStatus>, bytes: &[u8]) -> Result<CaseAsset> {
        let mut case = self.parse(bytes, id, status)?;
        let value: Value = serde_json::from_slice(bytes)?;
        let mut history = case_history::load(&self.root, id)?;
        let revision = if history.revisions.is_empty() {
            let revision = case_history::observe(&self.root, id, bytes)?.revision;
            history = case_history::load(&self.root, id)?;
            revision
        } else {
            let last = history.revisions.last().unwrap();
            ensure!(
                last.digest == Some(format!("content:{}", STANDARD.encode(bytes))),
                "case history differs from asset"
            );
            last.revision
        };
        // Sidecars predate embedded provenance. Only an untouched legacy baseline
        // followed by known status-only transitions can still inherit that source.
        // An edit/restore/save (even of identical text) permanently ends eligibility.
        if case.provenance.is_none()
            && history
                .revisions
                .first()
                .is_some_and(|r| r.operation == "legacy_baseline")
            && history
                .revisions
                .iter()
                .skip(1)
                .all(|r| r.operation == "status")
        {
            case.provenance = crate::execution_records::CaseProvenanceStore::new(
                self.root
                    .parent()
                    .context("case root has no project directory")?
                    .join("provenance"),
            )
            .load(&self.project, &case.id, &case.yaml_script)?;
        }
        Ok(CaseAsset {
            revision,
            case_id: case.id,
            name: case.name,
            yaml_script: case.yaml_script,
            expected_result: value
                .get("expected_result")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            status: case.status,
            provenance: case.provenance,
            digest: format!("content:{}", STANDARD.encode(bytes)),
        })
    }
    pub(super) fn edit(
        &self,
        id: &str,
        expected: &str,
        name: &str,
        script: &str,
        expected_result: &str,
    ) -> Result<CaseAsset> {
        ensure!(
            !name.trim().is_empty()
                && name.len() <= 500
                && !script.trim().is_empty()
                && script.len() <= 64 * 1024
                && expected_result.len() <= 16 * 1024,
            "invalid case edit size or content"
        );
        validate_script(script)?;
        self.with_lock(|guard| {
            let (status, before) = self.locate(id)?;
            ensure!(
                format!("content:{}", STANDARD.encode(&before)) == expected,
                "case content revision conflict"
            );
            let mut value: Value = serde_json::from_slice(&before)?;
            value["name"] = Value::String(name.into());
            value["yaml_script"] = Value::String(script.into());
            value["expected_result"] = Value::String(expected_result.into());
            value["status"] = serde_json::to_value(CaseStatus::Draft)?;
            value
                .as_object_mut()
                .context("invalid case")?
                .remove("provenance");
            let after = serde_json::to_vec_pretty(&value)?;
            self.transition(
                guard,
                id,
                status,
                CaseStatus::Draft,
                before,
                after.clone(),
                TransitionKind::Edit,
            )?;
            self.view(id, Some(CaseStatus::Draft), &after)
        })
    }
}
fn validate_script(script: &str) -> Result<()> {
    fn steps(value: &serde_yaml::Value, count: &mut usize) -> Result<()> {
        let steps = value.as_sequence().context("flow must be an array")?;
        ensure!(
            !steps.is_empty() && steps.len() <= 1000,
            "flow requires 1..1000 actions"
        );
        for step in steps {
            let object = step.as_mapping().context("action must be an object")?;
            ensure!(object.len() == 1, "action requires exactly one operation");
            let (key, value) = object.iter().next().unwrap();
            ensure!(
                matches!(key.as_str(), Some("aiAct" | "aiAssert" | "aiQuery"))
                    && value.as_str().is_some_and(|s| !s.trim().is_empty()),
                "unsupported or empty action"
            );
            *count += 1;
            ensure!(*count <= 1000, "too many actions");
        }
        Ok(())
    }
    let value: serde_yaml::Value = serde_yaml::from_str(script).context("invalid YAML document")?;
    let mut count = 0;
    if value.is_sequence() {
        return steps(&value, &mut count);
    }
    let object = value
        .as_mapping()
        .context("expected tasks object or action array")?;
    ensure!(
        object
            .keys()
            .all(|k| matches!(k.as_str(), Some("target" | "tasks"))),
        "unknown YAML field"
    );
    if let Some(target) = value.get("target") {
        let mapping = target.as_mapping().context("invalid target")?;
        ensure!(
            mapping.len() == 1
                && target
                    .get("url")
                    .and_then(serde_yaml::Value::as_str)
                    .is_some_and(|s| !s.trim().is_empty()),
            "invalid target"
        );
        let url =
            reqwest::Url::parse(target["url"].as_str().unwrap()).context("invalid target URL")?;
        ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none(),
            "invalid target URL"
        );
    }
    let tasks = value
        .get("tasks")
        .and_then(serde_yaml::Value::as_sequence)
        .context("tasks must be an array")?;
    ensure!(
        !tasks.is_empty() && tasks.len() <= 200,
        "tasks requires 1..200 entries"
    );
    for task in tasks {
        let mapping = task.as_mapping().context("invalid task")?;
        ensure!(
            mapping
                .keys()
                .all(|k| matches!(k.as_str(), Some("name" | "flow")))
                && task
                    .get("name")
                    .and_then(serde_yaml::Value::as_str)
                    .is_some_and(|s| !s.trim().is_empty()),
            "invalid task"
        );
        steps(task.get("flow").context("missing flow")?, &mut count)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn human_review_requires_exact_inspected_revision_under_transition_lock() {
        let (_dir, assets, case) = fixture();
        assets.save(&case).unwrap();
        let draft = assets.asset(&case.id).unwrap();
        assert!(assets
            .move_status_checked(&case.id, CaseStatus::Reviewed, None)
            .is_err());
        assert!(assets
            .move_status_checked(&case.id, CaseStatus::Reviewed, Some("wrong"))
            .is_err());
        assets
            .move_status_checked(&case.id, CaseStatus::Reviewed, Some(&draft.digest))
            .unwrap();
        assert!(assets
            .move_status_checked(&case.id, CaseStatus::Approved, None)
            .is_err());
        assert!(assets
            .move_status_checked(&case.id, CaseStatus::Approved, Some(&draft.digest))
            .is_err());
        let reviewed = assets.asset(&case.id).unwrap();
        let edited = assets
            .edit(
                &case.id,
                &reviewed.digest,
                "edited",
                "- aiAssert: corrected",
                "corrected",
            )
            .unwrap();
        assets
            .move_status_checked(&case.id, CaseStatus::Reviewed, Some(&edited.digest))
            .unwrap();
        assert!(assets
            .move_status_checked(&case.id, CaseStatus::Approved, Some(&reviewed.digest))
            .is_err());
        let inspected = assets.asset(&case.id).unwrap();
        assets
            .move_status_checked(&case.id, CaseStatus::Approved, Some(&inspected.digest))
            .unwrap();
        assert_eq!(assets.asset(&case.id).unwrap().status, CaseStatus::Approved);
    }
    #[test]
    fn repeated_unchanged_manual_saves_reset_approved_and_archived_without_poisoning() {
        let (_dir, assets, case) = fixture();
        let mut asset = assets.asset(&case.id).unwrap();
        for _ in 0..3 {
            asset = assets
                .edit(
                    &case.id,
                    &asset.digest,
                    &asset.name,
                    &asset.yaml_script,
                    &asset.expected_result,
                )
                .unwrap();
            assert_eq!(assets.list().unwrap().len(), 1);
        }
        for final_status in [CaseStatus::Approved, CaseStatus::Archived] {
            assets
                .move_status(&case.id, CaseStatus::Draft, CaseStatus::Reviewed)
                .unwrap();
            assets
                .move_status(&case.id, CaseStatus::Reviewed, CaseStatus::Approved)
                .unwrap();
            if final_status == CaseStatus::Archived {
                assets
                    .move_status(&case.id, CaseStatus::Approved, final_status)
                    .unwrap();
            }
            asset = assets.asset(&case.id).unwrap();
            asset = assets
                .edit(
                    &case.id,
                    &asset.digest,
                    &asset.name,
                    &asset.yaml_script,
                    &asset.expected_result,
                )
                .unwrap();
            assert_eq!(asset.status, CaseStatus::Draft);
            assert_eq!(assets.list().unwrap().len(), 1);
        }
        assets
            .move_status_checked(&case.id, CaseStatus::Reviewed, Some(&asset.digest))
            .unwrap();
    }
    #[test]
    fn legacy_and_explicit_unchanged_edit_journals_recover_at_every_boundary() {
        for explicit in [false, true] {
            for status in [
                CaseStatus::Draft,
                CaseStatus::Approved,
                CaseStatus::Archived,
            ] {
                for boundary in 0..3 {
                    let (_dir, assets, mut case) = fixture();
                    if status != CaseStatus::Draft {
                        assets
                            .move_status(&case.id, CaseStatus::Draft, CaseStatus::Reviewed)
                            .unwrap();
                        assets
                            .move_status(&case.id, CaseStatus::Reviewed, CaseStatus::Approved)
                            .unwrap();
                        if status == CaseStatus::Archived {
                            assets
                                .move_status(&case.id, CaseStatus::Approved, status)
                                .unwrap();
                        }
                    }
                    case.status = status;
                    let mut t = pending(&assets, &case);
                    t.kind = explicit.then_some(TransitionKind::Edit);
                    t.to = CaseStatus::Draft;
                    let mut after: Value = serde_json::from_slice(&t.before).unwrap();
                    after["status"] = serde_json::to_value(t.to).unwrap();
                    t.after = serde_json::to_vec_pretty(&after).unwrap();
                    write_journal(&assets, &t);
                    let src = assets.path(&case.id, Some(status)).unwrap();
                    let dst = assets.path(&case.id, Some(CaseStatus::Draft)).unwrap();
                    if boundary >= 1 {
                        AssetFile::new(dst.clone(), MAX_CASE)
                            .lock()
                            .unwrap()
                            .write(&t.after)
                            .unwrap();
                    }
                    if boundary >= 2 && src != dst {
                        std::fs::remove_file(src).unwrap();
                    }
                    let listed = assets.list().unwrap();
                    assert_eq!(listed.len(), 1);
                    assert_eq!(listed[0].status, CaseStatus::Draft);
                    assert_eq!(std::fs::read(dst).unwrap(), t.after);
                    let asset = assets.asset(&case.id).unwrap();
                    assets
                        .move_status_checked(&case.id, CaseStatus::Reviewed, Some(&asset.digest))
                        .unwrap();
                    assert_eq!(assets.list().unwrap()[0].status, CaseStatus::Reviewed);
                }
            }
        }
    }
    #[test]
    fn explicit_edit_cannot_mutate_immutable_metadata_or_poison_journal_before_write() {
        for key in ["priority", "requirement_id"] {
            let (_dir, assets, case) = fixture();
            let mut t = pending(&assets, &case);
            t.kind = Some(TransitionKind::Edit);
            t.to = CaseStatus::Draft;
            let mut after: Value = serde_json::from_slice(&t.before).unwrap();
            after[key] = serde_json::json!("changed");
            t.after = serde_json::to_vec_pretty(&after).unwrap();
            assert!(assets
                .with_lock(|guard| assets.transition(
                    guard,
                    &t.id,
                    t.from,
                    t.to,
                    t.before.clone(),
                    t.after.clone(),
                    TransitionKind::Edit
                ))
                .is_err());
            assert_eq!(assets.list().unwrap().len(), 1);
            assert_eq!(
                assets
                    .read(&assets.path(&case.id, Some(case.status)).unwrap())
                    .unwrap()
                    .unwrap(),
                t.before
            );
            // A persisted malicious journal also fails closed without altering the source.
            write_journal(&assets, &t);
            assert!(assets.list().is_err());
            assert_eq!(
                std::fs::read(assets.path(&case.id, Some(case.status)).unwrap()).unwrap(),
                t.before
            );
        }
    }
    fn fixture() -> (tempfile::TempDir, CaseAssets, TestCase) {
        let dir = tempfile::tempdir().unwrap();
        let assets = CaseAssets::new(dir.path().join("cases"), "project").unwrap();
        let case = TestCase {
            id: "receipt".into(),
            name: "Receipt".into(),
            requirement_id: "r".into(),
            test_type: "smoke".into(),
            yaml_script: "- aiAssert: receipt".into(),
            priority: "P1".into(),
            created_at: "2026-11-17".into(),
            status: CaseStatus::Draft,
            provenance: None,
        };
        assets.save(&case).unwrap();
        (dir, assets, case)
    }
    fn pending(assets: &CaseAssets, case: &TestCase) -> Transition {
        let before = assets
            .read(&assets.path(&case.id, Some(case.status)).unwrap())
            .unwrap()
            .unwrap();
        let mut after: Value = serde_json::from_slice(&before).unwrap();
        after["status"] = serde_json::to_value(CaseStatus::Reviewed).unwrap();
        Transition {
            history: None,
            schema: 1,
            kind: None,
            id: case.id.clone(),
            from: Some(case.status),
            to: CaseStatus::Reviewed,
            before,
            after: serde_json::to_vec_pretty(&after).unwrap(),
        }
    }
    fn write_journal(assets: &CaseAssets, t: &Transition) {
        let journal = AssetFile::new(assets.root.join(".transition.json"), MAX_JOURNAL);
        journal
            .lock()
            .unwrap()
            .write(&serde_json::to_vec(&Some(t)).unwrap())
            .unwrap();
    }
    #[test]
    fn interrupted_transition_recovers_at_each_durable_boundary() {
        for boundary in 0..3 {
            let (_dir, assets, case) = fixture();
            let t = pending(&assets, &case);
            write_journal(&assets, &t);
            if boundary >= 1 {
                AssetFile::new(
                    assets.path(&case.id, Some(CaseStatus::Reviewed)).unwrap(),
                    MAX_CASE,
                )
                .lock()
                .unwrap()
                .write(&t.after)
                .unwrap();
            }
            if boundary >= 2 {
                std::fs::remove_file(assets.path(&case.id, Some(CaseStatus::Draft)).unwrap())
                    .unwrap();
            }
            let listed = assets.list().unwrap();
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].status, CaseStatus::Reviewed);
            assert!(!assets
                .path(&case.id, Some(CaseStatus::Draft))
                .unwrap()
                .exists());
            assert_eq!(assets.list().unwrap().len(), 1);
            assert_eq!(
                std::fs::read(assets.path(&case.id, Some(CaseStatus::Reviewed)).unwrap()).unwrap(),
                t.after
            );
        }
    }
    #[test]
    fn conflicting_recovery_retains_all_bytes_and_rejects_bad_journal_identity() {
        let (_dir, assets, case) = fixture();
        let mut t = pending(&assets, &case);
        write_journal(&assets, &t);
        let target = assets.path(&case.id, Some(CaseStatus::Reviewed)).unwrap();
        AssetFile::new(target.clone(), MAX_CASE)
            .lock()
            .unwrap()
            .write(b"external conflict")
            .unwrap();
        assert!(assets.list().is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"external conflict");
        assert_eq!(
            std::fs::read(assets.path(&case.id, Some(CaseStatus::Draft)).unwrap()).unwrap(),
            t.before
        );
        t.id = "../escape".into();
        write_journal(&assets, &t);
        assert!(assets.list().is_err());
    }
    #[test]
    fn human_edit_cas_resets_review_and_preserves_expected_result_through_moves_and_typed_save() {
        let (_dir, assets, mut case) = fixture();
        case.provenance = Some(crate::execution_records::CaseProvenance {
            specification: None,
            schema_version: 1,
            project: "project".into(),
            case_id: case.id.clone(),
            yaml_script: case.yaml_script.clone(),
            created_at: case.created_at.clone(),
            contracts: None,
            model: serde_json::json!({"source":"legacy_config","model":"fixture"}),
            model_calls: None,
        });
        assets.save(&case).unwrap();
        assets
            .move_status(&case.id, CaseStatus::Draft, CaseStatus::Reviewed)
            .unwrap();
        assets
            .move_status(&case.id, CaseStatus::Reviewed, CaseStatus::Approved)
            .unwrap();
        let before = assets.asset(&case.id).unwrap();
        let edited = assets
            .edit(
                &case.id,
                &before.digest,
                "Changed",
                "- aiAssert: new assertion",
                "new assertion",
            )
            .unwrap();
        assert_eq!(edited.status, CaseStatus::Draft);
        assert!(edited.provenance.is_none());
        assert_eq!(edited.expected_result, "new assertion");
        assert!(assets
            .edit(
                &case.id,
                &before.digest,
                "Stale",
                "- aiAssert: stale",
                "stale"
            )
            .is_err());
        assets
            .move_status(&case.id, CaseStatus::Draft, CaseStatus::Reviewed)
            .unwrap();
        case = assets.list().unwrap().pop().unwrap();
        assets.save(&case).unwrap();
        assert_eq!(
            assets.asset(&case.id).unwrap().expected_result,
            "new assertion"
        );
        assert!(assets
            .edit(&case.id, &edited.digest, "Bad", "tasks: []", "")
            .is_err());
    }
    #[test]
    fn flat_legacy_cases_move_without_duplicates_and_bad_assets_are_not_overwritten() {
        let (_dir, assets, case) = fixture();
        std::fs::rename(
            assets.path(&case.id, Some(case.status)).unwrap(),
            assets.path(&case.id, None).unwrap(),
        )
        .unwrap();
        assert_eq!(assets.list().unwrap().len(), 1);
        assets
            .move_status(&case.id, CaseStatus::Draft, CaseStatus::Reviewed)
            .unwrap();
        assert_eq!(assets.list().unwrap().len(), 1);
        let path = assets.path(&case.id, Some(CaseStatus::Reviewed)).unwrap();
        std::fs::write(&path, b"broken").unwrap();
        assert!(assets.save(&case).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"broken");
    }
    #[cfg(unix)]
    #[test]
    fn symlink_leaf_and_status_directory_are_rejected() {
        let (_dir, assets, case) = fixture();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("victim"), b"unchanged").unwrap();
        let path = assets.path(&case.id, Some(case.status)).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(outside.path().join("victim"), &path).unwrap();
        assert!(assets.asset(&case.id).is_err());
        assert!(assets.save(&case).is_err());
        assert_eq!(
            std::fs::read(outside.path().join("victim")).unwrap(),
            b"unchanged"
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir_all(assets.root.join("draft")).unwrap();
        std::os::unix::fs::symlink(outside.path(), assets.root.join("draft")).unwrap();
        assert!(assets.list().is_err());
    }
    #[test]
    fn editor_accepts_supported_shapes_and_rejects_unknown_or_empty_actions() {
        for yaml in ["- aiAssert: receipt", "target:\n  url: https://example.test\ntasks:\n  - name: verify\n    flow:\n      - aiAssert: receipt"] {validate_script(yaml).unwrap();}
        for yaml in [
            "tasks: []",
            "- shell: rm",
            "- aiAssert: ''",
            "- aiAssert: x\n  aiAct: y",
            "target:\n url: file:///tmp/a\ntasks: []",
        ] {
            assert!(validate_script(yaml).is_err());
        }
    }
    #[test]
    fn full_history_records_cas_review_restore_and_tombstone() {
        let (_dir, assets, case) = fixture();
        assert_eq!(assets.asset(&case.id).unwrap().revision, 1);
        assets
            .move_status(&case.id, CaseStatus::Draft, CaseStatus::Reviewed)
            .unwrap();
        assets
            .move_status(&case.id, CaseStatus::Reviewed, CaseStatus::Approved)
            .unwrap();
        let approved = assets.asset(&case.id).unwrap();
        let edited = assets
            .edit(
                &case.id,
                &approved.digest,
                "changed",
                "- aiAssert: changed",
                "changed",
            )
            .unwrap();
        assert_eq!(edited.revision, 4);
        let count = assets.history(&case.id).unwrap().revisions.len();
        assert!(assets
            .edit(&case.id, &approved.digest, "bad", "- aiAssert: bad", "")
            .is_err());
        assert_eq!(assets.history(&case.id).unwrap().revisions.len(), count);
        let context = AuditContext {
            source: "local_web".into(),
            author: Some("local-owner".into()),
            reason: "Restore historical expectations".into(),
        };
        let restored = assets
            .restore(&case.id, 3, &edited.digest, context.clone())
            .unwrap();
        assert_eq!(restored.status, CaseStatus::Draft);
        assert!(restored.provenance.is_none());
        assert_eq!(restored.yaml_script, case.yaml_script);
        let h = assets.history(&case.id).unwrap();
        assert!(h.history_complete);
        assert_eq!(h.revisions.last().unwrap().context, context);
        assert_eq!(h.revisions.last().unwrap().parent_revision, Some(4));
        assets.delete(&case.id).unwrap();
        assert!(assets
            .history(&case.id)
            .unwrap()
            .revisions
            .last()
            .unwrap()
            .snapshot
            .is_none());
    }
    #[test]
    fn history_recovery_is_idempotent_at_all_commit_boundaries() {
        for boundary in 0..4 {
            let (_dir, assets, case) = fixture();
            let mut t = pending(&assets, &case);
            t.kind = Some(TransitionKind::Status);
            t.history = Some(
                case_history::prepare(
                    &assets.root,
                    &case.id,
                    Some(&t.before),
                    Some(&t.after),
                    "status",
                    AuditContext::default(),
                )
                .unwrap(),
            );
            write_journal(&assets, &t);
            if boundary >= 1 {
                assets.record_history(&t).unwrap();
            }
            if boundary >= 2 {
                AssetFile::new(assets.path(&case.id, Some(t.to)).unwrap(), MAX_CASE)
                    .lock()
                    .unwrap()
                    .write(&t.after)
                    .unwrap();
            }
            if boundary >= 3 {
                std::fs::remove_file(assets.path(&case.id, t.from).unwrap()).unwrap();
            }
            assert_eq!(assets.asset(&case.id).unwrap().revision, 2);
            assert_eq!(assets.history(&case.id).unwrap().revisions.len(), 2);
            assert_eq!(assets.history(&case.id).unwrap().revisions.len(), 2);
        }
    }
    #[test]
    fn legacy_current_snapshot_is_marked_unknown_and_history_tampering_blocks() {
        let (_dir, assets, case) = fixture();
        std::fs::remove_dir_all(assets.root.join(".history")).unwrap();
        let h = assets.history(&case.id).unwrap();
        assert!(!h.history_complete);
        assert_eq!(
            h.revisions.len(),
            1,
            "observing legacy content must not fabricate a second revision"
        );
        assert!(h.revisions[0].created_at.is_none());
        assert_eq!(h.revisions[0].context.author, None);
        let path = assets
            .root
            .join(".history")
            .join(format!("{}.json", case.id));
        let mut value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["revisions"][0]["snapshot"]["name"] = serde_json::json!("forged");
        std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(assets.asset(&case.id).is_err());
    }
    #[test]
    fn new_case_and_delete_journals_keep_history_across_partial_commit() {
        for deleting in [false, true] {
            let (_dir, assets, case) = fixture();
            let (status, before) = assets.locate(&case.id).unwrap();
            let (id, kind, after, before) = if deleting {
                (case.id.clone(), TransitionKind::Delete, vec![], before)
            } else {
                let mut value: Value = serde_json::from_slice(&before).unwrap();
                value["id"] = serde_json::json!("new-case");
                (
                    "new-case".into(),
                    TransitionKind::Save,
                    serde_json::to_vec_pretty(&value).unwrap(),
                    vec![],
                )
            };
            let history = case_history::prepare(
                &assets.root,
                &id,
                if before.is_empty() {
                    None
                } else {
                    Some(&before)
                },
                if after.is_empty() { None } else { Some(&after) },
                if deleting { "delete" } else { "save" },
                AuditContext::default(),
            )
            .unwrap();
            let t = Transition {
                schema: 1,
                history: Some(history),
                kind: Some(kind),
                id: id.clone(),
                from: if deleting { status } else { None },
                to: case.status,
                before,
                after,
            };
            write_journal(&assets, &t);
            assets.record_history(&t).unwrap();
            let h = assets.history(&id).unwrap();
            assert_eq!(h.revisions.len(), if deleting { 2 } else { 1 });
            assert_eq!(h.revisions.last().unwrap().snapshot.is_none(), deleting);
            if deleting {
                assert!(assets.locate(&id).is_err());
            } else {
                assert_eq!(assets.asset(&id).unwrap().revision, 1);
            }
        }
    }
    #[test]
    fn malformed_zero_history_journal_fails_without_mutating_assets() {
        for corrupt_baseline in [false, true] {
            let (_dir, assets, case) = fixture();
            let mut t = pending(&assets, &case);
            if corrupt_baseline {
                std::fs::remove_dir_all(assets.root.join(".history")).unwrap();
            }
            let mut history = case_history::prepare(
                &assets.root,
                &case.id,
                Some(&t.before),
                Some(&t.after),
                "status",
                AuditContext::default(),
            )
            .unwrap();
            if corrupt_baseline {
                history.baseline.as_mut().unwrap().revision = 0;
            } else {
                history.entry.revision = 0;
            }
            t.history = Some(history);
            write_journal(&assets, &t);
            let journal = std::fs::read(assets.root.join(".transition.json")).unwrap();
            let history_path = assets
                .root
                .join(".history")
                .join(format!("{}.json", case.id));
            let previous_history = std::fs::read(&history_path).ok();
            assert!(assets.asset(&case.id).is_err());
            assert_eq!(
                std::fs::read(assets.path(&case.id, Some(case.status)).unwrap()).unwrap(),
                t.before
            );
            assert_eq!(
                std::fs::read(assets.root.join(".transition.json")).unwrap(),
                journal
            );
            assert_eq!(std::fs::read(&history_path).ok(), previous_history);
        }
    }
    #[test]
    fn legacy_sidecar_survives_status_only_but_never_edit_or_restore() {
        for restore in [false, true] {
            let (_dir, assets, case) = fixture();
            std::fs::remove_dir_all(assets.root.join(".history")).unwrap();
            let source = crate::execution_records::CaseProvenance {
                specification: None,
                schema_version: 1,
                project: "project".into(),
                case_id: case.id.clone(),
                yaml_script: case.yaml_script.clone(),
                created_at: case.created_at.clone(),
                contracts: None,
                model: serde_json::json!({"source":"legacy_config","model":"fixture"}),
                model_calls: None,
            };
            crate::execution_records::CaseProvenanceStore::new(
                assets.root.parent().unwrap().join("provenance"),
            )
            .save(&source)
            .unwrap();
            let original = assets.asset(&case.id).unwrap();
            assert!(original.provenance.is_some());
            assert!(assets.history(&case.id).unwrap().revisions[0]
                .snapshot
                .as_ref()
                .unwrap()
                .get("provenance")
                .is_none_or(Value::is_null));
            assets
                .move_status(&case.id, CaseStatus::Draft, CaseStatus::Reviewed)
                .unwrap();
            assets
                .move_status(&case.id, CaseStatus::Reviewed, CaseStatus::Approved)
                .unwrap();
            let approved = assets.asset(&case.id).unwrap();
            assert!(approved.provenance.is_some());
            let changed = if restore {
                assets
                    .restore(&case.id, 1, &approved.digest, AuditContext::default())
                    .unwrap()
            } else {
                assets
                    .edit(
                        &case.id,
                        &approved.digest,
                        &approved.name,
                        &approved.yaml_script,
                        &approved.expected_result,
                    )
                    .unwrap()
            };
            assert_eq!(changed.yaml_script, original.yaml_script);
            assert!(changed.provenance.is_none());
            assets
                .move_status(&case.id, CaseStatus::Draft, CaseStatus::Reviewed)
                .unwrap();
            assets
                .move_status(&case.id, CaseStatus::Reviewed, CaseStatus::Approved)
                .unwrap();
            assert!(assets.asset(&case.id).unwrap().provenance.is_none());
        }
    }
}
