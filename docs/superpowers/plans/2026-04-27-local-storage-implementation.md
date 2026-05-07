# Local Storage Only - Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove Notion dependency, use local file-based storage under `~/.qinAegis/`, add `qinAegis export` command

**Architecture:** Local filesystem as storage backend. Projects stored under `~/.qinAegis/projects/<name>/`. Storage abstraction layer in `crates/core/src/storage.rs`.

**Tech Stack:** Rust (serde_yaml, tokio), file-based storage with JSON/YAML

---

## File Structure

```
crates/
├── cli/
│   └── src/
│       ├── commands/
│       │   ├── mod.rs              # MODIFY: remove notion, add project, export
│       │   ├── init.rs             # MODIFY: local init only, no OAuth
│       │   ├── explore.rs          # MODIFY: write to project dir
│       │   ├── generate.rs         # MODIFY: write cases to local
│       │   ├── run.rs              # MODIFY: read from local, remove notion
│       │   ├── project.rs          # CREATE: project add/list/remove
│       │   └── export.rs           # CREATE: export command
│       └── config.rs               # MODIFY: remove NotionConfig
└── core/
    └── src/
        ├── storage.rs              # CREATE: local storage abstraction
        └── lib.rs                  # MODIFY: export storage module

crates/notion/                      # DELETE entire directory
Cargo.toml                           # MODIFY: remove notion workspace member
```

---

## Task 1: Delete notion crate

**Files:**
- Delete: `crates/notion/` (entire directory)

- [ ] **Step 1: Remove notion crate directory**

Run: `rm -rf crates/notion`

- [ ] **Step 2: Update workspace Cargo.toml**

Modify: `Cargo.toml:3`
```toml
members = ["crates/cli", "crates/sandbox", "crates/core"]
```
(remove `crates/notion` from members list)

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "chore: remove notion crate (local storage only)"
```

---

## Task 2: Create storage module

**Files:**
- Create: `crates/core/src/storage.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Create storage.rs**

```rust
// crates/core/src/storage.rs
use serde::{Deserialize, Serialize};
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
    pub test_type: String,  // "smoke" | "full" | "perf" | "stress"
    pub yaml_script: String,
    pub priority: String,
    pub created_at: String,
}

pub struct LocalStorage;

impl LocalStorage {
    /// Get base path: ~/.qinAegis/
    pub fn base_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("qinAegis")
    }

    /// Get projects directory: ~/.qinAegis/projects/
    pub fn projects_dir() -> PathBuf {
        Self::base_path().join("projects")
    }

    /// Get project directory: ~/.qinAegis/projects/<name>/
    pub fn project_dir(name: &str) -> PathBuf {
        Self::projects_dir().join(name)
    }

    /// Get config.yaml path for a project
    pub fn project_config_path(name: &str) -> PathBuf {
        Self::project_dir(name).join("config.yaml")
    }

    /// Get spec.md path for a project
    pub fn project_spec_path(name: &str) -> PathBuf {
        Self::project_dir(name).join("spec.md")
    }

    /// Get cases directory: ~/.qinAegis/projects/<name>/cases/
    pub fn cases_dir(name: &str) -> PathBuf {
        Self::project_dir(name).join("cases")
    }

    /// Get a specific case file: ~/.qinAegis/projects/<name>/cases/<id>.json
    pub fn case_path(name: &str, case_id: &str) -> PathBuf {
        Self::cases_dir(name).join(format!("{}.json", case_id))
    }

    /// Get reports directory: ~/.qinAegis/projects/<name>/reports/
    pub fn reports_dir(name: &str) -> PathBuf {
        Self::project_dir(name).join("reports")
    }

    /// Get a specific report directory: ~/.qinAegis/projects/<name>/reports/<run-id>/
    pub fn report_dir(name: &str, run_id: &str) -> PathBuf {
        Self::reports_dir(name).join(run_id)
    }

    /// Ensure project directory structure exists
    pub fn init_project(name: &str, url: &str, tech_stack: Vec<String>) -> anyhow::Result<PathBuf> {
        let dir = Self::project_dir(name);
        std::fs::create_dir_all(&dir)?;
        std::fs::create_dir_all(Self::cases_dir(name))?;
        std::fs::create_dir_all(Self::reports_dir(name))?;

        let config = ProjectConfig {
            name: name.to_string(),
            url: url.to_string(),
            tech_stack,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let config_path = Self::project_config_path(name);
        let yaml = serde_yaml::to_string(&config)?;
        std::fs::write(&config_path, yaml)?;

        Ok(dir)
    }

    /// List all projects
    pub fn list_projects() -> anyhow::Result<Vec<String>> {
        let dir = Self::projects_dir();
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut projects = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    projects.push(name.to_string());
                }
            }
        }
        Ok(projects)
    }

    /// Load project config
    pub fn load_project(name: &str) -> anyhow::Result<ProjectConfig> {
        let path = Self::project_config_path(name);
        let content = std::fs::read_to_string(&path)?;
        let config: ProjectConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Save spec.md for a project
    pub fn save_spec(name: &str, markdown: &str) -> anyhow::Result<PathBuf> {
        let dir = Self::project_dir(name);
        std::fs::create_dir_all(&dir)?;
        let path = Self::project_spec_path(name);
        std::fs::write(&path, markdown)?;
        Ok(path)
    }

    /// Save a test case
    pub fn save_case(name: &str, case: &TestCase) -> anyhow::Result<PathBuf> {
        std::fs::create_dir_all(&Self::cases_dir(name))?;
        let path = Self::case_path(name, &case.id);
        let json = serde_json::to_string_pretty(case)?;
        std::fs::write(&path, json)?;
        Ok(path)
    }

    /// Load all test cases for a project
    pub fn load_cases(name: &str) -> anyhow::Result<Vec<TestCase>> {
        let dir = Self::cases_dir(name);
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut cases = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = std::fs::read_to_string(&path)?;
                let case: TestCase = serde_json::from_str(&content)?;
                cases.push(case);
            }
        }
        Ok(cases)
    }

    /// Delete a project
    pub fn delete_project(name: &str) -> anyhow::Result<()> {
        let dir = Self::project_dir(name);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }
}
```

