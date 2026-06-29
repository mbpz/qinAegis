// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! Pipeline orchestrator.
//!
//! Spawns the four reliability-loop workers (execute, heal, flaky, gate) as
//! a single `JoinSet`, plus a watchdog that cancels the shared
//! `CancellationToken` after a hard timeout. Each worker is independently
//! cancellable.

use crate::bus::ReliabilityBus;
use crate::events::ReliabilityEvent;
use crate::retry::retry_with_backoff_option;
use tokio::sync::broadcast::error::RecvError;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use serde::{Deserialize, Serialize};

const HEAL_MAX_ATTEMPTS: u8 = 2;
const GENERATE_MAX_ATTEMPTS: u8 = 3;
const RUN_WATCHDOG: Duration = Duration::from_secs(30 * 60);

pub struct PipelineConfig {
    pub heal_max_attempts: u8,
    pub generate_max_attempts: u8,
    pub watchdog: Duration,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            heal_max_attempts: HEAL_MAX_ATTEMPTS,
            generate_max_attempts: GENERATE_MAX_ATTEMPTS,
            watchdog: RUN_WATCHDOG,
        }
    }
}

/// Pipeline context — dependencies plumbed into the orchestrator and its
/// workers. `PipelineCtx::default()` is provided so unit tests don't have to
/// thread a real project root through.
pub struct PipelineCtx {
    /// Root under which the pipeline writes run artifacts. Healed YAML is
    /// persisted under `<project_root>/runs/<run_id>/healed/<case_id>.yaml`.
    pub project_root: PathBuf,
    /// Optional LLM-backed healer; when `None`, the heal stage emits
    /// `HealExhausted` immediately.
    pub llm: Option<Arc<crate::healer::Healer>>,
    /// Optional test-case generator — when `Some`, a Generate worker is
    /// spawned at pipeline start that calls
    /// `TestCaseGenerator::generate(spec_md, requirement)` once and publishes
    /// one `CaseGenerated` per generated case. When `None`, the pipeline
    /// expects `CaseGenerated` events from an external source.
    pub generator: Option<Arc<crate::generator::TestCaseGenerator>>,
    /// Optional critic — when `Some`, a Critic worker subscribes to
    /// `CaseGenerated`, calls `Critic::review(yaml, spec_md, requirement)`,
    /// re-publishes the event with `critic_score` set if the score meets the
    /// pass threshold (>= 70), otherwise emits `CriticRejected`. When `None`,
    /// the pipeline passes `CaseGenerated` straight through.
    pub critic: Option<Arc<crate::critic::Critic>>,
    /// Markdown spec shared with the generator and critic workers. Used as
    /// input to `TestCaseGenerator::generate` and `Critic::review`.
    pub spec_md: String,
    /// Requirement text shared with the generator and critic workers. Used
    /// as input to `TestCaseGenerator::generate` and `Critic::review`.
    pub requirement: String,
    /// Path to the persistent flaky history file. Loaded on pipeline start,
    /// written on `RunFinished`. Default is a relative `flaky_history.json`
    /// in the current working directory.
    pub flaky_history_path: PathBuf,
    /// Identifier for the current pipeline run. Threaded into every
    /// `CaseExecuted` event published by `execute_loop` so downstream
    /// consumers (flaky_loop aggregation, `RunFinished` correlation) can
    /// group outcomes by run. Default is `"current"` for tests and ad-hoc
    /// invocations that don't pass a real id.
    pub run_id: String,
}

