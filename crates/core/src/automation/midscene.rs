use crate::automation::{
    AutomationCommand, AutomationError, AutomationResponse, BrowserAutomation,
    ExploreResult, PageInfo, TestResult,
};
use crate::protocol::{JsonRpcRequest, JsonRpcResponse, MidsceneProcess};
use crate::prompts::ExplorerPrompt;
use async_trait::async_trait;

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

    async fn call(&self, req: JsonRpcRequest) -> Result<JsonRpcResponse, AutomationError> {
        self.process
            .call(req)
            .await
            .map_err(|e| AutomationError::ProcessDied(e.to_string()))
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
            serde_json::from_value(resp.data.unwrap_or_default())
                .map_err(|e| AutomationError::ParseError(e.to_string()))
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
}

impl BfsExplorer {
    pub fn new(automation: Box<dyn BrowserAutomation>) -> Self {
        Self { automation }
    }

    /// Perform BFS exploration starting from seed URLs.
    pub async fn explore(&mut self, seed_urls: &[String], max_depth: u32) -> Result<ExploreResult, AutomationError> {
        let mut visited = std::collections::HashSet::new();
        let mut queue: Vec<(String, u32)> = seed_urls.iter().map(|u| (u.clone(), 0)).collect();
        let mut pages: Vec<PageInfo> = Vec::new();

        while let Some((url, depth)) = queue.pop() {
            if visited.contains(&url) || depth > max_depth {
                continue;
            }
            visited.insert(url.clone());

            // Use ai_query to extract page info + links
            let prompt = ExplorerPrompt::new(crate::prompts::Locale::Zh).instruction;

            match self.automation.ai_query(&prompt).await {
                Ok(json_str) => {
                    if let Ok(info) = serde_json::from_str::<PageInfo>(&json_str) {
                        pages.push(info.clone());

                        // Queue discovered links
                        for link in info.links.iter().take(10) {
                            if !visited.contains(link) {
                                queue.push((link.clone(), depth + 1));
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("ai_query failed for {}: {}", url, e);
                }
            }
        }

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
