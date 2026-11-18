// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::critic::Critic;
use crate::generator::TestCaseGenerator;
use crate::llm::ArcLlmClient;
use crate::storage::{CaseStatus, LocalStorageInstance, Storage, TestCase as StorageTestCase};

pub struct TestCaseService {
    llm: ArcLlmClient,
    storage: LocalStorageInstance,
    model_trace: Option<std::sync::Arc<crate::models::ModelCallTrace>>,
    output_secrets: Vec<String>,
    cancel_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl TestCaseService {
    pub fn new(llm: ArcLlmClient, storage: LocalStorageInstance) -> Self {
        Self {
            llm,
            storage,
            model_trace: None,
            output_secrets: Vec::new(),
            cancel_flag: None,
        }
    }

    /// Frozen credentials are compared in memory only, before decoded model
    /// output can reach names, diagnostics, provenance or persistent assets.
    pub fn with_output_secrets(mut self, secrets: Vec<String>) -> Self {
        self.output_secrets = secrets.into_iter().filter(|s| !s.is_empty()).collect();
        self
    }

    /// Stops generation at model and storage boundaries. Already committed cases
    /// remain saved; cancellation cannot interrupt an atomic storage write.
    pub fn with_cancel_flag(
        mut self,
        flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        self.cancel_flag = Some(flag);
        self
    }

    fn ensure_not_cancelled(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.cancel_flag.as_ref().is_some_and(|flag| {
                flag.load(std::sync::atomic::Ordering::Acquire)
            }),
            "Generation cancelled"
        );
        Ok(())
    }

