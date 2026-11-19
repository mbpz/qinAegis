//! Versioned opt-in diagnostics and bounded owned-file evidence transport.
use crate::asset_file::AssetFile;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
pub const KINDS: [&str; 5] = ["screenshot", "trace", "console", "network", "steps"];
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapturePolicy {
    pub schema_version: u32,
    pub mode: String,
    pub max_artifact_bytes: u64,
    pub max_run_bytes: u64,
    pub max_events: u32,
    pub include_sensitive: bool,
}
impl Default for CapturePolicy {
    fn default() -> Self {
        Self {
            schema_version: 1,
            mode: "off".into(),
            max_artifact_bytes: 2 * 1024 * 1024,
            max_run_bytes: 8 * 1024 * 1024,
            max_events: 500,
            include_sensitive: false,
        }
    }
}
impl CapturePolicy {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1
                && matches!(
                    self.mode.as_str(),
                    "off" | "failure_only" | "key_steps" | "full"
                ),
            "invalid capture policy"
        );
        ensure!(
            (1024..=8388608).contains(&self.max_artifact_bytes)
                && (self.max_artifact_bytes..=33554432).contains(&self.max_run_bytes)
                && (1..=2000).contains(&self.max_events),
            "invalid capture quota"
        );
        ensure!(
            self.mode == "off" || self.include_sensitive,
            "sensitive capture requires explicit authorization"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapturePolicySnapshot {
    pub revision: u64,
    pub policy: CapturePolicy,
}
pub struct CapturePolicyStore {
    file: AssetFile,
}
impl CapturePolicyStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            file: AssetFile::new(root.join("capture/policy.json"), 8192),
        }
    }
    pub fn snapshot(&self) -> Result<CapturePolicySnapshot> {
        let g = self.file.lock()?;
        let s: CapturePolicySnapshot = match g.read()? {
            Some(b) => serde_json::from_slice(&b)?,
            None => CapturePolicySnapshot {
                revision: 0,
                policy: CapturePolicy::default(),
            },
        };
        s.policy.validate()?;
        Ok(s)
    }
    pub fn save(&self, expected: u64, policy: CapturePolicy) -> Result<CapturePolicySnapshot> {
        policy.validate()?;
        let g = self.file.lock()?;
        let old: CapturePolicySnapshot = match g.read()? {
            Some(b) => serde_json::from_slice(&b)?,
            None => CapturePolicySnapshot {
                revision: 0,
                policy: CapturePolicy::default(),
            },
        };
        old.policy.validate()?;
        ensure!(old.revision == expected, "capture policy revision conflict");
        let s = CapturePolicySnapshot {
            revision: expected
                .checked_add(1)
                .context("capture revision exhausted")?,
            policy,
        };
        g.write(&serde_json::to_vec(&s)?)?;
        Ok(s)
    }
}
/// Crash-safe intent for standalone viewer engines. An outstanding owner after
/// restart is unconfirmed, even when no browser task was active.
pub struct ViewerOwnerStore {
    file: AssetFile,
}
// The file lock guards other processes; serialize our short journal transactions
// locally so a concurrent admission read cannot fail against viewer cleanup.
static VIEWER_OWNER_TRANSACTIONS: Mutex<()> = Mutex::new(());
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewerOwners {
    schema_version: u32,
    owners: std::collections::BTreeSet<String>,
}
impl ViewerOwnerStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            file: AssetFile::new(root.join("capture/viewer-owners.json"), 8192),
        }
    }
    pub fn owners(&self) -> Result<std::collections::BTreeSet<String>> {
        let _transaction = VIEWER_OWNER_TRANSACTIONS
            .lock()
            .map_err(|_| anyhow::anyhow!("viewer ownership journal is unavailable"))?;
        let g = self.file.lock()?;
        let v: ViewerOwners = g
            .read()?
            .map(|b| serde_json::from_slice(&b))
            .transpose()?
            .unwrap_or(ViewerOwners {
                schema_version: 1,
                owners: Default::default(),
            });
        ensure!(
            v.schema_version == 1
                && v.owners.len() <= 2
                && v.owners.iter().all(|id| identifier(id)),
            "invalid viewer ownership journal"
        );
        Ok(v.owners)
    }
    pub fn begin(&self, id: &str) -> Result<()> {
        self.update(id, true)
    }
    pub fn release(&self, id: &str) -> Result<()> {
        self.update(id, false)
    }
    fn update(&self, id: &str, begin: bool) -> Result<()> {
        ensure!(identifier(id), "invalid viewer identity");
        let _transaction = VIEWER_OWNER_TRANSACTIONS
            .lock()
            .map_err(|_| anyhow::anyhow!("viewer ownership journal is unavailable"))?;
        let g = self.file.lock()?;
        let mut v: ViewerOwners = g
            .read()?
            .map(|b| serde_json::from_slice(&b))
            .transpose()?
            .unwrap_or(ViewerOwners {
                schema_version: 1,
                owners: Default::default(),
            });
        ensure!(
            v.schema_version == 1
                && v.owners.len() <= 2
                && v.owners.iter().all(|id| identifier(id)),
            "invalid viewer ownership journal"
        );
        if begin {
            ensure!(
                v.owners.len() < 2 && v.owners.insert(id.into()),
                "viewer owner capacity or identity conflict"
            );
        } else {
            v.owners.remove(id);
        }
        g.write(&serde_json::to_vec(&v)?)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CaptureChannel {
    pub kind: String,
    pub status: String,
    pub reason: Option<String>,
    pub bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CaptureArtifact {
    pub id: String,
    pub kind: String,
    pub file_name: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CaptureReceipt {
    pub schema_version: u32,
    pub policy: CapturePolicy,
    pub status: String,
    pub channels: Vec<CaptureChannel>,
    pub artifacts: Vec<CaptureArtifact>,
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && !s.starts_with('.')
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl CaptureReceipt {
    pub fn unknown(policy: CapturePolicy) -> Self {
        Self {
            schema_version: 1,
            policy,
            status: "partial".into(),
            channels: KINDS
                .iter()
                .map(|k| CaptureChannel {
                    kind: k.to_string(),
                    status: "unknown".into(),
                    reason: None,
                    bytes: 0,
                })
                .collect(),
            artifacts: vec![],
        }
    }
    pub fn validate_for_result(&self, passed: bool) -> Result<()> {
        self.validate()?;
        if self.status != "complete" {
            return Ok(());
        }
        let requested: &[&str] = match self.policy.mode.as_str() {
            "full" => &KINDS,
            "key_steps" => &["screenshot", "console", "network", "steps"],
            "failure_only" if !passed => &["screenshot", "console", "network", "steps"],
            "failure_only" => &[],
            _ => &[],
        };
        ensure!(
            self.channels.iter().all(|c| c.status
                == if requested.contains(&c.kind.as_str()) {
                    "collected"
                } else {
                    "not_requested"
                }),
            "complete receipt differs from requested capture mode"
        );
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        self.policy.validate()?;
        ensure!(
            self.schema_version == 1
                && matches!(self.status.as_str(), "disabled" | "complete" | "partial")
                && self.channels.len() == 5
                && self.artifacts.len() <= self.policy.max_events as usize + 4,
            "invalid capture receipt"
        );
        let mut kinds = std::collections::BTreeSet::new();
        for c in &self.channels {
            ensure!(
                KINDS.contains(&c.kind.as_str())
                    && kinds.insert(&c.kind)
                    && matches!(
                        c.status.as_str(),
                        "not_requested"
                            | "collected"
                            | "failed"
                            | "truncated"
                            | "unsupported"
                            | "unknown"
                    )
                    && c.reason.as_ref().is_none_or(|r| r.len() <= 2048),
                "invalid capture channel"
            );
        }
        let mut ids = std::collections::BTreeSet::new();
        for a in &self.artifacts {
            ensure!(
                identifier(&a.id)
                    && a.file_name.len() <= 128
                    && ids.insert(&a.id)
                    && KINDS.contains(&a.kind.as_str())
                    && a.bytes <= self.policy.max_artifact_bytes
                    && a.sha256.len() == 64
                    && a.sha256
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "invalid capture artifact"
            );
        }
        for c in &self.channels {
            let artifacts: Vec<_> = self.artifacts.iter().filter(|a| a.kind == c.kind).collect();
            let bytes = artifacts.iter().map(|a| a.bytes).sum::<u64>();
            ensure!(c.bytes == bytes, "channel bytes differ from attachments");
            if c.status == "collected" {
                ensure!(!artifacts.is_empty(), "collected channel lacks artifact");
            }
            if !artifacts.is_empty() {
                ensure!(
                    matches!(c.status.as_str(), "collected" | "truncated" | "failed"),
                    "artifact channel mismatch"
                );
            }
        }
        ensure!(
            self.artifacts.iter().map(|a| a.bytes).sum::<u64>() <= self.policy.max_run_bytes,
            "receipt run quota exceeded"
        );
        ensure!(
            self.status != "disabled"
                || (self.policy.mode == "off"
                    && self.channels.iter().all(|c| c.status == "not_requested")),
            "unexpected disabled receipt"
        );
        ensure!(
            self.policy.mode != "off"
                || (matches!(self.status.as_str(), "disabled" | "partial")
                    && self.artifacts.is_empty()
                    && self
                        .channels
                        .iter()
                        .all(|c| matches!(c.status.as_str(), "not_requested" | "unknown"))),
            "disabled capture contains data"
        );
        ensure!(
            self.status != "complete"
                || self
                    .channels
                    .iter()
                    .all(|c| matches!(c.status.as_str(), "collected" | "not_requested")),
            "incomplete capture marked complete"
        );
        Ok(())
    }
}
fn plain_dir(path: &Path) -> Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    ensure!(
        m.is_dir() && !m.file_type().is_symlink(),
        "unsafe evidence directory"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            m.file_attributes() & 0x400 == 0,
            "reparse evidence directory"
        );
    }
    Ok(())
}
pub(crate) fn read_file(path: &Path, limit: u64) -> Result<Vec<u8>> {
    use std::io::Read;
    let before = std::fs::symlink_metadata(path)?;
    ensure!(
        before.is_file() && !before.file_type().is_symlink() && before.len() <= limit,
        "unsafe evidence file"
    );
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000).share_mode(1);
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(if cfg!(target_os = "macos") {
            0x100
        } else {
            0x20000
        });
    }
    let f = options.open(path)?;
    let opened = f.metadata()?;
    let after = std::fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            before.nlink() == 1
                && opened.nlink() == 1
                && after.nlink() == 1
                && before.dev() == opened.dev()
                && before.ino() == opened.ino()
                && after.ino() == opened.ino()
                && after.dev() == opened.dev(),
            "evidence identity changed"
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::{fs::MetadataExt, io::AsRawHandle};
        ensure!(
            opened.file_attributes() & 0x400 == 0
                && before.file_attributes() & 0x400 == 0
                && after.file_attributes() & 0x400 == 0,
            "reparse evidence file"
        );
        #[repr(C)]
        struct Info {
            attributes: u32,
            creation: [u32; 2],
            access: [u32; 2],
            write: [u32; 2],
            volume: u32,
            size_high: u32,
            size_low: u32,
            links: u32,
            index_high: u32,
            index_low: u32,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetFileInformationByHandle(handle: *mut std::ffi::c_void, info: *mut Info) -> i32;
        }
        let mut info = std::mem::MaybeUninit::<Info>::uninit();
        ensure!(
            unsafe { GetFileInformationByHandle(f.as_raw_handle(), info.as_mut_ptr()) } != 0,
            "cannot verify evidence identity"
        );
        let info = unsafe { info.assume_init() };
        ensure!(info.links == 1, "linked evidence file");
        let check = options.open(path)?;
        let mut current = std::mem::MaybeUninit::<Info>::uninit();
        ensure!(
            unsafe { GetFileInformationByHandle(check.as_raw_handle(), current.as_mut_ptr()) } != 0,
            "cannot verify evidence path"
        );
        let current = unsafe { current.assume_init() };
        ensure!(
            current.volume == info.volume
                && current.index_low == info.index_low
                && current.index_high == info.index_high
                && current.links == 1
                && current.attributes & 0x400 == 0,
            "evidence path identity changed"
        );
    }
    ensure!(
        opened.is_file() && opened.len() == before.len() && after.len() == before.len(),
        "evidence length changed"
    );
    let mut bytes = vec![];
    f.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == before.len() && bytes.len() as u64 <= limit,
        "evidence changed while reading"
    );
    Ok(bytes)
}
#[derive(Clone)]
pub struct CaptureSession {
    pub snapshot: CapturePolicySnapshot,
    root: PathBuf,
    used: Arc<Mutex<u64>>,
    secrets: Vec<String>,
}
impl CaptureSession {
    pub fn new(snapshot: CapturePolicySnapshot, run_dir: PathBuf) -> Result<Self> {
        Self::new_with_budget(snapshot, run_dir, Arc::new(Mutex::new(0)))
    }
    pub fn new_with_budget(
        snapshot: CapturePolicySnapshot,
        run_dir: PathBuf,
        used: Arc<Mutex<u64>>,
    ) -> Result<Self> {
        snapshot.policy.validate()?;
        plain_dir(&run_dir)?;
        Ok(Self {
            snapshot,
            root: run_dir,
            used,
            secrets: vec![],
        })
    }
    pub fn with_known_secrets(mut self, mut secrets: Vec<String>) -> Self {
        secrets.retain(|s| !s.is_empty());
        secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
        secrets.dedup();
        self.secrets = secrets;
        self
    }
    fn metadata_safe(&self, receipt: &CaptureReceipt) -> bool {
        // Never rewrite identities or digests: a secret match invalidates the
        // diagnostic receipt, while the separately validated business result survives.
        fn contains(value: &serde_json::Value, secrets: &[String]) -> bool {
            match value {
                serde_json::Value::String(text) => {
                    secrets.iter().any(|secret| text.contains(secret))
                }
                serde_json::Value::Array(values) => values.iter().any(|v| contains(v, secrets)),
                serde_json::Value::Object(values) => values.values().any(|v| contains(v, secrets)),
                _ => false,
            }
        }
        serde_json::to_value(receipt).is_ok_and(|value| !contains(&value, &self.secrets))
    }
    pub fn accept(
        &self,
        process: &crate::protocol::MidsceneProcess,
        case: &str,
        mut receipt: CaptureReceipt,
    ) -> Result<CaptureReceipt> {
        if !self.metadata_safe(&receipt) {
            return Ok(CaptureReceipt::unknown(self.snapshot.policy.clone()));
        }
        receipt.validate()?;
        ensure!(
            receipt.policy == self.snapshot.policy && identifier(case),
            "capture identity mismatch"
        );
        let mut used = self
            .used
            .lock()
            .map_err(|_| anyhow::anyhow!("capture quota unavailable"))?;
        let mut accepted = vec![];
        for mut a in std::mem::take(&mut receipt.artifacts) {
            let result = (|| -> Result<()> {
                let ext = if a.kind == "trace" {
                    "zip"
                } else if a.kind == "screenshot" {
                    "png"
                } else {
                    "json"
                };
                ensure!(
                    a.file_name.ends_with(&format!(".{ext}")),
                    "attachment kind mismatch"
                );
                ensure!(
                    *used + a.bytes <= self.snapshot.policy.max_run_bytes,
                    "capture run quota exceeded"
                );
                let bytes = process.read_capture_attachment(&a.file_name, a.bytes)?;
                ensure!(digest(&bytes) == a.sha256, "capture digest mismatch");
                for p in [
                    self.root.clone(),
                    self.root.join("evidence"),
                    self.root.join("evidence").join(case),
                ] {
                    if !p.exists() {
                        std::fs::create_dir(&p)?;
                    }
                    plain_dir(&p)?;
                }
                let ext = if a.kind == "trace" {
                    "zip"
                } else if a.kind == "screenshot" {
                    "png"
                } else {
                    "json"
                };
                a.file_name = format!("{}.{}", a.id, ext);
                let destination = self.root.join("evidence").join(case).join(&a.file_name);
                ensure!(!destination.exists(), "committed artifact is immutable");
                let asset = AssetFile::new(
                    destination,
                    self.snapshot.policy.max_artifact_bytes as usize,
                );
                asset.lock()?.write(&bytes)?;
                *used += a.bytes;
                Ok(())
            })();
            if result.is_ok() {
                accepted.push(a);
            } else {
                receipt.status = "partial".into();
                if let Some(c) = receipt.channels.iter_mut().find(|c| c.kind == a.kind) {
                    c.status = "failed".into();
                    c.reason = Some("attachment validation, quota or persistence failed".into());
                    c.bytes = 0;
                }
            }
        }
        receipt.artifacts = accepted;
        for c in &mut receipt.channels {
            c.bytes = receipt
                .artifacts
                .iter()
                .filter(|a| a.kind == c.kind)
                .map(|a| a.bytes)
                .sum();
        }
        receipt.validate()?;
        Ok(receipt)
    }
}
pub fn read_committed_artifact(
    root: &Path,
    case: &str,
    receipt: &CaptureReceipt,
    id: &str,
) -> Result<Vec<u8>> {
    receipt.validate()?;
    ensure!(identifier(case), "invalid case identity");
    let a = receipt
        .artifacts
        .iter()
        .find(|a| a.id == id)
        .context("artifact not committed")?;
    let ext = if a.kind == "trace" {
        "zip"
    } else if a.kind == "screenshot" {
        "png"
    } else {
        "json"
    };
    ensure!(
        a.file_name == format!("{}.{}", a.id, ext),
        "invalid committed filename"
    );
    for p in [
        root.to_path_buf(),
        root.join("evidence"),
        root.join("evidence").join(case),
    ] {
        plain_dir(&p)?;
    }
    let b = read_file(
        &root.join("evidence").join(case).join(&a.file_name),
        a.bytes,
    )?;
    ensure!(
        b.len() as u64 == a.bytes && digest(&b) == a.sha256,
        "committed evidence changed"
    );
    Ok(b)
}

