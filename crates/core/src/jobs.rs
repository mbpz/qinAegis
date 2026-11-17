//! Bounded task history. Recovery marks unfinished work interrupted; it never replays it.
use crate::asset_file::AssetFile;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

const MAX_RECORDS: usize = 100;
const MAX_OUTPUT: usize = 64 * 1024;
const MAX_VALUE: usize = 32 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobRecord {
    pub id: String,
    pub job_type: String,
    pub status: String,
    pub output: String,
    pub output_start: u64,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub snapshot: Option<Value>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: u32,
    records: Vec<JobRecord>,
}

pub struct JobStore {
    file: AssetFile,
}

fn active(status: &str) -> bool {
    matches!(status, "running" | "cancelling")
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}
fn validate(record: &JobRecord) -> Result<()> {
    ensure!(valid_id(&record.id), "invalid task identity");
    ensure!(valid_id(&record.job_type), "invalid task type");
    ensure!(
        matches!(
            record.status.as_str(),
            "running" | "cancelling" | "completed" | "failed" | "stopped" | "interrupted"
        ),
        "invalid task state"
    );
    ensure!(
        record.updated_at >= record.created_at,
        "invalid task timestamps"
    );
    Ok(())
}
fn bounded(record: &mut JobRecord) -> Result<()> {
    validate(record)?;
    if record.output.len() > MAX_OUTPUT {
        let mut start = record.output.len() - MAX_OUTPUT;
        while !record.output.is_char_boundary(start) {
            start += 1;
        }
        record.output_start = record
            .output_start
            .checked_add(start as u64)
            .context("task log offset exhausted")?;
        record.output = record.output[start..].to_owned();
    }
    for value in [&mut record.result, &mut record.snapshot] {
        if let Some(item) = value {
            let bytes = serde_json::to_vec(item)?;
            if bytes.len() > MAX_VALUE {
                *item = serde_json::json!({"truncated":true,"original_bytes":bytes.len(),"reason":"task history field exceeds limit; consult immutable run evidence"});
            }
        }
    }
    if let Some(error) = &mut record.error {
        if error.len() > MAX_OUTPUT {
            let mut end = MAX_OUTPUT;
            while !error.is_char_boundary(end) {
                end -= 1;
            }
            error.truncate(end);
            error.push_str(" [truncated]");
        }
    }
    Ok(())
}

