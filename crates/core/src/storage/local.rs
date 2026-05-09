// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::storage::trait_def::{
    CaseStatus, ProjectConfig, Storage, StorageCredentials, StorageError, StorageTransaction, TestCase,
};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;

// ============================================================================
// LocalStorage path helpers (sync, used by CLI for path resolution)
// ============================================================================

pub struct LocalStorage;

impl LocalStorage {
    pub fn base_path() -> PathBuf {
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
        for status in &[CaseStatus::Draft, CaseStatus::Reviewed, CaseStatus::Approved, CaseStatus::Flaky, CaseStatus::Archived] {
            let path = Self::case_path_in_status(name, case_id, *status);
            if path.exists() {
                return Some(path);
            }
        }
        // Fallback: check flat cases dir for legacy cases
        let legacy = Self::case_path(name, case_id);
        if legacy.exists() {
            return Some(legacy);
        }
        None
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
        let yaml = serde_yaml::to_string(&config)
            .map_err(|e| anyhow::anyhow!("yaml error: {}", e))?;
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

    pub fn save_spec(name: &str, markdown: &str) -> anyhow::Result<()> {
        let spec_path = Self::project_spec_path(name);
        std::fs::create_dir_all(spec_path.parent().unwrap())?;
        std::fs::write(&spec_path, markdown)?;
        Ok(())
    }

    pub fn load_spec(name: &str) -> anyhow::Result<String> {
        let spec_path = Self::project_spec_path(name);
        Ok(std::fs::read_to_string(&spec_path)?)
    }

    pub fn save_case(name: &str, case: &TestCase) -> anyhow::Result<()> {
        let status_dir = Self::case_status_dir(name, case.status.clone());
        std::fs::create_dir_all(&status_dir)?;
        let path = status_dir.join(format!("{}.json", case.id));
        let json = serde_json::to_string_pretty(case)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn load_cases(name: &str) -> anyhow::Result<Vec<TestCase>> {
        let mut all_cases = Vec::new();
        for status in [
            CaseStatus::Draft,
            CaseStatus::Reviewed,
            CaseStatus::Approved,
            CaseStatus::Flaky,
            CaseStatus::Archived,
        ] {
            let dir = Self::case_status_dir(name, status);
            if !dir.exists() {
                continue;
            }
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    let content = std::fs::read_to_string(&path)?;
                    let case: TestCase = serde_json::from_str(&content)?;
                    all_cases.push(case);
                }
            }
        }
        Ok(all_cases)
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

    /// Load cases from a specific status subdirectory.
    async fn load_cases_in_dir(&self, name: &str, status: CaseStatus) -> Result<Vec<TestCase>, StorageError> {
        let dir = LocalStorage::case_status_dir(name, status);
        if tokio::fs::metadata(&dir).await.is_err() {
            return Ok(Vec::new());
        }
        let mut entries = tokio::fs::read_dir(&dir).await.map_err(|e| StorageError::Io(e))?;
        let mut cases = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| StorageError::Io(e))? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content = fs::read_to_string(&path).await.map_err(|e| StorageError::Io(e))?;
                let case: TestCase = serde_json::from_str(&content).map_err(|e| StorageError::Json(e))?;
                cases.push(case);
            }
        }
        Ok(cases)
    }

    /// Load cases from legacy flat cases directory (backwards compat).
    async fn load_cases_flat(&self, name: &str) -> Result<Vec<TestCase>, StorageError> {
        let dir = LocalStorage::cases_dir(name);
        if tokio::fs::metadata(&dir).await.is_err() {
            return Ok(Vec::new());
        }
        let mut entries = tokio::fs::read_dir(&dir).await.map_err(|e| StorageError::Io(e))?;
        let mut cases = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| StorageError::Io(e))? {
            let path = entry.path();
            // Only load files, skip status subdirectories
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content = fs::read_to_string(&path).await.map_err(|e| StorageError::Io(e))?;
                let case: TestCase = serde_json::from_str(&content).map_err(|e| StorageError::Json(e))?;
                cases.push(case);
            }
        }
        Ok(cases)
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
        !matches!(Self::load_credentials().unwrap_or_default(), StorageCredentials::Local)
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
        for status in &[CaseStatus::Draft, CaseStatus::Reviewed, CaseStatus::Approved, CaseStatus::Flaky, CaseStatus::Archived] {
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

        let yaml_content = serde_yaml::to_string(&config)
            .map_err(|e| StorageError::Parse(e))?;
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
        while let Some(entry) = entries.next_entry().await.map_err(|e| StorageError::Io(e))? {
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
        let spec_path = LocalStorage::project_spec_path(name);
        fs::write(&spec_path, markdown)
            .await
            .map_err(|e| StorageError::Io(e))?;
        Ok(())
    }

    async fn load_spec(&self, name: &str) -> Result<String, StorageError> {
        let spec_path = LocalStorage::project_spec_path(name);
        fs::read_to_string(&spec_path)
            .await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => StorageError::NotFound(format!("spec for {}", name)),
                _ => StorageError::Io(e),
            })
    }

    async fn save_case(&self, name: &str, case: &TestCase) -> Result<(), StorageError> {
        let case_path = LocalStorage::case_path_in_status(name, &case.id, case.status);
        // Ensure status directory exists
        if let Some(parent) = case_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| StorageError::Io(e))?;
        }
        let json_content = serde_json::to_string_pretty(case)
            .map_err(|e| StorageError::Json(e))?;
        fs::write(&case_path, json_content)
            .await
            .map_err(|e| StorageError::Io(e))?;
        Ok(())
    }

    async fn load_cases(&self, name: &str) -> Result<Vec<TestCase>, StorageError> {
        // Search all status directories
        let mut cases = Vec::new();
        for status in &[CaseStatus::Draft, CaseStatus::Reviewed, CaseStatus::Approved, CaseStatus::Flaky, CaseStatus::Archived] {
            let mut status_cases = self.load_cases_in_dir(name, *status).await?;
            cases.append(&mut status_cases);
        }
        // Also check legacy flat dir
        let legacy_cases = self.load_cases_flat(name).await?;
        cases.extend(legacy_cases);
        Ok(cases)
    }

    async fn load_cases_by_status(&self, name: &str, status: CaseStatus) -> Result<Vec<TestCase>, StorageError> {
        self.load_cases_in_dir(name, status).await
    }

    async fn delete_case(&self, name: &str, case_id: &str) -> Result<(), StorageError> {
        if let Some(path) = LocalStorage::find_case_path(name, case_id) {
            fs::remove_file(&path)
                .await
                .map_err(|e| StorageError::Io(e))?;
        }
        Ok(())
    }

    async fn move_case(&self, name: &str, case_id: &str, from: CaseStatus, to: CaseStatus) -> Result<(), StorageError> {
        if !from.can_transition_to(to) {
            return Err(StorageError::Internal(format!(
                "invalid transition: {} -> {}",
                from.as_str(),
                to.as_str()
            )));
        }
        let src = LocalStorage::case_path_in_status(name, case_id, from);
        if !src.exists() {
            return Err(StorageError::NotFound(format!(
                "case {} not found in {} status",
                case_id,
                from.as_str()
            )));
        }
        // Read the case, update status, save to new location, remove old
        let content = fs::read_to_string(&src).await.map_err(|e| StorageError::Io(e))?;
        let mut case: TestCase = serde_json::from_str(&content).map_err(|e| StorageError::Json(e))?;
        case.status = to;
        let dst = LocalStorage::case_path_in_status(name, case_id, to);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).await.map_err(|e| StorageError::Io(e))?;
        }
        fs::write(&dst, serde_json::to_string_pretty(&case).map_err(|e| StorageError::Json(e))?)
            .await
            .map_err(|e| StorageError::Io(e))?;
        fs::remove_file(&src).await.map_err(|e| StorageError::Io(e))?;
        Ok(())
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
