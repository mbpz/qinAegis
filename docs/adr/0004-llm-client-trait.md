# LlmClient Trait + Prompt i18n

<!-- product-status-2.0 -->
> **决策沿用与演进说明 · 2026-11-16**
>
> 决策状态：沿用 LlmClient 抽象，演进为可扩展注册与路由。2.0 支持模型配置、角色选择与冻结策略，但当前内置协议与实际视觉兼容性以进度为准；原 MiniMax/OpenAI/Claude、vision/json_schema 与多语言列举不表示每个厂商/能力均已接入验收。
>
> 正文的状态、任务勾选、代码和验收结论保留为原始记录，不作为 2.0 开发指令或交付证明。当前以[产品记忆](../../CONTEXT.md)、[2.0 批准方案](../plans/local-web.md)及[实现进度与验收缺口](../progress/local-web-2.0.md)为准。
<!-- /product-status-2.0 -->

`MiniMaxClient` 是硬编码的具体类型，Prompt 散布在 `generator.rs` 和 `critic.rs` 中。决定提取 `LlmClient` trait 支持多实现（MiniMax、OpenAI、Claude）。

`ChatOptions` 支持模型特定选项（vision、json_schema）。prompts 模块支持多语言，i18n 化。

所有 LLM 调用通过 trait interface，切换模型无需修改业务逻辑。
