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
                instruction: r#"分析当前页面，返回JSON格式：
{"title":"页面标题","primaryNav":["顶部导航项"],"mainFeatures":["主要功能列表"],"authRequired":false,"techStack":[],"forms":[],"keyElements":["关键可见元素"],"clickableElements":[{"description":"元素描述，如'底部导航栏的分类tab'","reason":"为什么值得点击"}]}

clickableElements要求：
- 识别页面中所有可交互的UI元素（Tab导航、商品卡片、搜索框、按钮、Banner等）
- 优先选择能够展现新内容区域的元素（底部Tab、分类入口、商品卡片、搜索结果）
- 避免重复点击相同区域的元素
- 每个元素用简短中文描述，ai_act能够理解并点击
- 通常返回3-8个最值得探索的元素
- description格式示例：'底部导航栏的购物车tab'、'福礼自选Banner'、'商品列表中的第1个商品卡片'"#.to_string(),
            },
            Locale::En => Self {
                instruction: r#"Analyze the current page and extract: title, top navigation, main features, whether login is required, detected tech stack, form information, key elements, and clickable UI elements.
Return JSON format:
{"title":"","primaryNav":[],"mainFeatures":[],"authRequired":false,"techStack":[],"forms":[],"keyElements":[],"clickableElements":[{"description":"element description","reason":"why worth clicking"}]}"#.to_string(),
            },
        }
    }
}
