// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use qin_aegis_core::{
    ArcLlmClient, CaseStatus, MiniMaxClient, TestCaseService,
    storage::{LocalStorage, Storage},
};
use crate::config::Config;

pub async fn run_review(
    project_name: &str,
    action: Option<ReviewAction>,
) -> anyhow::Result<()> {
    let config = Config::load()?
        .ok_or_else(|| anyhow::anyhow!("run qinAegis init first"))?;

    if !config.is_llm_configured() {
        anyhow::bail!("LLM not configured. Run 'qinAegis init' first.");
    }

    // Verify project exists
    let _ = LocalStorage::load_project(project_name)
        .map_err(|_| anyhow::anyhow!("Project '{}' not found.", project_name))?;

    let llm = ArcLlmClient::new(MiniMaxClient::new(
        config.llm.base_url,
        config.llm.api_key,
        config.llm.model,
    ));

    let service = TestCaseService::new(llm, qin_aegis_core::storage::LocalStorageInstance::new());

    match action {
        None => {
            // List all draft and reviewed cases
            println!("\n=== Draft Cases ===\n");
            let drafts = service.list_by_status(project_name, CaseStatus::Draft).await?;
            if drafts.is_empty() {
                println!("  (no draft cases)");
            }
            for c in &drafts {
                println!("  [{}] {} ({})", c.id, c.name, c.priority);
            }

            println!("\n=== Reviewed Cases ===\n");
            let reviewed = service.list_by_status(project_name, CaseStatus::Reviewed).await?;
            if reviewed.is_empty() {
                println!("  (no reviewed cases)");
            }
            for c in &reviewed {
                println!("  [{}] {} ({})", c.id, c.name, c.priority);
            }

            println!("\n=== Approved Cases ===\n");
            let approved = service.list_by_status(project_name, CaseStatus::Approved).await?;
            if approved.is_empty() {
                println!("  (no approved cases)");
            }
            for c in &approved {
                println!("  [{}] {} ({})", c.id, c.name, c.priority);
            }
        }
        Some(ReviewAction::Approve { case_id }) => {
            service.review_case(project_name, &case_id, true).await?;
            println!("✓ Case {} approved and ready to run.", case_id);
        }
        Some(ReviewAction::Reject { case_id }) => {
            service.reject_case(project_name, &case_id).await?;
            println!("✓ Case {} rejected back to draft for rewrite.", case_id);
        }
        Some(ReviewAction::Archive { case_id }) => {
            // Try to find the case and archive it
            let all_cases = qin_aegis_core::storage::LocalStorageInstance::new()
                .load_cases(project_name).await?;
            let target = all_cases.iter().find(|c| c.id == case_id)
                .ok_or_else(|| anyhow::anyhow!("Case '{}' not found.", case_id))?;
            service.archive_case(project_name, &case_id, target.status).await?;
            println!("✓ Case {} archived.", case_id);
        }
        Some(ReviewAction::Flaky { case_id }) => {
            let all_cases = qin_aegis_core::storage::LocalStorageInstance::new()
                .load_cases(project_name).await?;
            let target = all_cases.iter().find(|c| c.id == case_id)
                .ok_or_else(|| anyhow::anyhow!("Case '{}' not found.", case_id))?;
            qin_aegis_core::storage::LocalStorageInstance::new()
                .move_case(project_name, &case_id, target.status, CaseStatus::Flaky).await?;
            println!("✓ Case {} marked as flaky.", case_id);
        }
        Some(ReviewAction::Stabilize { case_id }) => {
            service.stabilize_case(project_name, &case_id).await?;
            println!("✓ Case {} stabilized back to approved.", case_id);
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub enum ReviewAction {
    /// Approve a draft case directly (Draft → Approved).
    Approve { case_id: String },
    /// Reject a reviewed case back to draft.
    Reject { case_id: String },
    /// Archive a case (from any state to Archived).
    Archive { case_id: String },
    /// Mark a case as flaky.
    Flaky { case_id: String },
    /// Stabilize a flaky case back to approved.
    Stabilize { case_id: String },
}