impl Default for PipelineCtx {
    fn default() -> Self {
        Self {
            project_root: PathBuf::from("."),
            llm: None,
            generator: None,
            critic: None,
            spec_md: String::new(),
            requirement: String::new(),
            flaky_history_path: PathBuf::from("flaky_history.json"),
            run_id: "current".into(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CaseMeta {
    #[serde(default)]
    pub heal_attempts: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flaky_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_score: Option<u8>,
}

#[derive(Debug, Default)]
pub struct MetaCollector {
    inner: Mutex<HashMap<String, CaseMeta>>,
}

impl MetaCollector {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn record_heal_attempt(&self, case_id: &str, attempts: u8) {
        let mut g = self.inner.lock().unwrap();
        g.entry(case_id.to_string()).or_default().heal_attempts = attempts;
    }
    pub fn record_flaky_score(&self, case_id: &str, score: f64) {
        let mut g = self.inner.lock().unwrap();
        g.entry(case_id.to_string()).or_default().flaky_score = Some(score);
    }
    pub fn record_critic_score(&self, case_id: &str, score: u8) {
        let mut g = self.inner.lock().unwrap();
        g.entry(case_id.to_string()).or_default().critic_score = Some(score);
    }
    pub fn snapshot(&self) -> HashMap<String, CaseMeta> {
        self.inner.lock().unwrap().clone()
    }
}

pub async fn run(bus: ReliabilityBus, ctx: PipelineCtx, cancel: CancellationToken) -> anyhow::Result<()> {
    let _cfg = PipelineConfig::default();
    let mut set: JoinSet<anyhow::Result<()>> = JoinSet::new();
    let meta = Arc::new(MetaCollector::new());

    // Load (or initialize) persistent flaky history once, share across stages.
    // ponytail: failures here are non-fatal — log and start empty so a
    // corrupt or unreadable file never blocks the pipeline.
    let flaky_history = match crate::flaky::FlakyHistory::load(&ctx.flaky_history_path) {
        Ok(h) => std::sync::Arc::new(std::sync::Mutex::new(h)),
        Err(e) => {
            tracing::warn!(
                "FlakyHistory load failed for {}: {} — starting empty",
                ctx.flaky_history_path.display(),
                e,
            );
            std::sync::Arc::new(std::sync::Mutex::new(crate::flaky::FlakyHistory::empty(
                &ctx.flaky_history_path,
            )))
        }
    };

    // Watchdog — fires cancellation after timeout.
    {
        let cancel = cancel.clone();
        set.spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(RUN_WATCHDOG) => {
                    tracing::warn!("pipeline watchdog fired");
                    cancel.cancel();
                }
                _ = cancel.cancelled() => {
                    // pipeline finished or was cancelled before the watchdog fired
                }
            }
            Ok(())
        });
    }

    // Execute stage: receives CaseGenerated and HealProduced, runs case, emits CaseExecuted.
    let executor = match crate::executor::TestExecutor::new(2, None, None).await {
        Ok(e) => std::sync::Arc::new(e),
        Err(err) => {
            tracing::error!("TestExecutor init failed: {} — execute stage will be a no-op", err);
            // ponytail: don't fail the whole pipeline; spawn a stub that just drops events.
            let bus2 = bus.clone();
            let cancel2 = cancel.clone();
            set.spawn(async move { execute_loop_stub(bus2, cancel2).await });
            return drain(set).await;
        }
    };
    {
        let bus = bus.clone();
        let cancel = cancel.clone();
        let exec = executor.clone();
        let run_id = ctx.run_id.clone();
        let meta = meta.clone();
        set.spawn(async move { execute_loop(bus, cancel, exec, run_id, meta).await });
    }

    // Heal stage: receives CaseExecuted(passed:false), produces HealProduced or HealExhausted.
    {
        let bus = bus.clone();
        let cancel = cancel.clone();
        let h = ctx.llm.clone();
        let root = ctx.project_root.clone();
        let meta = meta.clone();
        set.spawn(async move { heal_loop(bus, cancel, h, root, meta).await });
    }

    // FlakyTracker stage: receives CaseExecuted, publishes FlakyDetected/CaseQuarantined.
    {
        let bus = bus.clone();
        let cancel = cancel.clone();
        let history = flaky_history.clone();
        let meta = meta.clone();
        set.spawn(async move { flaky_loop(bus, cancel, history, meta).await });
    }

    // MetaWriter stage: on RunFinished, merges MetaCollector snapshot into
    // <project_root>/runs/<run_id>/summary.json as a "pipeline_meta" object,
    // preserving any pre-existing fields (passed, total, etc.).
    {
        let bus = bus.clone();
        let cancel = cancel.clone();
        let meta = meta.clone();
        let root = ctx.project_root.clone();
        set.spawn(async move { meta_writer_loop(bus, cancel, meta, root).await });
    }

    // GateLoop stage: receives RunFinished, runs GateService.
    {
        let bus = bus.clone();
        let cancel = cancel.clone();
        set.spawn(async move {
            // GateService constructed with default thresholds; real wiring later.
            use crate::gate::{GateService, GateThresholds};
            let svc = GateService::new("default".into(), None, GateThresholds::default());
            crate::gate_loop::GateLoop::run(bus, svc, cancel).await
        });
    }

    // Generate worker (once at start): calls TestCaseGenerator::generate and
    // publishes one CaseGenerated per generated case.
    if let Some(generator) = &ctx.generator {
        let bus = bus.clone();
        let cancel = cancel.clone();
        let generator = generator.clone();
        let spec_md = ctx.spec_md.clone();
        let requirement = ctx.requirement.clone();
        set.spawn(async move {
            generate_loop(bus, cancel, generator, spec_md, requirement).await
        });
    }

    // Critic worker (subscribes to CaseGenerated, scores + re-publishes).
    if let Some(critic) = &ctx.critic {
        let bus = bus.clone();
        let cancel = cancel.clone();
        let critic = critic.clone();
        let spec_md = ctx.spec_md.clone();
        let requirement = ctx.requirement.clone();
        set.spawn(async move {
            critic_loop(bus, cancel, critic, spec_md, requirement, GENERATE_MAX_ATTEMPTS).await
        });
    }

    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::error!("worker exited with error: {}", e),
            Err(je) => tracing::error!("worker panicked: {}", je),
        }
    }
    Ok(())
}