    pub fn with_model_trace(
        mut self,
        trace: std::sync::Arc<crate::models::ModelCallTrace>,
    ) -> Self {
        self.model_trace = Some(trace);
        self
    }
    fn trace_snapshot(&self) -> anyhow::Result<Vec<crate::models::ModelCallAttempt>> {
        self.model_trace
            .as_ref()
            .map(|t| t.snapshot())
            .transpose()
            .map(|v| v.unwrap_or_default())
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
        let echoed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let guarded = ArcLlmClient::new(OutputGuard {
            client: self.llm.clone(), secrets: self.output_secrets.clone(), echoed: echoed.clone(),
        });
        let generator = TestCaseGenerator::new(guarded.clone());
        let critic = Critic::new(guarded);

        anyhow::ensure!(
            self.model_trace.is_none() || context.is_some(),
            "model trace requires generation context"
        );
        let generation_start = self.trace_snapshot()?.len();
        self.ensure_not_cancelled()?;
        let generated = generator.generate(spec_markdown, requirement_text).await;
        self.ensure_not_cancelled()?;
        let cases = generated?;
        let generation_calls = self.trace_snapshot()?[generation_start..].to_vec();
        validate_generated_cases(&cases)?;
        for case in &cases {
            anyhow::ensure!(
                crate::storage::LocalStorage::find_case_path(project_name, &case.id).is_none(),
                "generated case identity already exists; review or regenerate with a new identity"
            );
        }
        let mut results = Vec::with_capacity(cases.len());

        for tc in &cases {
            let critic_start = self.trace_snapshot()?.len();
            self.ensure_not_cancelled()?;
            let review = critic
                .review(&tc.yaml_script, spec_markdown, requirement_text)
                .await;
            self.ensure_not_cancelled()?;

            if review.as_ref().err().is_some_and(|error| matches!(
                error.downcast_ref::<crate::llm::LlmError>(),
                Some(crate::llm::LlmError::EvidenceUnavailable)
            )) {
                anyhow::bail!("model call evidence persistence failed");
            }
            anyhow::ensure!(!echoed.load(std::sync::atomic::Ordering::Acquire), ECHO_REJECTION);
            let (score, issues) = match &review {
                Ok(r) => (r.score, r.issues.clone()),
                Err(_) => {
                    tracing::warn!("critic review failed; draft retained without a successful review");
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
                    specification: context.specification.clone(),
                    schema_version: 1,
                    project: project_name.into(),
                    case_id: tc.id.clone(),
                    yaml_script: tc.yaml_script.clone(),
                    created_at: storage_case.created_at.clone(),
                    contracts: context.contracts.clone(),
                    model: context.model.clone(),
                    model_calls: self
                        .model_trace
                        .as_ref()
                        .map(|_| -> anyhow::Result<_> {
                            Ok(crate::execution_records::GenerationModelCalls {
                                generator: generation_calls.clone(),
                                critic: self.trace_snapshot()?[critic_start..].to_vec(),
                            })
                        })
                        .transpose()?,
                };
                provenance.validate(project_name, &tc.id, &tc.yaml_script)?;
                storage_case.provenance = Some(provenance);
            }
            self.ensure_not_cancelled()?;
            self.storage.save_case(project_name, &storage_case).await?;

            results.push(SaveResult {
                case_id: tc.id.clone(),
                case_name: tc.name.clone(),
                score,
                issues,
                review_verified: review.is_ok(),
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

const ECHO_REJECTION: &str = "model output rejected because it contains configured credentials";

struct OutputGuard {
    client: ArcLlmClient,
    secrets: Vec<String>,
    echoed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl OutputGuard {
    fn reject_echo(&self, response: &str) -> Result<(), crate::llm::LlmError> {
        fn contains(value: &serde_json::Value, secret: &str) -> bool {
            match value {
                serde_json::Value::String(value) => value.contains(secret),
                serde_json::Value::Array(items) => items.iter().any(|v| contains(v, secret)),
                serde_json::Value::Object(items) => items.iter().any(|(k,v)| k.contains(secret) || contains(v, secret)),
                _ => false,
            }
        }
        let json = response.trim().trim_start_matches("```json").trim_start_matches("```")
            .trim_end_matches("```").trim();
        let decoded = serde_json::from_str::<serde_json::Value>(json).ok();
        if self.secrets.iter().any(|secret| response.contains(secret)
            || decoded.as_ref().is_some_and(|value| contains(value, secret))) {
            self.echoed.store(true, std::sync::atomic::Ordering::Release);
            return Err(crate::llm::LlmError::Api(ECHO_REJECTION.into()));
        }
        Ok(())
    }
}
#[async_trait::async_trait]
impl crate::llm::LlmClient for OutputGuard {
    async fn chat(&self, messages: &[crate::llm::Message]) -> Result<String, crate::llm::LlmError> {
        self.chat_with_options(messages, crate::llm::ChatOptions::default()).await
    }
    async fn chat_with_options(&self, messages: &[crate::llm::Message], options: crate::llm::ChatOptions) -> Result<String, crate::llm::LlmError> {
        let response = self.client.chat_with_options(messages, options).await?;
        self.reject_echo(&response)?;
        Ok(response)
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
    pub review_verified: bool,
}

#[cfg(test)]
mod output_secret_tests {
    use super::*;
    use crate::llm::{LlmClient, LlmError, Message};
    use std::sync::{Arc, Mutex};

    struct Responses(Mutex<std::collections::VecDeque<Result<String, LlmError>>>);
    #[async_trait::async_trait]
    impl LlmClient for Responses {
        async fn chat(&self, _: &[Message]) -> Result<String, LlmError> {
            self.0.lock().unwrap().pop_front().expect("unexpected model call")
        }
    }
    struct Project { name: String }
    impl Project {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let project = Self { name: format!("output-guard-{}-{}-{}", std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
                NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed)) };
            std::fs::create_dir_all(crate::storage::LocalStorage::project_dir(&project.name)).unwrap();
            project
        }
        fn exists(&self) -> bool {
            fn any_json(path: &std::path::Path) -> bool {
                std::fs::read_dir(path).ok().into_iter().flatten().filter_map(Result::ok)
                    .any(|entry| if entry.path().is_dir() { any_json(&entry.path()) }
                         else { entry.path().extension().is_some_and(|s| s == "json") })
            }
            any_json(&crate::storage::LocalStorage::cases_dir(&self.name))
        }
    }
    impl Drop for Project {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(crate::storage::LocalStorage::project_dir(&self.name)); }
    }
    fn case(id: &str) -> serde_json::Value {
        serde_json::json!({"id":id,"name":"Safe case","requirement_id":"r","type":"smoke",
            "priority":"P1","yaml_script":"tasks: []","expected_result":"receipt","tags":[]})
    }
    fn review() -> String {
        serde_json::json!({"score":90,"issues":[],"suggestions":[],"coverage":"checked"}).to_string()
    }
    fn service(responses: Vec<Result<String,LlmError>>) -> (TestCaseService,Arc<Responses>) {
        let queue = Arc::new(Responses(Mutex::new(responses.into())));
        (TestCaseService::new(ArcLlmClient(queue.clone()),LocalStorageInstance)
            .with_output_secrets(vec!["".into(),"private-credential".into()]),queue)
    }
    #[tokio::test]
    async fn later_case_credential_rejects_complete_batch_before_any_write() {
        let project=Project::new();
        let mut later=case("later"); later["tags"]=serde_json::json!(["private-credential"]);
        let (service,queue)=service(vec![Ok(serde_json::json!([case("first"),later]).to_string())]);
        let error=service.generate_and_save(&project.name,"spec","requirement").await.unwrap_err().to_string();
        assert!(error.contains(ECHO_REJECTION) && !error.contains("private-credential"));
        assert!(!project.exists());
        assert!(queue.0.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn decoded_unknown_json_keys_and_fenced_unicode_echo_are_rejected() {
        for fenced in [false,true] {
            let project=Project::new();
            let mut item=case("safe"); item["private-credential"]=serde_json::json!({"ignored":"field"});
            let response=serde_json::json!([item]).to_string().replace("private-credential",r"\u0070rivate-credential");
            let response=if fenced {format!("```json\n{response}\n```")} else {response};
            let (service,_)=service(vec![Ok(response)]);
            assert!(service.generate_and_save(&project.name,"spec","r").await.unwrap_err().to_string().contains(ECHO_REJECTION));
            assert!(!project.exists());
        }
    }
    #[tokio::test]
    async fn critic_decoded_echo_is_fatal_before_case_storage() {
        let project=Project::new();
        let response=review().replace("checked",r"\u0070rivate-credential");
        let (service,queue)=service(vec![Ok(serde_json::json!([case("safe")]).to_string()),Ok(response)]);
        let error=service.generate_and_save(&project.name,"spec","r").await.unwrap_err().to_string();
        assert!(error.contains(ECHO_REJECTION) && !error.contains("private-credential"));
        assert!(!project.exists());
        assert!(queue.0.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn missing_critic_evidence_is_fatal_even_without_registered_trace() {
        let project=Project::new();
        let (service,_)=service(vec![Ok(serde_json::json!([case("safe")]).to_string()),
            Err(LlmError::EvidenceUnavailable)]);
        assert!(service.generate_and_save(&project.name,"spec","r").await.unwrap_err()
            .to_string().contains("model call evidence persistence failed"));
        assert!(!project.exists());
    }
    #[tokio::test]
    async fn pre_cancelled_generation_makes_no_model_calls() {
        let project = Project::new();
        let (service, queue) = service(vec![Ok("unused".into())]);
        let service = service.with_cancel_flag(Arc::new(std::sync::atomic::AtomicBool::new(true)));
        assert_eq!(service.generate_and_save(&project.name, "spec", "r").await
            .unwrap_err().to_string(), "Generation cancelled");
        assert_eq!(queue.0.lock().unwrap().len(), 1);
        assert!(!project.exists());
    }

    #[tokio::test]
    async fn cancellation_during_generator_or_critic_prevents_storage() {
        struct CancelResponse {
            responses: Responses,
            flag: Arc<std::sync::atomic::AtomicBool>,
            calls: std::sync::atomic::AtomicUsize,
            cancel_on: usize,
        }
        #[async_trait::async_trait]
        impl LlmClient for CancelResponse {
            async fn chat(&self, messages: &[Message]) -> Result<String, LlmError> {
                let response = self.responses.chat(messages).await;
                if self.calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == self.cancel_on {
                    self.flag.store(true, std::sync::atomic::Ordering::Release);
                }
                response
            }
        }
        for cancel_on in [0, 1] {
            let project = Project::new();
            let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let client = Arc::new(CancelResponse {
                responses: Responses(Mutex::new(vec![
                    Ok(serde_json::json!([case("safe")]).to_string()), Ok(review()),
                ].into())),
                flag: flag.clone(),
                calls: std::sync::atomic::AtomicUsize::new(0),
                cancel_on,
            });
            let service = TestCaseService::new(ArcLlmClient(client.clone()), LocalStorageInstance)
                .with_cancel_flag(flag);
            assert_eq!(service.generate_and_save(&project.name, "spec", "r").await
                .unwrap_err().to_string(), "Generation cancelled");
            assert_eq!(client.calls.load(std::sync::atomic::Ordering::Relaxed), cancel_on + 1);
            assert!(!project.exists());
        }
    }

    #[tokio::test]
    async fn safe_output_is_unchanged_and_ordinary_critic_failure_stays_draft() {
        for failed in [false,true] {
            let project=Project::new();
            let (service,_)=service(vec![Ok(serde_json::json!([case("safe")]).to_string()),
                if failed {Err(LlmError::Api("private-credential provider diagnostic".into()))} else {Ok(review())}]);
            let result=service.generate_and_save(&project.name,"spec","r").await.unwrap();
            assert_eq!(result[0].score,if failed {0} else {90});
            assert_eq!(result[0].review_verified,!failed);
            let asset=crate::storage::LocalStorage::get_case_asset(&project.name,"safe").unwrap();
            assert_eq!(asset.yaml_script,"tasks: []");
            assert_eq!(asset.status,CaseStatus::Draft);
        }
    }
}
