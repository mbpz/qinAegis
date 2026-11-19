# BrowserAutomation Trait — Unified IPC Interface

<!-- product-status-2.0 -->
> **决策沿用与演进说明 · 2026-11-16**
>
> 决策状态：沿用接口解耦原则，具体方法与实现继续演进。BrowserAutomation 边界应支持替换执行适配器；2.0 保留 Playwright + Midscene.js，桌面无障碍仅作为另行授权和验收的可选适配能力。原 IPC/BFS 方法描述是决策背景，不保证与当前接口逐项一致。
>
> 正文的状态、任务勾选、代码和验收结论保留为原始记录，不作为 2.0 开发指令或交付证明。当前以[产品记忆](../../CONTEXT.md)、[2.0 批准方案](../plans/local-web.md)及[实现进度与验收缺口](../progress/local-web-2.0.md)为准。
<!-- /product-status-2.0 -->

Rust 和 TypeScript 之间的 IPC 通过三个独立模块（`protocol.rs`、`executor.rs`、`explorer.rs`）各自包装同一个 `MidsceneProcess`。三套接口语义不统一，调用方需要区分"探索模式"和"测试执行模式"。决定统一为 `BrowserAutomation` trait。

采用 `dyn BrowserAutomation` 动态分发，以支持运行时切换实现。错误模式统一为 `AutomationError`。BFS 探索逻辑从 TypeScript 移入 Rust，成为 `BrowserAutomation::explore` 方法。
