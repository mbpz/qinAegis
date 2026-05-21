# Rust Native Browser Automation — Tasks

**Spec ID:** 009-rust-native-automation
**Status:** In Progress
**Created:** 2026-05-21

---

## Task Breakdown

### P0: CDP 浏览器控制层

| # | Task | Status | Depends On |
|---|------|--------|------------|
| 1 | 添加 `chromiumoxide` 到 `crates/core/Cargo.toml` | SELECTED | — |
| 2 | 实现 `BrowserProcess`：启动 Chrome、连接到 CDP、页面管理 | SELECTED | 1 |
| 3 | 实现 `goto`、`screenshot`、`shutdown` 方法 | SELECTED | 2 |

### P0: AI 视觉层（替代 Midscene.js）

| # | Task | Status | Depends On |
|---|------|--------|------------|
| 4 | 实现 `ai_query`：CDP 截图 + LlmClient vision chat → JSON 解析 | SELECTED | 2, 3 |
| 5 | 实现 `ai_act`：截图 + LLM 分析坐标 → CDP 鼠标/键盘事件 | SELECTED | 4 |
| 6 | 实现 `ai_assert`：截图 + LLM 判断 → boolean | SELECTED | 4 |

### P0: NativeAutomation 实现

| # | Task | Status | Depends On |
|---|------|--------|------------|
| 7 | 创建 `crates/core/src/automation/native.rs`，实现 `BrowserAutomation` trait | SELECTED | 3, 4, 5, 6 |
| 8 | 实现 `run_yaml`：YAML 解析 + 逐条执行 aiAct/aiAssert/aiQuery | SELECTED | 7 |
| 9 | 实现 `explore` 委托（转发到 BfsExplorer） | SELECTED | 7 |
| 10 | 实现动作缓存 `ActionCache` | SELECTED | 7 |

### P1: 外部工具迁移

| # | Task | Status | Depends On |
|---|------|--------|------------|
| 11 | Lighthouse: 替换 sandbox 调用为 `tokio::process::Command` 直接调用 | DEFERRED | 7 |
| 12 | Locust: 替换 sandbox 调用为 `tokio::process::Command` 直接调用 | DEFERRED | 7 |
| 13 | ZAP: 替换 sandbox 调用为 `tokio::process::Command` 直接调用 | DEFERRED | 7 |

### P1: 清理

| # | Task | Status | Depends On |
|---|------|--------|------------|
| 14 | 移除 `protocol.rs` 中的 `MidsceneProcess` 和 JSON-RPC 类型 | DEFERRED | 7 |
| 15 | 移除 `midscene.rs` 中的 `MidsceneAutomation`（确认 NativeAutomation 完备后） | DEFERRED | 14 |
| 16 | 移除不必要的 `tokio::process` 相关特性 | DEFERRED | 14 |
| 17 | 从 `.gitignore` 移除 `sandbox/`？| DEFERRED | — |

### P2: 增强

| # | Task | Status | Depends On |
|---|------|--------|------------|
| 18 | 浏览器热重启（Hot Reload）/ CDP 断线重连 | DEFERRED | 7 |
| 19 | 并发浏览器实例池（当前 Semaphore 限制 4 并发） | DEFERRED | 7 |
| 20 | 浏览器启动缓存（复用已打开的浏览器） | DEFERRED | 7 |
