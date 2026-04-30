use crate::llm::{ArcLlmClient, LlmClient, Message};

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
}

impl TestCaseGenerator {
    pub fn new(llm: ArcLlmClient) -> Self {
        Self { llm }
    }

    pub async fn generate(
        &self,
        spec_markdown: &str,
        requirement_text: &str,
    ) -> anyhow::Result<Vec<TestCase>> {
        let prompt = format!(
            r#"你是一名资深 QA 工程师，熟悉 Midscene.js 的 YAML 测试格式。

项目规格书:
{}

需求描述:
{}

请生成符合以下规范的测试用例列表（JSON 格式）:

{{"id": "TC-001", "name": "用例标题", "requirement_id": "REQ-001", "type": "smoke|functional|performance|stress", "priority": "P0|P1|P2", "yaml_script": "完整的 Midscene YAML 脚本", "expected_result": "期望结果", "tags": ["tag1"]}}

规则:
1. P0 仅覆盖核心路径
2. yaml_script 使用 aiAct / aiAssert / aiQuery API
3. 不得使用 CSS selector 或 XPath
4. 每个用例必须有明确的 aiAssert 断言"#,
            spec_markdown, requirement_text
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

        let cases: Vec<TestCase> = serde_json::from_str(json_str)
            .map_err(|e| anyhow::anyhow!("failed to parse generated cases: {} | response: {}", e, json_str))?;

        Ok(cases)
    }
}
