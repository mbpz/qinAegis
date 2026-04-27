use qin_aegis_core::{TestExecutor, TestCaseRef, Reporter, LlmConfig};
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
    let project = LocalStorage::load_project(project_name)
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

    let executor = TestExecutor::new(concurrency, llm_config).await?;

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

    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.len() - passed;
    println!("\nRun complete: {}/{} passed", passed, failed);

    Ok(())
}
