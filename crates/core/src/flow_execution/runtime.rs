//! Sequential execution and private facts. Admission/model authorization and
//! durable storage are separate: this module never opens a Web run endpoint.
//! Facts can contain engine error text, screenshots and business output. Apply
//! the application's credential redaction and private evidence access controls
//! before persistence or transport. Callers must cancel via the flag and await
//! this future; externally aborting it cannot await asynchronous cleanup.
use super::{ExecutionDefinition, Operation, PreparedCase};
use crate::executor::{TestCaseRef, TestExecutor, TestResult};
use anyhow::{ensure, Context, Result};
pub use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::Instant;

#[async_trait]
pub trait CaseRunner: Send {
    async fn run_case(&mut self, case: &PreparedCase) -> Result<TestResult>;
    async fn run_step(&mut self, _step_id: &str, case: &PreparedCase) -> Result<TestResult> {
        self.run_case(case).await
    }
    /// Must terminate owned resources and await exit, independently of whether
    /// the run future completed, failed, or was dropped by a deadline.
    async fn cleanup(&mut self) -> Result<()>;
}
#[async_trait]
impl CaseRunner for TestExecutor {
    async fn run_case(&mut self, case: &PreparedCase) -> Result<TestResult> {
        TestExecutor::run_case(
            self,
            &TestCaseRef {
                id: case.reference.case_id.clone(),
                name: case.name.clone(),
                yaml_script: case.yaml_script.clone(),
                priority: "flow".into(),
                target_url: Some(case.target_url.clone()),
            },
        )
        .await
    }
    async fn cleanup(&mut self) -> Result<()> {
        self.terminate().await
    }
}
// Reserve room for the typed output envelope. Count JSON bytes into a bounded
// sink before cloning or retaining any untrusted runner fields, including on
// failed business assertions; escaping counts toward the same budget.
const MAX_DIAGNOSTIC_BYTES: usize = 4096;
struct DiagnosticBudget(String);
impl std::fmt::Write for DiagnosticBudget {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        if value.len() > MAX_DIAGNOSTIC_BYTES.saturating_sub(self.0.len()) {
            return Err(std::fmt::Error);
        }
        self.0.push_str(value);
        Ok(())
    }
}
fn diagnostic(error: &dyn std::fmt::Display) -> String {
    use std::fmt::Write;
    let mut budget = DiagnosticBudget(String::new());
    if write!(&mut budget, "{error}").is_err() {
        "execution diagnostic exceeds 4 KiB; raw diagnostic omitted".into()
    } else {
        budget.0
    }
}
const MAX_CASE_RESULT_BYTES: usize = 128 * 1024 - 32;
struct ResultBudget(usize);
impl std::io::Write for ResultBudget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_CASE_RESULT_BYTES.saturating_sub(self.0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "case result exceeds limit",
            ));
        }
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub fn validate_case_result_budget(result: &TestResult) -> Result<()> {
    ensure!(
        serde_json::to_writer(ResultBudget(0), result).is_ok(),
        "case result exceeds 128 KiB evidence limit; raw result omitted"
    );
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Running,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    Skipped,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptFact {
    pub attempt: u8,
    pub started_ms: u64,
    pub finished_ms: u64,
    pub status: Status,
    pub error: Option<String>,
    pub case_result: Option<TestResult>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepFact {
    pub step_id: String,
    pub status: Status,
    pub reason: Option<String>,
    pub attempts: Vec<AttemptFact>,
    pub outputs: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionFacts {
    pub schema_version: u32,
    pub flow_id: String,
    pub status: Status,
    pub steps: Vec<StepFact>,
    pub outputs: BTreeMap<String, Value>,
    pub error: Option<String>,
    pub cleanup_error: Option<String>,
    pub cleanup_started_ms: u64,
    pub finished_ms: u64,
}
fn offset(start: Instant) -> u64 {
    start.elapsed().as_millis().min(u64::MAX as u128) as u64
}
async fn cancelled(flag: Arc<AtomicBool>) {
    loop {
        if flag.load(Ordering::Acquire) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
/// Prepared snapshots are keyed by step, so two case steps can deliberately
/// reuse one publication while keeping each approved case reference explicit.
pub trait ExecutionObserver: Send + Sync {
    fn checkpoint(&self, facts: &ExecutionFacts) -> Result<()>;
}
struct NoObserver;
impl ExecutionObserver for NoObserver {
    fn checkpoint(&self, _: &ExecutionFacts) -> Result<()> {
        Ok(())
    }
}
pub async fn run(
    definition: &ExecutionDefinition,
    inputs: BTreeMap<String, Value>,
    prepared: &BTreeMap<String, PreparedCase>,
    runner: &mut dyn CaseRunner,
    cancel: Arc<AtomicBool>,
) -> Result<ExecutionFacts> {
    run_observed(definition, inputs, prepared, runner, cancel, &NoObserver).await
}
pub async fn run_observed(
    definition: &ExecutionDefinition,
    inputs: BTreeMap<String, Value>,
    prepared: &BTreeMap<String, PreparedCase>,
    runner: &mut dyn CaseRunner,
    cancel: Arc<AtomicBool>,
    observer: &dyn ExecutionObserver,
) -> Result<ExecutionFacts> {
    let order = definition.validate()?;
    definition.validate_inputs(&inputs)?;
    let expected: Vec<_> = definition
        .steps
        .iter()
        .filter_map(|s| match &s.operation {
            Operation::ExecuteCase { case } => Some((&s.id, case)),
            _ => None,
        })
        .collect();
    ensure!(
        prepared.len() == expected.len(),
        "prepared cases differ from execution definition"
    );
    for (id, reference) in expected {
        ensure!(
            prepared.get(id).is_some_and(|p| p.reference() == reference),
            "prepared case reference differs from definition"
        );
    }
    let start = Instant::now();
    let deadline = start + Duration::from_millis(definition.total_timeout_ms);
    let mut facts = ExecutionFacts {
        schema_version: 2,
        flow_id: definition.id.clone(),
        status: Status::Succeeded,
        steps: vec![],
        outputs: BTreeMap::new(),
        error: None,
        cleanup_error: None,
        cleanup_started_ms: 0,
        finished_ms: 0,
    };
    let mut outputs = BTreeMap::new();
    for id in order {
        let step = definition.steps.iter().find(|s| s.id == id).unwrap();
        let mut fact = StepFact {
            step_id: id.clone(),
            status: Status::Skipped,
            reason: None,
            attempts: vec![],
            outputs: BTreeMap::new(),
        };
        if facts.status == Status::Succeeded && cancel.load(Ordering::Acquire) {
            facts.status = Status::Cancelled;
            facts.error = Some("execution cancelled".into());
        } else if facts.status == Status::Succeeded && Instant::now() >= deadline {
            facts.status = Status::TimedOut;
            facts.error = Some("total execution timeout".into());
        }
        if facts.status != Status::Succeeded {
            fact.reason = Some("execution stopped after an earlier failure".into());
            facts.steps.push(fact);
            continue;
        }
        if step.depends_on.iter().any(|dep| !outputs.contains_key(dep)) {
            fact.reason = Some("upstream dependency was skipped".into());
            facts.steps.push(fact);
            continue;
        }
        let condition = step
            .condition
            .as_ref()
            .map(|c| ExecutionDefinition::evaluate_condition(c, &inputs, &outputs))
            .transpose();
        match condition {
            Ok(Some(false)) => {
                fact.reason = Some("condition evaluated to false".into());
                facts.steps.push(fact);
                continue;
            }
            Err(e) => {
                facts.status = Status::Failed;
                facts.error = Some(diagnostic(&e));
                fact.status = Status::Failed;
                fact.reason = facts.error.clone();
                facts.steps.push(fact);
                continue;
            }
            _ => {}
        }
        for attempt in 1..=step.max_attempts {
            let attempt_start = Instant::now();
            let began = offset(start);
            let mut preview = facts.clone();
            let mut active = fact.clone();
            active.status = Status::Running;
            active.attempts.push(AttemptFact {
                attempt,
                started_ms: began,
                finished_ms: began,
                status: Status::Running,
                error: None,
                case_result: None,
            });
            preview.status = Status::Running;
            preview.steps.push(active);
            if observer.checkpoint(&preview).is_err() {
                facts.status = Status::Failed;
                facts.error = Some("Cannot persist step start; action was not executed".into());
                fact.status = Status::Failed;
                fact.reason = facts.error.clone();
                break;
            }
            // Checkpoint I/O consumes the same bounded action budget.
            let step_deadline = attempt_start + Duration::from_millis(step.timeout_ms);
            let total_first = deadline <= step_deadline;
            let budget = deadline
                .min(step_deadline)
                .saturating_duration_since(Instant::now());
            let mut result = None;
            let mut operation_outputs = BTreeMap::new();
            let action = async {
                match &step.operation {
                    Operation::Wait { milliseconds } => {
                        let value =
                            ExecutionDefinition::resolve_source(milliseconds, &inputs, &outputs)?;
                        let ms = ExecutionDefinition::wait_duration(&value, step.timeout_ms)?;
                        let began = Instant::now();
                        tokio::time::sleep(Duration::from_millis(ms)).await;
                        operation_outputs.insert("elapsed_ms".into(), json!(offset(began)));
                    }
                    Operation::ExecuteCase { .. } => {
                        let mut output = runner
                            .run_step(&id, prepared.get(&id).context("prepared case unavailable")?)
                            .await?;
                        ensure!(
                            output.case_id == prepared[&id].reference().case_id
                                && output.retry_count == 0
                                && output.healed_yaml_script.is_none(),
                            "runner changed reviewed case identity or applied unapproved healing"
                        );
                        validate_case_result_budget(&output)?;
                        let passed = output.passed;
                        if passed {
                            // Keep the screenshot once, in attempt evidence.
                            // Dataflow carries result metadata, never a second
                            // copy of the base64 attachment.
                            let screenshot = output.screenshot_base64.take();
                            let metadata = serde_json::to_value(&output)?;
                            output.screenshot_base64 = screenshot;
                            operation_outputs.insert("result".into(), metadata);
                        }
                        result = Some(output);
                        ensure!(passed, "reviewed case business assertions failed");
                    }
                }
                definition.validate_step_outputs(&id, &operation_outputs)?;
                Ok::<_, anyhow::Error>(())
            };
            let (status, error) = if budget.is_zero() {
                drop(action);
                (
                    Status::TimedOut,
                    Some(
                        if total_first {
                            "total execution timeout"
                        } else {
                            "step execution timeout"
                        }
                        .into(),
                    ),
                )
            } else {
                tokio::select! {
                    biased;
                    _=cancelled(cancel.clone())=>(Status::Cancelled,Some("execution cancelled".into())),
                    outcome=tokio::time::timeout(budget,action)=>match outcome {
                        Ok(Ok(()))=>(Status::Succeeded,None),
                        Ok(Err(error))=>(Status::Failed,Some(diagnostic(&error))),
                        Err(_)=>(Status::TimedOut,Some(if total_first {"total execution timeout"}else{"step execution timeout"}.into())),
                    }
                }
            };
            fact.attempts.push(AttemptFact {
                attempt,
                started_ms: began,
                finished_ms: offset(start),
                status,
                error: error.clone(),
                case_result: result,
            });
            fact.status = status;
            let mut preview = facts.clone();
            let mut checkpoint = fact.clone();
            if status == Status::Succeeded {
                checkpoint.outputs = operation_outputs.clone();
            }
            preview.steps.push(checkpoint);
            preview.status = Status::Running;
            if observer.checkpoint(&preview).is_err() {
                facts.status = Status::Failed;
                facts.error = Some("Cannot persist step result; remaining actions stopped".into());
                fact.status = Status::Failed;
                fact.reason = facts.error.clone();
                break;
            }

            if status == Status::Succeeded {
                fact.outputs = operation_outputs;
                outputs.insert(id.clone(), fact.outputs.clone());
                break;
            }
            let retry = matches!(step.operation, Operation::Wait { .. })
                && status == Status::TimedOut
                && !total_first
                && attempt < step.max_attempts
                && Instant::now() < deadline;
            if !retry {
                facts.status = status;
                facts.error = error.clone();
                fact.reason = error;
                break;
            }
        }
        facts.steps.push(fact);
    }
    if facts.status == Status::Succeeded {
        let resolved = (|| -> Result<BTreeMap<String, Value>> {
            let mut result = BTreeMap::new();
            for (name, output) in &definition.outputs {
                let value = ExecutionDefinition::resolve_source(&output.source, &inputs, &outputs)?;
                ensure!(
                    output.value_type.accepts(&value),
                    "flow output type mismatch"
                );
                result.insert(name.clone(), value);
            }
            ensure!(
                serde_json::to_vec(&result)?.len() <= 128 * 1024,
                "flow output exceeds limit"
            );
            Ok(result)
        })();
        match resolved {
            Ok(value) => facts.outputs = value,
            Err(error) => {
                facts.status = Status::Failed;
                facts.error = Some(diagnostic(&error));
            }
        }
    }
    facts.cleanup_started_ms = offset(start);
    let cleanup = tokio::time::timeout(
        Duration::from_millis(definition.cleanup_timeout_ms),
        runner.cleanup(),
    )
    .await;
    facts.cleanup_error = match cleanup {
        Ok(Ok(())) => None,
        Ok(Err(e)) => Some(diagnostic(&e)),
        Err(_) => Some("execution cleanup timed out; owned resource exit unconfirmed".into()),
    };
    if facts.cleanup_error.is_some() {
        facts.outputs.clear();
        if facts.status == Status::Succeeded {
            facts.status = Status::Failed;
        }
    }
    facts.finished_ms = offset(start);
    Ok(facts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow_execution::{
        tests::{approved_fixture, flow},
        ExecutionStep,
    };
    use std::sync::atomic::AtomicUsize;
    struct Runner {
        calls: usize,
        cleanups: usize,
        delay: u64,
        passed: bool,
        cleanup_failure: bool,
        cleanup_delay: u64,
        active: Arc<AtomicUsize>,
        screenshot: Option<String>,
        error_override: Option<String>,
        runner_error: Option<String>,
        cleanup_error_override: Option<String>,
    }
    struct Active(Arc<AtomicUsize>);
    impl Drop for Active {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::AcqRel);
        }
    }
    #[async_trait]
    impl CaseRunner for Runner {
        async fn run_case(&mut self, case: &PreparedCase) -> Result<TestResult> {
            self.calls += 1;
            if let Some(error) = &self.runner_error {
                anyhow::bail!("{}", error);
            }
            self.active.fetch_add(1, Ordering::AcqRel);
            let _active = Active(self.active.clone());
            tokio::time::sleep(Duration::from_millis(self.delay)).await;
            Ok(TestResult {
                capture: None,
                case_id: case.reference().case_id.clone(),
                passed: self.passed,
                duration_ms: self.delay,
                screenshot_base64: self.screenshot.clone(),
                error_message: if self.error_override.is_some() {
                    self.error_override.clone()
                } else if self.passed {
                    None
                } else {
                    Some("original assertion failure".into())
                },
                retry_count: 0,
                healed_yaml_script: None,
                failed_step: None,
            })
        }
        async fn cleanup(&mut self) -> Result<()> {
            self.cleanups += 1;
            ensure!(
                self.active.load(Ordering::Acquire) == 0,
                "run future was not dropped before cleanup"
            );
            tokio::time::sleep(Duration::from_millis(self.cleanup_delay)).await;
            if let Some(error) = &self.cleanup_error_override {
                anyhow::bail!("{}", error);
            }
            ensure!(!self.cleanup_failure, "fixture cleanup failure");
            Ok(())
        }
    }
    fn runner() -> Runner {
        Runner {
            calls: 0,
            cleanups: 0,
            delay: 1,
            passed: true,
            cleanup_failure: false,
            cleanup_delay: 0,
            active: Arc::new(AtomicUsize::new(0)),
            screenshot: None,
            error_override: None,
            runner_error: None,
            cleanup_error_override: None,
        }
    }
    fn inputs() -> BTreeMap<String, Value> {
        BTreeMap::from([("enabled".into(), json!(true))])
    }
    fn flag() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }
    fn case_flow() -> (ExecutionDefinition, BTreeMap<String, PreparedCase>) {
        let (reference, project, snapshot, asset) = approved_fixture();
        let prepared =
            PreparedCase::from_asset(&reference, &project, &project.url, &snapshot, asset).unwrap();
        let mut definition = flow();
        definition.steps[0].operation = Operation::ExecuteCase { case: reference };
        definition.steps[0].condition = None;
        definition.required_permissions = [
            "browser.read",
            "browser.write",
            "model.send",
            "report.write",
        ]
        .map(String::from)
        .to_vec();
        (definition, BTreeMap::from([("pause".into(), prepared)]))
    }
    struct FailingObserver {
        calls: std::sync::atomic::AtomicUsize,
        fail_at: usize,
    }
    impl ExecutionObserver for FailingObserver {
        fn checkpoint(&self, _: &ExecutionFacts) -> Result<()> {
            ensure!(
                self.calls.fetch_add(1, Ordering::AcqRel) != self.fail_at,
                "fixture disk full"
            );
            Ok(())
        }
    }
    #[tokio::test]
    async fn failed_start_checkpoint_prevents_case_side_effect_and_always_cleans() {
        let (definition, prepared) = case_flow();
        let mut runner = runner();
        let observer = FailingObserver {
            calls: 0.into(),
            fail_at: 0,
        };
        let facts = run_observed(
            &definition,
            inputs(),
            &prepared,
            &mut runner,
            flag(),
            &observer,
        )
        .await
        .unwrap();
        assert_eq!(facts.status, Status::Failed);
        assert_eq!(runner.calls, 0);
        assert_eq!(runner.cleanups, 1);
        assert!(facts.error.unwrap().contains("not executed"));
    }
    #[tokio::test]
    async fn failed_end_checkpoint_prevents_all_later_case_actions() {
        let (mut definition, mut prepared) = case_flow();
        let mut second = definition.steps[0].clone();
        second.id = "second".into();
        second.depends_on = vec!["pause".into()];
        definition.steps.push(second);
        prepared.insert("second".into(), prepared["pause"].clone());
        let mut runner = runner();
        let observer = FailingObserver {
            calls: 0.into(),
            fail_at: 1,
        };
        let facts = run_observed(
            &definition,
            inputs(),
            &prepared,
            &mut runner,
            flag(),
            &observer,
        )
        .await
        .unwrap();
        assert_eq!(facts.status, Status::Failed);
        assert_eq!(runner.calls, 1);
        assert_eq!(runner.cleanups, 1);
        assert_eq!(facts.steps[1].status, Status::Skipped);
        assert!(facts.steps[0].outputs.is_empty());
    }
    #[tokio::test]
    async fn real_wait_and_skip_propagation_have_typed_outputs_and_cleanup() {
        let mut definition = flow();
        let mut runner = runner();
        let facts = run(&definition, inputs(), &BTreeMap::new(), &mut runner, flag())
            .await
            .unwrap();
        assert_eq!(facts.status, Status::Succeeded);
        assert!(facts.finished_ms >= 10);
        assert!(facts.steps[0].outputs["elapsed_ms"].is_u64());
        assert_eq!(runner.cleanups, 1);
        let mut next = definition.steps[0].clone();
        next.id = "dependent".into();
        next.depends_on = vec!["pause".into()];
        next.condition = None;
        definition.steps.push(next);
        let facts = run(
            &definition,
            BTreeMap::from([("enabled".into(), json!(false))]),
            &BTreeMap::new(),
            &mut runner,
            flag(),
        )
        .await
        .unwrap();
        assert!(facts
            .steps
            .iter()
            .all(|s| s.status == Status::Skipped && s.outputs.is_empty()));
        assert_eq!(
            facts.steps[1].reason.as_deref(),
            Some("upstream dependency was skipped")
        );
    }
    #[tokio::test]
    async fn cancellation_drops_active_action_before_bounded_cleanup() {
        let (definition, prepared) = case_flow();
        let mut runner = runner();
        runner.delay = 1000;
        let cancel = flag();
        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            trigger.store(true, Ordering::Release);
        });
        let facts = run(&definition, inputs(), &prepared, &mut runner, cancel)
            .await
            .unwrap();
        assert_eq!(facts.status, Status::Cancelled);
        assert_eq!(runner.calls, 1);
        assert_eq!(runner.cleanups, 1);
        assert!(facts.cleanup_error.is_none());
        assert!(facts.finished_ms < 500);
    }
    #[tokio::test]
    async fn deadlines_preserve_initial_failure_and_cleanup_failure_separately() {
        for total in [true, false] {
            let (mut definition, prepared) = case_flow();
            definition.steps[0].timeout_ms = if total { 30 } else { 15 };
            definition.total_timeout_ms = 30;
            let mut runner = runner();
            runner.delay = 1000;
            runner.cleanup_failure = true;
            let facts = run(&definition, inputs(), &prepared, &mut runner, flag())
                .await
                .unwrap();
            assert_eq!(facts.status, Status::TimedOut);
            assert!(facts
                .error
                .unwrap()
                .contains(if total { "total" } else { "step" }));
            assert_eq!(
                facts.cleanup_error.as_deref(),
                Some("fixture cleanup failure")
            );
            assert_eq!(runner.calls, 1);
            assert_eq!(runner.cleanups, 1);
        }
    }
    #[tokio::test]
    async fn business_failure_never_retries_or_weakens_assertions() {
        let (mut definition, prepared) = case_flow();
        let mut next: ExecutionStep = definition.steps[0].clone();
        next.id = "later".into();
        next.operation = Operation::Wait {
            milliseconds: crate::flow::InputSource::Literal { value: json!(1) },
        };
        definition.steps.push(next);
        let mut runner = runner();
        runner.passed = false;
        let facts = run(&definition, inputs(), &prepared, &mut runner, flag())
            .await
            .unwrap();
        assert_eq!(facts.status, Status::Failed);
        assert_eq!(runner.calls, 1);
        assert_eq!(facts.steps[0].attempts.len(), 1);
        assert_eq!(
            facts.steps[0].attempts[0]
                .case_result
                .as_ref()
                .unwrap()
                .error_message
                .as_deref(),
            Some("original assertion failure")
        );
        assert_eq!(facts.steps[1].status, Status::Skipped);
        assert_eq!(
            prepared["pause"].contracts().contracts()[0].original_assertions(),
            &["receipt matches input"]
        );
    }
    #[tokio::test]
    async fn pre_cancel_does_not_execute_and_cleanup_timeout_is_not_success() {
        let (definition, prepared) = case_flow();
        let mut runner = runner();
        runner.cleanup_delay = 100;
        let mut definition = definition;
        definition.cleanup_timeout_ms = 10;
        let cancel = flag();
        cancel.store(true, Ordering::Release);
        let facts = run(&definition, inputs(), &prepared, &mut runner, cancel)
            .await
            .unwrap();
        assert_eq!(facts.status, Status::Cancelled);
        assert_eq!(runner.calls, 0);
        assert!(facts.cleanup_error.unwrap().contains("unconfirmed"));
        assert_eq!(runner.cleanups, 1);
    }
    #[tokio::test]
    async fn skipped_required_flow_output_fails_instead_of_fabricating_a_value() {
        let mut definition = flow();
        definition.outputs.insert(
            "elapsed".into(),
            crate::flow::FlowOutput {
                value_type: crate::contracts::ValueType::Number,
                source: crate::flow::InputSource::StepOutput {
                    step: "pause".into(),
                    port: "elapsed_ms".into(),
                },
            },
        );
        let mut runner = runner();
        let facts = run(
            &definition,
            BTreeMap::from([("enabled".into(), json!(false))]),
            &BTreeMap::new(),
            &mut runner,
            flag(),
        )
        .await
        .unwrap();
        assert_eq!(facts.status, Status::Failed);
        assert!(facts.error.unwrap().contains("skipped"));
        assert!(facts.outputs.is_empty());
        assert_eq!(runner.cleanups, 1);
    }
    #[cfg(any(unix, windows))]
    #[tokio::test]
    async fn actual_protocol_process_bridge_cancels_reaps_descendant_and_removes_workspace() {
        use crate::protocol::{JsonRpcRequest, MidsceneProcess};
        use std::process::Stdio;
        let script = r#"
import subprocess,sys,json,time
child=subprocess.Popen([sys.executable,'-u','-c','import socket,time; s=socket.socket(); s.bind(("127.0.0.1",0)); s.listen(); print(s.getsockname()[1],flush=True); time.sleep(120)'],stdout=subprocess.PIPE,text=True)
port=int(child.stdout.readline())
for line in sys.stdin:
 req=json.loads(line)
 if req['method']=='currentUrl':
  print(json.dumps({'id':'0','ok':True,'data':{'port':port}}),flush=True)
 else:
  time.sleep(120)
"#;
        let mut command =
            tokio::process::Command::new(if cfg!(windows) { "python" } else { "python3" });
        command
            .args(["-u", "-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let (process, workspace) = MidsceneProcess::launch_owned_for_test(command)
            .await
            .unwrap();
        let port = process
            .call(JsonRpcRequest::CurrentUrl)
            .await
            .unwrap()
            .data
            .unwrap()["port"]
            .as_u64()
            .unwrap() as u16;
        assert!(std::net::TcpListener::bind(("127.0.0.1", port)).is_err());
        let mut executor = TestExecutor::from_process_for_test(process);
        let (mut definition, prepared) = case_flow();
        definition.steps[0].timeout_ms = 5000;
        definition.total_timeout_ms = 5000;
        definition.cleanup_timeout_ms = 6000;
        let cancel = flag();
        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            trigger.store(true, Ordering::Release);
        });
        let facts = run(&definition, inputs(), &prepared, &mut executor, cancel)
            .await
            .unwrap();
        assert_eq!(facts.status, Status::Cancelled);
        assert!(facts.cleanup_error.is_none(), "{:?}", facts.cleanup_error);
        assert!(
            !workspace.exists(),
            "owned workspace survived confirmed cleanup"
        );
        assert!(
            std::net::TcpListener::bind(("127.0.0.1", port)).is_ok(),
            "owned descendant retained its port"
        );
    }
    #[tokio::test]
    async fn oversized_failed_and_successful_case_results_are_omitted_and_cleaned_up() {
        for (passed, screenshot) in [(false, true), (false, false), (true, true)] {
            let (definition, prepared) = case_flow();
            let mut runner = runner();
            runner.passed = passed;
            let private = "private-runner-content".repeat(10_000);
            if screenshot {
                runner.screenshot = Some(private);
            } else {
                runner.error_override = Some(private);
            }
            let facts = run(&definition, inputs(), &prepared, &mut runner, flag())
                .await
                .unwrap();
            assert_eq!(facts.status, Status::Failed);
            assert!(facts.error.as_ref().unwrap().contains("evidence limit"));
            assert_eq!(runner.calls, 1);
            assert_eq!(runner.cleanups, 1);
            assert!(facts.steps[0].attempts[0].case_result.is_none());
            assert!(facts.steps[0].outputs.is_empty());
            let bytes = serde_json::to_vec(&facts).unwrap();
            assert!(bytes.len() < 4096);
            assert!(!String::from_utf8(bytes)
                .unwrap()
                .contains("private-runner-content"));
        }
    }
    #[tokio::test]
    async fn successful_screenshot_is_retained_only_in_attempt_evidence() {
        let (definition, prepared) = case_flow();
        let mut runner = runner();
        runner.screenshot = Some("bounded-screenshot".into());
        let facts = run(&definition, inputs(), &prepared, &mut runner, flag())
            .await
            .unwrap();
        assert_eq!(facts.status, Status::Succeeded);
        assert_eq!(
            facts.steps[0].attempts[0]
                .case_result
                .as_ref()
                .unwrap()
                .screenshot_base64
                .as_deref(),
            Some("bounded-screenshot")
        );
        assert!(facts.steps[0].outputs["result"]["screenshot_base64"].is_null());
        assert_eq!(
            serde_json::to_string(&facts)
                .unwrap()
                .matches("bounded-screenshot")
                .count(),
            1
        );
    }
    #[test]
    fn result_size_budget_counts_json_escaping_without_buffering_the_payload() {
        let result = TestResult {
            capture: None,
            case_id: "case".into(),
            passed: false,
            duration_ms: 0,
            screenshot_base64: None,
            error_message: Some("\n".repeat(70_000)),
            retry_count: 0,
            healed_yaml_script: None,
            failed_step: None,
        };
        assert!(validate_case_result_budget(&result).is_err());
    }
    #[tokio::test]
    async fn arbitrary_runner_and_cleanup_errors_have_bounded_independent_diagnostics() {
        let (definition, prepared) = case_flow();
        let mut runner = runner();
        runner.runner_error = Some("private-runner-body".repeat(100_000));
        runner.cleanup_error_override = Some("private-cleanup-body".repeat(100_000));
        let facts = run(&definition, inputs(), &prepared, &mut runner, flag())
            .await
            .unwrap();
        assert_eq!(facts.status, Status::Failed);
        assert!(facts.error.as_ref().unwrap().contains("4 KiB"));
        assert!(facts.cleanup_error.as_ref().unwrap().contains("4 KiB"));
        assert_eq!(runner.cleanups, 1);
        let serialized = serde_json::to_string(&facts).unwrap();
        assert!(serialized.len() < 4096);
        assert!(
            !serialized.contains("private-runner-body")
                && !serialized.contains("private-cleanup-body")
        );
        assert_eq!(diagnostic(&"x".repeat(4096)).len(), 4096);
        assert!(!diagnostic(&"界".repeat(2000)).contains('界'));
    }
}