/// Encode an already validated bounded attachment for the local RPC transport.
pub fn encode_base64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn viewer_ownership_is_bounded_durable_and_requires_explicit_confirmed_release() {
        let directory = tempfile::tempdir().unwrap();
        let store = ViewerOwnerStore::new(directory.path().into());
        assert!(store.owners().unwrap().is_empty());
        store.begin("viewer-a").unwrap();
        store.begin("viewer-b").unwrap();
        assert!(store.begin("viewer-c").is_err());
        assert!(store.begin("../escape").is_err());
        assert_eq!(
            ViewerOwnerStore::new(directory.path().into())
                .owners()
                .unwrap()
                .len(),
            2
        );
        store.release("viewer-a").unwrap();
        assert!(store.owners().unwrap().contains("viewer-b"));
        store.release("viewer-b").unwrap();
        assert!(store.owners().unwrap().is_empty());
    }
    #[test]
    fn viewer_admission_reads_do_not_race_local_cleanup_transactions() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(ViewerOwnerStore::new(directory.path().into()));
        std::thread::scope(|scope| {
            let writer = store.clone();
            scope.spawn(move || {
                for _ in 0..40 {
                    writer.begin("viewer-fixture").unwrap();
                    writer.release("viewer-fixture").unwrap();
                }
            });
            for _ in 0..4 {
                let reader = store.clone();
                scope.spawn(move || {
                    for _ in 0..80 {
                        let owners = reader.owners().unwrap();
                        assert!(owners.len() <= 1);
                    }
                });
            }
        });
        assert!(store.owners().unwrap().is_empty());
    }
    #[test]
    fn complete_capture_requires_the_requested_mode_and_actual_business_verdict() {
        for mode in ["full", "key_steps", "failure_only"] {
            let policy = CapturePolicy {
                mode: mode.into(),
                include_sensitive: true,
                ..Default::default()
            };
            let mut receipt = CaptureReceipt::unknown(policy);
            receipt.status = "complete".into();
            for c in &mut receipt.channels {
                c.status = "not_requested".into();
            }
            assert_eq!(
                receipt.validate_for_result(true).is_ok(),
                mode == "failure_only"
            );
            assert!(receipt.validate_for_result(false).is_err());
            receipt.status = "partial".into();
            for c in &mut receipt.channels {
                c.status = "unknown".into();
            }
            receipt.validate_for_result(false).unwrap();
        }
    }
    #[test]
    fn policy_cas_and_authorization() {
        let dir = tempfile::tempdir().unwrap();
        let store = CapturePolicyStore::new(dir.path().into());
        let original = store.snapshot().unwrap();
        assert_eq!(original.policy.mode, "off");
        let mut p = original.policy.clone();
        p.mode = "full".into();
        assert!(store.save(0, p.clone()).is_err());
        p.include_sensitive = true;
        let next = store.save(0, p.clone()).unwrap();
        assert_eq!(next.revision, 1);
        assert!(store.save(0, p).is_err());
        assert_eq!(store.snapshot().unwrap(), next);
        let mut bad = next.policy;
        bad.max_events = 2001;
        assert!(bad.validate().is_err());
    }
    #[test]
    fn unknown_never_claims_disabled_or_complete() {
        let r = CaptureReceipt::unknown(CapturePolicy::default());
        r.validate().unwrap();
        assert_eq!(r.status, "partial");
        assert!(r.channels.iter().all(|c| c.status == "unknown"));
        let mut forged = r;
        forged.status = "complete".into();
        assert!(forged.validate().is_err());
    }
    #[test]
    fn committed_download_verifies_digest_and_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("evidence/case");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("steps-1.json"), b"[]").unwrap();
        let p = CapturePolicy {
            mode: "full".into(),
            include_sensitive: true,
            ..Default::default()
        };
        let mut r = CaptureReceipt::unknown(p);
        r.artifacts.push(CaptureArtifact {
            id: "steps-1".into(),
            kind: "steps".into(),
            file_name: "steps-1.json".into(),
            bytes: 2,
            sha256: digest(b"[]"),
        });
        let channel = r.channels.iter_mut().find(|c| c.kind == "steps").unwrap();
        channel.status = "collected".into();
        channel.bytes = 2;
        assert_eq!(
            read_committed_artifact(dir.path(), "case", &r, "steps-1").unwrap(),
            b"[]"
        );
        std::fs::write(path.join("steps-1.json"), b"{}").unwrap();
        assert!(read_committed_artifact(dir.path(), "case", &r, "steps-1").is_err());
        assert!(read_committed_artifact(dir.path(), "../case", &r, "steps-1").is_err());
    }
    #[test]
    fn multiple_screenshots_have_distinct_committed_names() {
        let p = CapturePolicy {
            mode: "key_steps".into(),
            include_sensitive: true,
            ..Default::default()
        };
        let mut r = CaptureReceipt::unknown(p);
        for n in 1..=2 {
            r.artifacts.push(CaptureArtifact {
                id: format!("screenshot-{n}"),
                kind: "screenshot".into(),
                file_name: format!("screenshot-{n}.png"),
                bytes: 1,
                sha256: digest(b"x"),
            });
        }
        let c = r
            .channels
            .iter_mut()
            .find(|c| c.kind == "screenshot")
            .unwrap();
        c.status = "collected".into();
        c.bytes = 2;
        r.validate().unwrap();
        r.channels[0].bytes = 1;
        assert!(r.validate().is_err());
    }
    #[cfg(unix)]
    #[test]
    fn links_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("file");
        std::fs::write(&p, b"data").unwrap();
        std::fs::hard_link(&p, dir.path().join("hard")).unwrap();
        assert!(read_file(&p, 4).is_err());
        std::os::unix::fs::symlink(&p, dir.path().join("sym")).unwrap();
        assert!(read_file(&dir.path().join("sym"), 4).is_err());
    }
}
