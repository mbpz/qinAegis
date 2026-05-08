// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! Quality knowledge base — persistent learning from test results.
//!
//! Stores three knowledge artifacts per project:
//! - `failure-patterns.json` — AI-classified failure reasons
//! - `flakiness.json` — per-case flakiness stats
//! - `coverage.json` — page → case mapping

use crate::executor::TestResult;
use crate::storage::{LocalStorage, TestCase};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Failure Patterns
// ============================================================================

/// Classification of a test failure root cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailureCategory {
    /// The product has a bug.
    ProductBug,
    /// The test case is incorrect or outdated.
    TestIssue,
    /// Environment problem (network, server down, timeout).
    Environment,
    /// LLM/vision model made a wrong assertion.
    ModelHallucination,
    /// Unknown cause.
    Unknown,
}

impl FailureCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureCategory::ProductBug => "product_bug",
            FailureCategory::TestIssue => "test_issue",
            FailureCategory::Environment => "environment",
            FailureCategory::ModelHallucination => "model_hallucination",
            FailureCategory::Unknown => "unknown",
        }
    }

    /// Heuristic classification based on error message patterns.
    pub fn classify(error_message: &str) -> Self {
        let msg = error_message.to_lowercase();
        if msg.contains("timeout") || msg.contains("connection refused") || msg.contains("dns") {
            FailureCategory::Environment
        } else if msg.contains("not found") && (msg.contains("element") || msg.contains("selector")) {
            FailureCategory::TestIssue
        } else if msg.contains("assertion") && (msg.contains("ai") || msg.contains("model") || msg.contains("hallucinat")) {
            FailureCategory::ModelHallucination
        } else if msg.contains("assert") || msg.contains("expect") || msg.contains("expected") {
            FailureCategory::ProductBug
        } else {
            FailureCategory::Unknown
        }
    }
}

impl std::fmt::Display for FailureCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single failure pattern entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    pub case_id: String,
    pub case_name: String,
    pub category: FailureCategory,
    pub error_snippet: String,
    pub run_id: String,
    pub timestamp: String,
    pub count: u64,
}

/// Aggregated failure patterns for a project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FailurePatterns {
    pub patterns: Vec<FailurePattern>,
    /// Category → count summary
    pub summary: HashMap<String, u64>,
}

impl FailurePatterns {
    /// Record a failure and update patterns.
    pub fn record(
        &mut self,
        case_id: &str,
        case_name: &str,
        error_message: &str,
        run_id: &str,
    ) {
        let category = FailureCategory::classify(error_message);

        // Update existing pattern or add new
        if let Some(existing) = self.patterns.iter_mut().find(|p| p.case_id == case_id && p.category == category) {
            existing.count += 1;
            existing.error_snippet = error_message.chars().take(200).collect();
            existing.run_id = run_id.to_string();
        } else {
            self.patterns.push(FailurePattern {
                case_id: case_id.to_string(),
                case_name: case_name.to_string(),
                category,
                error_snippet: error_message.chars().take(200).collect(),
                run_id: run_id.to_string(),
                timestamp: chrono_lite_now(),
                count: 1,
            });
        }

        // Update summary
        *self.summary.entry(category.as_str().to_string()).or_insert(0) += 1;
    }

    /// Get the most frequent failure category.
    pub fn dominant_category(&self) -> Option<FailureCategory> {
        self.summary
            .iter()
            .max_by_key(|(_, count)| *count)
            .and_then(|(cat, _)| match cat.as_str() {
                "product_bug" => Some(FailureCategory::ProductBug),
                "test_issue" => Some(FailureCategory::TestIssue),
                "environment" => Some(FailureCategory::Environment),
                "model_hallucination" => Some(FailureCategory::ModelHallucination),
                "unknown" => Some(FailureCategory::Unknown),
                _ => None,
            })
    }
}

// ============================================================================
// Flakiness Tracking
// ============================================================================

/// Per-case flakiness record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakinessRecord {
    pub case_id: String,
    pub case_name: String,
    pub total_runs: u64,
    pub total_passes: u64,
    pub total_failures: u64,
    /// Consecutive failure count (resets on pass).
    pub consecutive_failures: u64,
    /// Flaky score (0-100): higher means more unstable.
    /// Calculated as: (passes / total) inverted, weighted by consecutive flip count.
    pub flaky_score: f64,
    /// Last N run results (true = pass, false = fail).
    #[serde(default)]
    pub recent_history: Vec<bool>,
}

impl FlakinessRecord {
    pub fn new(case_id: &str, case_name: &str) -> Self {
        Self {
            case_id: case_id.to_string(),
            case_name: case_name.to_string(),
            total_runs: 0,
            total_passes: 0,
            total_failures: 0,
            consecutive_failures: 0,
            flaky_score: 0.0,
            recent_history: Vec::new(),
        }
    }

