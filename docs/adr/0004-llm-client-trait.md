# LlmClient Trait + Prompt i18n

`MiniMaxClient` 是硬编码的具体类型，Prompt 散布在 `generator.rs` 和 `critic.rs` 中。决定提取 `LlmClient` trait 支持多实现（MiniMax、OpenAI、Claude）。

`ChatOptions` 支持模型特定选项（vision、json_schema）。prompts 模块支持多语言，i18n 化。

所有 LLM 调用通过 trait interface，切换模型无需修改业务逻辑。
