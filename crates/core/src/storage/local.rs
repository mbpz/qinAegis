// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::storage::trait_def::{
    CaseStatus, ProjectConfig, Storage, StorageCredentials, StorageError, StorageTransaction,
    TestCase,
};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
mod case_assets;
mod case_history;
pub use case_history::{AuditContext, CaseHistory, CaseRevision};
mod spec_assets;
pub use case_assets::CaseAsset;
pub use spec_assets::ProjectSpec;

// ============================================================================
// LocalStorage path helpers (sync, used by CLI for path resolution)
// ============================================================================

pub struct LocalStorage;

impl LocalStorage {
    /// Product data root. The service validates explicit overrides as absolute
    /// paths before use; unlike OS known folders this also isolates Windows.
    pub fn base_path() -> PathBuf {
        if let Some(path) = std::env::var_os("QINAEGIS_DATA_DIR") {
            return PathBuf::from(path);
        }
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".qinAegis")
    }

    pub fn projects_dir() -> PathBuf {
        Self::base_path().join("projects")
    }

    pub fn project_dir(name: &str) -> PathBuf {
        Self::projects_dir().join(name)
    }

    pub fn project_config_path(name: &str) -> PathBuf {
        Self::project_dir(name).join("config.yaml")
    }

    pub fn project_spec_path(name: &str) -> PathBuf {
        Self::project_dir(name).join("spec.md")
    }

    pub fn cases_dir(name: &str) -> PathBuf {
        Self::project_dir(name).join("cases")
    }

    pub fn case_status_dir(name: &str, status: CaseStatus) -> PathBuf {
        Self::cases_dir(name).join(status.dir_name())
    }

    pub fn case_path_in_status(name: &str, case_id: &str, status: CaseStatus) -> PathBuf {
        Self::case_status_dir(name, status).join(format!("{}.json", case_id))
    }

    /// Legacy flat path (used as fallback during migration).
    pub fn case_path(name: &str, case_id: &str) -> PathBuf {
        Self::cases_dir(name).join(format!("{}.json", case_id))
    }

    /// Find the actual path of a case by searching all status directories.
    pub fn find_case_path(name: &str, case_id: &str) -> Option<PathBuf> {
        Self::case_assets(name)
            .and_then(|assets| assets.locate_path(case_id))
            .ok()
    }

    fn case_assets(name: &str) -> anyhow::Result<case_assets::CaseAssets> {
        case_assets::CaseAssets::new(Self::cases_dir(name), name)
    }
    pub fn save_case_with_context(
        project: &str,
        case: &TestCase,
        context: AuditContext,
    ) -> anyhow::Result<()> {
        Self::case_assets(project)?
            .with_context(context)?
            .save(case)
    }
    pub fn delete_case_with_context(
        project: &str,
        id: &str,
        context: AuditContext,
    ) -> anyhow::Result<()> {
        Self::case_assets(project)?
            .with_context(context)?
            .delete(id)
    }
    pub fn edit_case_asset_with_context(
        project: &str,
        id: &str,
        expected: &str,
        name: &str,
        script: &str,
        expected_result: &str,
        context: AuditContext,
    ) -> anyhow::Result<CaseAsset> {
        Self::case_assets(project)?.with_context(context)?.edit(
            id,
            expected,
            name,
            script,
            expected_result,
        )
    }
    pub fn change_case_status_with_context(
        project: &str,
        id: &str,
        to: CaseStatus,
        expected: Option<&str>,
        context: AuditContext,
    ) -> anyhow::Result<()> {
        Self::case_assets(project)?
            .with_context(context)?
            .move_status_checked(id, to, expected)
    }
    pub fn get_case_history(project: &str, id: &str) -> anyhow::Result<CaseHistory> {
        Self::case_assets(project)?.history(id)
    }
    pub fn get_case_revision(
        project: &str,
        id: &str,
        revision: u64,
    ) -> anyhow::Result<CaseRevision> {
        Self::get_case_history(project, id)?
            .revisions
            .into_iter()
            .find(|r| r.revision == revision)
            .ok_or_else(|| anyhow::anyhow!("case revision not found"))
    }
    pub fn restore_case_revision(
        project: &str,
        id: &str,
        revision: u64,
        expected: &str,
        context: AuditContext,
    ) -> anyhow::Result<CaseAsset> {
        Self::case_assets(project)?.restore(id, revision, expected, context)
    }
    pub fn get_case_asset(project: &str, id: &str) -> anyhow::Result<CaseAsset> {
        Self::case_assets(project)?.asset(id)
    }
    pub fn change_case_status_checked(
        project: &str,
        id: &str,
        to: CaseStatus,
        expected: Option<&str>,
    ) -> anyhow::Result<()> {
        Self::case_assets(project)?.move_status_checked(id, to, expected)
    }
    pub fn edit_case_asset(
        project: &str,
        id: &str,
        expected: &str,
        name: &str,
        script: &str,
        expected_result: &str,
    ) -> anyhow::Result<CaseAsset> {
        Self::case_assets(project)?.edit(id, expected, name, script, expected_result)
    }

    pub fn reports_dir(name: &str) -> PathBuf {
        Self::project_dir(name).join("reports")
    }

    pub fn report_dir(name: &str, run_id: &str) -> PathBuf {
        Self::reports_dir(name).join(run_id)
    }

    /// Runs directory for a project (reports dir)
    pub fn runs_dir(name: &str) -> PathBuf {
        Self::reports_dir(name)
    }

    /// Run directory for a specific run
    pub fn run_dir(name: &str, run_id: &str) -> PathBuf {
        Self::report_dir(name, run_id)
    }

    /// Knowledge directory for a project (stores baseline, coverage, etc.)
    pub fn knowledge_dir(name: &str) -> PathBuf {
        Self::project_dir(name).join("knowledge")
    }

    /// Baseline performance data path
    pub fn baseline_path(name: &str) -> PathBuf {
        Self::knowledge_dir(name).join("baseline.json")
    }

    /// Cloud credentials path (local storage only, not synced to cloud)
    pub fn credentials_path() -> PathBuf {
        Self::base_path().join("credentials.json")
    }

    /// Initialize a new project synchronously (for CLI compatibility)
    pub fn init_project(
        name: &str,
        url: &str,
        tech_stack: Vec<String>,
    ) -> anyhow::Result<ProjectConfig> {
        // Use blocking fs operations to avoid nested runtime issue
        let base_dir = Self::project_dir(name);
        std::fs::create_dir_all(&base_dir)?;

        let config = ProjectConfig {
            name: name.to_string(),
            url: url.to_string(),
            tech_stack,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let config_path = Self::project_config_path(name);
        let yaml =
            serde_yaml::to_string(&config).map_err(|e| anyhow::anyhow!("yaml error: {}", e))?;
        std::fs::write(&config_path, yaml)?;

        // Create directories
        std::fs::create_dir_all(Self::cases_dir(name))?;
        std::fs::create_dir_all(Self::reports_dir(name))?;
        std::fs::create_dir_all(Self::knowledge_dir(name))?;

        // Create spec directory
        std::fs::create_dir_all(Self::project_dir(name).join("spec"))?;

        // Initialize cases subdirectories
        for status in [
            CaseStatus::Draft,
            CaseStatus::Reviewed,
            CaseStatus::Approved,
            CaseStatus::Flaky,
            CaseStatus::Archived,
        ] {
            std::fs::create_dir_all(Self::case_status_dir(name, status))?;
        }

        Ok(config)
    }

    pub fn list_projects() -> anyhow::Result<Vec<String>> {
        let dir = Self::projects_dir();
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut projects = vec![];
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let config_path = Self::project_config_path(&name);
            if config_path.exists() {
                projects.push(name);
            }
        }
        Ok(projects)
    }

    pub fn load_project(name: &str) -> anyhow::Result<ProjectConfig> {
        let config_path = Self::project_config_path(name);
        let content = std::fs::read_to_string(&config_path)?;
        let config: ProjectConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn delete_project(name: &str) -> anyhow::Result<()> {
        let dir = Self::project_dir(name);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    fn spec_assets(name: &str) -> anyhow::Result<spec_assets::SpecAssets> {
        spec_assets::SpecAssets::new(Self::project_dir(name), name)
    }
    /// Inspect the managed project identity without reading its specification.
    pub fn checked_project_config(name: &str) -> anyhow::Result<ProjectConfig> {
        Ok(Self::spec_assets(name)?.project_config())
    }
    pub fn get_project_spec(name: &str) -> anyhow::Result<ProjectSpec> {
        Self::spec_assets(name)?.get()
    }
    pub fn save_project_spec(
        name: &str,
        expected: Option<&str>,
        markdown: &str,
    ) -> anyhow::Result<ProjectSpec> {
        Self::spec_assets(name)?.save(expected, markdown)
    }
    pub fn save_spec(name: &str, markdown: &str) -> anyhow::Result<()> {
        Self::spec_assets(name)?.save_current(markdown)
    }
    pub fn load_spec(name: &str) -> anyhow::Result<String> {
        let spec = Self::get_project_spec(name)?;
        anyhow::ensure!(spec.exists, "specification missing");
        Ok(spec.markdown)
    }

    pub fn save_case(name: &str, case: &TestCase) -> anyhow::Result<()> {
        Self::case_assets(name)?.save(case)
    }

    #[cfg(test)]
    fn save_case_at(path: &std::path::Path, project: &str, case: &TestCase) -> anyhow::Result<()> {
        if let Some(source) = &case.provenance {
            source.validate(project, &case.id, &case.yaml_script)?;
        }
        let asset = crate::asset_file::AssetFile::new(path.to_path_buf(), 32 * 1024 * 1024);
        let guard = asset.lock()?;
        // The source and script share one atomic replacement and durability boundary.
        guard.write(&serde_json::to_vec_pretty(case)?)
    }

    pub fn load_cases(name: &str) -> anyhow::Result<Vec<TestCase>> {
        Self::case_assets(name)?.list()
    }
}

// ============================================================================
// LocalStorage implementation
// ============================================================================

#[derive(Clone)]
pub struct LocalStorageInstance;

impl LocalStorageInstance {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalStorageInstance {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalStorageInstance {
    /// Knowledge directory for a project
    pub fn knowledge_dir(name: &str) -> PathBuf {
        LocalStorage::knowledge_dir(name)
    }

    /// Baseline performance data path
    pub fn baseline_path(name: &str) -> PathBuf {
        LocalStorage::baseline_path(name)
    }

    /// Save cloud credentials locally (credentials are stored locally only).
    pub fn save_credentials(credentials: &StorageCredentials) -> anyhow::Result<()> {
        if matches!(credentials, StorageCredentials::Local) {
            // Remove credentials file if switching to local
            let path = LocalStorage::credentials_path();
            if path.exists() {
                std::fs::remove_file(path)?;
            }
            return Ok(());
        }
        let path = LocalStorage::credentials_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(credentials)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load cloud credentials from local storage.
    pub fn load_credentials() -> anyhow::Result<StorageCredentials> {
        let path = LocalStorage::credentials_path();
        if !path.exists() {
            return Ok(StorageCredentials::default());
        }
        let content = std::fs::read_to_string(path)?;
        let creds: StorageCredentials = serde_json::from_str(&content)?;
        Ok(creds)
    }

    /// Check if cloud credentials are configured (not Local).
    pub fn has_cloud_credentials() -> bool {
        !matches!(
            Self::load_credentials().unwrap_or_default(),
            StorageCredentials::Local
        )
    }
}

#[async_trait]
impl Storage for LocalStorageInstance {
    async fn init_project(
        &self,
        name: &str,
        url: &str,
        tech_stack: Vec<String>,
    ) -> Result<ProjectConfig, StorageError> {
        let project_path = LocalStorage::project_dir(name);
        let config_path = LocalStorage::project_config_path(name);
        let cases_path = LocalStorage::cases_dir(name);
        let reports_path = LocalStorage::reports_dir(name);

        fs::create_dir_all(&project_path)
            .await
            .map_err(|e| StorageError::Io(e))?;
        fs::create_dir_all(&cases_path)
            .await
            .map_err(|e| StorageError::Io(e))?;
        // Create status subdirectories for case lifecycle
        for status in &[
            CaseStatus::Draft,
            CaseStatus::Reviewed,
            CaseStatus::Approved,
            CaseStatus::Flaky,
            CaseStatus::Archived,
        ] {
            fs::create_dir_all(LocalStorage::case_status_dir(name, *status))
                .await
                .map_err(|e| StorageError::Io(e))?;
        }
        fs::create_dir_all(&reports_path)
            .await
            .map_err(|e| StorageError::Io(e))?;

        let config = ProjectConfig {
            name: name.to_string(),
            url: url.to_string(),
            tech_stack,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let yaml_content = serde_yaml::to_string(&config).map_err(|e| StorageError::Parse(e))?;
        fs::write(&config_path, yaml_content)
            .await
            .map_err(|e| StorageError::Io(e))?;

        Ok(config)
    }

    async fn list_projects(&self) -> Result<Vec<String>, StorageError> {
        let projects_path = LocalStorage::projects_dir();
        if tokio::fs::metadata(&projects_path).await.is_err() {
            return Ok(Vec::new());
        }

        let mut entries = tokio::fs::read_dir(&projects_path)
            .await
            .map_err(|e| StorageError::Io(e))?;

        let mut projects = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| StorageError::Io(e))?
        {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    projects.push(name.to_string());
                }
            }
        }

        projects.sort();
        Ok(projects)
    }

    async fn load_project(&self, name: &str) -> Result<ProjectConfig, StorageError> {
        let config_path = LocalStorage::project_config_path(name);
        let content = fs::read_to_string(&config_path)
            .await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => StorageError::NotFound(name.to_string()),
                _ => StorageError::Io(e),
            })?;
        let config: ProjectConfig =
            serde_yaml::from_str(&content).map_err(|e| StorageError::Parse(e))?;
        Ok(config)
    }

    async fn delete_project(&self, name: &str) -> Result<(), StorageError> {
        let project_path = LocalStorage::project_dir(name);
        if project_path.exists() {
            fs::remove_dir_all(&project_path)
                .await
                .map_err(|e| StorageError::Io(e))?;
        }
        Ok(())
    }

    async fn save_spec(&self, name: &str, markdown: &str) -> Result<(), StorageError> {
        LocalStorage::save_spec(name, markdown).map_err(|e| StorageError::Internal(e.to_string()))
    }
    async fn load_spec(&self, name: &str) -> Result<String, StorageError> {
        LocalStorage::load_spec(name).map_err(|e| StorageError::Internal(e.to_string()))
    }

    async fn save_case(&self, name: &str, case: &TestCase) -> Result<(), StorageError> {
        LocalStorage::save_case(name, case).map_err(|e| StorageError::Internal(e.to_string()))
    }

    async fn load_cases(&self, name: &str) -> Result<Vec<TestCase>, StorageError> {
        LocalStorage::load_cases(name).map_err(|e| StorageError::Internal(e.to_string()))
    }

    async fn load_cases_by_status(
        &self,
        name: &str,
        status: CaseStatus,
    ) -> Result<Vec<TestCase>, StorageError> {
        Ok(LocalStorage::load_cases(name)
            .map_err(|e| StorageError::Internal(e.to_string()))?
            .into_iter()
            .filter(|c| c.status == status)
            .collect())
    }

    async fn delete_case(&self, name: &str, case_id: &str) -> Result<(), StorageError> {
        LocalStorage::case_assets(name)
            .and_then(|assets| assets.delete(case_id))
            .map_err(|e| StorageError::Internal(e.to_string()))
    }

    async fn move_case(
        &self,
        name: &str,
        case_id: &str,
        from: CaseStatus,
        to: CaseStatus,
    ) -> Result<(), StorageError> {
        LocalStorage::case_assets(name)
            .and_then(|assets| assets.move_status(case_id, from, to))
            .map_err(|e| StorageError::Internal(e.to_string()))
    }

    async fn begin_transaction(&self) -> Result<Box<dyn StorageTransaction>, StorageError> {
        Ok(Box::new(LocalTransaction {
            operations: Vec::new(),
        }))
    }
}

