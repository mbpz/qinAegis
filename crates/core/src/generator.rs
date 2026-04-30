use crate::llm::{ArcLlmClient, ChatOptions, LlmClient, Message};
use crate::prompts::{GeneratorPrompts, Locale};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestCase {
    pub id: String,
    pub name: String,
    pub requirement_id: String,
    #[serde(rename = "type")]
    pub case_type: String,
    pub priority: String,
    pub yaml_script: String,
    pub expected_result: String,
    pub tags: Vec<String>,
}

pub struct TestCaseGenerator {
    llm: ArcLlmClient,
    locale: Locale,
    options: ChatOptions,
}

impl TestCaseGenerator {
    pub fn new(llm: ArcLlmClient) -> Self {
        Self {
            llm,
            locale: Locale::default(),
            options: ChatOptions::new(),
        }
    }

    pub fn with_locale(mut self, locale: Locale) -> Self {
        self.locale = locale;
        self
    }

    pub fn with_options(mut self, options: ChatOptions) -> Self {
        self.options = options;
        self
    }

    pub async fn generate(
        &self,
        spec_markdown: &str,
        requirement_text: &str,
    ) -> anyhow::Result<Vec<TestCase>> {
        let prompts = GeneratorPrompts::new(self.locale, spec_markdown, requirement_text);

        let messages = if let Some(system) = self.options.system_prompt.clone() {
            vec![
                Message::system(system),
                Message::user(prompts.user),
            ]
        } else {
            vec![
                Message::system(prompts.system),
                Message::user(prompts.user),
            ]
        };

        let response = self.llm.chat_with_options(&messages, self.options.clone()).await?;

        let json_str = response
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let cases: Vec<TestCase> = serde_json::from_str(json_str)
            .map_err(|e| anyhow::anyhow!("failed to parse generated cases: {} | response: {}", e, json_str))?;

        Ok(cases)
    }
}
