# TestCaseService — Business Logic Separation

业务逻辑（生成-审查-保存流程）直接写在 CLI 命令中，`run_generate` 手动编排 `MiniMaxClient → TestCaseGenerator → Critic → LocalStorage`，无法独立测试。

决定提取 `TestCaseService`，独立于 `TestExecutor`，由 CLI 持有实例。`TestCaseService` 持有 `Storage` 和 `LlmClient`，拥有 generate + review + persist 的完整流程。
