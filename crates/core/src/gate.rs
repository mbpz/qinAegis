// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! Gate service - Quality gate for E2E, Performance, and Stress tests

use crate::performance::LighthouseResult;
use crate::storage::LocalStorage;
use crate::stress::LocustResult;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Gate thresholds configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateThresholds {
    /// Minimum E2E pass rate (0-100)
    pub e2e_pass_rate_min: f64,
    /// Maximum performance regression percentage
    pub perf_regression_max: f64,
    /// Minimum RPS for stress test
    pub stress_rps_min: f64,
    /// Maximum P95 latency in ms for stress test
    pub stress_p95_max_ms: f64,
    /// Maximum error rate percentage for stress test
    pub stress_error_rate_max: f64,
}

impl Default for GateThresholds {
    fn default() -> Self {
        Self {
            e2e_pass_rate_min: 100.0,
            perf_regression_max: 10.0,
            stress_rps_min: 100.0,
            stress_p95_max_ms: 2000.0,
            stress_error_rate_max: 5.0,
        }
    }
}

impl GateThresholds {
    pub fn new(
        e2e_pass_rate_min: f64,
        perf_regression_max: f64,
        stress_rps_min: f64,
        stress_p95_max_ms: f64,
        stress_error_rate_max: f64,
    ) -> Self {
        Self {
            e2e_pass_rate_min,
            perf_regression_max,
            stress_rps_min,
            stress_p95_max_ms,
            stress_error_rate_max,
        }
    }
}

/// Gate status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateStatus {
    Passed,
    Failed,
    Skipped,
}

impl GateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            GateStatus::Passed => "passed",
            GateStatus::Failed => "failed",
            GateStatus::Skipped => "skipped",
        }
    }

    pub fn is_passed(&self) -> bool {
        matches!(self, GateStatus::Passed)
    }
}

/// E2E test gate result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2EGateResult {
    pub status: GateStatus,
    pub passed: usize,
    pub total: usize,
    pub pass_rate: f64,
    pub threshold: f64,
}

/// Performance test gate result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfGateResult {
    pub status: GateStatus,
    pub regressed: bool,
    pub regressions: Vec<String>,
    pub threshold: f64,
}

/// Stress test gate result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressGateResult {
    pub status: GateStatus,
    pub rps: f64,
    pub p95_ms: f64,
    pub error_rate: f64,
    pub thresholds: StressThresholdsResult,
}

/// Stress thresholds (for JSON output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressThresholdsResult {
    pub rps_min: f64,
    pub p95_max_ms: f64,
    pub error_rate_max: f64,
}

/// Complete gate result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub project: String,
    pub run_id: String,
    pub timestamp: String,
    pub e2e: E2EGateResult,
    pub performance: PerfGateResult,
    pub stress: StressGateResult,
    pub overall_passed: bool,
    pub exit_code: i32,
}

/// Internal data structure for loading results
struct GateResults {
    e2e_summary: Option<E2ESummary>,
    lighthouse: Option<LighthouseResult>,
    locust: Option<LocustResult>,
}

struct E2ESummary {
    passed: usize,
    total: usize,
}

/// Gate service for evaluating quality gates
pub struct GateService {
    project: String,
    run_id: Option<String>,
    thresholds: GateThresholds,
}

impl GateService {
    /// Create a new GateService
    pub fn new(
        project: String,
        run_id: Option<String>,
        thresholds: GateThresholds,
    ) -> Self {
        Self {
            project,
            run_id,
            thresholds,
        }
    }

    /// Check all gates and return result
    pub async fn check(&self) -> anyhow::Result<GateResult> {
        let results = self.load_results()?;
        let evaluated = self.evaluate(&results);

        let run_id = self.run_id.clone().unwrap_or_else(|| "latest".to_string());
        let timestamp = chrono_lite_timestamp();

        Ok(GateResult {
            project: self.project.clone(),
            run_id,
            timestamp,
            e2e: evaluated.e2e,
            performance: evaluated.performance,
            stress: evaluated.stress,
            overall_passed: evaluated.overall_passed,
            exit_code: if evaluated.overall_passed { 0 } else { 1 },
        })
    }

    fn load_results(&self) -> anyhow::Result<GateResults> {
        // Determine run_id
        let run_id = if let Some(ref rid) = self.run_id {
            rid.clone()
        } else {
            self.find_latest_run_id()?
        };

        let run_dir = LocalStorage::run_dir(&self.project, &run_id);
        if !run_dir.exists() {
            anyhow::bail!("Run directory not found: {}", run_dir.display());
        }

        // Load E2E results
        let e2e_summary = self.load_e2e_summary(&run_dir)?;

        // Load Lighthouse results
        let lighthouse = self.load_lighthouse(&run_dir)?;

        // Load Locust results
        let locust = self.load_locust(&run_dir)?;

        Ok(GateResults {
            e2e_summary,
            lighthouse,
            locust,
        })
    }