async fn execute_loop(
    bus: ReliabilityBus,
    cancel: CancellationToken,
    executor: std::sync::Arc<crate::executor::TestExecutor>,
    run_id: String,
    meta: Arc<MetaCollector>,
) -> anyhow::Result<()> {
    let mut rx = bus.subscribe();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            res = rx.recv() => match res {
                Ok(ReliabilityEvent::CaseGenerated { case_id, yaml, critic_score, .. }) => {
                    meta.record_critic_score(&case_id, critic_score.unwrap_or(0));
                    let case = crate::executor::TestCaseRef {
                        id: case_id.clone(),
                        yaml_script: yaml.clone(),
                        name: case_id.clone(),
                        priority: "p2".into(),
                        target_url: None,
                    };
                    let r = executor.run_case(&case).await?;
                    bus.publish(ReliabilityEvent::CaseExecuted {
                        case_id: r.case_id,
                        run_id: run_id.clone(),
                        passed: r.passed,
                        error: r.error_message,
                        failed_step: r.failed_step,
                        yaml,
                    });
                }
                Ok(ReliabilityEvent::HealProduced { case_id, healed_yaml, .. }) => {
                    let case = crate::executor::TestCaseRef {
                        id: case_id.clone(),
                        yaml_script: healed_yaml.clone(),
                        name: case_id.clone(),
                        priority: "p2".into(),
                        target_url: None,
                    };
                    let r = executor.run_case(&case).await?;
                    bus.publish(ReliabilityEvent::CaseExecuted {
                        case_id: r.case_id,
                        run_id: run_id.clone(),
                        passed: r.passed,
                        error: r.error_message,
                        failed_step: r.failed_step,
                        yaml: healed_yaml,
                    });
                }
                Ok(_) => continue,
                Err(RecvError::Lagged(n)) => tracing::warn!("execute_loop lagged {}", n),
                Err(e) => return Err(e.into()),
            }
        }
    }
}

