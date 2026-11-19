//! Explicit, revision-bound cleanup of owned interrupted Allure output only.
use anyhow::{ensure, Context, Result};
use qin_aegis_core::{execution_records::RunStore, LocalStorage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{hash_map::RandomState, BTreeMap},
    fs,
    hash::{BuildHasher, Hasher},
    io::Read,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime},
};

const MARKER: &str = ".qinaegis-report-stage.json";
const MAX_ENTRIES: usize = 4096;
const MAX_BYTES: u64 = 128 * 1024 * 1024;
const TTL: Duration = Duration::from_secs(300);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    project: String,
    run_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cleanup {
    project: String,
    run_id: String,
    preview_token: String,
    approved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ownership {
    schema_version: u32,
    kind: String,
    project: String,
    run_id: String,
    stage: String,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
struct Candidate {
    name: String,
    kind: String,
    files: usize,
    bytes: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
struct Manual {
    name: String,
    reason: String,
}
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    name: String,
    size: u64,
    modified: Option<SystemTime>,
    identity: (u64, u64),
    digest: (u64, u64),
}
#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    candidates: Vec<Candidate>,
    manual_review: Vec<Manual>,
    entries: Vec<Entry>,
    completed: bool,
    protected_report: bool,
}
struct Preview {
    created: Instant,
    root: PathBuf,
    snapshot: Snapshot,
}
struct Cache {
    hashes: [RandomState; 2],
    previews: BTreeMap<String, Preview>,
}
fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(Cache {
            hashes: [RandomState::new(), RandomState::new()],
            previews: BTreeMap::new(),
        })
    })
}
fn identity(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 128
            && !value.starts_with('.')
            && value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.')),
        "invalid report identity"
    );
    Ok(())
}
fn directory(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        meta.is_dir() && !meta.file_type().is_symlink(),
        "unsafe report directory"
    );
    Ok(())
}
fn run_root(project: &str, run_id: &str) -> Result<PathBuf> {
    identity(project)?;
    identity(run_id)?;
    let path = LocalStorage::run_dir(project, run_id);
    ensure!(path.is_absolute(), "report root must be absolute");
    for parent in path.ancestors().take(5) {
        directory(parent)?;
    }
    let root = path.canonicalize()?;
    for parent in root.ancestors() {
        directory(parent)?;
    }
    Ok(root)
}
fn file_entry(
    path: &Path,
    name: String,
    keys: &[RandomState; 2],
    total: &mut u64,
) -> Result<Entry> {
    let before = fs::symlink_metadata(path)?;
    ensure!(
        before.is_file() && !before.file_type().is_symlink(),
        "non-regular or linked residue"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(before.nlink() == 1, "hardlinked residue");
    }
    *total = total
        .checked_add(before.len())
        .context("residue size overflow")?;
    ensure!(*total <= MAX_BYTES, "residue exceeds inspection limit");
    let mut hashes = [keys[0].build_hasher(), keys[1].build_hasher()];
    let mut input = fs::File::open(path)?;
    let mut buffer = [0u8; 65536];
    let mut size = 0;
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        size += read as u64;
        ensure!(size <= before.len(), "residue changed during inspection");
        for hash in &mut hashes {
            hash.write(&buffer[..read]);
        }
    }
    let after = fs::symlink_metadata(path)?;
    ensure!(
        size == before.len()
            && after.len() == before.len()
            && after.modified().ok() == before.modified().ok()
            && !after.file_type().is_symlink(),
        "residue changed during inspection"
    );
    #[cfg(unix)]
    let id = {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            before.dev() == after.dev() && before.ino() == after.ino(),
            "residue replaced during inspection"
        );
        (before.dev(), before.ino())
    };
    #[cfg(not(unix))]
    let id = (0, 0);
    Ok(Entry {
        name,
        size,
        modified: before.modified().ok(),
        identity: id,
        digest: (hashes[0].finish(), hashes[1].finish()),
    })
}
fn scan(
    root: &Path,
    project: &str,
    run_id: &str,
    keys: &[RandomState; 2],
) -> Result<(Snapshot, String)> {
    directory(root)?;
    let record = RunStore::new(root.to_owned()).load()?;
    ensure!(
        record.project == project && record.run_id == run_id,
        "run evidence identity mismatch"
    );
    ensure!(
        record.status != "running",
        "running evidence cannot be cleaned"
    );
    let mut snapshot = Snapshot {
        candidates: Vec::new(),
        manual_review: Vec::new(),
        entries: Vec::new(),
        completed: false,
        protected_report: false,
    };
    let mut total = 0;
    snapshot.entries.push(file_entry(
        &root.join("run.json"),
        "run.json".into(),
        keys,
        &mut total,
    )?);
    let mut roots = fs::read_dir(root)?
        .take(MAX_ENTRIES + 1)
        .collect::<std::io::Result<Vec<_>>>()?;
    ensure!(
        roots.len() <= MAX_ENTRIES,
        "run directory exceeds inspection limit"
    );
    roots.sort_by_key(|entry| entry.file_name());
    let mut remaining_entries = MAX_ENTRIES - roots.len();
    for entry in roots {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name != "allure-report" && !name.starts_with(".allure-stage-") {
            continue;
        }
        let path = root.join(&name);
        if name == "allure-report" && fs::symlink_metadata(path.join("index.html")).is_ok() {
            snapshot.protected_report = true;
            let valid = (|| -> Result<()> {
                directory(&path)?;
                let mut size = 0;
                let index = file_entry(
                    &path.join("index.html"),
                    "allure-report/index.html".into(),
                    keys,
                    &mut size,
                )?;
                ensure!(
                    index.size > 0 && index.size <= 64 * 1024 * 1024,
                    "invalid report index"
                );
                let metadata = file_entry(
                    &path.join("qinaegis-reporter.json"),
                    "allure-report/qinaegis-reporter.json".into(),
                    keys,
                    &mut size,
                )?;
                ensure!(metadata.size <= 4096, "oversized reporter metadata");
                let reporter: Value =
                    serde_json::from_slice(&fs::read(path.join("qinaegis-reporter.json"))?)?;
                ensure!(
                    reporter["schema_version"] == 1
                        && reporter["reporter"] == "allure"
                        && reporter["mode"] == "awesome-single-file"
                        && reporter["version"]
                            .as_str()
                            .is_some_and(|v| !v.is_empty() && v.len() <= 64),
                    "invalid reporter metadata"
                );
                snapshot.entries.extend([index, metadata]);
                Ok(())
            })();
            snapshot.completed = valid.is_ok();
            if valid.is_err() {
                snapshot.manual_review.push(Manual{name,reason:"Output has an index asset but cannot be verified as a completed Allure report; preserved for manual review".into()});
            }
            continue;
        }
        let inspect = (|| -> Result<(Candidate, Vec<Entry>)> {
            directory(&path)?;
            let mut marker_total = 0;
            let marker_entry = file_entry(
                &path.join(MARKER),
                format!("{name}/{MARKER}"),
                keys,
                &mut marker_total,
            )?;
            ensure!(marker_entry.size <= 4096, "oversized ownership marker");
            let owner: Ownership = serde_json::from_slice(&fs::read(path.join(MARKER))?)?;
            ensure!(
                owner.schema_version == 1 && owner.project == project && owner.run_id == run_id,
                "foreign ownership marker"
            );
            ensure!(
                owner.stage.starts_with(".allure-stage-")
                    && owner.stage.len() <= 128
                    && owner
                        .stage
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_')),
                "invalid ownership stage"
            );
            ensure!(
                name == owner.stage || name == "allure-report",
                "ownership stage mismatch"
            );
            ensure!(
                matches!(
                    owner.kind.as_str(),
                    "allure_report_stage" | "allure_results_stage"
                ),
                "unknown ownership kind"
            );
            ensure!(
                name != "allure-report" || owner.kind == "allure_report_stage",
                "invalid published residue kind"
            );
            let mut children = fs::read_dir(&path)?
                .take(remaining_entries + 1)
                .collect::<std::io::Result<Vec<_>>>()?;
            ensure!(
                children.len() <= remaining_entries,
                "too many residue entries"
            );
            remaining_entries -= children.len();
            children.sort_by_key(|entry| entry.file_name());
            let mut entries = Vec::new();
            let mut bytes = 0;
            for child in children {
                let child_name = child
                    .file_name()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("invalid residue filename"))?;
                let fingerprint = file_entry(
                    &child.path(),
                    format!("{name}/{child_name}"),
                    keys,
                    &mut total,
                )?;
                bytes += fingerprint.size;
                entries.push(fingerprint);
            }
            if owner.kind == "allure_results_stage" {
                qin_aegis_core::allure::validate_results_residue(&record, &path)?;
            }
            Ok((
                Candidate {
                    name: name.clone(),
                    kind: if name == "allure-report" {
                        "incomplete_allure_report".into()
                    } else {
                        owner.kind
                    },
                    files: entries.len(),
                    bytes,
                },
                entries,
            ))
        })();
        match inspect {
            Ok((candidate,entries))=>{snapshot.candidates.push(candidate);snapshot.entries.extend(entries);}
            Err(_)=>snapshot.manual_review.push(Manual{name,reason:"Unverified ownership, unsupported files or inspection limit; preserved for manual review".into()}),
        }
    }
    snapshot.entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((snapshot, record.status))
}