    fn find_latest_run_id(&self) -> anyhow::Result<String> {
        let runs_dir = LocalStorage::runs_dir(&self.project);
        if !runs_dir.exists() {
            anyhow::bail!("No runs found for project '{}'", self.project);
        }

        let entries = std::fs::read_dir(&runs_dir)
            .map_err(|e| anyhow::anyhow!("Failed to read runs directory: {}", e))?;

        let mut run_ids: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();

        if run_ids.is_empty() {
            anyhow::bail!("No runs found for project '{}'", self.project);
        }

        // Sort descending (newest first)
        run_ids.sort_by(|a, b| b.cmp(a));

        Ok(run_ids.remove(0))
    }

    fn load_e2e_summary(&self, run_dir: &PathBuf) -> anyhow::Result<Option<E2ESummary>> {
        let summary_path = run_dir.join("summary.json");
        if !summary_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&summary_path)?;
        let summary: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse summary.json: {}", e))?;

        let passed = summary.get("passed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let total = summary.get("total")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        Ok(Some(E2ESummary { passed, total }))
    }

    fn load_lighthouse(&self, run_dir: &PathBuf) -> anyhow::Result<Option<LighthouseResult>> {
        let lh_path = run_dir.join("lighthouse.json");
        if !lh_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&lh_path)?;
        let result: LighthouseResult = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse lighthouse.json: {}", e))?;

        Ok(Some(result))
    }

    fn load_locust(&self, run_dir: &PathBuf) -> anyhow::Result<Option<LocustResult>> {
        let locust_path = run_dir.join("locust-summary.json");
        if !locust_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&locust_path)?;
        let result: LocustResult = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse locust-summary.json: {}", e))?;

        Ok(Some(result))
    }

    fn evaluate(&self, results: &GateResults) -> EvaluatedGate {
        let e2e = self.evaluate_e2e(&results.e2e_summary);
        let performance = self.evaluate_performance(&results.lighthouse);
        let stress = self.evaluate_stress(&results.locust);

        let overall_passed = e2e.status == GateStatus::Passed
            && performance.status == GateStatus::Passed
            && stress.status == GateStatus::Passed;

        EvaluatedGate {
            e2e,
            performance,
            stress,
            overall_passed,
        }
    }

    fn evaluate_e2e(&self, summary: &Option<E2ESummary>) -> E2EGateResult {
        let (status, passed, total, pass_rate) = match summary {
            None => (GateStatus::Skipped, 0, 0, 0.0),
            Some(s) if s.total == 0 => (GateStatus::Passed, s.passed, s.total, 100.0),
            Some(s) => {
                let rate = (s.passed as f64 / s.total as f64) * 100.0;
                let passed_gate = rate >= self.thresholds.e2e_pass_rate_min;
                let status = if passed_gate { GateStatus::Passed } else { GateStatus::Failed };
                (status, s.passed, s.total, rate)
            }
        };

        E2EGateResult {
            status,
            passed,
            total,
            pass_rate,
            threshold: self.thresholds.e2e_pass_rate_min,
        }
    }

    fn evaluate_performance(&self, lighthouse: &Option<LighthouseResult>) -> PerfGateResult {
        let (status, regressed, regressions) = match lighthouse {
            None => (GateStatus::Skipped, false, vec![]),
            Some(lh) => {
                let mut regressions = Vec::new();

                // Check performance score (0-1 scale, lower is worse)
                if lh.metrics.performance < 0.8 {
                    regressions.push("performance_score".to_string());
                }

                // Check LCP (largest contentful paint)
                if lh.metrics.largest_contentful_paint > 4000.0 {
                    regressions.push("lcp".to_string());
                }

                // Check CLS (cumulative layout shift)
                if lh.metrics.cumulative_layout_shift > 0.25 {
                    regressions.push("cls".to_string());
                }

                let regressed = !regressions.is_empty();
                let status = if !regressed {
                    GateStatus::Passed
                } else {
                    GateStatus::Failed
                };

                (status, regressed, regressions)
            }
        };

        PerfGateResult {
            status,
            regressed,
            regressions,
            threshold: self.thresholds.perf_regression_max,
        }
    }

    fn evaluate_stress(&self, locust: &Option<LocustResult>) -> StressGateResult {
        let (status, rps, p95_ms, error_rate) = match locust {
            None => (GateStatus::Skipped, 0.0, 0.0, 0.0),
            Some(lr) => {
                let stats = &lr.stats;
                let rps = stats.rps;
                let p95_ms = stats.p95_response_time;
                let error_rate = if stats.total_requests > 0 {
                    (stats.total_failures as f64 / stats.total_requests as f64) * 100.0
                } else {
                    0.0
                };

                let passed_rps = rps >= self.thresholds.stress_rps_min;
                let passed_p95 = p95_ms <= self.thresholds.stress_p95_max_ms;
                let passed_error = error_rate <= self.thresholds.stress_error_rate_max;

                let passed_gate = passed_rps && passed_p95 && passed_error;
                let status = if passed_gate { GateStatus::Passed } else { GateStatus::Failed };

                (status, rps, p95_ms, error_rate)
            }
        };

        StressGateResult {
            status,
            rps,
            p95_ms,
            error_rate,
            thresholds: StressThresholdsResult {
                rps_min: self.thresholds.stress_rps_min,
                p95_max_ms: self.thresholds.stress_p95_max_ms,
                error_rate_max: self.thresholds.stress_error_rate_max,
            },
        }
    }
}

