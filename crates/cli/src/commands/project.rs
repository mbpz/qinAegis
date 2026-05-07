// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use qin_aegis_core::storage::LocalStorage;
use crate::config::Config;

pub async fn add_project(name: &str, url: &str) -> anyhow::Result<()> {
    let config = Config::load()?
        .ok_or_else(|| anyhow::anyhow!("run qinAegis init first"))?;

    if !config.is_llm_configured() {
        anyhow::bail!("LLM not configured. Run 'qinAegis init' first.");
    }

    let tech_stack = vec![];
    LocalStorage::init_project(name, url, tech_stack)?;
    let project_dir = LocalStorage::project_dir(name);

    println!("✓ Project '{}' created at {}", name, project_dir.display());
    println!("  URL: {}", url);
    println!("\nNext: qinAegis explore {}", name);

    Ok(())
}

pub async fn list_projects() -> anyhow::Result<()> {
    let projects = LocalStorage::list_projects()?;

    if projects.is_empty() {
        println!("No projects found.");
        println!("Run 'qinAegis project add <name>' to create one.");
        return Ok(());
    }

    println!("Projects ({}):\n", projects.len());
    for name in &projects {
        match LocalStorage::load_project(name) {
            Ok(cfg) => {
                println!("  {}  {}", name, cfg.url);
            }
            Err(_) => {
                println!("  {}  (broken config)", name);
            }
        }
    }

    Ok(())
}

pub async fn remove_project(name: &str) -> anyhow::Result<()> {
    let projects = LocalStorage::list_projects()?;
    if !projects.contains(&name.to_string()) {
        anyhow::bail!("Project '{}' does not exist", name);
    }

    LocalStorage::delete_project(name)?;
    println!("✓ Project '{}' deleted", name);

    Ok(())
}
