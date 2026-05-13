// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

/// Self-Healing service for qinAegis.
/// When a test case fails, this module analyzes the failure and generates
/// a corrected YAML script to retry the failed step.
/// Healed results are stored separately from the original case (never overwriting approved assets).

use crate::llm::{ArcLlmClient, ChatOptions, LlmClient, Message};

const HEAL_SYSTEM_PROMPT: &str = r#"You are a QA engineer specializing in AI browser test automation using Midscene.js.
You are helping fix a failed test step.

Given:
1. The original test YAML script (Midscene format)
2. The error message from the failed step
3. The step that failed (aiAct, aiAssert, or aiQuery)

Your task:
- Analyze why the step failed
- Generate a corrected version of ONLY the failing step
- Return the complete corrected YAML script (not just the fixed step)

YAML format for Midscene:
```yaml
target:
  url: https://example.com
tasks:
  - name: step name
    flow:
      - aiAct: natural language action
      - aiAssert: natural language assertion
      - aiQuery: natural language extraction
```

OR flat array format:
```yaml
- aiAct: action 1
- aiAssert: assertion 1
- aiAct: action 2
```

Rules:
1. Only fix the failing step, keep all other steps unchanged
2. aiAct: be more specific about the target element using visual context
3. aiAssert: make the assertion more robust, less brittle
4. Return the COMPLETE YAML script with all steps (original + fixed)
5. Do NOT change the target URL
6. Keep the same format (array vs tasks format)

Respond with ONLY the corrected YAML script, no explanations."#;

const HEAL_USER_TEMPLATE: &str = r#"Original YAML script:
```yaml
{original_yaml}
```

Failed step: {failed_step}
Error message: {error_message}

Please generate the corrected YAML script:"#;

#[derive(Clone)]
pub struct Healer {
    llm: ArcLlmClient,
}

impl Healer {
    pub fn new(llm: ArcLlmClient) -> Self {
        Self { llm }
    }

    /// Analyze a failed test and generate a corrected YAML script.
    ///
    /// Returns the corrected YAML string, or None if healing failed.
    pub async fn heal(
        &self,
        original_yaml: &str,
        failed_step: &str,
        error_message: &str,
    ) -> Option<String> {
        let user_content = HEAL_USER_TEMPLATE
            .replace("{original_yaml}", original_yaml)
            .replace("{failed_step}", failed_step)
            .replace("{error_message}", error_message);

        let messages = vec![
            Message::system(HEAL_SYSTEM_PROMPT),
            Message::user(user_content),
        ];

        let options = ChatOptions::new()
            .with_max_tokens(4096)
            .with_temperature(0.3);

        match self.llm.chat_with_options(&messages, options).await {
            Ok(response) => {
                // Extract YAML from response (strip markdown code blocks if present)
                let yaml = extract_yaml(&response);
                if yaml.is_empty() {
                    tracing::warn!("Healer: LLM returned empty response");
                    None
                } else {
                    Some(yaml)
                }
            }
            Err(e) => {
                tracing::warn!("Healer: LLM call failed: {}", e);
                None
            }
        }
    }
}

/// Extract YAML content from LLM response, stripping markdown code blocks.
fn extract_yaml(response: &str) -> String {
    let response = response.trim();
    // Strip ```yaml ... ``` wrapper
    if response.starts_with("```yaml") && response.ends_with("```") {
        let start = response.find('\n').map(|p| p + 1).unwrap_or(7);
        let end = response.len().saturating_sub(3);
        response[start..end].trim().to_string()
    } else if response.starts_with("```") && response.ends_with("```") {
        let start = response.find('\n').map(|p| p + 1).unwrap_or(4);
        let end = response.len().saturating_sub(3);
        response[start..end].trim().to_string()
    } else {
        response.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_yaml_with_fence() {
        let input = "```yaml\ntarget:\n  url: https://example.com\ntasks:\n  - name: test\n    flow:\n      - aiAct: click login\n```";
        let result = extract_yaml(input);
        assert!(result.contains("url: https://example.com"));
        assert!(result.starts_with("target:"));
    }

    #[test]
    fn test_extract_yaml_no_fence() {
        let input = "target:\n  url: https://example.com\ntasks:\n  - name: test";
        let result = extract_yaml(input);
        assert!(result.contains("url: https://example.com"));
    }
}
