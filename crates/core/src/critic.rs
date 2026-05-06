// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::llm::{ArcLlmClient, ChatOptions, LlmClient, Message};
use crate::prompts::{CriticPrompts, Locale};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CriticReview {
    pub score: u8,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
    pub coverage: String,
}

pub struct Critic {
    llm: ArcLlmClient,
    locale: Locale,
    options: ChatOptions,
}

impl Critic {
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

    pub async fn review(
        &self,
        test_case_yaml: &str,
        spec_markdown: &str,
        requirement_text: &str,
    ) -> anyhow::Result<CriticReview> {
        let prompts = CriticPrompts::new(self.locale, spec_markdown, test_case_yaml, requirement_text);

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

        let review: CriticReview = serde_json::from_str(json_str)
            .map_err(|e| anyhow::anyhow!("failed to parse critic review: {} | response: {}", e, json_str))?;

        Ok(review)
    }
}
