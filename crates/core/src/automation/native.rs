use crate::automation::{
    AutomationCommand, AutomationError, AutomationResponse,
    BrowserAutomation, ExploreResult, TestResult,
};
use crate::llm::{ArcLlmClient, ChatOptions, LlmClient, Message};
use async_trait::async_trait;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::Page;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct NativeAutomation {
    inner: Arc<Mutex<NativeInner>>,
    llm: ArcLlmClient,
}

struct NativeInner {
    browser: Option<Browser>,
    page: Option<Page>,
    _handler_handle: tokio::task::JoinHandle<()>,
}

impl NativeAutomation {
    /// Launch a headless Chrome browser via CDP and return a NativeAutomation instance.
    pub async fn new(llm: ArcLlmClient) -> Result<Self, AutomationError> {
        let config = BrowserConfig::builder()
            .no_sandbox()
            .build()
            .map_err(|e| AutomationError::Internal(format!("browser config: {}", e)))?;

        let (browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| AutomationError::CdpConnectionFailed(e.to_string()))?;

        let _handler_handle = tokio::spawn(async move {
            while let Some(_event) = handler.next().await {
                // consume CDP events (noop)
            }
        });

        let page = browser
            .new_page("about:blank")
            .await
            .map_err(|e| AutomationError::Internal(format!("new page: {}", e)))?;

        Ok(Self {
            inner: Arc::new(Mutex::new(NativeInner {
                browser: Some(browser),
                page: Some(page),
                _handler_handle,
            })),
            llm,
        })
    }

    /// Take a screenshot and return base64-encoded PNG.
    async fn screenshot_base64(&self) -> Result<String, AutomationError> {
        let inner = self.inner.lock().await;
        let page = inner.page.as_ref().ok_or_else(|| {
            AutomationError::Internal("browser already shut down".into())
        })?;
        let params = chromiumoxide::page::ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .build();
        let png_data = page
            .screenshot(params)
            .await
            .map_err(|e| AutomationError::Internal(format!("screenshot: {}", e)))?;
        Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png_data))
    }

    /// Build a vision user message with a base64 screenshot inline.
    fn vision_message(text: &str, b64: &str) -> Message {
        let content = format!(
            "![page screenshot](data:image/png;base64,{})\n\n{}",
            b64, text
        );
        Message {
            role: "user".to_string(),
            content,
        }
    }

    /// LLM vision chat: screenshot + prompt -> string response.
    async fn llm_vision(&self, prompt: &str, options: ChatOptions) -> Result<String, AutomationError> {
        let b64 = self.screenshot_base64().await?;
        let msg = Self::vision_message(prompt, &b64);
        self.llm
            .chat_with_options(&[msg], options.with_vision())
            .await
            .map_err(|e| AutomationError::LlmQueryFailed(e.to_string()))
    }
}

#[async_trait]
impl BrowserAutomation for NativeAutomation {
    async fn execute(&self, _cmd: AutomationCommand) -> Result<AutomationResponse, AutomationError> {
        Err(AutomationError::UnsupportedCommand(
            "use specific methods instead".into(),
        ))
    }

    async fn explore(&self, _url: &str, _depth: u32) -> Result<ExploreResult, AutomationError> {
        Err(AutomationError::UnsupportedCommand(
            "explore: use BfsExplorer with NativeAutomation".into(),
        ))
    }

    async fn run_yaml(&self, _yaml_script: &str, _case_id: &str) -> Result<TestResult, AutomationError> {
        Err(AutomationError::UnsupportedCommand(
            "YAML execution not yet implemented for NativeAutomation".into(),
        ))
    }

    async fn screenshot(&self) -> Result<String, AutomationError> {
        self.screenshot_base64().await
    }

    fn current_url(&self) -> Option<String> {
        let inner = futures::executor::block_on(self.inner.lock());
        let page = inner.page.as_ref()?;
        futures::executor::block_on(page.url()).ok()?
    }