struct EvaluatedGate {
    e2e: E2EGateResult,
    performance: PerfGateResult,
    stress: StressGateResult,
    overall_passed: bool,
}

/// Print gate result in table format
pub fn print_gate_result(result: &GateResult, verbose: bool) {
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    qinAegis Gate Report                      ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Project: {:<50} ║", result.project);
    println!("║  Run ID:  {:<50} ║", result.run_id);
    println!("╠══════════════════════════════════════════════════════════════╣");

    // E2E row
    let e2e_icon = if result.e2e.status == GateStatus::Passed { "✓" } else { "✗" };
    println!(
        "║  [{}] E2E Tests          {}/{} passed ({:.1}%)  [threshold: {:.0}%] ║",
        e2e_icon,
        result.e2e.passed,
        result.e2e.total,
        result.e2e.pass_rate,
        result.e2e.threshold
    );

    // Performance row
    let perf_icon = if result.performance.status == GateStatus::Passed { "✓" } else { "✗" };
    let perf_detail = if result.performance.regressed {
        format!("{} regressed", result.performance.regressions.len())
    } else {
        "No regression detected".to_string()
    };
    println!(
        "║  [{}] Performance       {} [threshold: {:.0}%]    ║",
        perf_icon,
        format!("{:<25}", perf_detail),
        result.performance.threshold
    );

    // Stress row
    let stress_icon = if result.stress.status == GateStatus::Passed { "✓" } else { "✗" };
    if result.stress.status == GateStatus::Skipped {
        println!(
            "║  [-] Stress Test       SKIPPED (no results)                   ║"
        );
    } else {
        println!(
            "║  [{}] Stress Test       RPS: {:.0}, P95: {:.0}ms                 ║",
            stress_icon, result.stress.rps, result.stress.p95_ms
        );
    }

    println!("╠══════════════════════════════════════════════════════════════╣");
    let result_text = if result.overall_passed { "PASS" } else { "FAIL" };
    println!("║  RESULT: {:<53} ║", result_text);
    println!("╚══════════════════════════════════════════════════════════════╝");

    if verbose {
        println!();
        println!("Exit code: {}", result.exit_code);
    }
}

/// Print gate result in JSON format
pub fn print_gate_result_json(result: &GateResult) {
    let json = serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".to_string());
    println!("{}", json);
}

// Helper function for timestamp
fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_status_default_thresholds() {
        let thresholds = GateThresholds::default();
        assert_eq!(thresholds.e2e_pass_rate_min, 100.0);
        assert_eq!(thresholds.perf_regression_max, 10.0);
        assert_eq!(thresholds.stress_rps_min, 100.0);
        assert_eq!(thresholds.stress_p95_max_ms, 2000.0);
        assert_eq!(thresholds.stress_error_rate_max, 5.0);
    }

    #[test]
    fn test_gate_status_is_passed() {
        assert!(GateStatus::Passed.is_passed());
        assert!(!GateStatus::Failed.is_passed());
        assert!(!GateStatus::Skipped.is_passed());
    }

    #[test]
    fn test_gate_status_as_str() {
        assert_eq!(GateStatus::Passed.as_str(), "passed");
        assert_eq!(GateStatus::Failed.as_str(), "failed");
        assert_eq!(GateStatus::Skipped.as_str(), "skipped");
    }
}