async fn heal_loop(
    bus: ReliabilityBus,
    cancel: CancellationToken,
    healer: Option<std::sync::Arc<crate::healer::Healer>>,
    project_root: PathBuf,
    meta: Arc<MetaCollector>,
) -> anyhow::Result<()> {
    use std::collections::HashMap;
    let max_attempts = HEAL_MAX_ATTEMPTS;
    let mut rx = bus.subscribe();
    let mut attempts: HashMap<String, u8> = HashMap::new();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            res = rx.recv() => match res {
                Ok(ReliabilityEvent::CaseExecuted { case_id, run_id, passed, error, failed_step, yaml, .. }) if !passed => {
                    let err = error.clone().unwrap_or_else(|| "unknown".into());
                    let step = failed_step.unwrap_or_else(|| "unknown".into());
                    let n = attempts.get(&case_id).copied().unwrap_or(0);
                    // Audit-trail publish: emit HealRequested before invoking the
                    // healer (or declaring exhaustion) so subscribers observe the
                    // request as soon as it's made, regardless of outcome.
                    // The original YAML is carried in CaseExecuted (forwarded from
                    // CaseGenerated/HealProduced by execute_loop) so the audit
                    // trail records exactly what was run.
                    bus.publish(ReliabilityEvent::HealRequested {
                        case_id: case_id.clone(),
                        original_yaml: yaml.clone(),
                        error: err.clone(),
                        failed_step: step.clone(),
                        attempt: n + 1,
                    });
                    if n >= max_attempts {
                        bus.publish(ReliabilityEvent::HealExhausted {
                            case_id: case_id.clone(), attempts: n, last_error: err,
                        });
                        meta.record_heal_attempt(&case_id, n);
                        continue;
                    }
                    let next_n = n + 1;
                    if let Some(h) = &healer {
                        // Wrap the LLM call in a small backoff loop so a
                        // transient 429 / network blip doesn't immediately
                        // mark a case as HealExhausted. 3 attempts, 100..2000ms.
                        let heal_result = retry_with_backoff_option(
                            3,
                            || 100u64..2000u64,
                            || h.heal("", &step, &err),
                        )
                        .await;
                        match heal_result {
                            Some(yaml) => {
                                attempts.insert(case_id.clone(), next_n);
                                meta.record_heal_attempt(&case_id, next_n);
                                // Persist healed YAML before publishing so subscribers
                                // can assume the file exists by the time they observe
                                // the event. Best-effort: log on failure but do not
                                // fail the loop — the YAML is recoverable by re-running
                                // the healer.
                                let store = crate::healed_store::HealedStore::new(project_root.clone());
                                if let Err(e) = store.write(&run_id, &case_id, &yaml) {
                                    tracing::warn!(
                                        "failed to persist healed YAML for case {} in run {}: {}",
                                        case_id, run_id, e,
                                    );
                                }
                                bus.publish(ReliabilityEvent::HealProduced {
                                    case_id, healed_yaml: yaml, attempt: next_n,
                                });
                            }
                            None => {
                                attempts.insert(case_id.clone(), next_n);
                                meta.record_heal_attempt(&case_id, next_n);
                                bus.publish(ReliabilityEvent::HealExhausted {
                                    case_id, attempts: next_n, last_error: err,
                                });
                            }
                        }
                    } else {
                        meta.record_heal_attempt(&case_id, n);
                        bus.publish(ReliabilityEvent::HealExhausted {
                            case_id, attempts: n, last_error: err,
                        });
                    }
                }
                Ok(_) => continue,
                Err(RecvError::Lagged(n)) => tracing::warn!("heal_loop lagged {}", n),
                Err(e) => return Err(e.into()),
            }
        }
    }
}

/// Test-only re-export of `heal_loop` that omits the `project_root` parameter
/// (only used for healed-YAML persistence, which is irrelevant for tests that
/// don't run a real healer).
pub(crate) async fn heal_loop_for_test(
    bus: ReliabilityBus,
    cancel: CancellationToken,
    healer: Option<std::sync::Arc<crate::healer::Healer>>,
    meta: Arc<MetaCollector>,
) -> anyhow::Result<()> {
    heal_loop(bus, cancel, healer, std::path::PathBuf::from("."), meta).await
}

/// Generate worker: calls `TestCaseGenerator::generate(spec_md, requirement)`
/// once at pipeline start and publishes one `CaseGenerated` per generated
/// case. The Critic worker (if present) picks them up downstream to score
/// them; otherwise downstream consumers see `critic_score = None`.
async fn generate_loop(
    bus: ReliabilityBus,
    cancel: CancellationToken,
    generator: Arc<crate::generator::TestCaseGenerator>,
    spec_md: String,
    requirement: String,
) -> anyhow::Result<()> {
    let cases = match generator.generate(&spec_md, &requirement).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("generate_loop failed: {}", e);
            return Ok(());
        }
    };
    for case in cases {
        if cancel.is_cancelled() {
            return Ok(());
        }
        bus.publish(ReliabilityEvent::CaseGenerated {
            case_id: case.id.clone(),
            yaml: case.yaml_script.clone(),
            // Critic will fill this in if a Critic worker is wired up;
            // downstream stages treat `None` as "not yet scored".
            critic_score: None,
        });
    }
    Ok(())
}

