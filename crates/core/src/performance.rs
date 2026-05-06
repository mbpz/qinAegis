// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighthouseMetrics {
    pub performance: f64,
    pub accessibility: f64,
    pub best_practices: f64,
    pub seo: f64,
    pub first_contentful_paint: f64,
    pub largest_contentful_paint: f64,
    pub cumulative_layout_shift: f64,
    pub total_blocking_time: f64,
    pub speed_index: f64,
    pub ttfb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LighthouseResult {
    pub url: String,
    pub score: f64,
    pub metrics: LighthouseMetrics,
    pub timestamp: String,
    pub report_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceComparison {
    pub current: LighthouseResult,
    pub baseline: LighthouseResult,
    pub regression: Vec<MetricRegression>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricRegression {
    pub metric_name: String,
    pub baseline_value: f64,
    pub current_value: f64,
    pub threshold_percent: f64,
    pub regressed: bool,
}

impl PerformanceComparison {
    pub fn compare(
        current: LighthouseResult,
        baseline: LighthouseResult,
        threshold_percent: f64,
    ) -> Self {
        let mut regression = Vec::new();

        let check_metric = |name: &str, baseline_val: f64, current_val: f64| {
            let change = if baseline_val > 0.0 {
                ((current_val - baseline_val) / baseline_val) * 100.0
            } else {
                0.0
            };
            MetricRegression {
                metric_name: name.to_string(),
                baseline_value: baseline_val,
                current_value: current_val,
                threshold_percent,
                regressed: change < -threshold_percent,
            }
        };

        regression.push(check_metric(
            "performance",
            baseline.metrics.performance,
            current.metrics.performance,
        ));
        regression.push(check_metric(
            "first_contentful_paint",
            baseline.metrics.first_contentful_paint,
            current.metrics.first_contentful_paint,
        ));
        regression.push(check_metric(
            "largest_contentful_paint",
            baseline.metrics.largest_contentful_paint,
            current.metrics.largest_contentful_paint,
        ));
        regression.push(check_metric(
            "cumulative_layout_shift",
            baseline.metrics.cumulative_layout_shift,
            current.metrics.cumulative_layout_shift,
        ));
        regression.push(check_metric(
            "total_blocking_time",
            baseline.metrics.total_blocking_time,
            current.metrics.total_blocking_time,
        ));

        let passed = !regression.iter().any(|r| r.regressed);

        Self {
            current,
            baseline,
            regression,
            passed,
        }
    }
}
