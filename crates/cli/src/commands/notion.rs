use crate::config::Config;
use qin_aegis_notion::{NotionClient, get_notion_token};

pub async fn list_projects() -> anyhow::Result<()> {
    let token = get_notion_token()?
        .ok_or_else(|| anyhow::anyhow!("Not logged in. Run 'qinAegis setup' first."))?;

    let config = Config::load()?
        .ok_or_else(|| anyhow::anyhow!("No configuration. Run 'qinAegis setup' first."))?;

    let db_id = &config.notion.workspace_id;
    if db_id.is_empty() {
        anyhow::bail!("Projects DB not configured. Run 'qinAegis init' first.");
    }

    let notion = NotionClient::new(&token);
    let projects = notion.query_projects(db_id).await?;

    println!("Projects:");
    if projects.is_empty() {
        println!("  (no projects found)");
    }
    for project in projects {
        let status_str = match project.status {
            qin_aegis_notion::models::ProjectStatus::Active => "active",
            qin_aegis_notion::models::ProjectStatus::Archived => "archived",
        };
        println!("  - {} ({}) - {}", project.name, project.id, status_str);
    }

    Ok(())
}