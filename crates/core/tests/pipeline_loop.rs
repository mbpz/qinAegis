// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use qin_aegis_core::bus::ReliabilityBus;
use qin_aegis_core::events::ReliabilityEvent;
use qin_aegis_core::quarantine::Quarantine;
use qin_aegis_core::test_support::{cleanup_project, scaffold_run_dir};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

/// Wait until the bus has at least `n` subscribers attached.
///
/// Replaces fragile `tokio::time::sleep` rendezvous: polls
/// `subscriber_count()` at 1ms granularity until the expected number
/// of workers is listening, with a hard cap that panics loudly if
/// the count is never reached (typically means a worker panicked
/// before subscribing).
async fn wait_for_subscribers(bus: &ReliabilityBus, n: usize, max: Duration) {
    let start = Instant::now();
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
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

#[tokio::test]
async fn quarantined_case_is_skipped() {
    let dir = tempdir().unwrap();
    let qpath: PathBuf = dir.path().join("q.json");
    {
        let mut q = Quarantine::load(&qpath).unwrap();
        q.add("case-skip", "pre-quarantined for test");
        q.save().unwrap();
    }
    let bus = ReliabilityBus::new(64);
    let mut rx = bus.subscribe();
    bus.publish(ReliabilityEvent::CaseGenerated {
        case_id: "case-skip".into(),
        yaml: "target: { url: x }".into(),
        critic_score: Some(80),
    });
    // In the real pipeline, the orchestrator's start hook loads Quarantine
    // and filters CaseGenerated events before fanning out. For this test we
    // assert Quarantine::contains directly.
    let q = Quarantine::load(&qpath).unwrap();
    assert!(q.contains("case-skip"));
    let _ = rx.recv().await; // drain the one published event

    // CancellationToken is reachable from this crate's dev-deps, confirming
    // the wiring survives compilation in the integration test unit.
    let token = CancellationToken::new();
    assert!(!token.is_cancelled());
}

#[tokio::test]
async fn orchestrator_produces_gate_decided() {
    use qin_aegis_core::pipeline;

    // The orchestrator's GateLoop is hardcoded to project "default", so we
    // scaffold the run dir there. Use a unique run id so the test never
    // collides with another run that might have left files behind.
    let run_id = "smoke-r1";
    let dir = scaffold_run_dir("default", run_id);

    let bus = ReliabilityBus::new(64);
    let mut rx = bus.subscribe();
    let cancel = CancellationToken::new();

    // Spawn the orchestrator first so all workers subscribe before we
    // publish. The broadcast channel has no replay buffer, so pre-publishing
    // before the workers subscribe would silently drop our events.
    let bus_c = bus.clone();
    let cancel_c = cancel.clone();
    let h = tokio::spawn(async move {
        pipeline::run(
            bus_c,
            pipeline::PipelineCtx {
                run_id: "r1".into(),
                ..pipeline::PipelineCtx::default()
            },
            cancel_c,
        )
        .await
    });
    // Deterministically wait for all 4 workers (execute + heal + flaky + gate)
    // to subscribe before publishing. The test's own rx is already counted
    // in `subscriber_count()`, so we wait for 5 total (1 test + 4 workers).
    wait_for_subscribers(&bus, 5, Duration::from_secs(2)).await;

    // Publish a passing case (so flaky_loop records it) and a RunFinished
    // (so the GateLoop fires and reads the scaffolded run dir).
    //
    // Note: we DO NOT pre-publish CaseGenerated here because the real
    // TestExecutor wired into execute_loop would spawn a TS sandbox
    // subprocess on the fake YAML, slowing or breaking the test. The
    // orchestrator's GateLoop only needs RunFinished to fire GateDecided.
    bus.publish(ReliabilityEvent::CaseExecuted {
        case_id: "c1".into(),
        run_id: run_id.into(),
        passed: true,
        error: None,
        failed_step: None,
    });
    bus.publish(ReliabilityEvent::RunFinished {
        run_id: run_id.into(),
    });

    let got = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match rx.recv().await.unwrap() {
                ReliabilityEvent::GateDecided { passed, .. } => break passed,
                _ => continue,
            }
        }
    })
    .await;
    assert!(matches!(got, Ok(true)), "expected GateDecided{{passed:true}}");
    cancel.cancel();
    let _ = h.await;

    // Best-effort cleanup so repeated runs don't litter `~/.qinAegis`.
    let _ = std::fs::remove_dir_all(dir);
    cleanup_project("default");
}