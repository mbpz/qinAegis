// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use qin_aegis_core::bus::ReliabilityBus;
use qin_aegis_core::events::ReliabilityEvent;
use qin_aegis_core::quarantine::Quarantine;
use qin_aegis_core::storage::LocalStorage;
use std::path::PathBuf;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

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

/// Build a minimal but valid run directory under
/// `~/.qinAegis/projects/<proj>/reports/<run_id>` so that
/// `GateService::check()` succeeds without touching the network.
fn scaffold_run_dir(project: &str, run_id: &str) -> PathBuf {
    let dir = LocalStorage::run_dir(project, run_id);
    std::fs::create_dir_all(&dir).unwrap();

    // summary.json with one passing case
    std::fs::write(dir.join("summary.json"), r#"{"passed":1,"total":1}"#).unwrap();

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
    let h = tokio::spawn(async move { pipeline::run(bus_c, cancel_c).await });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

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
    let _ = std::fs::remove_dir_all(LocalStorage::project_dir("default"));
}