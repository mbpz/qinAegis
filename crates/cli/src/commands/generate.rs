use qin_aegis_core::{TestCaseGenerator, Critic, MiniMaxClient};
use qin_aegis_core::storage::LocalStorage;
use crate::config::Config;
use std::path::Path;

pub async fn run_generate(project_name: &str, requirement_text: &str, spec_path: &Path) -> anyhow::Result<()> {
    let config = Config::load()?
        .ok_or_else(|| anyhow::anyhow!("run qinAegis init first"))?;

    // Check project exists
    let _ = LocalStorage::load_project(project_name)
        .map_err(|_| anyhow::anyhow!("Project '{}' not found.", project_name))?;

    let spec_markdown = std::fs::read_to_string(spec_path)?;

    println!("Generating test cases for requirement: {}", requirement_text);

    let llm = MiniMaxClient::new(
        config.llm.base_url,
        config.llm.api_key,
        config.llm.model,
    );

    let generator = TestCaseGenerator::new(llm.clone());
    let cases = generator.generate(&spec_markdown, requirement_text).await?;

    println!("\n✓ Generated {} test cases", cases.len());

    let critic = Critic::new(llm);

    // Save cases to local storage
    for tc in &cases {
        let review = critic.review(&tc.yaml_script, &spec_markdown, requirement_text).await;

        let (score, issues) = match review {
            Ok(r) => (r.score, r.issues),
            Err(e) => {
                println!("  {} - critic failed: {}", tc.name, e);
                (0, vec![])
            }
        };

        println!("  {} - score: {}/10", tc.name, score);
        if !issues.is_empty() {
            for issue in &issues {
                println!("    ⚠ {}", issue);
            }
        }

        let test_case = qin_aegis_core::storage::TestCase {
            id: tc.id.clone(),
            name: tc.name.clone(),
            requirement_id: tc.requirement_id.clone(),
            test_type: tc.case_type.clone(),
            yaml_script: tc.yaml_script.clone(),
            priority: tc.priority.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        LocalStorage::save_case(project_name, &test_case)?;
    }

    println!("\n✓ Test cases saved to ~/.qinAegis/projects/{}/cases/", project_name);

    Ok(())
}
