// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! Test-only utilities for integration tests. Exposed via `pub` so external
//! integration tests in `tests/` can import it. The module compiles into
//! production binaries but the helpers themselves are only invoked from
//! tests; cost is negligible.

use crate::storage::LocalStorage;
use std::path::PathBuf;

/// Scaffold a fake run directory at `~/.qinAegis/projects/<proj>/reports/<run_id>/`
/// with minimal valid `summary.json`, `lighthouse.json`, `locust-summary.json`
/// so `GateService::check()` returns a real GateResult without bailing.
///
/// Returns the path to the run dir.
pub fn scaffold_run_dir(project: &str, run_id: &str) -> PathBuf {
    let dir = LocalStorage::run_dir(project, run_id);
    std::fs::create_dir_all(&dir).expect("failed to create run dir");

    // summary.json with one passing case
    std::fs::write(
        dir.join("summary.json"),
        r#"{"passed":1,"total":1}"#,
    )
    .expect("failed to write summary.json");

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
    .expect("failed to write lighthouse.json");

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
    .expect("failed to write locust-summary.json");

    dir
}

/// Best-effort cleanup of a scaffolded project dir. Swallows errors.
pub fn cleanup_project(project: &str) {
    let _ = std::fs::remove_dir_all(LocalStorage::project_dir(project));
}