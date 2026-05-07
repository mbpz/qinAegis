// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use qin_aegis_core::{
    BrowserAutomation, LighthouseResult, LocustResult, MidsceneAutomation,
    PerformanceComparison, SandboxConfig, StressTestConfig,
};
use crate::config::Config;

pub async fn run_performance(url: &str, threshold_percent: f64) -> anyhow::Result<()> {
    let config = Config::load()?
        .ok_or_else(|| anyhow::anyhow!("run qinAegis init first"))?;

    println!("Running Lighthouse performance test for {}", url);

    // Run Lighthouse via JSON-RPC to sandbox executor
    let result = run_lighthouse_rpc(url, &config).await?;

    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    let report_dir = qin_aegis_core::Reporter::report_dir(&format!("perf_{}", run_id));
    std::fs::create_dir_all(&report_dir)?;
    let report_path = report_dir.join("lighthouse_report.json");
    std::fs::write(&report_path, serde_json::to_string_pretty(&result)?)?;

    // Compare with baseline
    if let Some(baseline) = fetch_baseline(url, &config).await? {
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
    let app_config = Config::load()?
        .ok_or_else(|| anyhow::anyhow!("run qinAegis init first"))?;

    println!(
        "Running stress test: {} users, {} spawn rate, {}s duration",
        config.users, config.spawn_rate, config.duration_seconds
    );

    let result = run_locust_rpc(&config, &app_config).await?;

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

async fn run_lighthouse_rpc(url: &str, config: &Config) -> anyhow::Result<LighthouseResult> {
    let sandbox_config = Some(SandboxConfig {
        cdp_port: config.sandbox.cdp_port,
    });
    let automation = MidsceneAutomation::new(None, sandbox_config).await?;
    let result = automation.run_lighthouse(url).await
        .map_err(|e| anyhow::anyhow!("Lighthouse RPC failed: {}", e))?;
    automation.shutdown().await.ok();
    Ok(result)
}

async fn run_locust_rpc(stress_config: &StressTestConfig, app_config: &Config) -> anyhow::Result<LocustResult> {
    let sandbox_config = Some(SandboxConfig {
        cdp_port: app_config.sandbox.cdp_port,
    });
    let automation = MidsceneAutomation::new(None, sandbox_config).await?;
    let result = automation.run_stress(stress_config).await
        .map_err(|e| anyhow::anyhow!("Locust RPC failed: {}", e))?;
    automation.shutdown().await.ok();
    Ok(result)
}

async fn fetch_baseline(
    _url: &str,
    _config: &Config,
) -> anyhow::Result<Option<LighthouseResult>> {
    // TODO: Load baseline from ~/.qinAegis/projects/<name>/knowledge/baseline.json
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
