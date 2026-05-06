// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::automation::{AutomationError, BfsExplorer, BrowserAutomation, ExploreResult, MidsceneAutomation};
use crate::protocol::{LlmConfig, SandboxConfig};

pub struct Explorer {
    automation: MidsceneAutomation,
    bfs: BfsExplorer,
}

impl Explorer {
    pub async fn new(llm_config: Option<LlmConfig>, sandbox_config: Option<SandboxConfig>) -> anyhow::Result<Self> {
        let automation: Result<MidsceneAutomation, AutomationError> =
            MidsceneAutomation::new(llm_config, sandbox_config).await;
        let automation = automation?;
        let bfs = BfsExplorer::new(Box::new(automation.clone()));
        Ok(Self { automation, bfs })
    }

    pub async fn explore(&mut self, seed_url: &str, max_depth: u32) -> anyhow::Result<ExploreResult> {
        self.bfs
            .explore(&[seed_url.to_string()], max_depth)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.automation.shutdown().await.map_err(|e| anyhow::anyhow!("{}", e))
    }
}

impl Clone for Explorer {
    fn clone(&self) -> Self {
        Self {
            automation: self.automation.clone(),
            bfs: BfsExplorer::new(Box::new(self.automation.clone())),
        }
    }
}
