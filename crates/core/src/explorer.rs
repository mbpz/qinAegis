use crate::protocol::{JsonRpcRequest, MidsceneProcess};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PageInfo {
    pub url: String,
    pub title: String,
    pub primary_nav: Vec<String>,
    pub main_features: Vec<String>,
    pub auth_required: bool,
    pub tech_stack: Vec<String>,
    pub forms: Vec<FormInfo>,
    pub links: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormInfo {
    pub action: String,
    pub method: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExploreResult {
    pub pages: Vec<PageInfo>,
    pub markdown: String,
}

pub struct Explorer {
    process: MidsceneProcess,
}

impl Explorer {
    pub async fn new() -> anyhow::Result<Self> {
        let process = MidsceneProcess::spawn().await?;
        Ok(Self { process })
    }

    pub async fn explore(&mut self, seed_url: &str, max_depth: u32) -> anyhow::Result<ExploreResult> {
        let req = JsonRpcRequest::Explore {
            url: seed_url.to_string(),
            depth: max_depth,
        };

        let resp = self.process.call(req).await?;

        if resp.ok {
            let result: ExploreResult = serde_json::from_value(resp.data.unwrap_or_default())?;
            Ok(result)
        } else {
            anyhow::bail!("explore failed: {}", resp.error.unwrap_or_default())
        }
    }

    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.process.call(JsonRpcRequest::Shutdown).await?;
        Ok(())
    }
}