    /// Update with a new run result.
    pub fn record_run(&mut self, passed: bool) {
        self.total_runs += 1;
        if passed {
            self.total_passes += 1;
            self.consecutive_failures = 0;
        } else {
            self.total_failures += 1;
            self.consecutive_failures += 1;
        }

        // Keep last 20 runs
        self.recent_history.push(passed);
        if self.recent_history.len() > 20 {
            self.recent_history.remove(0);
        }

        // Calculate flaky score
        if self.total_runs >= 3 {
            // Count state flips in recent history
            let flips: u64 = self.recent_history.windows(2).filter(|w| w[0] != w[1]).count() as u64;
            let pass_rate = self.total_passes as f64 / self.total_runs as f64;
            // Flaky if pass rate is mid-range (not 0 or 100) or has many flips
            self.flaky_score = ((1.0 - (pass_rate - 0.5).abs() * 2.0) * 50.0 + flips as f64 * 5.0).min(100.0);
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FlakinessIndex {
    pub cases: HashMap<String, FlakinessRecord>,
}

impl FlakinessIndex {
    pub fn record(&mut self, case_id: &str, case_name: &str, passed: bool) {
        self.cases
            .entry(case_id.to_string())
            .or_insert_with(|| FlakinessRecord::new(case_id, case_name))
            .record_run(passed);
    }
}

// ============================================================================
// Coverage Tracking
// ============================================================================

/// Page → case coverage mapping.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoverageMap {
    /// page_url → list of case_ids that cover this page.
    pub page_to_cases: HashMap<String, Vec<String>>,
    /// Total unique pages covered.
    pub total_pages: usize,
    /// Total cases.
    pub total_cases: usize,
}

impl CoverageMap {
    pub fn add_coverage(&mut self, page_url: &str, case_id: &str) {
        self.page_to_cases
            .entry(page_url.to_string())
            .or_default()
            .push(case_id.to_string());
        self.total_pages = self.page_to_cases.len();
    }

    pub fn covered_pages(&self) -> Vec<&String> {
        self.page_to_cases.keys().collect()
    }

    pub fn cases_for_page(&self, page_url: &str) -> &[String] {
        self.page_to_cases.get(page_url).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

// ============================================================================
// Knowledge Base (aggregate persistence)
// ============================================================================

pub struct KnowledgeBase {
    project_name: String,
}

impl KnowledgeBase {
    pub fn new(project_name: &str) -> Self {
        Self {
            project_name: project_name.to_string(),
        }
    }

    fn knowledge_dir(&self) -> PathBuf {
        crate::storage::LocalStorage::project_dir(&self.project_name).join("knowledge")
    }

    fn ensure_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.knowledge_dir())
    }

    // --- Failure Patterns ---

    pub fn load_failure_patterns(&self) -> anyhow::Result<FailurePatterns> {
        let path = self.knowledge_dir().join("failure-patterns.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content).unwrap_or_default())
        } else {
            Ok(FailurePatterns::default())
        }
    }

    pub fn save_failure_patterns(&self, patterns: &FailurePatterns) -> anyhow::Result<()> {
        self.ensure_dir()?;
        let path = self.knowledge_dir().join("failure-patterns.json");
        std::fs::write(&path, serde_json::to_string_pretty(patterns)?)?;
        Ok(())
    }

    /// Record failures from a test run.
    pub fn record_failures(
        &self,
        project_name: &str,
        results: &[TestResult],
        run_id: &str,
    ) -> anyhow::Result<()> {
        let mut patterns = self.load_failure_patterns()?;
        for r in results.iter().filter(|r| !r.passed) {
            if let Some(ref err) = r.error_message {
                // Try to load the case name from storage
                let case_name = if let Some(case_path) = LocalStorage::find_case_path(project_name, &r.case_id) {
                    std::fs::read_to_string(&case_path)
                        .ok()
                        .and_then(|c| serde_json::from_str::<TestCase>(&c).ok())
                        .map(|c| c.name)
                        .unwrap_or_else(|| r.case_id.clone())
                } else {
                    r.case_id.clone()
                };
                patterns.record(&r.case_id, &case_name, err, run_id);
            }
        }
        self.save_failure_patterns(&patterns)?;
        Ok(())
    }

    // --- Flakiness ---

    pub fn load_flakiness(&self) -> anyhow::Result<FlakinessIndex> {
        let path = self.knowledge_dir().join("flakiness.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content).unwrap_or_default())
        } else {
            Ok(FlakinessIndex::default())
        }
    }

    pub fn save_flakiness(&self, index: &FlakinessIndex) -> anyhow::Result<()> {
        self.ensure_dir()?;
        let path = self.knowledge_dir().join("flakiness.json");
        std::fs::write(&path, serde_json::to_string_pretty(index)?)?;
        Ok(())
    }

    /// Record flakiness from a test run.
    pub fn record_flakiness(&self, results: &[TestResult]) -> anyhow::Result<()> {
        let mut index = self.load_flakiness()?;
        for r in results {
            index.record(&r.case_id, &r.case_id, r.passed);
        }
        self.save_flakiness(&index)?;
        Ok(())
    }

    // --- Coverage ---

    pub fn load_coverage(&self) -> anyhow::Result<CoverageMap> {
        let path = self.knowledge_dir().join("coverage.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content).unwrap_or_default())
        } else {
            Ok(CoverageMap::default())
        }
    }

    pub fn save_coverage(&self, coverage: &CoverageMap) -> anyhow::Result<()> {
        self.ensure_dir()?;
        let path = self.knowledge_dir().join("coverage.json");
        std::fs::write(&path, serde_json::to_string_pretty(coverage)?)?;
        Ok(())
    }
}

fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_secs())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failure_category_as_str() {
        assert_eq!(FailureCategory::ProductBug.as_str(), "product_bug");
        assert_eq!(FailureCategory::TestIssue.as_str(), "test_issue");
        assert_eq!(FailureCategory::Environment.as_str(), "environment");
        assert_eq!(FailureCategory::ModelHallucination.as_str(), "model_hallucination");
        assert_eq!(FailureCategory::Unknown.as_str(), "unknown");
    }

