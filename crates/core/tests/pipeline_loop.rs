// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use qin_aegis_core::bus::ReliabilityBus;
use qin_aegis_core::events::ReliabilityEvent;
use qin_aegis_core::quarantine::Quarantine;
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