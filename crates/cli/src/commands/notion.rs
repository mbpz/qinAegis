// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::config::Config;
use qin_aegis_core::storage::LocalStorage;

pub async fn list_projects() -> anyhow::Result<()> {
    let config = Config::load()?
        .ok_or_else(|| anyhow::anyhow!("No configuration. Run 'qinAegis init' first."))?;

    if !config.is_llm_configured() {
        anyhow::bail!("LLM not configured. Run 'qinAegis init' first.");
    }

    let projects = LocalStorage::list_projects()?;

    println!("Projects:");
    if projects.is_empty() {
        println!("  (no projects found)");
    }
    for name in projects {
        println!("  - {}", name);
    }

    Ok(())
}