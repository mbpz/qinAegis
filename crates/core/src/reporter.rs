// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::executor::TestResult;
use std::path::PathBuf;

pub struct Reporter;

impl Reporter {
    pub fn report_dir(run_id: &str) -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("qinAegis")
            .join("reports")
            .join(run_id)
    }

    pub fn save_html_local(run_id: &str, case_id: &str, html: &str) -> anyhow::Result<PathBuf> {
        let dir = Self::report_dir(run_id);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.html", case_id));
        std::fs::write(&path, html)?;
        Ok(path)
    }

    pub fn save_summary(run_id: &str, results: &[TestResult]) -> anyhow::Result<PathBuf> {
        let dir = Self::report_dir(run_id);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("summary.json");

        let summary = serde_json::json!({
            "run_id": run_id,
            "total": results.len(),
            "passed": results.iter().filter(|r| r.passed).count(),
            "failed": results.iter().filter(|r| !r.passed).count(),
            "results": results,
        });

        std::fs::write(&path, serde_json::to_string_pretty(&summary)?)?;
        Ok(path)
    }
}
