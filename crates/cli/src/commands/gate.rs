// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! Gate command - Quality gate CLI interface

use qin_aegis_core::storage::LocalStorage;
use qin_aegis_core::gate::{GateService, GateThresholds};
use clap::Parser;

/// Gate command arguments
#[derive(Debug, Parser)]
#[command(name = "gate")]
#[command(about = "Run quality gate checks on test results")]
pub struct GateCommand {
    /// Project name
    #[arg(long)]
    pub project: String,

    /// Run ID (latest if not specified)
    #[arg(long)]
    pub run_id: Option<String>,

    /// Minimum E2E pass rate (0-100)
    #[arg(long, default_value = "100")]
    pub e2e_threshold: f64,

    /// Maximum performance regression (%)
    #[arg(long, default_value = "10")]
    pub perf_threshold: f64,

    /// Minimum RPS for stress test
    #[arg(long, default_value = "100")]
    pub stress_rps_min: f64,

    /// Maximum P95 latency (ms)
    #[arg(long, default_value = "2000")]
    pub stress_p95_max: f64,

    /// Maximum error rate (%)
    #[arg(long, default_value = "5")]
    pub stress_error_max: f64,

    /// Output JSON format
    #[arg(long)]
    pub output_json: bool,

    /// Verbose output
    #[arg(long, short)]
    pub verbose: bool,
}

impl GateCommand {
    pub async fn execute(&self) -> anyhow::Result<i32> {
        // Verify project exists
        if LocalStorage::load_project(&self.project).is_err() {
            anyhow::bail!(
                "Project '{}' not found. Run 'qinAegis project add' first.",
                self.project
            );
        }

        let thresholds = GateThresholds::new(
            self.e2e_threshold,
            self.perf_threshold,
            self.stress_rps_min,
            self.stress_p95_max,
            self.stress_error_max,
        );

        let service = GateService::new(
            self.project.clone(),
            self.run_id.clone(),
            thresholds,
        );

        let result = service.check().await?;

        if self.output_json {
            qin_aegis_core::gate::print_gate_result_json(&result);
        } else {
            qin_aegis_core::gate::print_gate_result(&result, self.verbose);
        }

        Ok(result.exit_code)
    }
}