// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use qin_aegis_core::{TestExecutor, TestCaseRef, Reporter, LlmConfig, SandboxConfig};
use qin_aegis_core::knowledge::KnowledgeBase;
use qin_aegis_core::storage::LocalStorage;
use crate::config::Config;

pub async fn run_tests(
    project_name: &str,
    test_type: &str,
    concurrency: usize,
) -> anyhow::Result<()> {
    let config = Config::load()?
        .ok_or_else(|| anyhow::anyhow!("run qinAegis init first"))?;

    // Load project
    let _project = LocalStorage::load_project(project_name)
        .map_err(|_| anyhow::anyhow!("Project '{}' not found. Run 'qinAegis project add' first.", project_name))?;

    // Load cases
    let mut cases = LocalStorage::load_cases(project_name)?;

    if cases.is_empty() {
        println!("No test cases found for project '{}'.", project_name);
        println!("Run 'qinAegis generate' first.");
        return Ok(());
    }

    // Filter by test type if specified
    if test_type != "all" {
        cases.retain(|c| c.test_type == test_type);
    }

    if cases.is_empty() {
        println!("No {} test cases found.", test_type);
        return Ok(());
    }

    println!("Running {} test cases (concurrency={})...", cases.len(), concurrency);

    let llm_config = Some(LlmConfig {
        api_key: config.llm.api_key,
        base_url: config.llm.base_url,
        model: config.llm.model,
    });

    let sandbox_config = Some(SandboxConfig {
        cdp_port: config.sandbox.cdp_port,
    });

    let executor = TestExecutor::new(concurrency, llm_config, sandbox_config).await?;

    let case_refs: Vec<TestCaseRef> = cases
        .iter()
        .map(|c| TestCaseRef {
            id: c.id.clone(),
            yaml_script: c.yaml_script.clone(),
            name: c.name.clone(),
            priority: c.priority.clone(),
        })
        .collect();

    let results = executor.run_parallel(case_refs).await?;
    executor.shutdown().await?;

    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    // Save summary
    let report_dir = LocalStorage::report_dir(project_name, &run_id);
    std::fs::create_dir_all(&report_dir)?;

    let summary_path = Reporter::save_summary(&run_id, &results)?;
    println!("Summary saved: {}", summary_path.display());

    // Generate HTML report
    match Reporter::generate_run_report(project_name, &run_id, &results, None, None) {
        Ok(html_path) => println!("HTML report saved: {}", html_path.display()),
        Err(e) => eprintln!("Warning: HTML report generation failed: {}", e),
    }

    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.len() - passed;
    println!("\nRun complete: {}/{} passed", passed, failed);

    // Record knowledge base artifacts
    let kb = KnowledgeBase::new(project_name);
    if let Err(e) = kb.record_failures(project_name, &results, &run_id) {
        eprintln!("Warning: Failed to record failure patterns: {}", e);
    }
    if let Err(e) = kb.record_flakiness(&results) {
        eprintln!("Warning: Failed to record flakiness: {}", e);
    }

    // Print knowledge insights
    if failed > 0 {
        match kb.load_failure_patterns() {
            Ok(patterns) => {
                if let Some(dominant) = patterns.dominant_category() {
                    println!("\n📊 Knowledge insight: dominant failure category is '{}'", dominant.as_str());
                }
            }
            Err(_) => {}
        }
    }

    // Flag flaky cases
    match kb.load_flakiness() {
        Ok(index) => {
            let flaky_cases: Vec<_> = index.cases.iter()
                .filter(|(_, r)| r.flaky_score > 30.0 && r.total_runs >= 3)
                .collect();
            if !flaky_cases.is_empty() {
                println!("\n⚠️  Flaky cases detected (score > 30):");
                for (id, record) in flaky_cases {
                    println!("  - {}: flaky_score={:.0}, {}/{} passes",
                        id, record.flaky_score, record.total_passes, record.total_runs);
                }
            }
        }
        Err(_) => {}
    }

    Ok(())
}