    async fn goto(&self, url: &str) -> Result<(), AutomationError> {
        let inner = self.inner.lock().await;
        let page = inner.page.as_ref().ok_or_else(|| {
            AutomationError::Internal("browser already shut down".into())
        })?;
        page
            .goto(url)
            .await
            .map_err(|e| AutomationError::NavigationFailed(e.to_string()))?;
        Ok(())
    }

    async fn ai_query(&self, prompt: &str) -> Result<String, AutomationError> {
        let options = ChatOptions::new()
            .with_max_tokens(2048)
            .with_temperature(0.1);
        let query = format!(
            "You are a web page analyzer. Given the screenshot of a webpage, answer the following question:\n\n{}\n\n\
             Respond concisely and ONLY with the requested information, in JSON format when applicable.",
            prompt
        );
        self.llm_vision(&query, options).await
    }

    async fn ai_act(&self, action: &str) -> Result<(), AutomationError> {
        let options = ChatOptions::new()
            .with_max_tokens(1024)
            .with_temperature(0.1);
        let query = format!(
            "You are a web automation agent. Given the screenshot of a webpage, \
             you must perform the following action:\n\n{}\n\n\
             First, analyze the screenshot to understand what element to interact with.\n\
             Then output a JSON object with one of these formats:\n\
             - For click: {{\"type\":\"click\",\"selector\":\"CSS selector of the target element\"}}\n\
             - For type: {{\"type\":\"type\",\"selector\":\"CSS selector of the input\",\"value\":\"text to type\"}}\n\
             - For scroll: {{\"type\":\"scroll\",\"direction\":\"down|up\",\"amount\":300}}\n\
             - For select: {{\"type\":\"select\",\"selector\":\"CSS selector\",\"value\":\"option value\"}}\n\n\
             ONLY output the JSON object, nothing else.",
            action
        );
        let resp = self.llm_vision(&query, options).await?;

        let parsed: serde_json::Value = serde_json::from_str(&resp)
            .map_err(|e| AutomationError::ParseError(format!("ai_act response: {}", e)))?;

        let action_type = parsed["type"].as_str().unwrap_or("click");
        let selector = parsed["selector"].as_str().unwrap_or("");
        let value = parsed["value"].as_str().unwrap_or("");

        let inner = self.inner.lock().await;
        let page = inner.page.as_ref().ok_or_else(|| {
            AutomationError::Internal("browser already shut down".into())
        })?;

        match action_type {
            "click" => {
                page
                    .find_element(selector)
                    .await
                    .map_err(|e| AutomationError::Internal(format!("find element: {}", e)))?
                    .click()
                    .await
                    .map_err(|e| AutomationError::Internal(format!("click: {}", e)))?;
            }
            "type" => {
                let element = page
                    .find_element(selector)
                    .await
                    .map_err(|e| AutomationError::Internal(format!("find element: {}", e)))?;
                element
                    .click()
                    .await
                    .map_err(|e| AutomationError::Internal(format!("click: {}", e)))?;
                element
                    .type_str(value)
                    .await
                    .map_err(|e| AutomationError::Internal(format!("type: {}", e)))?;
            }
            other => {
                return Err(AutomationError::UnsupportedCommand(format!(
                    "ai_act action type '{}' not implemented",
                    other
                )));
            }
        }

        Ok(())
    }

    async fn ai_assert(&self, assertion: &str) -> Result<(), AutomationError> {
        let options = ChatOptions::new()
            .with_max_tokens(256)
            .with_temperature(0.0);
        let query = format!(
            "You are a web page tester. Given the screenshot of a webpage, \
             determine if the following assertion is TRUE or FALSE:\n\n\
             {}\n\n\
             Reply with exactly one word: PASS or FAIL.",
            assertion
        );
        let resp = self.llm_vision(&query, options).await?;
        let trimmed = resp.trim().to_uppercase();
        if trimmed.starts_with("FAIL") {
            return Err(AutomationError::Internal(format!(
                "assertion failed: {}",
                assertion
            )));
        }
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), AutomationError> {
        let mut guard = self.inner.lock().await;
        guard.page.take();
        guard.browser.take();
        drop(guard);
        Ok(())
    }
}
