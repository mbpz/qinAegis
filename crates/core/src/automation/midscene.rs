// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::automation::{
    AutomationCommand, AutomationError, AutomationResponse,
    BrowserAutomation, ExploreResult, PageInfo, TestResult,
};
use super::trait_def::AiPageInfo;
use crate::performance::LighthouseResult;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse, MidsceneProcess};
use crate::prompts::ExplorerPrompt;
use crate::sandbox::{SandboxAdapter, PlaywrightBrowserAdapter};
use crate::stress::{LocustResult, StressTestConfig};
use async_trait::async_trait;
use std::sync::Arc;

/// BrowserAutomation implementation backed by MidsceneProcess (TS executor).
#[derive(Clone)]
pub struct MidsceneAutomation {
    process: MidsceneProcess,
}

impl MidsceneAutomation {
    pub async fn new(
        llm_config: Option<crate::protocol::LlmConfig>,
        sandbox_config: Option<crate::protocol::SandboxConfig>,
    ) -> Result<Self, AutomationError> {
        let process = MidsceneProcess::spawn(llm_config, sandbox_config)
            .await
            .map_err(|e| AutomationError::ProcessDied(e.to_string()))?;
        Ok(Self { process })
    }

    /// Spawn with an explicit SandboxAdapter (enables CDP retry and hot reload).
    pub async fn with_adapter(
        llm_config: Option<crate::protocol::LlmConfig>,
        adapter: Arc<dyn SandboxAdapter>,
    ) -> Result<Self, AutomationError> {
        let process = MidsceneProcess::with_adapter(llm_config, adapter)
            .await
            .map_err(|e| AutomationError::ProcessDied(e.to_string()))?;
        Ok(Self { process })
    }

    /// Create a PlaywrightBrowserAdapter for the given CDP port (no Docker needed).
    pub fn playwright_adapter(cdp_port: u16) -> Arc<dyn SandboxAdapter> {
        Arc::new(PlaywrightBrowserAdapter::new(cdp_port))
    }

    /// Create a PlaywrightBrowserAdapter with auto-detection (no Docker needed).
    pub fn playwright_adapter_auto() -> Arc<dyn SandboxAdapter> {
        Arc::new(PlaywrightBrowserAdapter::new(9222))
    }

    async fn call(&self, req: JsonRpcRequest) -> Result<JsonRpcResponse, AutomationError> {
        self.process
            .call(req)
            .await
            .map_err(|e| AutomationError::ProcessDied(e.to_string()))
    }

    /// Run a Lighthouse performance audit via the TS sandbox.
    pub async fn run_lighthouse(&self, url: &str) -> Result<LighthouseResult, AutomationError> {
        let req = JsonRpcRequest::Lighthouse { url: url.to_string() };
        let resp = self.call(req).await?;
        if resp.ok {
            serde_json::from_value(resp.data.unwrap_or_default())
                .map_err(|e| AutomationError::ParseError(e.to_string()))
        } else {
            Err(AutomationError::Internal(resp.error.unwrap_or_default()))
        }
    }

    /// Run a Locust stress test via the TS sandbox.
    pub async fn run_stress(&self, config: &StressTestConfig) -> Result<LocustResult, AutomationError> {
        let req = JsonRpcRequest::Stress {
            target_url: config.target_url.clone(),
            users: config.users,
            spawn_rate: config.spawn_rate,
            duration: config.duration_seconds,
        };
        let resp = self.call(req).await?;
        if resp.ok {
            serde_json::from_value(resp.data.unwrap_or_default())
                .map_err(|e| AutomationError::ParseError(e.to_string()))
        } else {
            Err(AutomationError::Internal(resp.error.unwrap_or_default()))
        }
    }

    fn explore_result_from_response(resp: JsonRpcResponse) -> Result<ExploreResult, AutomationError> {
        if resp.ok {
            serde_json::from_value(resp.data.unwrap_or_default())
                .map_err(|e| AutomationError::ParseError(e.to_string()))
        } else {
            Err(AutomationError::Internal(resp.error.unwrap_or_default()))
        }
    }

    fn test_result_from_response(
        case_id: String,
        resp: JsonRpcResponse,
    ) -> Result<TestResult, AutomationError> {
        if resp.ok {
            let result: TestResult = serde_json::from_value(resp.data.unwrap_or_default())
                .map_err(|e| AutomationError::ParseError(e.to_string()))?;
            Ok(result)
        } else {
            Ok(TestResult {
                case_id,
                passed: false,
                duration_ms: 0,
                screenshot_base64: None,
                error_message: resp.error,
            })
        }
    }
}

#[async_trait]
impl BrowserAutomation for MidsceneAutomation {
    async fn execute(&self, _cmd: AutomationCommand) -> Result<AutomationResponse, AutomationError> {
        // This method exists for trait completeness but individual methods should be used.
        Err(AutomationError::UnsupportedCommand(
            "use specific methods instead".into(),
        ))
    }

