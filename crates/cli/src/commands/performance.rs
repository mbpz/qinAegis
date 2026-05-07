// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use qin_aegis_core::{PerformanceComparison, StressTestConfig, LighthouseResult};

pub async fn run_performance(url: &str, threshold_percent: f64) -> anyhow::Result<()> {
    println!("Running Lighthouse performance test for {}", url);

    // Run Lighthouse via JSON-RPC
    let result = run_lighthouse_rpc(url).await?;

    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    let report_dir = qin_aegis_core::Reporter::report_dir(&format!("perf_{}", run_id));
    std::fs::create_dir_all(&report_dir)?;
    let report_path = report_dir.join("lighthouse_report.json");
    std::fs::write(&report_path, serde_json::to_string_pretty(&result)?)?;

    // Compare with baseline - local baseline storage pending
    if let Some(baseline) = fetch_baseline(url).await? {
        let comparison = PerformanceComparison::compare(result.clone(), baseline, threshold_percent);

        if !comparison.passed {
            println!("\nWARNING: PERFORMANCE REGRESSION DETECTED:");
            for reg in &comparison.regression {
                if reg.regressed {
                    println!(
                        "  - {}: {} → {} ({:.1}% change)",
                        reg.metric_name,
                        reg.baseline_value,
                        reg.current_value,
                        ((reg.current_value - reg.baseline_value) / reg.baseline_value) * 100.0
                    );
                }
            }
        }

        let comment = generate_pr_comment(&comparison);
        println!("\n## GitHub Actions Comment:\n{}", comment);
    }

    println!("\nPerformance test complete. Report: {}", report_path.display());
    Ok(())
}

pub async fn run_stress(config: StressTestConfig) -> anyhow::Result<()> {
    println!(
        "Running stress test: {} users, {} spawn rate, {}s duration",
        config.users, config.spawn_rate, config.duration_seconds
    );

    let result = run_locust_rpc(&config).await?;

    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    let report_dir = qin_aegis_core::Reporter::report_dir(&format!("stress_{}", run_id));
    std::fs::create_dir_all(&report_dir)?;
    let report_path = report_dir.join("locust_stats.json");
    std::fs::write(&report_path, serde_json::to_string_pretty(&result)?)?;

    println!("\nStress test complete:");
    println!("  Total requests: {}", result.stats.total_requests);
    println!("  Total failures: {}", result.stats.total_failures);
    println!("  Avg response time: {:.2}ms", result.stats.avg_response_time);
    println!("  P95 response time: {:.2}ms", result.stats.p95_response_time);
    println!("  RPS: {:.2}", result.stats.rps);

    if !result.errors.is_empty() {
        println!("\n⚠️  Errors detected:");
        for error in &result.errors {
            println!("  - {}", error);
        }
    }

    Ok(())
}

async fn run_lighthouse_rpc(url: &str) -> anyhow::Result<qin_aegis_core::LighthouseResult> {
    // TODO: Implement JSON-RPC call to sandbox executor.ts
    // For now, return a placeholder that indicates integration is pending
    println!("Note: Lighthouse JSON-RPC integration pending - using mock result");
    Ok(qin_aegis_core::LighthouseResult {
        url: url.to_string(),
        score: 0.85,
        metrics: qin_aegis_core::LighthouseMetrics {
            performance: 0.85,
            accessibility: 0.9,
            best_practices: 0.95,
            seo: 0.88,
            first_contentful_paint: 1200.0,
            largest_contentful_paint: 2500.0,
            cumulative_layout_shift: 0.05,
            total_blocking_time: 200.0,
            speed_index: 3.5,
            ttfb: 180.0,
        },
        timestamp: chrono::Utc::now().to_rfc3339(),
        report_path: None,
    })
}

async fn run_locust_rpc(config: &StressTestConfig) -> anyhow::Result<qin_aegis_core::LocustResult> {
    println!("Note: Locust JSON-RPC integration pending - using mock result");
    Ok(qin_aegis_core::LocustResult {
        target_url: config.target_url.clone(),
        stats: qin_aegis_core::LocustStats {
            total_requests: 1000,
            total_failures: 5,
            median_response_time: 150.0,
            avg_response_time: 200.0,
            p95_response_time: 450.0,
            p99_response_time: 800.0,
            rps: 50.0,
            duration: config.duration_seconds as f64,
        },
        timestamp: chrono::Utc::now().to_rfc3339(),
        errors: vec![],
    })
}

async fn fetch_baseline(
    _url: &str,
) -> anyhow::Result<Option<LighthouseResult>> {
    Ok(None)
}

fn generate_pr_comment(comparison: &PerformanceComparison) -> String {
    format!(
        r#"## Performance Test Results

### URL: {}

| Metric | Baseline | Current | Change |
|--------|----------|---------|--------|
| Performance Score | {:.2} | {:.2} | {:.1}% |
| LCP | {:.0}ms | {:.0}ms | {:.1}% |
| CLS | {:.3} | {:.3} | {:.1}% |

### Status: {}

<!-- qin-aegis-metrics: {} -->"#,
        comparison.current.url,
        comparison.baseline.metrics.performance,
        comparison.current.metrics.performance,
        ((comparison.current.metrics.performance - comparison.baseline.metrics.performance) / comparison.baseline.metrics.performance) * 100.0,
        comparison.baseline.metrics.largest_contentful_paint,
        comparison.current.metrics.largest_contentful_paint,
        ((comparison.current.metrics.largest_contentful_paint - comparison.baseline.metrics.largest_contentful_paint) / comparison.baseline.metrics.largest_contentful_paint) * 100.0,
        comparison.baseline.metrics.cumulative_layout_shift,
        comparison.current.metrics.cumulative_layout_shift,
        ((comparison.current.metrics.cumulative_layout_shift - comparison.baseline.metrics.cumulative_layout_shift) / comparison.baseline.metrics.cumulative_layout_shift) * 100.0,
        if comparison.passed { "✅ PASSED" } else { "❌ REGRESSION" },
        serde_json::to_string(comparison).unwrap_or_default()
    )
}
