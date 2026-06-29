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
use tokio::sync::broadcast::error::RecvError;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

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
    /// Path to the persistent flaky history file. Loaded on pipeline start,
    /// written on `RunFinished`. Default is a relative `flaky_history.json`
    /// in the current working directory.
    pub flaky_history_path: PathBuf,
}

impl Default for PipelineCtx {
    fn default() -> Self {
        Self {
            project_root: PathBuf::from("."),
            llm: None,
            flaky_history_path: PathBuf::from("flaky_history.json"),
        }
    }
}

pub async fn run(bus: ReliabilityBus, ctx: PipelineCtx, cancel: CancellationToken) -> anyhow::Result<()> {
    let _cfg = PipelineConfig::default();
    let mut set: JoinSet<anyhow::Result<()>> = JoinSet::new();

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
        set.spawn(async move { execute_loop(bus, cancel, exec).await });
    }

    // Heal stage: receives CaseExecuted(passed:false), produces HealProduced or HealExhausted.
    {
        let bus = bus.clone();
        let cancel = cancel.clone();
        let h = ctx.llm.clone();
        let root = ctx.project_root.clone();
        set.spawn(async move { heal_loop(bus, cancel, h, root).await });
    }

    // FlakyTracker stage: receives CaseExecuted, publishes FlakyDetected/CaseQuarantined.
    {
        let bus = bus.clone();
        let cancel = cancel.clone();
        let history = flaky_history.clone();
        set.spawn(async move { flaky_loop(bus, cancel, history).await });
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
) -> anyhow::Result<()> {
    let mut rx = bus.subscribe();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            res = rx.recv() => match res {
                Ok(ReliabilityEvent::CaseGenerated { case_id, yaml, .. }) => {
                    let case = crate::executor::TestCaseRef {
                        id: case_id.clone(),
                        yaml_script: yaml,
                        name: case_id.clone(),
                        priority: "p2".into(),
                        target_url: None,
                    };
                    let r = executor.run_case(&case).await?;
                    bus.publish(ReliabilityEvent::CaseExecuted {
                        case_id: r.case_id,
                        run_id: "r-current".into(),
                        passed: r.passed,
                        error: r.error_message,
                        failed_step: r.failed_step,
                    });
                }
                Ok(ReliabilityEvent::HealProduced { case_id, healed_yaml, .. }) => {
                    let case = crate::executor::TestCaseRef {
                        id: case_id.clone(),
                        yaml_script: healed_yaml,
                        name: case_id.clone(),
                        priority: "p2".into(),
                        target_url: None,
                    };
                    let r = executor.run_case(&case).await?;
                    bus.publish(ReliabilityEvent::CaseExecuted {
                        case_id: r.case_id,
                        run_id: "r-current".into(),
                        passed: r.passed,
                        error: r.error_message,
                        failed_step: r.failed_step,
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
) -> anyhow::Result<()> {
    use std::collections::HashMap;
    let max_attempts = HEAL_MAX_ATTEMPTS;
    let mut rx = bus.subscribe();
    let mut attempts: HashMap<String, u8> = HashMap::new();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            res = rx.recv() => match res {
                Ok(ReliabilityEvent::CaseExecuted { case_id, run_id, passed, error, failed_step, .. }) if !passed => {
                    let err = error.clone().unwrap_or_else(|| "unknown".into());
                    let step = failed_step.unwrap_or_else(|| "unknown".into());
                    let n = attempts.get(&case_id).copied().unwrap_or(0);
                    if n >= max_attempts {
                        bus.publish(ReliabilityEvent::HealExhausted {
                            case_id: case_id.clone(), attempts: n, last_error: err,
                        });
                        continue;
                    }
                    let next_n = n + 1;
                    if let Some(h) = &healer {
                        match h.heal("", &step, &err).await {
                            Some(yaml) => {
                                attempts.insert(case_id.clone(), next_n);
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
                                bus.publish(ReliabilityEvent::HealExhausted {
                                    case_id, attempts: next_n, last_error: err,
                                });
                            }
                        }
                    } else {
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

async fn flaky_loop(
    bus: ReliabilityBus,
    cancel: CancellationToken,
    history: Arc<std::sync::Mutex<crate::flaky::FlakyHistory>>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::ReliabilityBus;
    use crate::events::ReliabilityEvent;

    /// Build a minimal but valid run directory under `~/.qinAegis/projects/<proj>/reports/<run_id>`
    /// so that `GateService::check()` succeeds without touching the network.
    ///
    /// Returns the canonical run directory path.
    fn scaffold_run_dir(project: &str, run_id: &str) -> std::path::PathBuf {
        let dir = crate::storage::LocalStorage::run_dir(project, run_id);
        std::fs::create_dir_all(&dir).unwrap();

        // summary.json with one passing case
        std::fs::write(
            dir.join("summary.json"),
            r#"{"passed":1,"total":1}"#,
        )
        .unwrap();

        // lighthouse.json with passing metrics
        std::fs::write(
            dir.join("lighthouse.json"),
            r#"{
                "url": "https://example.com",
                "score": 1.0,
                "metrics": {
                    "performance": 1.0,
                    "accessibility": 1.0,
                    "best_practices": 1.0,
                    "seo": 1.0,
                    "first_contentful_paint": 500.0,
                    "largest_contentful_paint": 1000.0,
                    "cumulative_layout_shift": 0.05,
                    "total_blocking_time": 50.0,
                    "speed_index": 500.0,
                    "ttfb": 50.0
                },
                "timestamp": "0",
                "report_path": null
            }"#,
        )
        .unwrap();

        // locust-summary.json with passing stress numbers
        std::fs::write(
            dir.join("locust-summary.json"),
            r#"{
                "target_url": "https://example.com",
                "stats": {
                    "total_requests": 1000,
                    "total_failures": 0,
                    "median_response_time": 50.0,
                    "avg_response_time": 60.0,
                    "p95_response_time": 150.0,
                    "p99_response_time": 200.0,
                    "rps": 200.0,
                    "duration": 30.0
                },
                "timestamp": "0",
                "errors": []
            }"#,
        )
        .unwrap();

        dir
    }

    #[tokio::test]
    async fn pipeline_runs_to_gate_decided() {
        // The orchestrator currently hardcodes project "default" for its
        // GateService, so we must scaffold the run dir under the same name.
        let project = "default";
        let run_id = "pipeline-r1";
        let dir = scaffold_run_dir(project, run_id);

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
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

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
        let _ = std::fs::remove_dir_all(
            crate::storage::LocalStorage::project_dir(project),
        );
    }
}