    async fn explore(&self, url: &str, depth: u32) -> Result<ExploreResult, AutomationError> {
        let req = JsonRpcRequest::Explore {
            url: url.to_string(),
            depth,
        };
        let resp = self.call(req).await?;
        Self::explore_result_from_response(resp)
    }

    async fn run_yaml(&self, yaml_script: &str, case_id: &str) -> Result<TestResult, AutomationError> {
        let req = JsonRpcRequest::RunYaml {
            yaml_script: yaml_script.to_string(),
            case_id: case_id.to_string(),
            target_url: None,
        };
        let resp = self.call(req).await?;
        Self::test_result_from_response(case_id.to_string(), resp)
    }

    async fn screenshot(&self) -> Result<String, AutomationError> {
        let req = JsonRpcRequest::Screenshot;
        let resp = self.call(req).await?;
        if resp.ok {
            serde_json::from_value(resp.data.unwrap_or_default())
                .map_err(|e| AutomationError::ParseError(e.to_string()))
        } else {
            Err(AutomationError::Internal(resp.error.unwrap_or_default()))
        }
    }

    async fn goto(&self, url: &str) -> Result<(), AutomationError> {
        let req = JsonRpcRequest::Goto {
            url: url.to_string(),
        };
        let resp = self.call(req).await?;
        if resp.ok {
            Ok(())
        } else {
            Err(AutomationError::NavigationFailed(
                resp.error.unwrap_or_default(),
            ))
        }
    }

    async fn ai_query(&self, prompt: &str) -> Result<String, AutomationError> {
        let req = JsonRpcRequest::AiQuery(prompt.to_string());
        let resp = self.call(req).await?;
        if resp.ok {
            let data = resp.data.unwrap_or_default();
            let json_str = match data {
                serde_json::Value::String(s) => s,
                other => serde_json::to_string(&other).unwrap_or_default(),
            };
            Ok(json_str)
        } else {
            Err(AutomationError::LlmQueryFailed(
                resp.error.unwrap_or_default(),
            ))
        }
    }

    async fn ai_act(&self, action: &str) -> Result<(), AutomationError> {
        let req = JsonRpcRequest::AiAct(action.to_string());
        let resp = self.call(req).await?;
        if resp.ok {
            Ok(())
        } else {
            Err(AutomationError::Internal(resp.error.unwrap_or_default()))
        }
    }

    async fn ai_assert(&self, assertion: &str) -> Result<(), AutomationError> {
        let req = JsonRpcRequest::AiAssert(assertion.to_string());
        let resp = self.call(req).await?;
        if resp.ok {
            Ok(())
        } else {
            Err(AutomationError::Internal(resp.error.unwrap_or_default()))
        }
    }

    async fn shutdown(&self) -> Result<(), AutomationError> {
        let req = JsonRpcRequest::Shutdown;
        self.call(req).await?;
        Ok(())
    }
}

/// BFS exploration implemented in Rust.
///
/// This moves the link-crawling logic from TypeScript to Rust,
/// using the Midscene TS process only for AI page extraction.
pub struct BfsExplorer {
    automation: Box<dyn BrowserAutomation>,
    auth: Option<AuthConfig>,
}

/// Auth configuration for exploring behind login
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub username: String,
    pub password: String,
    pub login_prompt: Option<String>,
}

impl BfsExplorer {
    pub fn new(automation: Box<dyn BrowserAutomation>, auth: Option<AuthConfig>) -> Self {
        Self { automation, auth }
    }