/// Critic worker: subscribes to `CaseGenerated`, scores each case via
/// `Critic::review`, and re-publishes with `critic_score` set when the score
/// meets the pass threshold (>= 70). Cases that fail to clear the threshold
/// are emitted as `CriticRejected` up to `max_attempts` times before being
/// dropped (the orchestrator doesn't currently re-trigger generation on
/// rejection — that loop is left to a future follow-up).
async fn critic_loop(
    bus: ReliabilityBus,
    cancel: CancellationToken,
    critic: Arc<crate::critic::Critic>,
    spec_md: String,
    requirement: String,
    max_attempts: u8,
) -> anyhow::Result<()> {
    use std::collections::HashMap;
    let mut attempts: HashMap<String, u8> = HashMap::new();
    let mut rx = bus.subscribe();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            res = rx.recv() => match res {
                Ok(ReliabilityEvent::CaseGenerated { case_id, yaml, critic_score }) => {
                    // Only score cases the generator emitted without a score.
                    // A pre-scored event signals that someone else (e.g. an
                    // upstream service) already evaluated it — pass it through
                    // untouched.
                    if critic_score.is_some() {
                        continue;
                    }
                    let review = match critic.review(&yaml, &spec_md, &requirement).await {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::warn!("critic_loop review failed for {}: {}", case_id, e);
                            continue;
                        }
                    };
                    if review.score >= 70 {
                        bus.publish(ReliabilityEvent::CaseGenerated {
                            case_id,
                            yaml,
                            critic_score: Some(review.score),
                        });
                    } else {
                        let n = attempts.get(&case_id).copied().unwrap_or(0);
                        if n >= max_attempts {
                            bus.publish(ReliabilityEvent::CriticRejected {
                                case_id: case_id.clone(),
                                reason: format!("score {} < 70 after {} attempts", review.score, n),
                                attempt: n,
                            });
                            attempts.remove(&case_id);
                        } else {
                            let next_n = n + 1;
                            attempts.insert(case_id.clone(), next_n);
                            bus.publish(ReliabilityEvent::CriticRejected {
                                case_id,
                                reason: format!("score {} < 70", review.score),
                                attempt: next_n,
                            });
                        }
                    }
                }
                Ok(_) => continue,
                Err(RecvError::Lagged(n)) => tracing::warn!("critic_loop lagged {}", n),
                Err(e) => return Err(e.into()),
            }
        }
    }
}

async fn flaky_loop(
    bus: ReliabilityBus,
    cancel: CancellationToken,
    history: Arc<std::sync::Mutex<crate::flaky::FlakyHistory>>,
    meta: Arc<MetaCollector>,
) -> anyhow::Result<()> {
    use std::collections::HashMap;
    use std::sync::Arc;
    let mut rx = bus.subscribe();
    let mut windows: HashMap<String, Arc<crate::flaky::FlakyWindow>> = HashMap::new();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            res = rx.recv() => match res {
                Ok(ReliabilityEvent::CaseExecuted { case_id, passed, .. }) => {
                    // Update in-memory window (drives FlakyTracker decisions).
                    let w = windows
                        .entry(case_id.clone())
                        .or_insert_with(|| Arc::new(crate::flaky::FlakyWindow::new(10)))
                        .clone();
                    w.record(passed);
                    // Persist outcome to disk-backed history.
                    if let Ok(mut h) = history.lock() {
                        h.record(&case_id, passed);
                    }
                    if w.is_flaky() {
                        meta.record_flaky_score(&case_id, w.flip_rate());
                    }
                    crate::flaky::FlakyTracker::process(w, &case_id, &bus).await;
                }
                Ok(ReliabilityEvent::RunFinished { .. }) => {
                    // Flush history to disk at the end of each run so
                    // cross-run trend data survives a crash or restart.
                    if let Ok(h) = history.lock() {
                        if let Err(e) = h.save() {
                            tracing::warn!("FlakyHistory save failed: {}", e);
                        }
                    }
                }
                Ok(_) => continue,
                Err(RecvError::Lagged(n)) => tracing::warn!("flaky_loop lagged {}", n),
                Err(e) => return Err(e.into()),
            }
        }
    }
}

async fn execute_loop_stub(bus: ReliabilityBus, cancel: CancellationToken) -> anyhow::Result<()> {
    let mut rx = bus.subscribe();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            _ = rx.recv() => continue,
        }
    }
}

async fn drain(mut set: JoinSet<anyhow::Result<()>>) -> anyhow::Result<()> {
    while let Some(r) = set.join_next().await {
        if let Err(e) = r? { tracing::error!("worker error: {}", e); }
    }
    Ok(())
}