    #[test]
    fn test_failure_category_classify_environment() {
        assert_eq!(FailureCategory::classify("connection timeout"), FailureCategory::Environment);
        assert_eq!(FailureCategory::classify("DNS lookup failed"), FailureCategory::Environment);
        assert_eq!(FailureCategory::classify("connection refused"), FailureCategory::Environment);
    }

    #[test]
    fn test_failure_category_classify_test_issue() {
        assert_eq!(FailureCategory::classify("element not found"), FailureCategory::TestIssue);
        assert_eq!(FailureCategory::classify("selector not found: #login-btn"), FailureCategory::TestIssue);
    }

    #[test]
    fn test_failure_patterns_record_new() {
        let mut patterns = FailurePatterns::default();
        // Use a clear environment error message to avoid classification ambiguity
        patterns.record("case-1", "Login Test", "request timeout after 30s", "run-1");

        assert_eq!(patterns.patterns.len(), 1);
        assert_eq!(patterns.patterns[0].case_id, "case-1");
        assert_eq!(patterns.patterns[0].case_name, "Login Test");
        assert_eq!(patterns.patterns[0].count, 1);
        assert!(patterns.summary.get("environment").is_some());
    }

    #[test]
    fn test_failure_patterns_record_increments_existing() {
        let mut patterns = FailurePatterns::default();
        patterns.record("case-1", "Login Test", "timeout error", "run-1");
        patterns.record("case-1", "Login Test", "timeout error", "run-2");

        assert_eq!(patterns.patterns.len(), 1);
        assert_eq!(patterns.patterns[0].count, 2);
        assert_eq!(patterns.patterns[0].run_id, "run-2");
    }

    #[test]
    fn test_failure_patterns_record_different_categories() {
        let mut patterns = FailurePatterns::default();
        patterns.record("case-1", "Test 1", "timeout error", "run-1");
        patterns.record("case-2", "Test 2", "assertion failed", "run-1");

        assert_eq!(patterns.patterns.len(), 2);
    }

    #[test]
    fn test_failure_patterns_dominant_category() {
        let mut patterns = FailurePatterns::default();
        patterns.record("case-1", "Test 1", "timeout error", "run-1");
        patterns.record("case-2", "Test 2", "timeout error", "run-1");
        patterns.record("case-3", "Test 3", "assertion failed", "run-1");

        assert_eq!(patterns.dominant_category(), Some(FailureCategory::Environment));
    }

    #[test]
    fn test_failure_patterns_dominant_category_empty() {
        let patterns = FailurePatterns::default();
        assert_eq!(patterns.dominant_category(), None);
    }

    #[test]
    fn test_flakiness_index_record() {
        let mut index = FlakinessIndex::default();
        index.record("case-1", "case-1", true);
        index.record("case-1", "case-1", false);
        index.record("case-1", "case-1", true);

        assert_eq!(index.cases.len(), 1);
        let case = &index.cases["case-1"];
        assert_eq!(case.total_runs, 3);
        assert_eq!(case.total_passes, 2);
        assert_eq!(case.total_failures, 1);
    }
}
