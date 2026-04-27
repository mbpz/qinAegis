use qin_aegis_core::{TestExecutor, TestCaseRef, Reporter, LlmConfig};
use qin_aegis_notion::{NotionClient};
use qin_aegis_notion::writer::NotionWriter;
use qin_aegis_notion::models::TestCase;
use crate::config::Config;

pub async fn run_tests(
    test_type: &str,
    _project_id: &str,
    concurrency: usize,
) -> anyhow::Result<()> {
    println!("Loading test cases from Notion...");
    let token = qin_aegis_notion::auth::get_notion_token()?
        .ok_or_else(|| anyhow::anyhow!("not logged in, run qinAegis init first"))?;
    let notion = NotionClient::new(&token);

    // Query approved test cases
    let test_cases_db_id = std::env::var("QIN_AEGIS_TEST_CASES_DB_ID")
        .unwrap_or_else(|_| "test_cases_db".to_string());

    let cases: Vec<TestCase> = notion
        .query_test_cases(&test_cases_db_id, Some(test_type), Some("Approved"))
        .await?;

    if cases.is_empty() {
        println!("No approved {} test cases found.", test_type);
        return Ok(());
    }

    println!("Running {} test cases (concurrency={})...", cases.len(), concurrency);

    // Load config for LLM settings
    let llm_config = Config::load()?
        .map(|cfg| LlmConfig {
            api_key: cfg.llm.api_key,
            base_url: cfg.llm.base_url,
            model: cfg.llm.model,
        });

    let executor = TestExecutor::new(concurrency, llm_config).await?;

    let case_refs: Vec<TestCaseRef> = cases
        .iter()
        .map(|c| {
            let priority_str = match c.priority {
                qin_aegis_notion::models::Priority::P0 => "high",
                qin_aegis_notion::models::Priority::P1 => "medium",
                qin_aegis_notion::models::Priority::P2 => "low",
            };
            TestCaseRef {
                id: c.id.clone(),
                yaml_script: c.yaml_script.clone(),
                name: c.name.clone(),
                priority: priority_str.to_string(),
            }
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
    let summary_path = Reporter::save_summary(&run_id, &results)?;
    println!("Summary saved: {}", summary_path.display());

    // Write to Notion
    println!("Writing results to Notion...");
    let test_results_db_id = std::env::var("QIN_AEGIS_TEST_RESULTS_DB_ID")
        .unwrap_or_else(|_| "test_results_db".to_string());
    let writer = NotionWriter::new(&notion, &test_results_db_id);

    for (case, result) in cases.iter().zip(results.iter()) {
        let result_json = serde_json::json!({
            "passed": result.passed,
            "duration_ms": result.duration_ms,
            "error_message": result.error_message,
        });
        let _page_id = writer
            .write_result(
                &case.id,
                &case.name,
                &case.id,
                &result_json,
                &run_id,
                None,
            )
            .await?;
        println!(
            "  {}: {}",
            case.name,
            if result.passed { "PASS" } else { "FAIL" }
        );
    }

    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.len() - passed;
    println!("\nRun complete: {}/{} passed", passed, failed);

    Ok(())
}
