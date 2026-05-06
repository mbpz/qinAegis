// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

#[cfg(test)]
mod tests {
    use crate::performance::{
        LighthouseMetrics, LighthouseResult, MetricRegression, PerformanceComparison,
    };

    fn make_metrics(performance: f64, fcp: f64, lcp: f64, cls: f64, tbt: f64) -> LighthouseMetrics {
        LighthouseMetrics {
            performance,
            accessibility: 0.9,
            best_practices: 0.95,
            seo: 0.88,
            first_contentful_paint: fcp,
            largest_contentful_paint: lcp,
            cumulative_layout_shift: cls,
            total_blocking_time: tbt,
            speed_index: fcp * 1.1,
            ttfb: 150.0,
        }
    }

    fn make_result(score: f64, metrics: LighthouseMetrics) -> LighthouseResult {
        LighthouseResult {
            url: "https://example.com".to_string(),
            score,
            metrics,
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            report_path: None,
        }
    }

    // ========================================================================
    // PerformanceComparison
    // ========================================================================

    #[test]
    fn test_performance_comparison_no_regression() {
        let baseline = make_result(90.0, make_metrics(90.0, 1.0, 2.0, 0.05, 100.0));
        let current = make_result(92.0, make_metrics(92.0, 0.9, 1.8, 0.04, 90.0));

        let comparison = PerformanceComparison::compare(current, baseline, 10.0);

        assert!(comparison.passed);
        assert!(!comparison.regression.iter().any(|r| r.regressed));
    }

    #[test]
    fn test_performance_comparison_with_regression() {
        let baseline = make_result(90.0, make_metrics(90.0, 1.0, 2.0, 0.05, 100.0));
        let current = make_result(85.0, make_metrics(85.0, 1.5, 3.0, 0.15, 300.0));

        let comparison = PerformanceComparison::compare(current, baseline, 10.0);

        assert!(!comparison.passed);
        assert!(comparison.regression.iter().any(|r| r.regressed));
    }

    #[test]
    fn test_metric_regression_calculation() {
        let baseline = make_result(100.0, make_metrics(100.0, 1.0, 2.0, 0.05, 100.0));
        let current = make_result(80.0, make_metrics(80.0, 1.2, 2.4, 0.06, 120.0));

        let comparison = PerformanceComparison::compare(current, baseline, 10.0);

        let lcp_reg = comparison
            .regression
            .iter()
            .find(|r| r.metric_name == "largest_contentful_paint")
            .unwrap();
        assert!(lcp_reg.regressed);
    }

    #[test]
    fn test_metric_regression_zero_baseline() {
        let baseline_metrics = make_metrics(0.0, 0.0, 0.0, 0.0, 0.0);
        let baseline = make_result(0.0, baseline_metrics);

        let current_metrics = make_metrics(50.0, 1.0, 2.0, 0.05, 100.0);
        let current = make_result(50.0, current_metrics);

        let comparison = PerformanceComparison::compare(current, baseline, 10.0);
        assert!(comparison.passed);
    }

    #[test]
    fn test_lighthouse_metrics_serialization() {
        let metrics = make_metrics(95.0, 0.8, 1.5, 0.03, 50.0);
        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("\"performance\""));
        assert!(json.contains("\"largest_contentful_paint\""));
    }

    #[test]
    fn test_lighthouse_result_serialization() {
        let result = make_result(95.0, make_metrics(95.0, 0.8, 1.5, 0.03, 50.0));
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"url\""));
        assert!(json.contains("\"score\""));
        assert!(json.contains("\"metrics\""));
    }
}