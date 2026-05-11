// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

// ============================================================================
// Prompt localization
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    En,
    Zh,
}

impl Default for Locale {
    fn default() -> Self {
        Self::Zh
    }
}

impl Locale {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "en" | "english" => Self::En,
            _ => Self::Zh,
        }
    }
}

// ============================================================================
// Generator prompts
// ============================================================================

#[derive(Debug, Clone)]
pub struct GeneratorPrompts {
    pub system: String,
    pub user: String,
}

impl GeneratorPrompts {
    pub fn new(locale: Locale, spec_markdown: &str, requirement_text: &str) -> Self {
        match locale {
            Locale::Zh => Self {
                system: "你是一名资深 QA 工程师，熟悉 Midscene.js 的 YAML 测试格式。".to_string(),
                user: format!(
                    r#"项目规格书:
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
                ),
            },
            Locale::En => Self {
                system: "You are a senior QA engineer familiar with Midscene.js YAML test format.".to_string(),
                user: format!(
                    r#"Project specification:
{}

Requirement:
{}

Generate test cases in JSON format:

{{"id": "TC-001", "name": "Case title", "requirement_id": "REQ-001", "type": "smoke|functional|performance|stress", "priority": "P0|P1|P2", "yaml_script": "Complete Midscene YAML script", "expected_result": "Expected result", "tags": ["tag1"]}}

Rules:
1. P0 covers only critical paths
2. yaml_script uses aiAct / aiAssert / aiQuery API
3. No CSS selectors or XPath
4. Each case must have a clear aiAssert assertion"#,
                    spec_markdown, requirement_text
                ),
            },
        }
    }
}

// ============================================================================
// Critic prompts
// ============================================================================

#[derive(Debug, Clone)]
pub struct CriticPrompts {
    pub system: String,
    pub user: String,
}

impl CriticPrompts {
    pub fn new(locale: Locale, spec_markdown: &str, test_case_yaml: &str, requirement_text: &str) -> Self {
        match locale {
            Locale::Zh => Self {
                system: "你是一名资深 QA 工程师，擅长审核测试用例的完整性和可执行性。".to_string(),
                user: format!(
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
                ),
            },
            Locale::En => Self {
                system: "You are a senior QA engineer specializing in reviewing test case completeness and executability.".to_string(),
                user: format!(
                    r#"Review the following test case and evaluate its completeness and executability:

Specification context:
{}

Test case:
{}

Requirement:
{}

Return JSON:
{{"score": 1-10, "issues": ["issue description"], "suggestions": ["improvement suggestions"], "coverage": "P0 coverage evaluation"}}"#,
                    spec_markdown, test_case_yaml, requirement_text
                ),
            },
        }
    }
}

// ============================================================================
// Explorer prompts
// ============================================================================

#[derive(Debug, Clone)]
pub struct ExplorerPrompt {
    pub instruction: String,
}

impl ExplorerPrompt {
    pub fn new(locale: Locale) -> Self {
        match locale {
            Locale::Zh => Self {
                instruction: r#"分析当前页面，提取：标题、顶部导航、主要功能、是否需要登录、检测到的技术栈、表单信息、关键元素、所有内部链接的实际href URL地址。
重要：links字段必须是完整的URL地址或以/开头的相对路径，不能是链接文字。
返回JSON格式：
{"title":"","primaryNav":[],"mainFeatures":[],"authRequired":false,"techStack":[],"forms":[],"keyElements":[],"links":[]}"#.to_string(),
            },
            Locale::En => Self {
                instruction: r#"Analyze the current page and extract: title, top navigation, main features, whether login is required, detected tech stack, form information, key elements, all internal links.
Return JSON format:
{"title":"","primaryNav":[],"mainFeatures":[],"authRequired":false,"techStack":[],"forms":[],"keyElements":[],"links":[]}"#.to_string(),
            },
        }
    }
}
