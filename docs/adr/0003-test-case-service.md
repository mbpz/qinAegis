# TestCaseService — Business Logic Separation

<!-- product-status-2.0 -->
> **决策沿用与演进说明 · 2026-11-16**
>
> 决策状态：沿用业务服务与入口解耦原则。原 CLI 持有 TestCaseService 的装配方式属于历史设计；2.0 从本地 Web 服务进入，生成/审核/保存应与契约治理、模型路由、执行和证据清晰分工。原示例不等于当前全部调用链已统一到同一服务。
>
> 正文的状态、任务勾选、代码和验收结论保留为原始记录，不作为 2.0 开发指令或交付证明。当前以[产品记忆](../../CONTEXT.md)、[2.0 批准方案](../plans/local-web.md)及[实现进度与验收缺口](../progress/local-web-2.0.md)为准。
<!-- /product-status-2.0 -->

业务逻辑（生成-审查-保存流程）直接写在 CLI 命令中，`run_generate` 手动编排 `MiniMaxClient → TestCaseGenerator → Critic → LocalStorage`，无法独立测试。

决定提取 `TestCaseService`，独立于 `TestExecutor`，由 CLI 持有实例。`TestCaseService` 持有 `Storage` 和 `LlmClient`，拥有 generate + review + persist 的完整流程。
