//! Allure result-file adapter. Immutable run evidence remains the source of truth.
//! Schema: https://allurereport.org/docs/how-it-works-test-result-file/
use crate::execution_records::RunRecord;
use anyhow::{ensure, Context, Result};
use base64::Engine;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn attachment(
    files: &mut BTreeMap<String, Vec<u8>>,
    list: &mut Vec<Value>,
    source: String,
    name: &str,
    mime: &str,
    bytes: Vec<u8>,
) {
    list.push(json!({"name":name,"source":source,"type":mime}));
    files.insert(source, bytes);
}

fn results(record: &RunRecord) -> Result<BTreeMap<String, Vec<u8>>> {
    record.validate()?;
    ensure!(record.status != "running", "Allure requires a terminal run");
    let mut files = BTreeMap::new();
    for (index, case) in record.cases.iter().enumerate() {
        let prefix = format!("case-{index:04}");
        let result = record.results.iter().find(|r| r.case_id == case.case_id);
        let status = match result {
            Some(r) if r.passed => "passed",
            Some(_) => "failed",
            None if record.status == "stopped" => "skipped",
            None => "broken",
        };
        let mut attachments = Vec::new();
        attachment(
            &mut files,
            &mut attachments,
            format!("{prefix}-script-attachment.yaml"),
            "Frozen script",
            "application/yaml",
            case.yaml_script.as_bytes().to_vec(),
        );
        attachment(
            &mut files,
            &mut attachments,
            format!("{prefix}-source-attachment.json"),
            "Generation source (null means legacy)",
            "application/json",
            serde_json::to_vec_pretty(&case.provenance)?,
        );
        attachment(
            &mut files,
            &mut attachments,
            format!("{prefix}-model-attachment.json"),
            "Redacted execution model",
            "application/json",
            serde_json::to_vec_pretty(&record.model)?,
        );
        if let Some(r) = result {
            let mut evidence = serde_json::to_value(r)?;
            evidence
                .as_object_mut()
                .unwrap()
                .remove("screenshot_base64");
            attachment(
                &mut files,
                &mut attachments,
                format!("{prefix}-result-attachment.json"),
                "Recorded case result (duration_ms)",
                "application/json",
                serde_json::to_vec_pretty(&evidence)?,
            );
            if let Some(encoded) = &r.screenshot_base64 {
                let image = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .context("invalid evidence screenshot base64")?;
                ensure!(
                    image.starts_with(b"\x89PNG\r\n\x1a\n"),
                    "evidence screenshot is not PNG"
                );
                attachment(
                    &mut files,
                    &mut attachments,
                    format!("{prefix}-screenshot-attachment.png"),
                    "Screenshot",
                    "image/png",
                    image,
                );
            }
            if r.error_message.is_some() || r.failed_step.is_some() {
                attachment(
                    &mut files,
                    &mut attachments,
                    format!("{prefix}-error-attachment.json"),
                    "Recorded failure",
                    "application/json",
                    serde_json::to_vec_pretty(
                        &json!({"error":r.error_message,"failed_step":r.failed_step}),
                    )?,
                );
            }
        }
        let identity = serde_json::to_string(&(&record.project, &case.case_id))?;
        let parameters = json!([{"name":"target_url","value":case.target_url},
            {"name":"contract_inputs","value":serde_json::to_string(&case.provenance.as_ref().and_then(|p| p.contracts.as_ref()))?}]);
        // Run timestamps apply to the batch. Per-case start times are not recorded;
        // omitting them avoids inventing a timeline from aggregate durations.
        let value = json!({
            "uuid":serde_json::to_string(&(&record.project,&record.run_id,&case.case_id))?,
            "testCaseId":identity,"fullName":identity,"name":case.name,
            "historyId":serde_json::to_string(&(&record.project,&case.case_id,&parameters))?,
            "parameters":parameters,"status":status,"stage":"finished",
            "statusDetails":{"message":match result { Some(r) => r.error_message.as_deref(), None => record.error.as_deref() }.unwrap_or("")},
            "description":"Frozen QinAegis execution evidence. Contract semantics remain subject to human review; no inferred step results.",
            "labels":[{"name":"parentSuite","value":record.project},{"name":"suite","value":record.test_type},{"name":"framework","value":"QinAegis"}],
            "attachments":attachments,"steps":[]
        });
        files.insert(
            format!("{prefix}-result.json"),
            serde_json::to_vec_pretty(&value)?,
        );
    }
    // An infrastructure failure is visible even when every returned case passed.
    if record.status == "failed" || record.status == "stopped" {
        let status = if record.status == "failed" {
            "broken"
        } else {
            "skipped"
        };
        files.insert("execution-result.json".into(), serde_json::to_vec_pretty(&json!({
            "uuid":format!("{}:{}:execution",record.project,record.run_id),
            "testCaseId":format!("{}:execution-lifecycle",record.project),
            "fullName":format!("{}:execution-lifecycle",record.project),
            "name":"Execution lifecycle", "stage":"finished","status":status,
            "statusDetails":{"message":record.error.as_deref().unwrap_or("Execution stopped before normal completion")},
            "labels":[{"name":"parentSuite","value":record.project},{"name":"suite","value":"Execution lifecycle"}],
            "attachments":[],"steps":[]
        }))?);
    }
    files.insert(
        "qinaegis-run.json".into(),
        serde_json::to_vec_pretty(record)?,
    );
    Ok(files)
}

