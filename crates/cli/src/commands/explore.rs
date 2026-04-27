use qin_aegis_core::{Explorer, LlmConfig};
use qin_aegis_core::storage::LocalStorage;
use crate::config::Config;

pub async fn run_explore(project_name: &str, seed_url: Option<String>, max_depth: u32) -> anyhow::Result<()> {
    // Load config
    let config = Config::load()?
        .ok_or_else(|| anyhow::anyhow!("run qinAegis init first"))?;

    if !config.is_llm_configured() {
        anyhow::bail!("LLM not configured. Run 'qinAegis init' first.");
    }

    // Check project exists, get URL
    let project = LocalStorage::load_project(project_name)
        .map_err(|_| anyhow::anyhow!("Project '{}' not found. Run 'qinAegis project add' first.", project_name))?;

    let url = seed_url.unwrap_or_else(|| project.url.clone());

    println!("Exploring {} from {}", project_name, url);
    println!("Max depth: {}\n", max_depth);

    let llm_config = Some(LlmConfig {
        api_key: config.llm.api_key,
        base_url: config.llm.base_url,
        model: config.llm.model,
    });

    let mut explorer = Explorer::new(llm_config).await?;

    let result = explorer.explore(&url, max_depth).await?;

    let mut all_markdown = String::from("# 项目规格书\n\n");
    all_markdown.push_str(&result.markdown);

    let spec_path = LocalStorage::save_spec(project_name, &all_markdown)?;
    println!("\n✓ Exploration complete: {} pages", result.pages.len());
    println!("✓ Spec saved to: {}", spec_path.display());

    explorer.shutdown().await?;
    Ok(())
}