pub fn handles(method: &str) -> bool {
    matches!(method, "previewReportCleanup" | "cleanupReportResidue")
}
/// The caller must retain the application lock throughout dispatch, preventing
/// task admission between the active-worker check and filesystem operations.
pub fn dispatch(method: &str, params: &Value, active_workers: usize) -> Result<Value> {
    dispatch_at(method, params, active_workers, run_root)
}
fn dispatch_at(
    method: &str,
    params: &Value,
    active_workers: usize,
    resolve: impl Fn(&str, &str) -> Result<PathBuf>,
) -> Result<Value> {
    ensure!(
        active_workers == 0,
        "wait for all active or cancelling tasks before report maintenance"
    );
    let mut state = cache()
        .lock()
        .map_err(|_| anyhow::anyhow!("report preview state unavailable"))?;
    state.previews.retain(|_, p| p.created.elapsed() < TTL);
    if method == "previewReportCleanup" {
        let p: Request = serde_json::from_value(params.clone())?;
        let root = resolve(&p.project, &p.run_id)?;
        let (snapshot, status) = scan(&root, &p.project, &p.run_id, &state.hashes)?;
        let token = if snapshot.candidates.is_empty() {
            None
        } else {
            Some(crate::server::random_token()?)
        };
        let mut preserved = vec!["run.json", "allure-results"];
        if snapshot.protected_report {
            preserved.push("allure-report");
        }
        let response = json!({"project":p.project,"run_id":p.run_id,"run_status":status,"completed_report_preserved":snapshot.completed,"cleanup_available":token.is_some(),"preview_token":token,"expires_in_seconds":TTL.as_secs(),"candidates":snapshot.candidates,"manual_review":snapshot.manual_review,"preserved":preserved});
        if let Some(token) = token {
            if state.previews.len() >= 2 {
                if let Some(oldest) = state
                    .previews
                    .iter()
                    .min_by_key(|(_, v)| v.created)
                    .map(|(k, _)| k.clone())
                {
                    state.previews.remove(&oldest);
                }
            }
            state.previews.insert(
                token,
                Preview {
                    created: Instant::now(),
                    root,
                    snapshot,
                },
            );
        }
        return Ok(response);
    }
    ensure!(
        method == "cleanupReportResidue",
        "unknown report maintenance operation"
    );
    let p: Cleanup = serde_json::from_value(params.clone())?;
    ensure!(p.approved, "explicit report cleanup confirmation required");
    let preview = state
        .previews
        .remove(&p.preview_token)
        .context("report cleanup preview expired; inspect again")?;
    let root = resolve(&p.project, &p.run_id)?;
    ensure!(
        root == preview.root,
        "report cleanup identity changed; inspect again"
    );
    let (current, _) = scan(&root, &p.project, &p.run_id, &state.hashes)?;
    ensure!(
        current == preview.snapshot,
        "report residue changed; inspect again"
    );
    let mut removed = Vec::new();
    let mut files = 0;
    let mut bytes = 0;
    for candidate in current.candidates {
        let path = root.join(&candidate.name);
        directory(&path)?;
        // The ownership marker is removed last, so interrupted cleanup remains
        // inspectable. No recursive delete or browser-supplied path is used.
        for entry in current.entries.iter().filter(|e| {
            e.name.starts_with(&format!("{}/", candidate.name))
                && !e.name.ends_with(&format!("/{MARKER}"))
        }) {
            fs::remove_file(root.join(&entry.name))?;
        }
        fs::remove_file(path.join(MARKER))?;
        fs::remove_dir(path)?;
        removed.push(candidate.name);
        files += candidate.files;
        bytes += candidate.bytes;
    }
    #[cfg(unix)]
    fs::File::open(&root)?.sync_all()?;
    Ok(json!({"ok":true,"removed":removed,"removed_files":files,"removed_bytes":bytes}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use qin_aegis_core::execution_records::{RunCase, RunRecord};
    static SERIAL: Mutex<()> = Mutex::new(());
    struct Fixture {
        root: PathBuf,
        record: RunRecord,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "qinaegis-report-maintenance-{}",
                crate::server::random_token().unwrap()
            ));
            fs::create_dir(&root).unwrap();
            let root = root.canonicalize().unwrap();
            let record = RunRecord::prepare(
                "run-1".into(),
                "project".into(),
                "smoke".into(),
                json!({"source":"legacy_config","model":"fixture"}),
                None,
                vec![RunCase {
                    case_revision: None,
                    case_id: "receipt".into(),
                    name: "Receipt".into(),
                    yaml_script: "- aiAssert: receipt".into(),
                    target_url: "https://example.invalid".into(),
                    provenance: None,
                }],
            )
            .unwrap();
            let store = RunStore::new(root.clone());
            store.begin(&record).unwrap();
            let record = store
                .finish("failed", vec![], Some("fixture interruption".into()))
                .unwrap();
            Self { root, record }
        }
        fn stage(&self, name: &str, kind: &str) -> PathBuf {
            let path = self.root.join(name);
            fs::create_dir(&path).unwrap();
            fs::write(path.join(MARKER),serde_json::to_vec(&json!({"schema_version":1,"project":"project","run_id":"run-1","kind":kind,"stage":if name=="allure-report"{".allure-stage-original"}else{name}})).unwrap()).unwrap();
            path
        }
        fn call(&self, method: &str, params: Value, active: usize) -> Result<Value> {
            dispatch_at(method, &params, active, |project, run| {
                identity(project)?;
                identity(run)?;
                ensure!(project == "project" && run == "run-1", "wrong identity");
                Ok(self.root.clone())
            })
        }
        fn preview(&self) -> Value {
            self.call(
                "previewReportCleanup",
                json!({"project":"project","run_id":"run-1"}),
                0,
            )
            .unwrap()
        }
        fn cleanup(&self, token: &Value) -> Result<Value> {
            self.call(
                "cleanupReportResidue",
                json!({"project":"project","run_id":"run-1","preview_token":token,"approved":true}),
                0,
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn owned_residue_cleanup_preserves_evidence_results_and_completed_report() {
        let _serial = SERIAL.lock().unwrap();
        let f = Fixture::new();
        let before = fs::read(f.root.join("run.json")).unwrap();
        let results = f.root.join("allure-results");
        fs::create_dir(&results).unwrap();
        fs::write(results.join("facts"), b"frozen").unwrap();
        let report = f.root.join("allure-report");
        fs::create_dir(&report).unwrap();
        fs::write(report.join("index.html"), b"completed report").unwrap();
        fs::write(report.join("qinaegis-reporter.json"),serde_json::to_vec(&json!({"schema_version":1,"reporter":"allure","mode":"awesome-single-file","version":"3.33.0"})).unwrap()).unwrap();
        let first = f.stage(".allure-stage-export", "allure_results_stage");
        let frozen = serde_json::to_vec_pretty(&f.record).unwrap();
        fs::write(first.join("qinaegis-run.json"), &frozen[..frozen.len() / 2]).unwrap();
        let second = f.stage(".allure-stage-html", "allure_report_stage");
        fs::write(second.join("summary.json"), b"partial").unwrap();
        let preview = f.preview();
        assert_eq!(preview["candidates"].as_array().unwrap().len(), 2);
        assert_eq!(preview["completed_report_preserved"], true);
        assert_eq!(
            f.cleanup(&preview["preview_token"]).unwrap()["removed"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(!first.exists() && !second.exists());
        assert_eq!(fs::read(f.root.join("run.json")).unwrap(), before);
        assert_eq!(fs::read(results.join("facts")).unwrap(), b"frozen");
        assert_eq!(
            fs::read(report.join("index.html")).unwrap(),
            b"completed report"
        );
        assert!(f
            .cleanup(&preview["preview_token"])
            .unwrap_err()
            .to_string()
            .contains("expired"));
    }
    #[test]
    fn incomplete_published_report_is_cleaned_only_with_ownership_and_without_index() {
        let _serial = SERIAL.lock().unwrap();
        let f = Fixture::new();
        let output = f.stage("allure-report", "allure_report_stage");
        fs::write(output.join("summary.json"), b"partial").unwrap();
        let preview = f.preview();
        assert_eq!(preview["candidates"][0]["kind"], "incomplete_allure_report");
        f.cleanup(&preview["preview_token"]).unwrap();
        assert!(!output.exists());
        fs::create_dir(&output).unwrap();
        fs::write(output.join("unknown"), b"keep").unwrap();
        assert_eq!(f.preview()["cleanup_available"], false);
        assert!(output.exists());
    }
    #[test]
    fn stale_preview_confirmation_and_active_worker_fail_closed() {
        let _serial = SERIAL.lock().unwrap();
        let f = Fixture::new();
        let stage = f.stage(".allure-stage-owned", "allure_report_stage");
        fs::write(stage.join("summary.json"), b"before").unwrap();
        let preview = f.preview();
        let params = json!({"project":"project","run_id":"run-1","preview_token":preview["preview_token"],"approved":true});
        assert!(f.call("cleanupReportResidue", params.clone(), 1).is_err());
        let mut unconfirmed = params.clone();
        unconfirmed["approved"] = json!(false);
        assert!(f.call("cleanupReportResidue", unconfirmed, 0).is_err());
        fs::write(stage.join("summary.json"), b"after!").unwrap();
        assert!(f
            .cleanup(&preview["preview_token"])
            .unwrap_err()
            .to_string()
            .contains("changed"));
        assert!(stage.exists());
        let preview = f.preview();
        let mut wrong = params;
        wrong["preview_token"] = preview["preview_token"].clone();
        wrong["run_id"] = json!("../run-1");
        assert!(f.call("cleanupReportResidue", wrong, 0).is_err());
        assert!(stage.exists());
    }
    #[test]
    fn unowned_foreign_nested_and_modified_export_assets_are_preserved() {
        let _serial = SERIAL.lock().unwrap();
        let f = Fixture::new();
        fs::create_dir(f.root.join(".allure-stage-legacy")).unwrap();
        let foreign = f.stage(".allure-stage-foreign", "allure_report_stage");
        let mut marker: Value =
            serde_json::from_slice(&fs::read(foreign.join(MARKER)).unwrap()).unwrap();
        marker["run_id"] = json!("other");
        fs::write(foreign.join(MARKER), serde_json::to_vec(&marker).unwrap()).unwrap();
        let nested = f.stage(".allure-stage-nested", "allure_report_stage");
        fs::create_dir(nested.join("subdir")).unwrap();
        let modified = f.stage(".allure-stage-modified", "allure_results_stage");
        fs::write(modified.join("qinaegis-run.json"), b"not frozen").unwrap();
        let preview = f.preview();
        assert_eq!(preview["cleanup_available"], false);
        assert_eq!(preview["manual_review"].as_array().unwrap().len(), 4);
    }
    #[cfg(unix)]
    #[test]
    fn linked_assets_are_never_followed_or_cleaned() {
        let _serial = SERIAL.lock().unwrap();
        let f = Fixture::new();
        let stage = f.stage(".allure-stage-link", "allure_report_stage");
        let victim = f.root.join("private");
        fs::write(&victim, b"unchanged").unwrap();
        std::os::unix::fs::symlink(&victim, stage.join("link")).unwrap();
        assert_eq!(f.preview()["cleanup_available"], false);
        fs::remove_file(stage.join("link")).unwrap();
        fs::hard_link(&victim, stage.join("hardlink")).unwrap();
        assert_eq!(f.preview()["cleanup_available"], false);
        assert_eq!(fs::read(victim).unwrap(), b"unchanged");
    }
    #[test]
    fn expired_preview_and_running_evidence_are_rejected() {
        let _serial = SERIAL.lock().unwrap();
        let f = Fixture::new();
        f.stage(".allure-stage-expire", "allure_report_stage");
        let preview = f.preview();
        cache()
            .lock()
            .unwrap()
            .previews
            .get_mut(preview["preview_token"].as_str().unwrap())
            .unwrap()
            .created = Instant::now() - TTL;
        assert!(f
            .cleanup(&preview["preview_token"])
            .unwrap_err()
            .to_string()
            .contains("expired"));
        let mut record = f.record.clone();
        record.status = "running".into();
        record.finished_at = None;
        record.error = None;
        fs::write(
            f.root.join("run.json"),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        assert!(f
            .call(
                "previewReportCleanup",
                json!({"project":"project","run_id":"run-1"}),
                0
            )
            .is_err());
    }
}
