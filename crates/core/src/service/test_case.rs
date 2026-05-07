// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::generator::TestCaseGenerator;
use crate::critic::Critic;
use crate::llm::ArcLlmClient;
use crate::storage::{CaseStatus, LocalStorageInstance, Storage, TestCase as StorageTestCase};

pub struct TestCaseService {
    llm: ArcLlmClient,
    storage: LocalStorageInstance,
}

impl TestCaseService {
    pub fn new(llm: ArcLlmClient, storage: LocalStorageInstance) -> Self {
        Self { llm, storage }
    }

    pub async fn generate_and_save(
        &self,
        project_name: &str,
        spec_markdown: &str,
        requirement_text: &str,
    ) -> anyhow::Result<Vec<SaveResult>> {
        let generator = TestCaseGenerator::new(self.llm.clone());
        let critic = Critic::new(self.llm.clone());

        let cases = generator.generate(spec_markdown, requirement_text).await?;
        let mut results = Vec::with_capacity(cases.len());

        for tc in &cases {
            let review = critic
                .review(&tc.yaml_script, spec_markdown, requirement_text)
                .await;

            let (score, issues) = match &review {
                Ok(r) => (r.score, r.issues.clone()),
                Err(e) => {
                    tracing::warn!("critic review failed for {}: {}", tc.name, e);
                    (0, vec![])
                }
            };

            let storage_case = StorageTestCase {
                id: tc.id.clone(),
                name: tc.name.clone(),
                requirement_id: tc.requirement_id.clone(),
                test_type: tc.case_type.clone(),
                yaml_script: tc.yaml_script.clone(),
                priority: tc.priority.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                status: CaseStatus::Draft,
            };

            self.storage.save_case(project_name, &storage_case).await?;

            results.push(SaveResult {
                case_id: tc.id.clone(),
                case_name: tc.name.clone(),
                score,
                issues,
                saved: true,
            });
        }

        Ok(results)
    }

    /// Review a draft case (accepts it, moving to reviewed or approved based on score).
    pub async fn review_case(
        &self,
        project_name: &str,
        case_id: &str,
        approved: bool,
    ) -> anyhow::Result<()> {
        if approved {
            self.storage
                .move_case(project_name, case_id, CaseStatus::Draft, CaseStatus::Approved)
                .await?;
        } else {
            self.storage
                .move_case(project_name, case_id, CaseStatus::Draft, CaseStatus::Reviewed)
                .await?;
        }
        Ok(())
    }

    /// Approve a reviewed case (moves to approved, ready to run).
    pub async fn approve_case(&self, project_name: &str, case_id: &str) -> anyhow::Result<()> {
        self.storage
            .move_case(project_name, case_id, CaseStatus::Reviewed, CaseStatus::Approved)
            .await?;
        Ok(())
    }

    /// Reject a reviewed case back to draft for rewrite.
    pub async fn reject_case(&self, project_name: &str, case_id: &str) -> anyhow::Result<()> {
        self.storage
            .move_case(project_name, case_id, CaseStatus::Reviewed, CaseStatus::Draft)
            .await?;
        Ok(())
    }

    /// Mark a flaky case as stabilized (back to approved).
    pub async fn stabilize_case(&self, project_name: &str, case_id: &str) -> anyhow::Result<()> {
        self.storage
            .move_case(project_name, case_id, CaseStatus::Flaky, CaseStatus::Approved)
            .await?;
        Ok(())
    }

    /// Archive a case (from approved or flaky).
    pub async fn archive_case(&self, project_name: &str, case_id: &str, current_status: CaseStatus) -> anyhow::Result<()> {
        self.storage
            .move_case(project_name, case_id, current_status, CaseStatus::Archived)
            .await?;
        Ok(())
    }

    /// List cases by status.
    pub async fn list_by_status(
        &self,
        project_name: &str,
        status: CaseStatus,
    ) -> anyhow::Result<Vec<StorageTestCase>> {
        Ok(self.storage.load_cases_by_status(project_name, status).await?)
    }
}

#[derive(Debug)]
pub struct SaveResult {
    pub case_id: String,
    pub case_name: String,
    pub score: u8,
    pub issues: Vec<String>,
    pub saved: bool,
}
