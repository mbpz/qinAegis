// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::protocol::{JsonRpcRequest, LlmConfig, MidsceneProcess, SandboxConfig};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{Mutex, Semaphore};

/// Extract `failed_step` from a JSON-RPC response data field.
fn extract_failed_step(data: &Option<serde_json::Value>) -> Option<String> {
    data.as_ref()
        .and_then(|v| v.get("failed_step"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseRef {
    pub id: String,
    pub yaml_script: String,
    pub name: String,
    pub priority: String,
    pub target_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<crate::capture::CaptureReceipt>,
    pub case_id: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub screenshot_base64: Option<String>,
    pub error_message: Option<String>,
    /// Number of self-heal retries attempted (0 = no healing attempted)
    #[serde(default)]
    pub retry_count: u8,
    /// Healed YAML script (original is preserved in TestCaseRef)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healed_yaml_script: Option<String>,
    /// Description of the last attempted YAML step (e.g. "- aiAssert: button visible")
    /// Populated on failure by the sandbox so the Healer can target the real
    /// failing call instead of hardcoding "aiAssert".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_step: Option<String>,
}

#[derive(Clone)]
pub struct TestExecutor {
    lifecycle_failed: Arc<AtomicBool>,
    capture: Option<Arc<crate::capture::CaptureSession>>,
    capture_ready: Arc<Mutex<bool>>,
    known_secrets: Vec<String>,
    process: Arc<Mutex<MidsceneProcess>>,
    semaphore: Arc<Semaphore>,
}

impl TestExecutor {
    #[cfg(test)]
    pub(crate) fn from_process_for_test(process: MidsceneProcess) -> Self {
        Self {
            lifecycle_failed: Arc::new(AtomicBool::new(false)),
            capture: None,
            known_secrets: vec![],
            capture_ready: Arc::new(Mutex::new(false)),
            process: Arc::new(Mutex::new(process)),
            semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    pub async fn new(
        max_concurrency: usize,
        llm_config: Option<LlmConfig>,
        sandbox_config: Option<SandboxConfig>,
    ) -> anyhow::Result<Self> {
        Self::new_cancellable(
            max_concurrency,
            llm_config,
            sandbox_config,
            Arc::new(AtomicBool::new(false)),
        )
        .await
    }

    pub async fn new_cancellable(
        max_concurrency: usize,
        llm_config: Option<LlmConfig>,
        sandbox_config: Option<SandboxConfig>,
        cancel: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            max_concurrency > 0 && max_concurrency <= 32,
            "executor concurrency must be 1..32"
        );
        anyhow::ensure!(
            !cancel.load(Ordering::Acquire),
            "execution cancelled before engine startup"
        );
        let known_secrets = llm_config
            .as_ref()
            .map(|c| c.api_key.clone())
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect();
        let process = MidsceneProcess::spawn(llm_config, sandbox_config)
            .await?
            .with_cancellation(cancel);
        let semaphore = Arc::new(Semaphore::new(max_concurrency));
        Ok(Self {
            lifecycle_failed: Arc::new(AtomicBool::new(false)),
            capture: None,
            known_secrets,
            capture_ready: Arc::new(Mutex::new(false)),
            process: Arc::new(Mutex::new(process)),
            semaphore,
        })
    }

    /// Supply every secret frozen at admission, including configured secondary
    /// candidates. This affects diagnostic validation only, never approved inputs.
    pub fn with_known_secrets(mut self, secrets: Vec<String>) -> Self {
        self.known_secrets
            .extend(secrets.into_iter().filter(|s| !s.is_empty()));
        self.known_secrets
            .sort_by_key(|s| std::cmp::Reverse(s.len()));
        self.known_secrets.dedup();
        if let Some(capture) = &self.capture {
            self.capture = Some(Arc::new(
                (**capture)
                    .clone()
                    .with_known_secrets(self.known_secrets.clone()),
            ));
        }
        self
    }
    pub fn with_capture(
        self,
        snapshot: crate::capture::CapturePolicySnapshot,
        run_dir: std::path::PathBuf,
    ) -> anyhow::Result<Self> {
        self.with_capture_budget(snapshot, run_dir, Arc::new(std::sync::Mutex::new(0)))
    }
    pub fn with_capture_budget(
        mut self,
        snapshot: crate::capture::CapturePolicySnapshot,
        run_dir: std::path::PathBuf,
        budget: Arc<std::sync::Mutex<u64>>,
    ) -> anyhow::Result<Self> {
        self.capture = Some(Arc::new(
            crate::capture::CaptureSession::new_with_budget(snapshot, run_dir, budget)?
                .with_known_secrets(self.known_secrets.clone()),
        ));
        Ok(self)
    }
    pub async fn run_case(&self, case: &TestCaseRef) -> anyhow::Result<TestResult> {
        let _permit = self.semaphore.acquire().await?;
        let process = self.process.lock().await;
        if self.lifecycle_failed() {
            return Ok(TestResult {
                capture: self.capture.as_ref().map(|capture| {
                    crate::capture::CaptureReceipt::unknown(capture.snapshot.policy.clone())
                }),
                case_id: case.id.clone(),
                passed: false,
                duration_ms: 0,
                screenshot_base64: None,
                error_message: Some("Not executed after an earlier engine cleanup failure".into()),
                retry_count: 0,
                healed_yaml_script: None,
                failed_step: None,
            });
        }
        if let Some(c) = &self.capture {
            let mut ready = self.capture_ready.lock().await;
            if !*ready {
                let r = process.call(JsonRpcRequest::CaptureCapabilities).await?;
                anyhow::ensure!(r.ok, "engine capture negotiation failed");
                let d = r.data.unwrap_or_default();
                anyhow::ensure!(
                    d["protocol_version"] == 1
                        && d["artifact_transport"] == "owned_files_v1"
                        && d["modes"].as_array().is_some_and(|a| a
                            .iter()
                            .any(|v| v.as_str() == Some(&c.snapshot.policy.mode)))
                        && crate::capture::KINDS.iter().all(|k| d["kinds"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(k)))),
                    "engine capture capability mismatch"
                );
                *ready = true;
            }
        }
        let response = process
            .call(JsonRpcRequest::RunYaml {
                yaml_script: case.yaml_script.clone(),
                case_id: case.id.clone(),
                target_url: case.target_url.clone(),
                capture_policy: self.capture.as_ref().map(|c| c.snapshot.policy.clone()),
            })
            .await;
        let mut capture = self
            .capture
            .as_ref()
            .map(|c| crate::capture::CaptureReceipt::unknown(c.snapshot.policy.clone()));
        let mut resp = match response {
            Ok(r) => r,
            Err(e) => {
                if self.capture.is_none() {
                    return Err(e);
                }
                return Ok(TestResult {
                    capture,
                    case_id: case.id.clone(),
                    passed: false,
                    duration_ms: 0,
                    screenshot_base64: None,
                    error_message: Some(e.to_string()),
                    retry_count: 0,
                    healed_yaml_script: None,
                    failed_step: None,
                });
            }
        };
        if let Some(data) = resp.data.as_mut().and_then(|value| value.as_object_mut()) {
            // A lifecycle fault is independent of the validated assertion facts.
            // Never persist an untrusted error body; malformed markers also fail
            // infrastructure while leaving otherwise valid business data intact.
            if let Some(error) = data.remove("lifecycle_error") {
                let known = matches!(
                    error.as_str(),
                    Some(
                        "browser_cleanup_failed"
                            | "temporary_cleanup_failed"
                            | "browser_and_temporary_cleanup_failed"
                    )
                );
                if !known {
                    tracing::warn!("engine returned an invalid lifecycle failure marker");
                }
                self.lifecycle_failed.store(true, Ordering::Release);
            }
        }
        if let Some(c) = &self.capture {
            if let Some(d) = resp.data.as_mut() {
                // Diagnostic protocol faults cannot erase a separately valid
                // assertion result. No inline screenshot may escape this path.
                let inline = d.get("screenshot_base64").is_some_and(|v| !v.is_null());
                let raw = d.as_object_mut().and_then(|d| d.remove("capture"));
                if let Some(object) = d.as_object_mut() {
                    object.insert("screenshot_base64".into(), serde_json::Value::Null);
                }
                if !inline {
                    if let Some(raw) = raw {
                        if let Ok(receipt) =
                            serde_json::from_value::<crate::capture::CaptureReceipt>(raw)
                        {
                            let passed = d.get("passed").and_then(|v| v.as_bool()).unwrap_or(false);
                            if receipt.validate_for_result(passed).is_ok() {
                                if let Ok(accepted) = c.accept(&process, &case.id, receipt) {
                                    capture = Some(accepted);
                                }
                            }
                        }
                    }
                }
            }
        }
        if resp.ok {
            let mut result: TestResult = serde_json::from_value(resp.data.unwrap_or_default())?;
            anyhow::ensure!(result.case_id == case.id, "engine case identity mismatch");
            result.retry_count = 0;
            result.capture = capture;
            Ok(result)
        } else {
            Ok(TestResult {
                capture,
                case_id: case.id.clone(),
                passed: false,
                duration_ms: 0,
                screenshot_base64: None,
                error_message: resp.error.or(Some("unknown error".into())),
                retry_count: 0,
                healed_yaml_script: None,
                failed_step: extract_failed_step(&resp.data),
            })
        }
    }
    pub async fn run_parallel(&self, cases: Vec<TestCaseRef>) -> anyhow::Result<Vec<TestResult>> {
        let mut handles = vec![];
        for case in cases {
            let executor = self.clone();
            handles.push(tokio::spawn(async move {
                match executor.run_case(&case).await {
                    Ok(r) => Ok(r),
                    Err(e) if executor.capture.is_none() => Ok(TestResult {
                        capture: None,
                        case_id: case.id,
                        passed: false,
                        duration_ms: 0,
                        screenshot_base64: None,
                        error_message: Some(e.to_string()),
                        retry_count: 0,
                        healed_yaml_script: None,
                        failed_step: None,
                    }),
                    Err(e) => Err(e),
                }
            }));
        }
        let mut results = vec![];
        let mut first_error = None;
        for handle in handles {
            match handle.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error.into());
                    }
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(results)
    }

    /// Terminate the owned process tree and wait for cleanup without relying
    /// on a shutdown RPC, including after a dropped or cancelled call.
    pub async fn terminate(&self) -> anyhow::Result<()> {
        self.process.lock().await.terminate().await
    }

    /// Assertion results remain readable even if an engine cleanup stage failed.
    /// Callers must fail the infrastructure run after confirming actual termination.
    pub fn lifecycle_failed(&self) -> bool {
        self.lifecycle_failed.load(Ordering::Acquire)
    }

    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let process = self.process.lock().await;
        let response = process.call(JsonRpcRequest::Shutdown).await;
        // Always release the child, even when shutdown failed or cancellation was set.
        let cleanup = process.terminate().await;
        let response = response?;
        cleanup?;
        anyhow::ensure!(
            response.ok,
            "executor shutdown failed: {}",
            response.error.unwrap_or_default()
        );
        anyhow::ensure!(
            !self.lifecycle_failed(),
            "engine browser or temporary workspace cleanup failed"
        );
        Ok(())
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;

    #[tokio::test]
    async fn cancelled_executor_does_not_spawn_an_engine() {
        let cancel = Arc::new(AtomicBool::new(true));
        let error = TestExecutor::new_cancellable(1, None, None, cancel)
            .await
            .err()
            .expect("cancelled construction must fail");
        assert!(error
            .to_string()
            .contains("cancelled before engine startup"));
    }

    #[tokio::test]
    async fn invalid_concurrency_is_rejected_before_engine_startup() {
        let error = TestExecutor::new_cancellable(0, None, None, Arc::new(AtomicBool::new(false)))
            .await
            .err()
            .expect("zero concurrency must not hang");
        assert!(error.to_string().contains("concurrency"));
    }
}

#[cfg(all(test, unix))]
mod capture_tests {
    use super::*;
    #[tokio::test]
    async fn queued_cases_do_not_execute_after_an_engine_cleanup_failure() {
        use crate::capture::*;
        for produced_result in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let request_count = directory.path().join("requests");
            let response = if produced_result { serde_json::json!({"id":"0","ok":true,"data":{
            "case_id":"first","passed":true,"duration_ms":12,"screenshot_base64":null,
            "error_message":null,"retry_count":0,"healed_yaml_script":null,"failed_step":null,
            "lifecycle_error":"browser_cleanup_failed"}}) } else { serde_json::json!({"id":"0","ok":false,"error":"original execution failure","data":{"lifecycle_error":"temporary_cleanup_failed"}}) }.to_string();
            let capabilities = serde_json::json!({"id":"0","ok":true,"data":{"protocol_version":1,"modes":["off"],"kinds":KINDS,"artifact_transport":"owned_files_v1"}}).to_string();
            let mut cmd = tokio::process::Command::new("sh");
            cmd.arg("-c").arg("read line; printf '%s\\n' \"$1\"; while read line; do case \"$line\" in *run_yaml*) printf 'run\\n' >> \"$3\"; printf '%s\\n' \"$2\";; *shutdown*) printf '%s\\n' '{\"id\":\"0\",\"ok\":true,\"data\":null}';; esac; done").arg("fixture").arg(capabilities).arg(response).arg(&request_count).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
            let (process, _) = MidsceneProcess::launch_owned_for_test(cmd).await.unwrap();
            let executor = TestExecutor::from_process_for_test(process)
                .with_capture(
                    CapturePolicySnapshot {
                        revision: 0,
                        policy: CapturePolicy::default(),
                    },
                    directory.path().into(),
                )
                .unwrap();
            let cases = ["first", "queued"].map(|id| TestCaseRef {
                id: id.into(),
                name: id.into(),
                yaml_script: "fixture".into(),
                priority: "P1".into(),
                target_url: None,
            });
            let results = executor.run_parallel(cases.into()).await.unwrap();
            assert_eq!(results.len(), 2);
            assert_eq!(results[0].case_id, "first");
            assert_eq!(results[0].passed, produced_result);
            assert_eq!(results[0].duration_ms, if produced_result { 12 } else { 0 });
            if !produced_result {
                assert_eq!(
                    results[0].error_message.as_deref(),
                    Some("original execution failure")
                );
            }
            assert_eq!(results[1].case_id, "queued");
            assert!(!results[1].passed);
            assert_eq!(results[1].duration_ms, 0);
            assert_eq!(
                results[1].error_message.as_deref(),
                Some("Not executed after an earlier engine cleanup failure")
            );
            assert!(results[1].failed_step.is_none());
            assert_eq!(results[1].capture.as_ref().unwrap().status, "partial");
            assert!(results[1]
                .capture
                .as_ref()
                .unwrap()
                .channels
                .iter()
                .all(|c| c.status == "unknown"));
            assert_eq!(std::fs::read_to_string(request_count).unwrap(), "run\n");
            assert!(executor.shutdown().await.is_err());
        }
    }
    #[tokio::test]
    async fn lifecycle_failure_is_separate_even_without_a_capture_session() {
        let response = serde_json::json!({"id":"0","ok":true,"data":{
            "case_id":"case","passed":true,"duration_ms":12,"screenshot_base64":null,
            "error_message":null,"retry_count":0,"healed_yaml_script":null,"failed_step":null,
            "lifecycle_error":"temporary_cleanup_failed"}})
        .to_string();
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg("read line; printf '%s\\n' \"$1\"; read line; printf '%s\\n' '{\"id\":\"0\",\"ok\":true,\"data\":null}'; cat >/dev/null").arg("fixture").arg(response).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        let (process, _) = MidsceneProcess::launch_owned_for_test(cmd).await.unwrap();
        let executor = TestExecutor::from_process_for_test(process);
        let result = executor
            .run_case(&TestCaseRef {
                id: "case".into(),
                name: "case".into(),
                yaml_script: "fixture".into(),
                priority: "P1".into(),
                target_url: None,
            })
            .await
            .unwrap();
        assert!(result.passed);
        assert_eq!(result.duration_ms, 12);
        assert!(result.capture.is_none());
        assert!(executor.lifecycle_failed());
        assert!(executor.shutdown().await.is_err());
    }
    #[tokio::test]
    async fn cleanup_failures_preserve_business_facts_but_fail_shutdown_and_run_status() {
        use crate::{
            capture::*,
            execution_records::{RunCase, RunRecord, RunStore},
        };
        for passed in [true, false] {
            for marker in [
                serde_json::json!("browser_cleanup_failed"),
                serde_json::json!("temporary_cleanup_failed"),
                serde_json::json!("browser_and_temporary_cleanup_failed"),
                serde_json::json!("injected private error"),
                serde_json::json!({"private":"untrusted diagnostic"}),
                serde_json::Value::Null,
            ] {
                let policy = CapturePolicySnapshot {
                    revision: 0,
                    policy: CapturePolicy::default(),
                };
                let mut receipt = CaptureReceipt::unknown(policy.policy.clone());
                receipt.status = "disabled".into();
                for channel in &mut receipt.channels {
                    channel.status = "not_requested".into();
                }
                let response = serde_json::json!({"id":"0","ok":true,"data":{
                    "case_id":"case","passed":passed,"duration_ms":12,"screenshot_base64":null,
                    "error_message":if passed {None} else {Some("original assertion failed")},
                    "retry_count":0,"healed_yaml_script":null,"failed_step":null,
                    "capture":receipt,"lifecycle_error":marker}})
                .to_string();
                let capabilities = serde_json::json!({"id":"0","ok":true,"data":{"protocol_version":1,"modes":["off"],"kinds":KINDS,"artifact_transport":"owned_files_v1"}}).to_string();
                let mut cmd = tokio::process::Command::new("sh");
                cmd.arg("-c").arg("read line; printf '%s\\n' \"$1\"; read line; printf '%s\\n' \"$2\"; read line; printf '%s\\n' '{\"id\":\"0\",\"ok\":true,\"data\":null}'; cat >/dev/null").arg("fixture").arg(capabilities).arg(response).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
                let (process, _) = MidsceneProcess::launch_owned_for_test(cmd).await.unwrap();
                let directory = tempfile::tempdir().unwrap();
                let executor = TestExecutor::from_process_for_test(process)
                    .with_capture(policy.clone(), directory.path().into())
                    .unwrap();
                let result = executor
                    .run_case(&TestCaseRef {
                        id: "case".into(),
                        name: "case".into(),
                        yaml_script: "fixture".into(),
                        priority: "P1".into(),
                        target_url: None,
                    })
                    .await
                    .unwrap();
                assert_eq!(result.passed, passed);
                assert_eq!(result.duration_ms, 12);
                assert_eq!(
                    result.error_message.as_deref(),
                    if passed {
                        None
                    } else {
                        Some("original assertion failed")
                    }
                );
                assert_eq!(result.capture.as_ref().unwrap().status, "disabled");
                assert!(executor.lifecycle_failed());
                let error = executor.shutdown().await.unwrap_err().to_string();
                assert_eq!(
                    error,
                    "engine browser or temporary workspace cleanup failed"
                );
                // Successful outer termination does not clear the infrastructure fault.
                executor.terminate().await.unwrap();
                assert!(executor.lifecycle_failed());
                let store = RunStore::new(directory.path().into());
                let mut record = RunRecord::prepare(
                    "run".into(),
                    "project".into(),
                    "smoke".into(),
                    serde_json::json!({"source":"legacy_config","model":"fixture"}),
                    None,
                    vec![RunCase {
                        case_revision: None,
                        case_id: "case".into(),
                        name: "case".into(),
                        yaml_script: "fixture".into(),
                        target_url: "http://fixture.invalid".into(),
                        provenance: None,
                    }],
                )
                .unwrap();
                record.capture_policy = Some(policy);
                store.begin(&record).unwrap();
                let saved = store.finish("failed", vec![result], Some(error)).unwrap();
                assert_eq!(saved.status, "failed");
                assert_eq!(saved.results[0].passed, passed);
                assert!(!serde_json::to_string(&saved).unwrap().contains("private"));
            }
        }
    }
    #[tokio::test]
    async fn invalid_missing_or_secret_diagnostics_never_erase_valid_business_results() {
        use crate::capture::*;
        let policy = CapturePolicy {
            mode: "full".into(),
            include_sensitive: true,
            ..Default::default()
        };
        for passed in [true, false] {
            for variant in [
                "missing",
                "malformed",
                "wrong_mode",
                "primary_secret",
                "secondary_secret",
                "escaped_secret",
                "inline",
                "foreign",
            ] {
                let mut receipt = CaptureReceipt::unknown(policy.clone());
                if variant.ends_with("secret") {
                    receipt.channels[0].reason = Some(
                        if variant == "primary_secret" {
                            "primary-key"
                        } else if variant == "escaped_secret" {
                            "escaped\"private\\key"
                        } else {
                            "secondary-key"
                        }
                        .into(),
                    );
                }
                if variant == "wrong_mode" {
                    receipt.status = "complete".into();
                    for c in &mut receipt.channels {
                        c.status = "not_requested".into();
                    }
                }
                let mut data = serde_json::json!({"case_id":"case","passed":passed,"duration_ms":12,"screenshot_base64":null,"error_message":if passed {None} else {Some("original assertion failed")},"retry_count":0,"healed_yaml_script":null,"failed_step":null,"capture":receipt});
                if variant == "foreign" {
                    data["case_id"] = serde_json::json!("foreign-case");
                }
                if variant == "missing" {
                    data.as_object_mut().unwrap().remove("capture");
                }
                if variant == "malformed" {
                    data["capture"] = serde_json::json!({"schema_version":99});
                }
                if variant == "inline" {
                    data["screenshot_base64"] = serde_json::json!("forbidden legacy image");
                }
                let response = serde_json::json!({"id":"0","ok":true,"data":data}).to_string();
                let capabilities = serde_json::json!({"id":"0","ok":true,"data":{"protocol_version":1,"modes":["full"],"kinds":KINDS,"artifact_transport":"owned_files_v1"}}).to_string();
                let mut cmd = tokio::process::Command::new("sh");
                cmd.arg("-c").arg("read line; printf '%s\n' \"$1\"; read line; printf '%s\n' \"$2\"; cat >/dev/null").arg("fixture").arg(capabilities).arg(response).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
                let (process, _) = MidsceneProcess::launch_owned_for_test(cmd).await.unwrap();
                let directory = tempfile::tempdir().unwrap();
                let executor = TestExecutor::from_process_for_test(process)
                    .with_capture(
                        CapturePolicySnapshot {
                            revision: 1,
                            policy: policy.clone(),
                        },
                        directory.path().into(),
                    )
                    .unwrap()
                    .with_known_secrets(vec![
                        "primary-key".into(),
                        "secondary-key".into(),
                        "escaped\"private\\key".into(),
                    ]);
                let result = executor
                    .run_case(&TestCaseRef {
                        id: "case".into(),
                        yaml_script: "- aiAssert: fixture".into(),
                        name: "case".into(),
                        priority: "P1".into(),
                        target_url: None,
                    })
                    .await;
                if variant == "foreign" {
                    assert!(result
                        .unwrap_err()
                        .to_string()
                        .contains("case identity mismatch"));
                    executor.terminate().await.unwrap();
                    continue;
                }
                let result = result.unwrap();
                assert_eq!(result.passed, passed, "{variant}");
                assert_eq!(
                    result.error_message.as_deref(),
                    if passed {
                        None
                    } else {
                        Some("original assertion failed")
                    }
                );
                assert!(result.screenshot_base64.is_none());
                let receipt = result.capture.unwrap();
                assert!(
                    receipt
                        .channels
                        .iter()
                        .all(|c| c.status == "unknown" && c.reason.is_none()),
                    "{variant}"
                );
                assert!(receipt.artifacts.is_empty());
                assert!(!serde_json::to_string(&receipt).unwrap().contains("-key"));
                executor.terminate().await.unwrap();
            }
        }
    }
    #[tokio::test]
    async fn unsupported_engine_is_rejected_before_actions() {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg("read line; printf '%s\\n' '{\"id\":\"0\",\"ok\":true,\"data\":{\"protocol_version\":0}}'; read line; exit 9").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        let (p, _) = MidsceneProcess::launch_owned_for_test(cmd).await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let e = TestExecutor::from_process_for_test(p)
            .with_capture(
                crate::capture::CapturePolicySnapshot {
                    revision: 0,
                    policy: Default::default(),
                },
                dir.path().into(),
            )
            .unwrap();
        let error = e
            .run_case(&TestCaseRef {
                id: "case".into(),
                yaml_script: "tasks: []".into(),
                name: "test".into(),
                priority: "P1".into(),
                target_url: None,
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("capability mismatch"));
        e.terminate().await.unwrap();
    }
    #[tokio::test]
    async fn owned_files_are_migrated_and_shared_quota_is_enforced() {
        use crate::capture::*;
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c")
            .arg("cat >/dev/null")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let (p, workspace) = MidsceneProcess::launch_owned_for_test(cmd).await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let policy = CapturePolicy {
            mode: "full".into(),
            include_sensitive: true,
            max_artifact_bytes: 1024,
            max_run_bytes: 1024,
            ..Default::default()
        };
        let shared_budget = Arc::new(std::sync::Mutex::new(0));
        let session = CaptureSession::new_with_budget(
            CapturePolicySnapshot {
                revision: 1,
                policy: policy.clone(),
            },
            dir.path().into(),
            shared_budget.clone(),
        )
        .unwrap();
        let name = "qinaegis-capture-0123456789abcdef.json";
        let bytes = vec![b' '; 600];
        std::fs::write(workspace.join(name), &bytes).unwrap();
        let mut r = CaptureReceipt::unknown(policy);
        r.artifacts.push(CaptureArtifact {
            id: "steps-1".into(),
            kind: "steps".into(),
            file_name: name.into(),
            bytes: 600,
            sha256: digest(&bytes),
        });
        let c = r.channels.iter_mut().find(|c| c.kind == "steps").unwrap();
        c.status = "collected".into();
        c.bytes = 600;
        let committed = session.accept(&p, "first", r.clone()).unwrap();
        assert_eq!(
            read_committed_artifact(dir.path(), "first", &committed, "steps-1").unwrap(),
            bytes
        );
        let other_run = tempfile::tempdir().unwrap();
        let other_session = CaptureSession::new_with_budget(
            session.snapshot.clone(),
            other_run.path().into(),
            shared_budget,
        )
        .unwrap();
        let limited = other_session.accept(&p, "second", r.clone()).unwrap();
        assert_eq!(limited.status, "partial");
        assert!(limited.artifacts.is_empty());
        assert!(p.read_capture_attachment("../escape", 0).is_err());
        std::fs::write(workspace.join(name), b"changed").unwrap();
        assert!(session.accept(&p, "third", r).unwrap().artifacts.is_empty());
        let fresh = CaptureSession::new(
            CapturePolicySnapshot {
                revision: 1,
                policy: session.snapshot.policy.clone(),
            },
            dir.path().into(),
        )
        .unwrap();
        let mut bad = committed.clone();
        bad.artifacts[0].file_name = name.into();
        bad.artifacts[0].sha256 = "0".repeat(64);
        std::fs::write(workspace.join(name), &bytes).unwrap();
        assert!(fresh
            .accept(&p, "digest", bad)
            .unwrap()
            .artifacts
            .is_empty());
        let marker = std::fs::read(workspace.join(".qinaegis-owner")).unwrap();
        std::fs::write(workspace.join(".qinaegis-owner"), b"tampered").unwrap();
        assert!(p.read_capture_attachment(name, 600).is_err());
        std::fs::write(workspace.join(".qinaegis-owner"), marker).unwrap();
        p.terminate().await.unwrap();
    }
}
