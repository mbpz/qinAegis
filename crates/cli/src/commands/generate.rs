// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use qin_aegis_core::{ArcLlmClient, MiniMaxClient, TestCaseService};
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

    let llm = ArcLlmClient::new(MiniMaxClient::new(
        config.llm.base_url,
        config.llm.api_key,
        config.llm.model,
    ));

    let service = TestCaseService::new(llm, qin_aegis_core::storage::LocalStorageInstance::new());
    let results = service.generate_and_save(project_name, &spec_markdown, requirement_text).await?;

    println!("\n✓ Generated and saved {} test cases", results.len());
    for r in &results {
        println!("  {} - score: {}/10", r.case_name, r.score);
        if !r.issues.is_empty() {
            for issue in &r.issues {
                println!("    ⚠ {}", issue);
            }
        }
    }

    println!("\n✓ Test cases saved to ~/.qinAegis/projects/{}/cases/", project_name);

    Ok(())
}