impl JobStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            file: AssetFile::new(path.into(), 16 * 1024 * 1024),
        }
    }

    fn decode(bytes: Option<Vec<u8>>) -> Result<Document> {
        let Some(bytes) = bytes else {
            return Ok(Document {
                schema_version: 1,
                records: Vec::new(),
            });
        };
        let doc: Document = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("invalid task history document"))?;
        ensure!(
            doc.schema_version == 1 && doc.records.len() <= MAX_RECORDS,
            "unsupported or oversized task history"
        );
        let mut ids = std::collections::BTreeSet::new();
        for record in &doc.records {
            validate(record)?;
            ensure!(ids.insert(&record.id), "duplicate task history identity");
        }
        Ok(doc)
    }

    /// Call once during service startup while holding the single-instance lock.
    pub fn load_and_recover(&self) -> Result<Vec<JobRecord>> {
        let guard = self.file.lock()?;
        let mut doc = Self::decode(guard.read()?)?;
        let mut changed = false;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        for record in &mut doc.records {
            if active(&record.status) {
                record.status = "interrupted".into();
                record.error =
                    Some("service exited before worker completion; task was not replayed".into());
                record.updated_at = now.max(record.updated_at);
                changed = true;
            }
            bounded(record)?;
        }
        if changed {
            guard.write(&serde_json::to_vec(&doc)?)?;
        }
        Ok(doc.records)
    }

    /// Terminal records are immutable. Repeating the identical checkpoint is idempotent.
    pub fn checkpoint(&self, mut record: JobRecord) -> Result<()> {
        bounded(&mut record)?;
        let guard = self.file.lock()?;
        let mut doc = Self::decode(guard.read()?)?;
        if let Some(previous) = doc.records.iter_mut().find(|old| old.id == record.id) {
            ensure!(
                previous.job_type == record.job_type && previous.created_at == record.created_at,
                "task identity conflict"
            );
            if !active(&previous.status) {
                ensure!(
                    serde_json::to_value(&*previous)? == serde_json::to_value(&record)?,
                    "terminal task is immutable"
                );
                return Ok(());
            }
            ensure!(
                record.updated_at >= previous.updated_at,
                "stale task checkpoint"
            );
            ensure!(
                previous.status != "cancelling" || record.status != "running",
                "task cancellation cannot regress"
            );
            *previous = record;
        } else {
            if doc.records.len() == MAX_RECORDS {
                let candidate = doc
                    .records
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| !active(&r.status))
                    .min_by_key(|(_, r)| r.updated_at)
                    .map(|(i, _)| i)
                    .context("task history full of active work")?;
                doc.records.remove(candidate);
            }
            doc.records.push(record);
        }
        guard.write(&serde_json::to_vec(&doc)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(id: &str, status: &str) -> JobRecord {
        JobRecord {
            id: id.into(),
            job_type: "generate".into(),
            status: status.into(),
            output: String::new(),
            output_start: 0,
            result: None,
            error: None,
            snapshot: None,
            created_at: 1,
            updated_at: 2,
        }
    }
    #[test]
    fn restart_recovers_once_without_replay_and_terminal_is_immutable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let store = JobStore::new(&path);
        store.checkpoint(record("one", "running")).unwrap();
        let recovered = store.load_and_recover().unwrap();
        assert_eq!(recovered[0].status, "interrupted");
        let before = std::fs::read(&path).unwrap();
        assert_eq!(store.load_and_recover().unwrap()[0].status, "interrupted");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(store.checkpoint(record("one", "completed")).is_err());
        store.checkpoint(recovered[0].clone()).unwrap();
    }
    #[test]
    fn utf8_output_and_large_values_have_explicit_limits() {
        let dir = tempfile::tempdir().unwrap();
        let store = JobStore::new(dir.path().join("history.json"));
        let mut r = record("one", "completed");
        r.output = "中".repeat(40000);
        r.snapshot = Some(serde_json::json!({"input":"x".repeat(40000)}));
        store.checkpoint(r).unwrap();
        let loaded = store.load_and_recover().unwrap();
        assert!(loaded[0].output.len() <= MAX_OUTPUT);
        assert_eq!(
            loaded[0].output_start as usize + loaded[0].output.len(),
            120000
        );
        assert_eq!(loaded[0].snapshot.as_ref().unwrap()["truncated"], true);
    }
    #[test]
    fn retention_never_evicts_active_work() {
        let dir = tempfile::tempdir().unwrap();
        let store = JobStore::new(dir.path().join("history.json"));
        store.checkpoint(record("active", "running")).unwrap();
        for i in 0..105 {
            let mut r = record(&format!("job-{i}"), "completed");
            r.updated_at = i + 3;
            store.checkpoint(r).unwrap();
        }
        let loaded = store.load_and_recover().unwrap();
        assert_eq!(loaded.len(), 100);
        assert!(loaded.iter().any(|r| r.id == "active"));
        assert!(!loaded.iter().any(|r| r.id == "job-0"));
    }
    #[test]
    fn rejects_invalid_identity_and_cancel_regression() {
        let dir = tempfile::tempdir().unwrap();
        let store = JobStore::new(dir.path().join("history.json"));
        assert!(store.checkpoint(record("../outside", "running")).is_err());
        store.checkpoint(record("one", "cancelling")).unwrap();
        assert!(store.checkpoint(record("one", "running")).is_err());
    }
    #[test]
    fn full_active_history_and_stale_checkpoint_fail_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let store = JobStore::new(&path);
        for i in 0..100 {
            store
                .checkpoint(record(&format!("job-{i}"), "running"))
                .unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        assert!(store.checkpoint(record("overflow", "running")).is_err());
        let mut old = record("job-0", "running");
        old.updated_at = 1;
        assert!(store.checkpoint(old).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn corrupt_history_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        std::fs::write(&path, "invalid secret source").unwrap();
        let store = JobStore::new(&path);
        assert!(store.load_and_recover().is_err());
        assert!(store.checkpoint(record("one", "running")).is_err());
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "invalid secret source"
        );
    }
    #[cfg(unix)]
    #[test]
    fn private_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        JobStore::new(&path)
            .checkpoint(record("one", "completed"))
            .unwrap();
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
