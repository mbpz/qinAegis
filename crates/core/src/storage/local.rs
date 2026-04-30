use crate::storage::trait_def::{
    ProjectConfig, Storage, StorageError, StorageTransaction, TestCase,
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

    pub fn case_path(name: &str, case_id: &str) -> PathBuf {
        Self::cases_dir(name).join(format!("{}.json", case_id))
    }

    pub fn reports_dir(name: &str) -> PathBuf {
        Self::project_dir(name).join("reports")
    }

    pub fn report_dir(name: &str, run_id: &str) -> PathBuf {
        Self::reports_dir(name).join(run_id)
    }

    /// Blocking shim for CLI compatibility — delegates to LocalStorageInstance.
    pub fn init_project(
        name: &str,
        url: &str,
        tech_stack: Vec<String>,
    ) -> anyhow::Result<ProjectConfig> {
        tokio::runtime::Handle::current().block_on(
            LocalStorageInstance::new().init_project(name, url, tech_stack),
        )
        .map_err(|e| anyhow::anyhow!("{}", e))
    }

    pub fn list_projects() -> anyhow::Result<Vec<String>> {
        tokio::runtime::Handle::current()
            .block_on(LocalStorageInstance::new().list_projects())
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    pub fn load_project(name: &str) -> anyhow::Result<ProjectConfig> {
        tokio::runtime::Handle::current()
            .block_on(LocalStorageInstance::new().load_project(name))
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    pub fn delete_project(name: &str) -> anyhow::Result<()> {
        tokio::runtime::Handle::current()
            .block_on(LocalStorageInstance::new().delete_project(name))
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    pub fn save_spec(name: &str, markdown: &str) -> anyhow::Result<()> {
        tokio::runtime::Handle::current()
            .block_on(LocalStorageInstance::new().save_spec(name, markdown))
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    pub fn load_spec(name: &str) -> anyhow::Result<String> {
        tokio::runtime::Handle::current()
            .block_on(LocalStorageInstance::new().load_spec(name))
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    pub fn save_case(name: &str, case: &TestCase) -> anyhow::Result<()> {
        tokio::runtime::Handle::current()
            .block_on(LocalStorageInstance::new().save_case(name, case))
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    pub fn load_cases(name: &str) -> anyhow::Result<Vec<TestCase>> {
        tokio::runtime::Handle::current()
            .block_on(LocalStorageInstance::new().load_cases(name))
            .map_err(|e| anyhow::anyhow!("{}", e))
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
        let case_path = LocalStorage::case_path(name, &case.id);
        let json_content = serde_json::to_string_pretty(case)
            .map_err(|e| StorageError::Json(e))?;
        fs::write(&case_path, json_content)
            .await
            .map_err(|e| StorageError::Io(e))?;
        Ok(())
    }

    async fn load_cases(&self, name: &str) -> Result<Vec<TestCase>, StorageError> {
        let cases_path = LocalStorage::cases_dir(name);
        if tokio::fs::metadata(&cases_path).await.is_err() {
            return Ok(Vec::new());
        }

        let mut entries = tokio::fs::read_dir(&cases_path)
            .await
            .map_err(|e| StorageError::Io(e))?;

        let mut cases = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| StorageError::Io(e))? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content =
                    fs::read_to_string(&path).await.map_err(|e| StorageError::Io(e))?;
                let case: TestCase =
                    serde_json::from_str(&content).map_err(|e| StorageError::Json(e))?;
                cases.push(case);
            }
        }

        Ok(cases)
    }

    async fn delete_case(&self, name: &str, case_id: &str) -> Result<(), StorageError> {
        let case_path = LocalStorage::case_path(name, case_id);
        if case_path.exists() {
            fs::remove_file(&case_path)
                .await
                .map_err(|e| StorageError::Io(e))?;
        }
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
