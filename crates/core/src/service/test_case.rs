// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::generator::TestCaseGenerator;
use crate::critic::Critic;
use crate::llm::ArcLlmClient;
use crate::storage::{LocalStorageInstance, Storage, TestCase as StorageTestCase};

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
}

#[derive(Debug)]
pub struct SaveResult {
    pub case_id: String,
    pub case_name: String,
    pub score: u8,
    pub issues: Vec<String>,
    pub saved: bool,
}