    /// Perform visual click-driven exploration.
    /// Instead of crawling by URL, uses AI to identify clickable elements
    /// and explores via ai_act clicks (handles SPA client-side routing).
    pub async fn explore(&mut self, seed_urls: &[String], max_depth: u32) -> Result<ExploreResult, AutomationError> {
        let mut visited_states: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut queue: Vec<(String, u32)> = seed_urls.iter().map(|u| (u.clone(), 0)).collect();
        let mut pages: Vec<PageInfo> = Vec::new();
        let mut clicks_count = 0;

        println!("[explorer] Starting visual exploration...");
        println!("[explorer] Seed URLs: {:?}", seed_urls);
        println!("[explorer] Max depth: {}", max_depth);

        // When auth is configured: login first, then explore post-login pages
        // When no auth: standard BFS on seed URLs
        if let Some(auth) = &self.auth {
            if !seed_urls.is_empty() {
                println!("[explorer] Auto-login before exploration...");
                let login_prompt = auth.login_prompt.clone()
                    .unwrap_or_else(|| format!("在账号框输入{}，在密码框输入{}，然后点击登录按钮", auth.username, auth.password));
                let full_action = login_prompt;
                if let Err(e) = self.automation.goto(&seed_urls[0]).await {
                    println!("[explorer] WARN: goto seed URL failed: {}", e);
                } else {
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    if self.automation.ai_act(&full_action).await.is_ok() {
                        println!("[explorer] Auto-login action completed");
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                        // After login, queue home page for exploration (don't re-visit login page)
                        let home_url = if let Some(pos) = seed_urls[0].find("://") {
                            let host = seed_urls[0][pos+3..].split('/').next().unwrap_or("");
                            let scheme = &seed_urls[0][..pos];
                            format!("{}://{}/", scheme, host)
                        } else {
                            "/".to_string()
                        };
                        let home_url_sig = format!("url:{}", home_url);
                        if !visited_states.contains(&home_url_sig) {
                            // Clear the login URL from queue, replace with home
                            queue.clear();
                            queue.push((home_url.clone(), 0));
                            visited_states.insert(home_url_sig);
                            println!("[explorer] Queuing post-login home: {}", home_url);
                        }
                    }
                }
            }
        }

        let explorer_prompt = ExplorerPrompt::new(crate::prompts::Locale::Zh).instruction;
        let max_clicks_per_page = 5;
        let max_total_clicks = 50;

        while let Some((url, depth)) = queue.pop() {
            if depth > max_depth {
                continue;
            }

            println!("[explorer] [depth {}] Navigating to: {}", depth, url);
            if let Err(e) = self.automation.goto(&url).await {
                println!("[explorer] WARN: goto failed for {}: {}", url, e);
                continue;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            // BFS exploration per page: click multiple elements before moving on
            let mut page_clicks = 0;
            let mut seen_elements: std::collections::HashSet<String> = std::collections::HashSet::new();

            loop {
                if clicks_count >= max_total_clicks {
                    println!("[explorer] Max total clicks reached, stopping");
                    break;
                }
                if page_clicks >= max_clicks_per_page {
                    println!("[explorer] Max clicks per page reached, moving on");
                    break;
                }

                // Query page for clickable elements
                match self.automation.ai_query(&explorer_prompt).await {
                    Ok(json_str) => {
                        match serde_json::from_str::<AiPageInfo>(&json_str) {
                            Ok(ai_info) => {
                                let mut info = PageInfo::from(ai_info);
                                info.url = url.clone();
                                pages.push(info.clone());

                                // Deduplicate by state signature
                                let state_sig = format!("{}|{}|{:?}", info.title, info.primary_nav.join(","), info.key_elements);
                                if visited_states.contains(&state_sig) {
                                    println!("[explorer] Already visited this page state, skipping");
                                    break;
                                }
                                visited_states.insert(state_sig);

                                let clickables = &info.clickable_elements;

                                // Filter out already-seen elements
                                let new_elements: Vec<_> = clickables.iter()
                                    .filter(|e| !seen_elements.contains(&e.description))
                                    .collect();

                                if new_elements.is_empty() {
                                    println!("[explorer] No more new clickable elements on this page");
                                    break;
                                }

                                // Click the first new element
                                let elem = &new_elements[0];
                                let action = format!("点击：{}", elem.description);
                                println!("[explorer] Click [{}]: {} ({})", page_clicks + 1, elem.description, elem.reason);

                                seen_elements.insert(elem.description.clone());

                                match self.automation.ai_act(&action).await {
                                    Ok(()) => {
                                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                                        page_clicks += 1;
                                        clicks_count += 1;
                                    }
                                    Err(e) => {
                                        println!("[explorer] WARN: ai_act failed: {}", e);
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                println!("[explorer] WARN: parse page info failed: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        println!("[explorer] WARN: ai_query failed: {}", e);
                        break;
                    }
                }
            }
        }

        println!("[explorer] Exploration complete! Crawled {} pages, total clicks: {}", pages.len(), clicks_count);
        let markdown = to_markdown(&pages);
        Ok(ExploreResult { pages, markdown })
    }
}

fn to_markdown(pages: &[PageInfo]) -> String {
    let mut md = "# 项目规格书\n\n".to_string();
    for page in pages {
        md += &format!("## {}\n", page.url);
        md += &format!("- **标题**: {}\n", page.title);
        md += &format!("- **导航**: [{}]\n", page.primary_nav.join(", "));
        md += &format!("- **功能**: {}\n", page.main_features.join(", "));
        md += &format!(
            "- **认证**: {}\n",
            if page.auth_required {
                "需要登录"
            } else {
                "无需登录"
            }
        );
        md += &format!("- **技术栈**: {}\n", page.tech_stack.join(", "));
        if !page.forms.is_empty() {
            md += &format!(
                "- **表单**: {}\n",
                page.forms
                    .iter()
                    .map(|f| format!("{} {} ({})", f.method.to_uppercase(), f.action, f.fields.join(", ")))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
        md += "\n";
    }
    md
}
