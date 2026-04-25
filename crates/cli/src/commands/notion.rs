use qin_aegis_notion::{
    NotionClient, get_notion_token,
    PROJECTS_DB_SPEC, REQUIREMENTS_DB_SPEC, TEST_CASES_DB_SPEC, TEST_RESULTS_DB_SPEC,
};
use anyhow::Context;
use std::path::PathBuf;

pub async fn init_databases() -> anyhow::Result<()> {
    let token = get_notion_token()?
        .ok_or_else(|| anyhow::anyhow!("Not logged in. Run 'qinAegis init' first."))?;

    let notion = NotionClient::new(&token);

    println!("Initializing Notion databases...");

    // Get parent page ID from config
    let config_path = config_path()?;
    let config_content = std::fs::read_to_string(&config_path)
        .context("No config found. Run 'qinAegis init' first.")?;

    let config: toml::Value = config_content.parse()
        .context("Invalid config format")?;

    let parent_page_id = config.get("notion")
        .and_then(|n| n.get("page_id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No notion page_id in config"))?;

    println!("Creating Projects database...");
    let projects_db_id = notion.create_database(parent_page_id, &PROJECTS_DB_SPEC).await?;
    println!("  Created: {}", projects_db_id);

    println!("Creating Requirements database...");
    let requirements_db_id = notion.create_database(parent_page_id, &REQUIREMENTS_DB_SPEC).await?;
    println!("  Created: {}", requirements_db_id);

    println!("Creating TestCases database...");
    let test_cases_db_id = notion.create_database(parent_page_id, &TEST_CASES_DB_SPEC).await?;
    println!("  Created: {}", test_cases_db_id);

    println!("Creating TestResults database...");
    let test_results_db_id = notion.create_database(parent_page_id, &TEST_RESULTS_DB_SPEC).await?;
    println!("  Created: {}", test_results_db_id);

    // Save DB IDs to config
    let mut config_table = toml::map::Map::new();
    if let Ok(c) = config_content.parse::<toml::Value>() {
        if let Some(t) = c.as_table() {
            config_table = t.clone();
        }
    }

    let mut databases_table = toml::map::Map::new();
    databases_table.insert("projects".to_string(), toml::Value::String(projects_db_id.clone()));
    databases_table.insert("requirements".to_string(), toml::Value::String(requirements_db_id.clone()));
    databases_table.insert("test_cases".to_string(), toml::Value::String(test_cases_db_id.clone()));
    databases_table.insert("test_results".to_string(), toml::Value::String(test_results_db_id));
    config_table.insert("databases".to_string(), toml::Value::Table(databases_table));

    std::fs::write(&config_path, toml::to_string_pretty(&toml::Value::Table(config_table))?)?;

    println!("\n✅ All databases initialized!");
    println!("Database IDs saved to ~/.config/qinAegis/config.toml");

    Ok(())
}

pub async fn list_projects() -> anyhow::Result<()> {
    let token = get_notion_token()?
        .ok_or_else(|| anyhow::anyhow!("Not logged in. Run 'qinAegis init' first."))?;

    let notion = NotionClient::new(&token);
    let config = load_config()?;

    let db_id = config.get("databases")
        .and_then(|d| d.get("projects"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Projects DB not configured. Run 'qinAegis init-db' first."))?;

    let projects = notion.query_projects(db_id).await?;

    println!("Projects:");
    for project in projects {
        let status_str = match project.status {
            qin_aegis_notion::models::ProjectStatus::Active => "active",
            qin_aegis_notion::models::ProjectStatus::Archived => "archived",
        };
        println!("  - {} ({}) - {}", project.name, project.id, status_str);
    }

    Ok(())
}

fn config_path() -> anyhow::Result<PathBuf> {
    Ok(dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
        .join("qinAegis")
        .join("config.toml"))
}

fn load_config() -> anyhow::Result<toml::Value> {
    let config_path = config_path()?;
    let content = std::fs::read_to_string(&config_path)?;
    content.parse().map_err(|e| anyhow::anyhow!("parse error: {}", e))
}