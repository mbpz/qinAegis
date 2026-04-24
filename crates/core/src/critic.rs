use crate::llm::{MiniMaxClient, Message};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CriticReview {
    pub score: u8,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
    pub coverage: String,
}

pub struct Critic {
    llm: MiniMaxClient,
}

impl Critic {
    pub fn new(llm: MiniMaxClient) -> Self {
        Self { llm }
    }

    pub async fn review(
        &self,
        test_case_yaml: &str,
        spec_markdown: &str,
        requirement_text: &str,
    ) -> anyhow::Result<CriticReview> {
        let prompt = format!(
            r#"审核以下测试用例，评估其完整性和可执行性：

规格书上下文:
{}

测试用例:
{}

需求:
{}

返回 JSON:
{{"score": 1-10, "issues": ["问题描述"], "suggestions": ["改进建议"], "coverage": "P0覆盖率评估"}}"#,
            spec_markdown, test_case_yaml, requirement_text
        );

        let response = self.llm.chat(&[
            Message {
                role: "user".to_string(),
                content: prompt,
            }
        ]).await?;

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
