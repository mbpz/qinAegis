# Rust Native Browser Automation — Architecture Plan

**Spec ID:** 009-rust-native-automation
**Status:** Draft
**Created:** 2026-05-21

---

## 架构概览

替换后架构：

```
Rust BrowserAutomation trait
    → NativeAutomation (新实现，纯 Rust)
        → chromiumoxide (CDP WebSocket 客户端)
            → Chrome/Chromium 浏览器进程
        → LlmClient (已有的 LLM 接口，新增 vision 支持)
        → Screenshot → base64 → LLM (替代 Midscene.js AI 视觉)
        → ActionCache (Rust 重写)
    → BfsExplorer (已有，继续使用 NativeAutomation)
    → External tools (直接 spawn 子进程)
        → npx @lhci/cli (Lighthouse)
        → locust (压力测试)
        → docker owasp/zap2docker-stable (安全扫描)
```

---

## 模块结构

### 新增/修改文件

```
crates/core/src/automation/
├── mod.rs              # + NativeAutomation 导出
├── trait_def.rs        # 不变
├── midscene.rs         # 保留（暂时），后续可删除
├── native.rs           # ★ 新增：NativeAutomation 实现
└── action_cache.rs     # ★ 新增：Rust 版的 AI 动作缓存

crates/core/src/
├── protocol.rs         # 移除：MidsceneProcess 及相关类型
├── llm.rs              # 修改：增强 vision 支持

crates/core/Cargo.toml  # +chromiumoxide, -tokio 部分特性
```

### `NativeAutomation` 结构

```rust
pub struct NativeAutomation {
    browser: Arc<Mutex<BrowserProcess>>,
    llm: Arc<dyn LlmClient>,
    action_cache: Arc<Mutex<ActionCache>>,
}

struct BrowserProcess {
    chrome: chromiumoxide::Browser,
    page: chromiumoxide::Page,
}
```

---

## 关键技术决策

### ADR-009-1: 使用 `chromiumoxide` 作为 CDP 客户端

- **选项 A: `chromiumoxide`** — 纯 Rust，异步，活跃维护，支持完整的 CDP 协议
- **选项 B: `headless_chrome`** — 较老，同步接口，社区维护不活跃
- **选项 C: 自定义 WebSocket CDP 连接** — 工作量大且易出错

**决策:** `chromiumoxide`。它提供了 `Browser::launch()`（管理 Chrome 进程）和完整的 CDP 域类型定义，AI 操作所需的 `Page.captureScreenshot`、`Input.dispatchMouseEvent`、`Runtime.evaluate` 均已支持。

### ADR-009-2: 用 Screenshot + LLM 替代 Midscene.js AI 视觉

Midscene.js 的核心能力是通过截图 + DOM 结构 + LLM 来实现视觉 AI。我们直接用 CDP 截图 + Rust 端 `LlmClient` 调用 LLM 来实现等价功能：

- **ai_query(prompt)** → CDP 截图(base64) → 构造 prompt（含截图 base64 + 指令） → `LlmClient.chat()` → 解析 JSON
- **ai_act(action)** → CDP 截图 → LLM 分析并返回点击坐标/输入文本 → CDP 执行鼠标/键盘事件
- **ai_assert(assertion)** → CDP 截图 → LLM 判断断言是否通过

### ADR-009-3: 动作缓存保留但简化

当前 TS 的 `ActionCache` 基于 DOM 快照哈希，过于复杂。简化版本：基于 URL + prompt 的 LRU 缓存。

### ADR-009-4: 外部工具仍以子进程方式运行

Lighthouse (`@lhci/cli`)、Locust、ZAP 是独立的 CLI 工具，重写为 Rust 版本不现实。通过 `tokio::process::Command` 直接调用，管理逻辑从 `sandbox/` 迁移到 Rust。

---

## 数据结构

```rust
/// CDP 浏览器进程封装
pub struct BrowserProcess {
    chrome: chromiumoxide::Browser,
    page: chromiumoxide::Page,
}

/// 动作缓存条目
struct CacheEntry {
    response: String,
    created_at: Instant,
}

/// 动作缓存（简化版）
pub struct ActionCache {
    entries: HashMap<String, (String, Instant)>,
    max_entries: usize,
    ttl: Duration,
}
```

---

## 数据流

### Explore 流程（关键路径）

```
用户输入 URL → 点击 Start Explore
    ↓
BfsExplorer::explore(url, depth)
    ↓
NativeAutomation::goto(url)
    ↓
chromiumoxide::Page::goto(url)
    ↓
NativeAutomation::screenshot()  +  NativeAutomation::ai_query(prompt)
    ↓
CDP Page.captureScreenshot  →  base64 → LlmClient.chat() → AiPageInfo JSON
    ↓
BfsExplorer 分析 clickableElements → NativeAutomation::ai_act("点击：xxx")
    ↓
CDP 截图 → LLM 分析坐标 → CDP Input.dispatchMouseEvent
    ↓
重复直到深度达到或无可点击元素
    ↓
返回 ExploreResult { pages, markdown }
```

### Test 执行流程

```
TestExecutor::run(test_case)
    ↓
解析 YAML → 获取 flow 步骤列表
    ↓
NativeAutomation::goto(target_url)
    ↓
逐条执行 flow
    ├── aiAct  → NativeAutomation::ai_act
    ├── aiAssert → NativeAutomation::ai_assert
    └── aiQuery → NativeAutomation::ai_query
    ↓
返回 TestResult { passed, duration, screenshot, error }
```
