// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! GateLoop worker.
//!
//! Subscribes to `ReliabilityEvent::RunFinished` on the shared `ReliabilityBus`,
//! runs `GateService::check` for each, and republishes the outcome as a
//! `ReliabilityEvent::GateDecided` on the same bus.

use crate::bus::ReliabilityBus;
use crate::events::ReliabilityEvent;
use crate::gate::GateService;
use tokio_util::sync::CancellationToken;

pub struct GateLoop;

impl GateLoop {
    /// Subscribes to `RunFinished` events and runs `GateService` for each.
    ///
    /// Returns once `cancel` is signalled. Per-event errors are logged and
    /// skipped so a transient gate failure does not tear down the loop.
    pub async fn run(
        bus: ReliabilityBus,
        svc: GateService,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let mut rx = bus.subscribe();
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                res = rx.recv() => {
                    let ev = match res {
                        Ok(e) => e,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("GateLoop lagged, dropped {} events", n);
                            continue;
                        }
                        Err(e) => return Err(e.into()),
                    };
                    if let ReliabilityEvent::RunFinished { run_id } = ev {
                        match svc.check().await {
                            Ok(result) => bus.publish(ReliabilityEvent::GateDecided {
                                run_id,
                                passed: result.overall_passed,
                                reasons: if result.overall_passed {
                                    vec![]
                                } else {
                                    vec![format!(
                                        "e2e:{:?}, perf:{:?}, stress:{:?}",
                                        result.e2e.status,
                                        result.performance.status,
                                        result.stress.status
                                    )]
                                },
                            }),
                            Err(e) => tracing::error!("GateLoop check failed: {}", e),
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::ReliabilityBus;
    use crate::events::ReliabilityEvent;
    use crate::gate::{GateService, GateThresholds};

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

        // lighthouse.json with passing metrics (perf 1.0, LCP 1000ms, CLS 0.05)
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
    async fn publishes_gate_decided_on_run_finished() {
        let dir = scaffold_run_dir("gate_loop_proj", "run-x");
        let bus = ReliabilityBus::new(16);
        let mut rx = bus.subscribe();
        let svc = GateService::new(
            "gate_loop_proj".into(),
            Some("run-x".into()),
            GateThresholds::default(),
        );
        let cancel = CancellationToken::new();
        let bus_clone = bus.clone();
        let t = tokio::spawn(async move {
            GateLoop::run(bus_clone, svc, cancel).await.unwrap();
        });
        // Deterministically wait for the worker to subscribe (test rx + worker = 2)
        // instead of a fragile fixed sleep.
        wait_for_subscribers(&bus, 2, std::time::Duration::from_secs(2)).await;
        bus.publish(ReliabilityEvent::RunFinished {
            run_id: "run-x".into(),
        });
        // Both this subscriber and the worker subscribe to the same broadcast channel,
        // so the first event we receive will be the RunFinished itself. Drain until
        // we see GateDecided, with a timeout to fail loudly if the worker never republishes.
        let got_decided = tokio::time::timeout(std::time::Duration::from_millis(500), async {
            loop {
                if matches!(rx.recv().await.unwrap(), ReliabilityEvent::GateDecided { .. }) {
                    return true;
                }
            }
        })
        .await
        .expect("timed out waiting for GateDecided");
        assert!(got_decided);
        t.abort();

        // Best-effort cleanup so repeated runs don't litter `~/.qinAegis`.
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(
            crate::storage::LocalStorage::project_dir("gate_loop_proj"),
        );
    }
}