pub fn write_summary_with_meta(path: &Path, meta: &HashMap<String, CaseMeta>) -> anyhow::Result<()> {
    use serde_json::Value;
    let mut summary: Value = if path.exists() {
        let s = std::fs::read_to_string(path)?;
        serde_json::from_str(&s).unwrap_or_else(|_| Value::Object(Default::default()))
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Value::Object(Default::default())
    };
    if let Some(obj) = summary.as_object_mut() {
        let meta_value = serde_json::to_value(meta)?;
        obj.insert("pipeline_meta".to_string(), meta_value);
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&summary)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

async fn meta_writer_loop(
    bus: ReliabilityBus,
    cancel: CancellationToken,
    meta: Arc<MetaCollector>,
    project_root: PathBuf,
) -> anyhow::Result<()> {
    let mut rx = bus.subscribe();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            res = rx.recv() => match res {
                Ok(ReliabilityEvent::RunFinished { run_id }) => {
                    let snapshot = meta.snapshot();
                    let summary_path = project_root.join("runs").join(&run_id).join("summary.json");
                    if let Err(e) = write_summary_with_meta(&summary_path, &snapshot) {
                        tracing::warn!("MetaWriter failed to write {}: {}", summary_path.display(), e);
                    }
                }
                Ok(_) => continue,
                Err(RecvError::Lagged(n)) => tracing::warn!("meta_writer lagged {}", n),
                Err(e) => return Err(e.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::ReliabilityBus;
    use crate::events::ReliabilityEvent;
    use tempfile::tempdir;

    /// Wait until the bus has at least `n` subscribers attached.
    ///
    /// Replaces fragile `tokio::time::sleep` rendezvous: polls
    /// `subscriber_count()` at 1ms granularity until the expected number
    /// of workers is listening, with a hard cap that panics loudly if
    /// the count is never reached (typically means a worker panicked
    /// before subscribing).
    async fn wait_for_subscribers(bus: &ReliabilityBus, n: usize, max: std::time::Duration) {
        let start = std::time::Instant::now();
        loop {
            if bus.subscriber_count() >= n {
                return;
            }
            if start.elapsed() > max {
                panic!(
                    "bus subscribers never reached {} (got {} after {:?})",
                    n,
                    bus.subscriber_count(),
                    start.elapsed(),
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    }

    #[tokio::test]
    async fn pipeline_runs_to_gate_decided() {
        // The orchestrator currently hardcodes project "default" for its
        // GateService, so we must scaffold the run dir under the same name.
        let project = "default";
        let run_id = "pipeline-r1";
        let dir = crate::test_support::scaffold_run_dir(project, run_id);

        let bus = ReliabilityBus::new(64);
        let mut rx = bus.subscribe();
        // Give the spawned task a moment to subscribe before publishing,
        // otherwise our pre-published events may be missed by the broadcast
        // (subscribers created after publish don't have any history).
        let cancel = CancellationToken::new();
        let bus_for_task = bus.clone();
        let cancel_for_task = cancel.clone();
        let handle = tokio::spawn(async move {
            super::run(bus_for_task, super::PipelineCtx::default(), cancel_for_task).await
        });
        // Deterministically wait for all 5 workers (execute + heal + flaky +
        // gate + meta_writer) to subscribe before publishing. The test's own
        // rx is already counted in `subscriber_count()`, so we wait for 6
        // total (1 test + 5 workers).
        wait_for_subscribers(&bus, 6, std::time::Duration::from_secs(2)).await;

        // Pre-publish events that simulate a clean single-case run.
        // Note: we DO NOT pre-publish CaseGenerated here because Task 10
        // wired the real TestExecutor::run_case into execute_loop, which
        // would spawn the TS sandbox subprocess on a fake YAML and slow
        // (or fail) the test. The orchestrator's GateLoop only needs
        // RunFinished + a passing CaseExecuted summary to fire GateDecided.
        bus.publish(ReliabilityEvent::CaseExecuted {
            case_id: "c1".into(),
            run_id: run_id.into(),
            passed: true,
            error: None,
            failed_step: None,
            yaml: "test-yaml".into(),
        });
        bus.publish(ReliabilityEvent::RunFinished { run_id: "pipeline-r1".into() });

        // Wait for GateDecided or timeout.
        let ev = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                match rx.recv().await.unwrap() {
                    ReliabilityEvent::GateDecided { .. } => break,
                    _ => continue,
                }
            }
        })
        .await;
        assert!(ev.is_ok(), "pipeline never produced GateDecided");
        cancel.cancel();
        let _ = handle.await;

        // Best-effort cleanup so repeated runs don't litter `~/.qinAegis`.
        let _ = std::fs::remove_dir_all(dir);
        crate::test_support::cleanup_project(project);
    }

    #[tokio::test]
    async fn heal_loop_publishes_heal_requested_before_healing() {
        use crate::events::ReliabilityEvent;

        let bus = ReliabilityBus::new(64);
        let mut rx = bus.subscribe();
        let cancel = CancellationToken::new();

        // Spawn heal_loop directly (no orchestrator).
        let bus_c = bus.clone();
        let cancel_c = cancel.clone();
        let h = tokio::spawn(async move {
            super::heal_loop_for_test(bus_c, cancel_c, None, std::sync::Arc::new(super::MetaCollector::new())).await
        });
        // Deterministically wait for the worker to subscribe (test rx + worker = 2)
        // instead of a fragile fixed sleep.
        wait_for_subscribers(&bus, 2, std::time::Duration::from_secs(2)).await;

        // Publish a failed CaseExecuted.
        bus.publish(ReliabilityEvent::CaseExecuted {
            case_id: "c1".into(),
            run_id: "r1".into(),
            passed: false,
            error: Some("boom".into()),
            failed_step: Some("aiAct: click X".into()),
            yaml: "test-yaml".into(),
        });

        // Drain for HealRequested.
        let got = tokio::time::timeout(std::time::Duration::from_millis(500), async {
            loop {
                match rx.recv().await {
                    Ok(ReliabilityEvent::HealRequested { case_id, .. }) => break case_id,
                    _ => continue,
                }
            }
        }).await;
        assert_eq!(got, Ok("c1".into()));
        cancel.cancel();
        let _ = h.await;
    }

    #[tokio::test]
    async fn heal_requested_carries_original_yaml() {
        let bus = ReliabilityBus::new(64);
        let mut rx = bus.subscribe();
        let cancel = CancellationToken::new();

        let bus_c = bus.clone();
        let cancel_c = cancel.clone();
        let h = tokio::spawn(async move {
            super::heal_loop_for_test(bus_c, cancel_c, None, std::sync::Arc::new(super::MetaCollector::new())).await
        });
        wait_for_subscribers(&bus, 2, std::time::Duration::from_secs(2)).await;

        let original = "target:\n  url: https://x\ntasks: []".to_string();
        bus.publish(ReliabilityEvent::CaseExecuted {
            case_id: "c1".into(),
            run_id: "r1".into(),
            passed: false,
            error: Some("boom".into()),
            failed_step: Some("aiAct: click X".into()),
            yaml: original.clone(),
        });

        let got = tokio::time::timeout(std::time::Duration::from_millis(500), async {
            loop {
                match rx.recv().await {
                    Ok(ReliabilityEvent::HealRequested { original_yaml, .. }) => break original_yaml,
                    _ => continue,
                }
            }
        }).await;
        assert_eq!(got, Ok(original));
        cancel.cancel();
        let _ = h.await;
    }

    #[test]
    fn write_summary_with_meta_preserves_existing_fields() {
        use std::collections::HashMap;
        let dir = tempdir().unwrap();
        let run_dir = dir.path().join("runs/r1");
        std::fs::create_dir_all(&run_dir).unwrap();
        let summary_path = run_dir.join("summary.json");
        std::fs::write(&summary_path, r#"{"passed": 5, "total": 5}"#).unwrap();

        let mut meta_map = HashMap::new();
        meta_map.insert("c1".to_string(), CaseMeta {
            heal_attempts: 2,
            flaky_score: Some(0.5),
            critic_score: Some(80),
        });
        write_summary_with_meta(&summary_path, &meta_map).unwrap();

        let body = std::fs::read_to_string(&summary_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["passed"], 5);
        assert_eq!(json["total"], 5);
        assert_eq!(json["pipeline_meta"]["c1"]["heal_attempts"], 2);
        assert_eq!(json["pipeline_meta"]["c1"]["flaky_score"], 0.5);
        assert_eq!(json["pipeline_meta"]["c1"]["critic_score"], 80);
    }

    #[test]
    fn write_summary_with_meta_creates_file_if_missing() {
        use std::collections::HashMap;
        let dir = tempdir().unwrap();
        let summary_path = dir.path().join("runs/r1/summary.json");

        let mut meta_map = HashMap::new();
        meta_map.insert("c1".to_string(), CaseMeta {
            heal_attempts: 0, flaky_score: None, critic_score: Some(90),
        });
        write_summary_with_meta(&summary_path, &meta_map).unwrap();

        let body = std::fs::read_to_string(&summary_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["pipeline_meta"]["c1"]["critic_score"], 90);
    }

    #[tokio::test]
    async fn generate_loop_publishes_case_generated_per_case() {
        use crate::generator::TestCaseGenerator;
        use crate::llm::TestLlm;

        // TestLlm returns a canned JSON array with 2 test cases.
        let canned = r#"[
          {"id":"c1","name":"case1","requirement_id":"r1","type":"e2e","priority":"p1","yaml_script":"target:\n  url: x","expected_result":"ok","tags":[]},
          {"id":"c2","name":"case2","requirement_id":"r1","type":"e2e","priority":"p2","yaml_script":"target:\n  url: y","expected_result":"ok","tags":[]}
        ]"#;
        let llm = crate::llm::ArcLlmClient::new(TestLlm::new(move |_msgs| {
            Ok(canned.to_string())
        }));
        let generator = Arc::new(TestCaseGenerator::new(llm));

        let bus = ReliabilityBus::new(64);
        let mut rx = bus.subscribe();
        let cancel = CancellationToken::new();

        let bus_c = bus.clone();
        let cancel_c = cancel.clone();
        let gen_c = generator.clone();
        let h = tokio::spawn(async move {
            super::generate_loop(bus_c, cancel_c, gen_c, "spec".into(), "req".into()).await
        });
        // generate_loop publishes synchronously inside the closure; just
        // await its completion before draining the receiver.
        h.await.unwrap().unwrap();

        // Collect all CaseGenerated events.
        let mut received = Vec::new();
        while let Ok(ReliabilityEvent::CaseGenerated { case_id, .. }) =
            tokio::time::timeout(
                std::time::Duration::from_millis(200),
                rx.recv(),
            )
            .await
            .unwrap_or(Err(tokio::sync::broadcast::error::RecvError::Closed))
        {
            received.push(case_id);
        }
        received.sort();
        assert_eq!(
            received,
            vec!["c1".to_string(), "c2".to_string()],
            "expected generate_loop to publish one CaseGenerated per case",
        );
        cancel.cancel();
    }

    #[tokio::test]
    async fn critic_loop_scores_and_passes_through() {
        use crate::critic::Critic;
        use crate::llm::TestLlm;

        // TestLlm returns a CriticReview JSON with score 85 (>= 70 threshold).
        let canned = r#"{"score":85,"issues":[],"suggestions":[],"coverage":"good"}"#;
        let llm = crate::llm::ArcLlmClient::new(TestLlm::new(move |_msgs| {
            Ok(canned.to_string())
        }));
        let critic = Arc::new(Critic::new(llm));

        let bus = ReliabilityBus::new(64);
        let mut rx = bus.subscribe();
        let cancel = CancellationToken::new();

        let bus_c = bus.clone();
        let cancel_c = cancel.clone();
        let critic_c = critic.clone();
        let h = tokio::spawn(async move {
            super::critic_loop(bus_c, cancel_c, critic_c, "spec".into(), "req".into(), 3).await
        });
        wait_for_subscribers(&bus, 2, std::time::Duration::from_secs(2)).await;

        bus.publish(ReliabilityEvent::CaseGenerated {
            case_id: "c1".into(),
            yaml: "target: x".into(),
            critic_score: None,
        });

        // Critic should re-publish with score 85.
        let got = tokio::time::timeout(std::time::Duration::from_millis(500), async {
            loop {
                match rx.recv().await {
                    Ok(ReliabilityEvent::CaseGenerated {
                        case_id,
                        critic_score,
                        ..
                    }) if critic_score.is_some() => break (case_id, critic_score.unwrap()),
                    _ => continue,
                }
            }
        })
        .await;
        assert_eq!(got, Ok(("c1".into(), 85u8)));
        cancel.cancel();
        let _ = h.await;
    }
}