/// Publish a complete, private Allure results directory. Repeating the exact
/// export is safe; a directory with foreign or modified files is never merged.
pub fn export_run(record: &RunRecord, directory: &Path) -> Result<()> {
    let files = results(record)?;
    let parent = directory
        .parent()
        .context("Allure directory needs a parent")?;
    fs::create_dir_all(parent)?;
    let lock_asset = crate::asset_file::AssetFile::new(parent.join(".allure-export.json"), 1024);
    let _guard = lock_asset.lock()?;
    if let Ok(meta) = fs::symlink_metadata(directory) {
        ensure!(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "unsafe Allure output directory"
        );
        ensure!(
            fs::read_dir(directory)?.count() == files.len(),
            "Allure output contains different files"
        );
        for (name, bytes) in &files {
            let path = directory.join(name);
            let meta = fs::symlink_metadata(&path)?;
            ensure!(
                meta.is_file() && !meta.file_type().is_symlink(),
                "unsafe Allure output file"
            );
            ensure!(
                fs::read(path)? == *bytes,
                "Allure output differs from frozen run"
            );
        }
        return Ok(());
    }
    let staging = parent.join(format!(
        ".allure-stage-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&staging)?;
    let publish = (|| -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))?;
        }
        let marker = staging.join(".qinaegis-report-stage.json");
        let mut marker_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)?;
        marker_file.write_all(&serde_json::to_vec(&json!({
            "schema_version":1,"kind":"allure_results_stage","project":record.project,
            "run_id":record.run_id,"stage":staging.file_name().and_then(|v|v.to_str()).context("invalid staging name")?
        }))?)?;
        marker_file.sync_all()?;
        for (name, bytes) in files {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(staging.join(name))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        #[cfg(unix)]
        fs::File::open(&staging)?.sync_all()?;
        fs::remove_file(marker)?;
        fs::rename(&staging, directory)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if publish.is_err() {
        let _ = fs::remove_dir_all(staging);
    }
    publish
}

/// A marked interrupted export may contain complete frozen files or their
/// prefixes when the process stopped in the middle of write_all.
pub fn validate_results_residue(record: &RunRecord, directory: &Path) -> Result<()> {
    let expected = results(record)?;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("invalid residue name"))?;
        if name == ".qinaegis-report-stage.json" {
            continue;
        }
        let bytes = expected.get(&name).context("unrecognized export residue")?;
        let meta = fs::symlink_metadata(entry.path())?;
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= bytes.len() as u64,
            "invalid export residue"
        );
        ensure!(
            bytes.starts_with(&fs::read(entry.path())?),
            "export residue differs from frozen run"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        execution_records::{RunCase, RunRecord},
        executor::TestResult,
    };
    fn run() -> RunRecord {
        let mut run = RunRecord::prepare(
            "run-1".into(),
            "project".into(),
            "smoke".into(),
            json!({"source":"legacy_config","model":"fixture"}),
            None,
            vec![RunCase {
                case_revision: None,
                case_id: "case-1".into(),
                name: "Receipt".into(),
                yaml_script: "assert receipt".into(),
                target_url: "https://example.invalid".into(),
                provenance: None,
            }],
        )
        .unwrap();
        run.status = "completed".into();
        run.finished_at = Some("2026-11-16".into());
        run.results.push(TestResult {
            capture: None,
            case_id: "case-1".into(),
            passed: true,
            duration_ms: 10,
            screenshot_base64: None,
            error_message: None,
            retry_count: 0,
            healed_yaml_script: None,
            failed_step: None,
        });
        run
    }
    fn result(files: &BTreeMap<String, Vec<u8>>, name: &str) -> Value {
        serde_json::from_slice(&files[name]).unwrap()
    }
    #[test]
    fn identities_statuses_legacy_and_lifecycle_failure() {
        let mut run = run();
        let first = results(&run).unwrap();
        assert_eq!(result(&first, "case-0000-result.json")["status"], "passed");
        assert_eq!(first["case-0000-source-attachment.json"], b"null");
        assert_eq!(
            result(&first, "case-0000-result-attachment.json")["duration_ms"],
            10
        );
        assert!(result(&first, "case-0000-result.json")
            .get("start")
            .is_none());
        run.run_id = "run-2".into();
        run.results[0].passed = false;
        let next = results(&run).unwrap();
        assert_eq!(result(&next, "case-0000-result.json")["status"], "failed");
        assert_eq!(
            result(&first, "case-0000-result.json")["historyId"],
            result(&next, "case-0000-result.json")["historyId"]
        );
        assert_ne!(
            result(&first, "case-0000-result.json")["uuid"],
            result(&next, "case-0000-result.json")["uuid"]
        );
        run.results[0].passed = true;
        run.status = "failed".into();
        run.error = Some("shutdown failed".into());
        let failed = results(&run).unwrap();
        assert_eq!(result(&failed, "case-0000-result.json")["status"], "passed");
        assert_eq!(result(&failed, "execution-result.json")["status"], "broken");
        run.results.clear();
        run.status = "stopped".into();
        assert_eq!(
            result(&results(&run).unwrap(), "case-0000-result.json")["status"],
            "skipped"
        );
        run.status = "failed".into();
        assert_eq!(
            result(&results(&run).unwrap(), "case-0000-result.json")["status"],
            "broken"
        );
    }
    #[test]
    fn atomic_idempotent_export_rejects_partial_and_malformed_evidence() {
        let root = std::env::temp_dir().join(format!(
            "qinaegis-allure-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let output = root.join("results");
        let mut run = run();
        run.results[0].screenshot_base64 = Some("invalid!".into());
        assert!(export_run(&run, &output).is_err());
        assert!(!output.exists());
        run.results[0].screenshot_base64 = None;
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("blocked"), b"file").unwrap();
        assert!(export_run(&run, &root.join("blocked/results")).is_err());
        export_run(&run, &output).unwrap();
        export_run(&run, &output).unwrap();
        fs::write(output.join("foreign-result.json"), b"{}").unwrap();
        assert!(export_run(&run, &output).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
