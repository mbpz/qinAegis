use qin_aegis_core::{TestCaseGenerator, Critic, MiniMaxClient};
use std::path::Path;

pub async fn run_generate(requirement_text: &str, spec_path: &Path) -> anyhow::Result<()> {
    let spec_markdown = std::fs::read_to_string(spec_path)?;

    println!("Generating test cases for requirement: {}", requirement_text);

    let llm = MiniMaxClient::new(
        std::env::var("MINIMAX_BASE_URL")
            .unwrap_or_else(|_| "https://api.minimax.chat/v1".to_string()),
        std::env::var("MINIMAX_API_KEY").unwrap_or_default(),
        std::env::var("MINIMAX_MODEL")
            .unwrap_or_else(|_| "MiniMax-VL-01".to_string()),
    );

    let generator = TestCaseGenerator::new(llm.clone());
    let cases = generator.generate(&spec_markdown, requirement_text).await?;

    println!("\n✓ Generated {} test cases", cases.len());

    let critic = Critic::new(llm);
    for tc in &cases {
        match critic.review(&tc.yaml_script, &spec_markdown, requirement_text).await {
            Ok(review) => {
                println!("  {} - score: {}/10", tc.name, review.score);
                if !review.issues.is_empty() {
                    for issue in &review.issues {
                        println!("    ⚠ {}", issue);
                    }
                }
            }
            Err(e) => {
                println!("  {} - critic failed: {}", tc.name, e);
            }
        }
    }

    println!("\n✓ Test cases ready (write to Notion in Phase 3)");
    Ok(())
}
