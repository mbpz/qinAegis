// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::automation::{
    AuthConfig, AutomationError, BfsExplorer, BrowserAutomation, ExploreResult, MidsceneAutomation,
};
use crate::protocol::{LlmConfig, SandboxConfig};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct Explorer {
    automation: MidsceneAutomation,
    bfs: BfsExplorer,
    auth: Option<AuthConfig>,
    cancellation: Arc<AtomicBool>,
}

impl Explorer {
    pub async fn new(
        llm_config: Option<LlmConfig>,
        sandbox_config: Option<SandboxConfig>,
        auth: Option<AuthConfig>,
    ) -> anyhow::Result<Self> {
        Self::new_cancellable(
            llm_config,
            sandbox_config,
            auth,
            Arc::new(AtomicBool::new(false)),
        )
        .await
    }

    pub async fn new_cancellable(
        llm_config: Option<LlmConfig>,
        sandbox_config: Option<SandboxConfig>,
        auth: Option<AuthConfig>,
        cancel: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        let automation: Result<MidsceneAutomation, AutomationError> =
            MidsceneAutomation::new_cancellable(llm_config, sandbox_config, cancel.clone()).await;
        let automation = automation?;
        let bfs = BfsExplorer::new(Box::new(automation.clone()), auth.clone());
        Ok(Self {
            automation,
            bfs,
            auth,
            cancellation: cancel,
        })
    }

    pub async fn explore(
        &mut self,
        seed_url: &str,
        max_depth: u32,
    ) -> anyhow::Result<ExploreResult> {
        let seeds = [seed_url.to_string()];
        let cancellation = self.cancellation.clone();
        let result = tokio::select! {
            biased;
            _ = async {
                while !cancellation.load(Ordering::Acquire) {
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
            } => Err(anyhow::anyhow!("exploration cancelled")),
            result = self.bfs.explore(&seeds, max_depth) => result.map_err(|e| anyhow::anyhow!("{}", e)),
        };
        if self.cancellation.load(Ordering::Acquire) {
            self.automation.terminate().await?;
        }
        result
    }

    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.automation
            .shutdown()
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }
}

impl Clone for Explorer {
    fn clone(&self) -> Self {
        Self {
            automation: self.automation.clone(),
            bfs: BfsExplorer::new(Box::new(self.automation.clone()), self.auth.clone()),
            auth: self.auth.clone(),
            cancellation: self.cancellation.clone(),
        }
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;

    #[tokio::test]
    async fn cancelled_explorer_does_not_spawn_an_engine() {
        let cancel = Arc::new(AtomicBool::new(true));
        let error = Explorer::new_cancellable(None, None, None, cancel)
            .await
            .err()
            .expect("cancelled construction must fail");
        assert!(error
            .to_string()
            .contains("cancelled before engine startup"));
    }
}
