// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::critic::Critic;
use crate::generator::TestCaseGenerator;
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
        self.generate_and_save_with_context(project_name, spec_markdown, requirement_text, None)
            .await
    }

    pub async fn generate_and_save_with_context(
        &self,
        project_name: &str,
        spec_markdown: &str,
        requirement_text: &str,
        context: Option<crate::execution_records::GenerationContext>,
    ) -> anyhow::Result<Vec<SaveResult>> {
        if let Some(context) = &context {
            context.validate(project_name)?;
        }
        let generator = TestCaseGenerator::new(self.llm.clone());
        let critic = Critic::new(self.llm.clone());

        let cases = generator.generate(spec_markdown, requirement_text).await?;
        validate_generated_cases(&cases)?;
        for case in &cases {
            anyhow::ensure!(
                crate::storage::LocalStorage::find_case_path(project_name, &case.id).is_none(),
                "generated case identity already exists; review or regenerate with a new identity"
            );
        }
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

            let mut storage_case = StorageTestCase {
                id: tc.id.clone(),
                name: tc.name.clone(),
                requirement_id: tc.requirement_id.clone(),
                test_type: tc.case_type.clone(),
                yaml_script: tc.yaml_script.clone(),
                priority: tc.priority.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                status: CaseStatus::Draft,
                provenance: None,
            };

            if let Some(context) = &context {
                let provenance = crate::execution_records::CaseProvenance {
                    schema_version: 1,
                    project: project_name.into(),
                    case_id: tc.id.clone(),
                    yaml_script: tc.yaml_script.clone(),
                    created_at: storage_case.created_at.clone(),
                    contracts: context.contracts.clone(),
                    model: context.model.clone(),
                };
                storage_case.provenance = Some(provenance);
            }
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
        // Draft -> Reviewed always first, then optionally Reviewed -> Approved
        self.storage
            .move_case(
                project_name,
                case_id,
                CaseStatus::Draft,
                CaseStatus::Reviewed,
            )
            .await?;
        if approved {
            self.storage
                .move_case(
                    project_name,
                    case_id,
                    CaseStatus::Reviewed,
                    CaseStatus::Approved,
                )
                .await?;
        }
        Ok(())
    }

    /// Approve a reviewed case (moves to approved, ready to run).
    pub async fn approve_case(&self, project_name: &str, case_id: &str) -> anyhow::Result<()> {
        self.storage
            .move_case(
                project_name,
                case_id,
                CaseStatus::Reviewed,
                CaseStatus::Approved,
            )
            .await?;
        Ok(())
    }

    /// Reject a reviewed case back to draft for rewrite.
    pub async fn reject_case(&self, project_name: &str, case_id: &str) -> anyhow::Result<()> {
        self.storage
            .move_case(
                project_name,
                case_id,
                CaseStatus::Reviewed,
                CaseStatus::Draft,
            )
            .await?;
        Ok(())
    }

    /// Mark a flaky case as stabilized (back to approved).
    pub async fn stabilize_case(&self, project_name: &str, case_id: &str) -> anyhow::Result<()> {
        self.storage
            .move_case(
                project_name,
                case_id,
                CaseStatus::Flaky,
                CaseStatus::Approved,
            )
            .await?;
        Ok(())
    }

    /// Archive a case (from approved or flaky).
    pub async fn archive_case(
        &self,
        project_name: &str,
        case_id: &str,
        current_status: CaseStatus,
    ) -> anyhow::Result<()> {
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
        Ok(self
            .storage
            .load_cases_by_status(project_name, status)
            .await?)
    }
}

// Generated data is untrusted. Validate the complete batch before any storage
// writes, rather than allowing a model to choose filesystem paths or overwrite
// an existing case that may already have been reviewed.
fn validate_generated_cases(cases: &[crate::generator::TestCase]) -> anyhow::Result<()> {
    anyhow::ensure!(
        !cases.is_empty() && cases.len() <= 200,
        "generated batch must contain 1..200 cases"
    );
    let mut ids = std::collections::BTreeSet::new();
    for case in cases {
        anyhow::ensure!(
            !case.id.is_empty()
                && case.id.len() <= 128
                && !case.id.starts_with('.')
                && case
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')),
            "invalid generated case identity"
        );
        anyhow::ensure!(ids.insert(&case.id), "duplicate generated case identity");
        anyhow::ensure!(
            !case.name.trim().is_empty() && !case.yaml_script.trim().is_empty(),
            "generated case name and script are required"
        );
    }
    Ok(())
}

#[cfg(test)]
mod generation_boundary_tests {
    use super::*;
    fn case(id: &str) -> crate::generator::TestCase {
        crate::generator::TestCase {
            id: id.into(),
            name: "Receipt".into(),
            requirement_id: "r".into(),
            case_type: "smoke".into(),
            priority: "P1".into(),
            yaml_script: "tasks: []".into(),
            expected_result: "receipt".into(),
            tags: vec![],
        }
    }
    #[test]
    fn generated_batch_rejects_paths_and_duplicate_ids_before_storage() {
        for id in ["", ".hidden", "../escape", "/tmp/out", "a/b", "a\\b"] {
            assert!(validate_generated_cases(&[case("valid"), case(id)]).is_err());
        }
        assert!(validate_generated_cases(&[case("same"), case("same")]).is_err());
        assert!(validate_generated_cases(&[]).is_err());
        assert!(validate_generated_cases(&[case("receipt-v1")]).is_ok());
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