- [ ] **Step 2: Update lib.rs to export storage module**

Modify: `crates/core/src/lib.rs`
```rust
pub mod explorer;
pub mod executor;
pub mod generator;
pub mod reporter;
pub mod protocol;
pub mod storage;  // ADD THIS LINE
```

- [ ] **Step 3: Add serde_yaml to workspace dependencies**

Modify: `Cargo.toml`
```toml
serde_yaml = "0.9"
```
(add to `[workspace.dependencies]`)

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/storage.rs crates/core/src/lib.rs Cargo.toml
git commit -m "feat: add LocalStorage module for file-based project management"
```

---

## Task 3: Modify config.rs - remove NotionConfig

**Files:**
- Modify: `crates/cli/src/config.rs:1-179`

- [ ] **Step 1: Rewrite Config struct without NotionConfig**

Replace the Config struct and related types (lines 4-88):

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    pub sandbox: SandboxConfig,
    #[serde(default)]
    pub exploration: ExplorationConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SandboxConfig {
    pub compose_file: String,
    pub steel_port: u16,
    pub cdp_port: u16,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExplorationConfig {
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    #[serde(default = "default_max_pages_per_seed")]
    pub max_pages_per_seed: u32,
}

fn default_max_depth() -> u32 { 3 }
fn default_max_pages_per_seed() -> u32 { 20 }

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "minimax".to_string(),
            base_url: "https://api.minimax.chat/v1".to_string(),
            api_key: String::new(),
            model: "MiniMax-VL-01".to_string(),
        }
    }
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            cdp_port: 9222,
        }
    }
}

impl Default for ExplorationConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_pages_per_seed: 20,
        }
    }
}
```

- [ ] **Step 2: Update Config methods (is_notion_configured, is_complete)**

Replace methods at lines 118-129:

```rust
    pub fn is_llm_configured(&self) -> bool {
        !self.llm.api_key.is_empty()
    }

    pub fn is_complete(&self) -> bool {
        self.is_llm_configured()
    }
```

- [ ] **Step 3: Update prompt_for_config - remove Notion OAuth**