// ============================================================================
// Local transaction guard
// ============================================================================

pub struct LocalTransaction {
    operations: Vec<Box<dyn Fn() -> Result<(), StorageError> + Send>>,
}

impl StorageTransaction for LocalTransaction {
    fn commit(&mut self) -> Result<(), StorageError> {
        for op in self.operations.drain(..) {
            op()?;
        }
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), StorageError> {
        self.operations.clear();
        Ok(())
    }
}

#[cfg(test)]
mod atomic_case_tests {
    use super::*;
    use crate::execution_records::CaseProvenance;

    fn case() -> TestCase {
        TestCase {
            id: "receipt".into(),
            name: "Receipt".into(),
            requirement_id: "r".into(),
            test_type: "smoke".into(),
            yaml_script: "assert receipt".into(),
            priority: "P1".into(),
            created_at: "2026-11-16".into(),
            status: CaseStatus::Draft,
            provenance: Some(CaseProvenance {
                specification: None,
                model_calls: None,
                schema_version: 1,
                project: "project".into(),
                case_id: "receipt".into(),
                yaml_script: "assert receipt".into(),
                created_at: "2026-11-16".into(),
                contracts: None,
                model: serde_json::json!({"source":"legacy_config","model":"fixture"}),
            }),
        }
    }

    #[test]
    fn failed_case_save_can_retry_same_identity_without_orphan_source() {
        let root = std::env::temp_dir().join(format!(
            "qinaegis-case-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let parent = root.join("draft");
        std::fs::write(&parent, b"blocked").unwrap();
        let path = parent.join("receipt.json");
        let mut value = case();
        assert!(LocalStorage::save_case_at(&path, "project", &value).is_err());
        std::fs::remove_file(&parent).unwrap();
        LocalStorage::save_case_at(&path, "project", &value).unwrap();
        let original = std::fs::read(&path).unwrap();
        value.yaml_script = "silently weakened check".into();
        assert!(LocalStorage::save_case_at(&path, "project", &value).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let mut stored: TestCase = serde_json::from_slice(&original).unwrap();
        stored.status = CaseStatus::Reviewed;
        let target = root.join("reviewed/receipt.json");
        LocalStorage::save_case_at(&target, "project", &stored).unwrap();
        let reviewed: TestCase = serde_json::from_slice(&std::fs::read(target).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(reviewed.provenance).unwrap(),
            serde_json::to_value(stored.provenance).unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn actual_model_trace_is_saved_in_same_atomic_case_asset() {
        use crate::execution_records::GenerationModelCalls;
        use crate::models::{ModelCallAttempt, ModelCallOutcome};
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("draft/receipt.json");
        let mut asset = case();
        let provenance = asset.provenance.as_mut().unwrap();
        provenance.model = serde_json::json!({"revision":1,"task":"generate","candidates":[
            {"id":"one","adapter":"openai_compatible","model":"fixture","capabilities":["text"],"midscene_family":null}]});
        provenance.model_calls = Some(GenerationModelCalls {
            generator: vec![ModelCallAttempt {
                call: 1,
                attempt: 1,
                profile_id: "one".into(),
                outcome: ModelCallOutcome::Succeeded,
            }],
            critic: vec![ModelCallAttempt {
                call: 2,
                attempt: 1,
                profile_id: "one".into(),
                outcome: ModelCallOutcome::Succeeded,
            }],
        });
        LocalStorage::save_case_at(&path, "project", &asset).unwrap();
        let before = std::fs::read(&path).unwrap();
        let saved: TestCase = serde_json::from_slice(&before).unwrap();
        assert_eq!(
            saved.provenance.unwrap().model_calls.unwrap().generator[0].profile_id,
            "one"
        );
        asset
            .provenance
            .as_mut()
            .unwrap()
            .model_calls
            .as_mut()
            .unwrap()
            .critic[0]
            .profile_id = "outside-route".into();
        assert!(LocalStorage::save_case_at(&path, "project", &asset).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn legacy_case_without_source_remains_readable() {
        let mut json = serde_json::to_value(case()).unwrap();
        json.as_object_mut().unwrap().remove("provenance");
        let legacy: TestCase = serde_json::from_value(json).unwrap();
        assert!(legacy.provenance.is_none());
    }
}
