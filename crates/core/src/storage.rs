use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub tech_stack: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub name: String,
    pub requirement_id: String,
    pub test_type: String,
    pub yaml_script: String,
    pub priority: String,
    pub created_at: String,
}

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

    pub fn init_project(name: &str, url: &str, tech_stack: Vec<String>) -> Result<ProjectConfig> {
        let project_path = Self::project_dir(name);
        let config_path = Self::project_config_path(name);
        let cases_path = Self::cases_dir(name);
        let reports_path = Self::reports_dir(name);

        fs::create_dir_all(&project_path)
            .with_context(|| format!("Failed to create project directory: {}", project_path.display()))?;
        fs::create_dir_all(&cases_path)
            .with_context(|| format!("Failed to create cases directory: {}", cases_path.display()))?;
        fs::create_dir_all(&reports_path)
            .with_context(|| format!("Failed to create reports directory: {}", reports_path.display()))?;

        let config = ProjectConfig {
            name: name.to_string(),
            url: url.to_string(),
            tech_stack,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let yaml_content = serde_yaml::to_string(&config)
            .context("Failed to serialize project config")?;
        fs::write(&config_path, yaml_content)
            .with_context(|| format!("Failed to write config to: {}", config_path.display()))?;

        Ok(config)
    }

    pub fn list_projects() -> Result<Vec<String>> {
        let projects_path = Self::projects_dir();
        if !projects_path.exists() {
            return Ok(Vec::new());
        }

        let entries = fs::read_dir(&projects_path)
            .with_context(|| format!("Failed to read projects directory: {}", projects_path.display()))?;

        let mut projects = Vec::new();
        for entry in entries {
            let entry = entry?;
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

    pub fn load_project(name: &str) -> Result<ProjectConfig> {
        let config_path = Self::project_config_path(name);
        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config from: {}", config_path.display()))?;
        let config: ProjectConfig = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse config from: {}", config_path.display()))?;
        Ok(config)
    }

    pub fn save_spec(name: &str, markdown: &str) -> Result<()> {
        let spec_path = Self::project_spec_path(name);
        fs::write(&spec_path, markdown)
            .with_context(|| format!("Failed to write spec to: {}", spec_path.display()))?;
        Ok(())
    }

    pub fn save_case(name: &str, case: &TestCase) -> Result<()> {
        let case_path = Self::case_path(name, &case.id);
        let json_content = serde_json::to_string_pretty(case)
            .context("Failed to serialize test case")?;
        fs::write(&case_path, json_content)
            .with_context(|| format!("Failed to write case to: {}", case_path.display()))?;
        Ok(())
    }

    pub fn load_cases(name: &str) -> Result<Vec<TestCase>> {
        let cases_path = Self::cases_dir(name);
        if !cases_path.exists() {
            return Ok(Vec::new());
        }

        let entries = fs::read_dir(&cases_path)
            .with_context(|| format!("Failed to read cases directory: {}", cases_path.display()))?;

        let mut cases = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read case file: {}", path.display()))?;
                let case: TestCase = serde_json::from_str(&content)
                    .with_context(|| format!("Failed to parse case from: {}", path.display()))?;
                cases.push(case);
            }
        }

        Ok(cases)
    }

    pub fn delete_project(name: &str) -> Result<()> {
        let project_path = Self::project_dir(name);
        if project_path.exists() {
            fs::remove_dir_all(&project_path)
                .with_context(|| format!("Failed to remove project directory: {}", project_path.display()))?;
        }
        Ok(())
    }
}