Replace the `prompt_for_config` function (lines 132-157):

```rust
/// Interactive setup prompts
pub fn prompt_for_config() -> anyhow::Result<Config> {
    println!("\n=== QinAegis Configuration Setup ===\n");

    let mut config = Config {
        llm: LlmConfig::default(),
        sandbox: SandboxConfig::default(),
        exploration: ExplorationConfig::default(),
    };

    // LLM Configuration
    println!("LLM Configuration (for Midscene AI):");
    config.llm.provider = prompt_with_default("  Provider", "minimax")?;
    config.llm.base_url = prompt_with_default("  Base URL", "https://api.minimax.chat/v1")?;
    config.llm.model = prompt_with_default("  Model", "MiniMax-VL-01")?;
    config.llm.api_key = prompt("  API Key")?;

    println!("\n✓ Configuration complete!\n");

    Ok(config)
}
```

- [ ] **Step 4: Remove NotionConfig default impl (lines 47-55)**

Delete the entire `impl Default for NotionConfig` block.

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/config.rs
git commit -m "refactor: remove NotionConfig from CLI config"
```

---

## Task 4: Rewrite init command - local only

**Files:**
- Modify: `crates/cli/src/commands/init.rs`

- [ ] **Step 1: Rewrite init.rs to not use Notion**

Replace entire file content:

```rust
use crate::config::{Config, prompt_for_config};
use qin_aegis_core::storage::LocalStorage;

