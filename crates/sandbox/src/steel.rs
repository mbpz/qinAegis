use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct HealthResponse {
    status: String,
}

pub struct SteelClient {
    base_url: String,
    client: Client,
}

impl SteelClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            client: Client::new(),
        }
    }

    pub async fn health_check(&self) -> anyhow::Result<bool> {
        let resp = self
            .client
            .get(format!("{}/health", self.base_url))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(false);
        }

        let health: HealthResponse = resp.json().await?;
        Ok(health.status == "ok")
    }

    pub fn cdp_ws_url(&self, port: u16) -> String {
        format!("ws://localhost:{}/devtools/browser", port)
    }
}