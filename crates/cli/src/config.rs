use std::path::PathBuf;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    pub notion: NotionConfig,
    pub sandbox: SandboxConfig,
    pub exploration: ExplorationConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct NotionConfig {
    pub workspace_id: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SandboxConfig {
    pub compose_file: String,
    pub steel_port: u16,
    pub cdp_port: u16,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExplorationConfig {
    pub max_depth: u32,
    pub max_pages_per_seed: u32,
    pub screenshot_dir: PathBuf,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("qinAegis")
            .join("config.toml");

        let content = std::fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