pub async fn run_init_and_setup() -> anyhow::Result<()> {
    // 1. Check if config exists
    match Config::load()? {
        Some(c) if c.is_complete() => {
            println!("Configuration already complete.");
            println!("  LLM: {} ({})", c.llm.model, c.llm.base_url);
            println!("\nTo reconfigure, delete ~/.qinAegis/config.toml and run init again.");
            return Ok(());
        }
        Some(_) => {
            println!("Incomplete configuration detected. Running setup...");
        }
        None => {
            println!("No configuration found. Creating new setup...");
        }
    }

    // 2. Prompt for config
    let config = prompt_for_config()?;
    config.save()?;

    // 3. Initialize projects directory
    let projects_dir = LocalStorage::projects_dir();
    std::fs::create_dir_all(&projects_dir)?;

    println!("\n✓ Initialization complete!");
    println!("  Config: ~/.qinAegis/config.toml");
    println!("  Projects: ~/.qinAegis/projects/");
    println!("\nNext steps:");
    println!("  qinAegis project add <name>  # Add a project");
    println!("  qinAegis explore <project>   # Explore a URL");

    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/cli/src/commands/init.rs
git commit -m "refactor: init command no longer requires Notion OAuth"
```

---

## Task 5: Create project command

**Files:**
- Create: `crates/cli/src/commands/project.rs`
- Modify: `crates/cli/src/commands/mod.rs`

- [ ] **Step 1: Create project.rs**

```rust
use qin_aegis_core::storage::LocalStorage;
use crate::config::Config;

pub async fn add_project(name: &str, url: &str) -> anyhow::Result<()> {
    let config = Config::load()?
        .ok_or_else(|| anyhow::anyhow!("run qinAegis init first"))?;

    if !config.is_llm_configured() {
        anyhow::bail!("LLM not configured. Run 'qinAegis init' first.");
    }

    let tech_stack = vec![]; // TODO: could prompt for this
    let dir = LocalStorage::init_project(name, url, tech_stack)?;

    println!("✓ Project '{}' created at {}", name, dir.display());
    println!("  URL: {}", url);
    println!("\nNext: qinAegis explore {}", name);

    Ok(())
}

pub async fn list_projects() -> anyhow::Result<()> {
    let projects = LocalStorage::list_projects()?;

    if projects.is_empty() {
        println!("No projects found.");
        println!("Run 'qinAegis project add <name>' to create one.");
        return Ok(());
    }

    println!("Projects ({}):\n", projects.len());
    for name in &projects {
        match LocalStorage::load_project(name) {
            Ok(cfg) => {
                println!("  {}  {}", name, cfg.url);
            }
            Err(_) => {
                println!("  {}  (broken config)", name);
            }
        }
    }

    Ok(())
}

pub async fn remove_project(name: &str) -> anyhow::Result<()> {
    let projects = LocalStorage::list_projects()?;
    if !projects.contains(&name.to_string()) {
        anyhow::bail!("Project '{}' does not exist", name);
    }

    LocalStorage::delete_project(name)?;
    println!("✓ Project '{}' deleted", name);

    Ok(())
}
```

- [ ] **Step 2: Update mod.rs to add project module**

Modify: `crates/cli/src/commands/mod.rs`

```rust
pub mod init;
pub mod explore;
pub mod generate;
pub mod run;
pub mod performance;
pub mod project;  // ADD THIS
pub mod export;    // ADD THIS
```

- [ ] **Step 3: Commit**

```bash
git add crates/cli/src/commands/project.rs crates/cli/src/commands/mod.rs
git commit -m "feat: add project command (add/list/remove)"
```

---

## Task 6: Create export command

**Files:**
- Create: `crates/cli/src/commands/export.rs`

- [ ] **Step 1: Create export.rs**

```rust
use qin_aegis_core::storage::LocalStorage;
use std::path::PathBuf;

pub async fn export_project(name: &str, format: &str) -> anyhow::Result<PathBuf> {
    let projects = LocalStorage::list_projects()?;
    if !projects.contains(&name.to_string()) {
        anyhow::bail!("Project '{}' does not exist", name);
    }

    let project_dir = LocalStorage::project_dir(name);
    let output_path = match format {
        "html" => export_html(name, &project_dir)?,
        "md" => export_markdown(name, &project_dir)?,
        "json" => export_json(name, &project_dir)?,
        _ => anyhow::bail!("Unknown format '{}'. Use html, md, or json.", format),
    };

    println!("✓ Exported to: {}", output_path.display());
    Ok(output_path)
}

fn export_html(name: &str, project_dir: &std::path::Path) -> anyhow::Result<PathBuf> {
    let spec_path = LocalStorage::project_spec_path(name);
    let spec = if spec_path.exists() {
        std::fs::read_to_string(&spec_path)?
    } else {
        "# No spec found".to_string()
    };

    let cases = LocalStorage::load_cases(name)?;

    let mut cases_html = String::new();
    for case in &cases {
        cases_html.push_str(&format!(
            "<li><strong>{}</strong> ({}) - {}</li>\n",
            case.name, case.test_type, case.priority
        ));
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>QinAegis - {}</title></head>
<body>
<h1>Project: {}</h1>
<h2>Spec</h2>
<pre>{}</pre>
<h2>Test Cases ({})</h2>
<ul>{}</ul>
</body>
</html>"#,
        name, name, spec, cases.len(), cases_html
    );

    let output_path = LocalStorage::base_path().join(format!("{}-export.html", name));
    std::fs::write(&output_path, html)?;
    Ok(output_path)
}

fn export_markdown(name: &str, _project_dir: &std::path::Path) -> anyhow::Result<PathBuf> {
    let mut md = String::new();

    // Spec
    let spec_path = LocalStorage::project_spec_path(name);
    if spec_path.exists() {
        md.push_str("# Project Spec\n\n");
        md.push_str(&std::fs::read_to_string(&spec_path)?);
        md.push_str("\n\n");
    }

    // Cases
    let cases = LocalStorage::load_cases(name)?;
    md.push_str(&format!("# Test Cases ({})\n\n", cases.len()));
    for case in &cases {
        md.push_str(&format!("## {} ({})\n\n", case.name, case.test_type));
        md.push_str("```yaml\n");
        md.push_str(&case.yaml_script);
        md.push_str("\n```\n\n");
    }

    let output_path = LocalStorage::base_path().join(format!("{}-export.md", name));
    std::fs::write(&output_path, md)?;
    Ok(output_path)
}

fn export_json(name: &str, _project_dir: &std::path::Path) -> anyhow::Result<PathBuf> {
    let config = LocalStorage::load_project(name)?;
    let cases = LocalStorage::load_cases(name)?;

    let export = serde_json::json!({
        "project": config,
        "cases": cases,
    });

    let output_path = LocalStorage::base_path().join(format!("{}-export.json", name));
    std::fs::write(&output_path, serde_json::to_string_pretty(&export)?)?;
    Ok(output_path)
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/cli/src/commands/export.rs
git commit -m "feat: add export command for html/md/json output"
```

---

## Task 7: Modify explore command

**Files:**
- Modify: `crates/cli/src/commands/explore.rs`

- [ ] **Step 1: Update explore.rs to write to project directory**

Replace entire file:

```rust
use qin_aegis_core::{Explorer, LlmConfig};
use qin_aegis_core::storage::LocalStorage;
use crate::config::Config;

pub async fn run_explore(project_name: &str, seed_url: Option<String>, max_depth: u32) -> anyhow::Result<()> {
    // Load config
    let config = Config::load()?
        .ok_or_else(|| anyhow::anyhow!("run qinAegis init first"))?;

    if !config.is_llm_configured() {
        anyhow::bail!("LLM not configured. Run 'qinAegis init' first.");
    }

    // Check project exists, get URL
    let project = LocalStorage::load_project(project_name)
        .map_err(|_| anyhow::anyhow!("Project '{}' not found. Run 'qinAegis project add' first.", project_name))?;

    let url = seed_url.unwrap_or_else(|| project.url.clone());

    println!("Exploring {} from {}", project_name, url);
    println!("Max depth: {}\n", max_depth);

    let llm_config = Some(LlmConfig {
        api_key: config.llm.api_key,
        base_url: config.llm.base_url,
        model: config.llm.model,
    });

    let mut explorer = Explorer::new(llm_config).await?;

    let result = explorer.explore(&url, max_depth).await?;

    let mut all_markdown = String::from("# 项目规格书\n\n");
    all_markdown.push_str(&result.markdown);

    let spec_path = LocalStorage::save_spec(project_name, &all_markdown)?;
    println!("\n✓ Exploration complete: {} pages", result.pages.len());
    println!("✓ Spec saved to: {}", spec_path.display());

    explorer.shutdown().await?;
    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/cli/src/commands/explore.rs
git commit -m "refactor: explore writes to project spec.md instead of temp dir"
```

---

## Task 8: Modify generate command

**Files:**
- Modify: `crates/cli/src/commands/generate.rs`

- [ ] **Step 1: Update generate.rs to write cases to local storage**

Replace entire file:

```rust
use qin_aegis_core::{TestCaseGenerator, Critic, MiniMaxClient};
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

    let llm = MiniMaxClient::new(
        config.llm.base_url,
        config.llm.api_key,
        config.llm.model,
    );

    let generator = TestCaseGenerator::new(llm.clone());
    let cases = generator.generate(&spec_markdown, requirement_text).await?;

    println!("\n✓ Generated {} test cases", cases.len());

    let critic = Critic::new(llm);

    // Save cases to local storage
    for tc in &cases {
        let review = critic.review(&tc.yaml_script, &spec_markdown, requirement_text).await;

        let (score, issues) = match review {
            Ok(r) => (r.score, r.issues),
            Err(e) => {
                println!("  {} - critic failed: {}", tc.name, e);
                (0, vec![])
            }
        };

        println!("  {} - score: {}/10", tc.name, score);
        if !issues.is_empty() {
            for issue in &issues {
                println!("    ⚠ {}", issue);
            }
        }

        let test_case = qin_aegis_core::storage::TestCase {
            id: tc.id.clone(),
            name: tc.name.clone(),
            requirement_id: tc.requirement_id.clone(),
            test_type: tc.case_type.clone(),
            yaml_script: tc.yaml_script.clone(),
            priority: tc.priority.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        LocalStorage::save_case(project_name, &test_case)?;
    }

    println!("\n✓ Test cases saved to ~/.qinAegis/projects/{}/cases/", project_name);

    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/cli/src/commands/generate.rs
git commit -m "refactor: generate writes cases to local storage"
```

---

## Task 9: Modify run command - read from local storage

**Files:**
- Modify: `crates/cli/src/commands/run.rs`

- [ ] **Step 1: Update run.rs to read from local storage**

Replace entire file:

```rust
use qin_aegis_core::{TestExecutor, TestCaseRef, Reporter, LlmConfig};
use qin_aegis_core::storage::LocalStorage;
use crate::config::Config;

pub async fn run_tests(
    project_name: &str,
    test_type: &str,
    concurrency: usize,
) -> anyhow::Result<()> {
    let config = Config::load()?
        .ok_or_else(|| anyhow::anyhow!("run qinAegis init first"))?;

    // Load project
    let project = LocalStorage::load_project(project_name)
        .map_err(|_| anyhow::anyhow!("Project '{}' not found. Run 'qinAegis project add' first.", project_name))?;

    // Load cases
    let mut cases = LocalStorage::load_cases(project_name)?;

    if cases.is_empty() {
        println!("No test cases found for project '{}'.", project_name);
        println!("Run 'qinAegis generate' first.");
        return Ok(());
    }

    // Filter by test type if specified
    if test_type != "all" {
        cases.retain(|c| c.test_type == test_type);
    }

    if cases.is_empty() {
        println!("No {} test cases found.", test_type);
        return Ok(());
    }

    println!("Running {} test cases (concurrency={})...", cases.len(), concurrency);

    let llm_config = Some(LlmConfig {
        api_key: config.llm.api_key,
        base_url: config.llm.base_url,
        model: config.llm.model,
    });

    let executor = TestExecutor::new(concurrency, llm_config).await?;

    let case_refs: Vec<TestCaseRef> = cases
        .iter()
        .map(|c| TestCaseRef {
            id: c.id.clone(),
            yaml_script: c.yaml_script.clone(),
            name: c.name.clone(),
            priority: c.priority.clone(),
        })
        .collect();

    let results = executor.run_parallel(case_refs).await?;
    executor.shutdown().await?;

    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    // Save summary
    let report_dir = LocalStorage::report_dir(project_name, &run_id);
    std::fs::create_dir_all(&report_dir)?;

    let summary_path = Reporter::save_summary(&run_id, &results)?;
    println!("Summary saved: {}", summary_path.display());

    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.len() - passed;
    println!("\nRun complete: {}/{} passed", passed, failed);

    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/cli/src/commands/run.rs
git commit -m "refactor: run reads cases from local storage, not Notion"
```

---

## Task 10: Update CLI main.rs to use new commands

**Files:**
- Modify: `crates/cli/src/main.rs`

- [ ] **Step 1: Check current main.rs structure**

```bash
head -100 crates/cli/src/main.rs
```

- [ ] **Step 2: Update command routing to use new module structure**

The main.rs should route:
- `qinAegis init` → `init::run_init_and_setup()`
- `qinAegis project add <name>` → `project::add_project()`
- `qinAegis project list` → `project::list_projects()`
- `qinAegis project remove <name>` → `project::remove_project()`
- `qinAegis explore <project>` → `explore::run_explore()`
- `qinAegis generate <project>` → `generate::run_generate()`
- `qinAegis run <project>` → `run::run_tests()`
- `qinAegis export <project>` → `export::export_project()`

- [ ] **Step 3: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "refactor: wire up new commands in main.rs"
```

---

## Task 11: Verify build

- [ ] **Step 1: Build the project**

```bash
cargo build --manifest-path /Users/jinguo.zeng/dmall/project/qinAegis/Cargo.toml 2>&1
```

Expected: Build succeeds with no errors

- [ ] **Step 2: Run tests if any**

```bash
cargo test --manifest-path /Users/jinguo.zeng/dmall/project/qinAegis/Cargo.toml 2>&1
```

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "chore: complete local storage migration - all tests pass"
```

---

## Spec Coverage Check

- [x] Remove Notion → Task 1
- [x] Local storage structure → Task 2
- [x] Init without OAuth → Task 4
- [x] Project commands → Task 5
- [x] Export command → Task 6
- [x] Explore writes to project dir → Task 7
- [x] Generate writes to cases/ → Task 8
- [x] Run reads from local → Task 9

---

## Notes

- HTML export uses inline `format!` string - simple but effective for a single-file export
- `TestCaseGenerator::generate()` returns `Vec<crate::generator::TestCase>` which has `case_type` field, mapped correctly to storage `test_type`
- The `cases` field in storage::TestCase uses `test_type` vs `type` - verify this is consistent