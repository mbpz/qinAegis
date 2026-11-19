//! Immutable complete snapshots. Only the case transaction lock may mutate these files.
use crate::asset_file::AssetFile;
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditContext {
    pub source: String,
    pub author: Option<String>,
    pub reason: String,
}
impl Default for AuditContext {
    fn default() -> Self {
        Self {
            source: "unknown".into(),
            author: None,
            reason: "not recorded by caller".into(),
        }
    }
}
impl AuditContext {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.source.trim().is_empty()
                && self.source.len() <= 128
                && !self.reason.trim().is_empty()
                && self.reason.len() <= 8192
                && self
                    .author
                    .as_ref()
                    .is_none_or(|a| !a.trim().is_empty() && a.len() <= 256),
            "invalid audit context"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseRevision {
    pub created_at: Option<String>,
    pub revision: u64,
    pub parent_revision: Option<u64>,
    pub snapshot: Option<Value>,
    pub digest: Option<String>,
    pub operation: String,
    pub context: AuditContext,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseHistory {
    pub schema_version: u32,
    pub case_id: String,
    pub history_complete: bool,
    pub revisions: Vec<CaseRevision>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HistoryMutation {
    pub before_digest: Option<String>,
    pub baseline: Option<CaseRevision>,
    pub entry: CaseRevision,
}
fn revision(
    n: u64,
    bytes: Option<&[u8]>,
    operation: &str,
    context: AuditContext,
) -> Result<CaseRevision> {
    Ok(CaseRevision {
        created_at: if operation == "legacy_baseline" {
            None
        } else {
            Some(chrono::Utc::now().to_rfc3339())
        },
        revision: n,
        parent_revision: if n == 1 { None } else { Some(n - 1) },
        snapshot: bytes.map(serde_json::from_slice).transpose()?,
        digest: bytes.map(|b| format!("content:{}", STANDARD.encode(b))),
        operation: operation.into(),
        context,
    })
}
impl CaseHistory {
    pub(super) fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 1, "invalid history schema");
        for (i, r) in self.revisions.iter().enumerate() {
            ensure!(
                r.revision == i as u64 + 1
                    && r.parent_revision == if i == 0 { None } else { Some(i as u64) },
                "broken history chain"
            );
            r.context.validate()?;
            if let Some(time) = &r.created_at {
                chrono::DateTime::parse_from_rfc3339(time).context("invalid history timestamp")?;
            } else {
                ensure!(
                    r.operation == "legacy_baseline",
                    "missing recorded timestamp"
                );
            }
            ensure!(
                matches!(
                    r.operation.as_str(),
                    "save"
                        | "edit"
                        | "status"
                        | "delete"
                        | "restore"
                        | "legacy_baseline"
                        | "legacy_observed"
                        | "legacy_recovered_transition"
                ),
                "invalid history operation"
            );
            match (&r.snapshot, &r.digest) {
                (Some(v), Some(d)) => {
                    let b = STANDARD.decode(
                        d.strip_prefix("content:")
                            .context("invalid history digest")?,
                    )?;
                    ensure!(
                        b.len() <= 128 * 1024
                            && serde_json::from_slice::<Value>(&b)? == *v
                            && v["id"].as_str() == Some(&self.case_id),
                        "history snapshot mismatch"
                    );
                }
                (None, None) => {
                    ensure!(r.operation == "delete", "only deletion can omit snapshot");
                }
                _ => anyhow::bail!("invalid history snapshot"),
            }
        }
        Ok(())
    }
}
fn file(root: &Path, id: &str) -> AssetFile {
    AssetFile::new(
        root.join(".history").join(format!("{id}.json")),
        16 * 1024 * 1024,
    )
}
pub(super) fn load(root: &Path, id: &str) -> Result<CaseHistory> {
    let h: CaseHistory = match file(root, id).lock()?.read()? {
        Some(b) => serde_json::from_slice(&b)?,
        None => CaseHistory {
            schema_version: 1,
            case_id: id.into(),
            history_complete: false,
            revisions: vec![],
        },
    };
    ensure!(h.case_id == id, "history identity mismatch");
    h.validate()?;
    Ok(h)
}
pub(super) fn observe(root: &Path, id: &str, bytes: &[u8]) -> Result<CaseRevision> {
    let h = load(root, id)?;
    ensure!(h.revisions.is_empty(), "baseline already exists");
    let entry = revision(1, Some(bytes), "legacy_baseline", AuditContext::default())?;
    apply(
        root,
        id,
        &HistoryMutation {
            before_digest: None,
            baseline: None,
            entry: entry.clone(),
        },
    )?;
    Ok(entry)
}
pub(super) fn prepare(
    root: &Path,
    id: &str,
    before: Option<&[u8]>,
    after: Option<&[u8]>,
    operation: &str,
    context: AuditContext,
) -> Result<HistoryMutation> {
    context.validate()?;
    let h = load(root, id)?;
    let baseline = if h.revisions.is_empty() {
        before
            .map(|b| revision(1, Some(b), "legacy_baseline", AuditContext::default()))
            .transpose()?
    } else {
        ensure!(
            h.revisions.last().unwrap().digest
                == before.map(|b| format!("content:{}", STANDARD.encode(b))),
            "current case differs from history"
        );
        None
    };
    let n = h.revisions.len() as u64 + u64::from(baseline.is_some()) + 1;
    let m = HistoryMutation {
        before_digest: before.map(|b| format!("content:{}", STANDARD.encode(b))),
        baseline,
        entry: revision(n, after, operation, context)?,
    };
    let mut preview = h;
    for r in m.baseline.iter().chain(std::iter::once(&m.entry)) {
        preview.revisions.push(r.clone());
    }
    preview.validate()?;
    ensure!(
        serde_json::to_vec(&preview)?.len() <= 16 * 1024 * 1024,
        "case history capacity exhausted"
    );
    Ok(m)
}
pub(super) fn apply(root: &Path, id: &str, m: &HistoryMutation) -> Result<()> {
    // Validate untrusted journal indices before subtraction or indexing. Recovery must
    // retain a malformed intent rather than panic or partially commit it.
    for r in m.baseline.iter().chain(std::iter::once(&m.entry)) {
        ensure!(
            r.revision > 0
                && r.parent_revision
                    == if r.revision == 1 {
                        None
                    } else {
                        Some(r.revision - 1)
                    },
            "invalid pending history revision"
        );
    }
    if let Some(baseline) = &m.baseline {
        ensure!(
            baseline.revision == 1 && m.entry.revision == 2,
            "invalid pending baseline"
        );
    }
    let mut h = load(root, id)?;
    ensure!(
        h.revisions.len() as u64 <= m.entry.revision,
        "history is ahead of pending transaction"
    );
    for r in m.baseline.iter().chain(std::iter::once(&m.entry)) {
        if let Some(existing) = h.revisions.get((r.revision - 1) as usize) {
            ensure!(existing == r, "conflicting history recovery");
        } else {
            ensure!(
                r.revision == h.revisions.len() as u64 + 1,
                "history revision gap"
            );
            if h.revisions.is_empty() {
                h.history_complete = r.operation != "legacy_baseline";
            }
            h.revisions.push(r.clone());
        }
    }
    let parent = m
        .entry
        .parent_revision
        .and_then(|r| h.revisions.get((r - 1) as usize))
        .and_then(|r| r.digest.clone());
    ensure!(
        parent == m.before_digest,
        "history mutation before snapshot differs"
    );
    h.validate()?;
    file(root, id).lock()?.write(&serde_json::to_vec(&h)?)?;
    Ok(())
}
