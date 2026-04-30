# BrowserAutomation Trait — Unified IPC Interface

Rust 和 TypeScript 之间的 IPC 通过三个独立模块（`protocol.rs`、`executor.rs`、`explorer.rs`）各自包装同一个 `MidsceneProcess`。三套接口语义不统一，调用方需要区分"探索模式"和"测试执行模式"。决定统一为 `BrowserAutomation` trait。

采用 `dyn BrowserAutomation` 动态分发，以支持运行时切换实现。错误模式统一为 `AutomationError`。BFS 探索逻辑从 TypeScript 移入 Rust，成为 `BrowserAutomation::explore` 方法。
