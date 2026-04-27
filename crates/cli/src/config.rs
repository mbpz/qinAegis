use std::path::PathBuf;
use std::io::{self, Write};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub notion: NotionConfig,
    pub llm: LlmConfig,
    pub sandbox: SandboxConfig,
    #[serde(default)]
    pub exploration: ExplorationConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NotionConfig {
    pub client_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub workspace_id: String,
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

impl Default for NotionConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            workspace_id: String::new(),
        }
    }
}

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
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("qinAegis");
        Self {
            compose_file: config_dir.join("docker-compose.sandbox.yml").to_string_lossy().to_string(),
            steel_port: 3333,
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

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("qinAegis")
            .join("config.toml")
    }

    pub fn load() -> anyhow::Result<Option<Self>> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(Some(config))
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(&self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn is_notion_configured(&self) -> bool {
        !self.notion.client_id.is_empty() && !self.notion.client_secret.is_empty()
    }

    pub fn is_llm_configured(&self) -> bool {
        !self.llm.api_key.is_empty()
    }

    pub fn is_complete(&self) -> bool {
        self.is_notion_configured() && self.is_llm_configured()
    }
}

/// Interactive setup prompts
pub fn prompt_for_config() -> anyhow::Result<Config> {
    println!("\n=== QinAegis Configuration Setup ===\n");

    let mut config = Config {
        notion: NotionConfig::default(),
        llm: LlmConfig::default(),
        sandbox: SandboxConfig::default(),
        exploration: ExplorationConfig::default(),
    };

    // Notion OAuth
    println!("Notion OAuth Configuration:");
    config.notion.client_id = prompt("  Client ID")?;
    config.notion.client_secret = prompt("  Client Secret")?;

    // LLM Configuration
    println!("\nLLM Configuration (for Midscene AI):");
    config.llm.provider = prompt_with_default("  Provider", "minimax")?;
    config.llm.base_url = prompt_with_default("  Base URL", "https://api.minimax.chat/v1")?;
    config.llm.model = prompt_with_default("  Model", "MiniMax-VL-01")?;
    config.llm.api_key = prompt("  API Key")?;

    println!("\n✓ Configuration complete!\n");

    Ok(config)
}

fn prompt(prompt: &str) -> anyhow::Result<String> {
    print!("{}: ", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_with_default(prompt: &str, default: &str) -> anyhow::Result<String> {
    print!("{} [{}]: ", prompt, default);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();
    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input.to_string())
    }
}
