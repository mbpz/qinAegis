# AI 自动化测试平台 qinAegis Roadmap

> 基于开源成熟项目（Midscene.js · Playwright · Stagehand · Shortest · k6 · Lighthouse CI）的本地优先修正版本
> 产物形态：macOS 桌面 GUI 应用 · brew install --cask 一键安装到 /Applications · 完全本地沙箱 · 数据全量维护在本地文件系统

---

## 目录

0. [2026-05-13 技术路线修订 - PC GUI](#0-2026-05-13-技术路线修订---pc-gui)
1. [项目概述](#1-项目概述)
2. [开源项目选型依据](#2-开源项目选型依据)
3. [整体架构](#3-整体架构)
4. [技术选型](#4-技术选型)
5. [Phase 1 · Bootstrap — 配置 + 沙箱搭建](#5-phase-1--bootstrap--配置--沙箱搭建)
6. [Phase 2 · AI Core — 视觉驱动执行引擎](#6-phase-2--ai-core--视觉驱动执行引擎)
7. [Phase 3 · Test Execution — 四类测试](#7-phase-3--test-execution--四类测试)
8. [Phase 4 · 本地数据模型](#8-phase-4--本地数据模型)
9. [Phase 5 · Distribution — Homebrew Cask 分发](#9-phase-5--distribution--homebrew-cask-分发)
10. [原方案 vs 修正方案对比](#10-原方案-vs-修正方案对比)
11. [开发里程碑](#11-开发里程碑)
12. [目录结构](#12-目录结构)
13. [竞品分析与功能差距](#13-竞品分析与功能差距)

---

## 0. 2026-05-13 技术路线修订 - PC GUI

qinAegis 从 CLI/TUI 升级为 **PC 桌面 GUI 应用**：

- **产物形态变更**：从 CLI/TUI 工具改为 macOS 桌面应用（.app bundle）
- **安装方式变更**：`brew install --cask mbpz/qinAegis/qinAegis` 一键安装到 `/Applications`
- **用户体验变更**：双击即可使用图形化界面，无需终端操作
- **代码签名**：ad-hoc 自签名（免费），Gatekeeper 会显示”未验证开发者”警告但可运行

推荐运行时分层：

```text
qinAegis Desktop GUI (tao + wry + React)
  ├─ Project / Requirement / Case / Run / Gate Services
  ├─ Local FS Storage (~/.qinAegis/projects)
  └─ Sandbox Runtime
       ├─ Playwright: browser/session lifecycle, CDP, trace
       ├─ Midscene: visual act/assert/extract
       ├─ MCP-style Observer: accessibility snapshot and structured page state
       ├─ Lighthouse CI: performance budget
       └─ k6: load and stress thresholds
```

新的超越路径：

```text
双击启动 -> 配置 AI 模型 -> 探索项目 -> 生成用例 -> 执行测试 -> 查看报告 -> 质量门禁
```

---

## 1. 项目概述

### 产品定位

一款运行在 macOS 本地的 **桌面 GUI AI 质量工程平台**，专为前端 Web 项目设计。核心特性：

- **桌面 GUI 应用**：双击即可使用，无需终端操作

- **完全本地沙箱化**：测试执行在 Playwright 管理的浏览器进程内进行，与宿主机完全隔离
- **AI 驱动但可控**：结构化页面观测优先，视觉大模型处理复杂 UI，approved 用例尽量稳定复用
- **本地文件系统存储**：项目规格书、需求、测试用例、测试结果全部存储在 `~/.qinAegis/projects/`
- **测试资产治理**：draft / reviewed / approved / flaky / archived 生命周期
- **统一质量门禁**：E2E 通过率、性能预算、压测阈值统一输出 gate 结果
- **brew install --cask 一键安装**：对标成熟 macOS 桌面应用

### 用户使用流程

```
brew install --cask mbpz/qinAegis/qinAegis
↓
双击 QinAegis.app 启动 GUI
↓
设置 AI 模型凭证（Settings 页面）
↓
点击 Explore，输入项目 URL 进行 AI 发现
↓
点击 Generate，输入需求生成测试用例
↓
点击 Run，执行冒烟/功能/性能/压测
↓
点击 Reports，查看测试报告和质量门禁状态
```

---

## 2. 开源项目选型依据

调研了以下 GitHub 成熟项目后，对原方案进行了实质性修正：

### 核心依赖项目

| 项目 | 用途 | 选用原因 | qinAegis 吸收点 |
|------|-----------|------|---------|
| [web-infra-dev/midscene](https://github.com/web-infra-dev/midscene) | AI 视觉执行引擎 | 视觉定位、自然语言操作、视觉断言、内置 Report | 复杂 UI 的 visual act/assert/extract |
| [microsoft/playwright](https://github.com/microsoft/playwright) / Playwright MCP | 稳定自动化与结构化观测 | trace、console、network、accessibility snapshot、CI 生态成熟 | deterministic fallback、证据采集、MCP-style observer |
| [browserbase/stagehand](https://github.com/browserbase/stagehand) | AI 浏览器操作抽象 | act/extract/observe/agent 分层清晰 | 统一 `observe/act/extract/assert` 内部接口 |
| [antiwork/shortest](https://github.com/antiwork/shortest) | 自然语言 E2E 测试格式参考 | plain English 测试体验好 | 测试 DSL 参考，但增加本地治理和 review 状态机 |
| [browser-use/browser-use](https://github.com/browser-use/browser-use) | 通用 AI 浏览器 Agent | 多步网页任务编排能力强 | 参考 Agent loop，不作为 approved 回归执行默认模式 |
| [steel-dev/steel-browser](https://github.com/steel-dev/steel-browser) | 浏览器沙箱参考 | 开箱即用浏览器会话与 CDP 基础设施 | 已被 Playwright 原生方案取代 |
| [grafana/k6](https://github.com/grafana/k6) | 压力测试 | thresholds、scenarios、checks 成熟 | load gate |
| [GoogleChrome/lighthouse-ci](https://github.com/GoogleChrome/lighthouse-ci) | 性能持续检测 | 性能预算、断言、CI 友好 | performance gate |

### 关键修正：为什么放弃自研执行引擎

原方案计划自研 Generator + Evaluator 双 Agent，并手写 CDP 指令序列。调研后发现：

1. **底层动作不再自研**：Midscene、Stagehand、Playwright 已覆盖浏览器动作生成与执行。
2. **手写 CDP 指令维护成本极高**：`Page.navigate`、`Runtime.evaluate` 等原始 CDP 指令在页面结构变化后会失效。
3. **不能只依赖视觉模型**：视觉模型适合复杂 UI，但稳定回归应优先使用 accessibility snapshot、DOM、network、console 等结构化信号。
4. **平台价值在上层闭环**：测试资产治理、失败复盘、质量门禁、覆盖缺口和本地知识库是 qinAegis 的主要差异化。

---

## 3. 整体架构

### 架构演进

```
Phase 1: CLI (Rust CLI工具)      ──►  Phase 2: TUI (ratatui)  ──►  Phase 3: GUI (tao + wry + React WebView)
     │                                   │                              │
  终端命令                                终端UI                          桌面应用
  无界面                                   键盘操作                        鼠标点击
                                                                 Double-click即可
```

### 当前架构 (Phase 3 — WebView GUI)

<div style="width: 1200px; box-sizing: border-box; position: relative; background: #0f172a; padding: 20px; border-radius: 12px;">
  <style scoped>
    .arch-title { text-align: center; font-size: 20px; font-weight: bold; color: #f1f5f9; margin-bottom: 16px; letter-spacing: 1px; }
    .arch-subtitle { text-align: center; font-size: 12px; color: #94a3b8; margin-bottom: 14px; }
    .arch-wrapper { display: flex; gap: 12px; }.arch-sidebar { width: 170px; flex-shrink: 0; }.arch-main { flex: 1; min-width: 0; }
    .arch-layer { margin: 8px 0; padding: 14px; border-radius: 8px; }.arch-layer-title { font-size: 12px; font-weight: bold; margin-bottom: 10px; text-align: center; }
    .arch-grid { display: grid; gap: 8px; }.arch-grid-2 { grid-template-columns: repeat(2, 1fr); }.arch-grid-4 { grid-template-columns: repeat(4, 1fr); }
    .arch-box { border-radius: 6px; padding: 8px; text-align: center; font-size: 11px; font-weight: 600; line-height: 1.35; color: #e2e8f0; background: rgba(30, 41, 59, 0.8); border: 1px solid rgba(148, 163, 184, 0.2); }
    .arch-box.done { background: rgba(16, 185, 129, 0.15); border: 1px solid #10b981; color: #6ee7b7; }
    .arch-box.todo { background: rgba(245, 158, 11, 0.15); border: 1px solid #f59e0b; color: #fcd34d; }
    .arch-sidebar-panel { border-radius: 8px; padding: 10px; background: rgba(30, 41, 59, 0.6); border: 1px solid #334155; margin-bottom: 8px; }
    .arch-sidebar-title { font-size: 11px; font-weight: bold; text-align: center; color: #94a3b8; margin-bottom: 6px; }
    .arch-sidebar-item { font-size: 10px; text-align: center; color: #cbd5e1; background: rgba(15, 23, 42, 0.5); padding: 5px; border-radius: 4px; margin: 3px 0; border: 1px solid rgba(51, 65, 85, 0.5); }
    .arch-sidebar-item.done { background: rgba(16, 185, 129, 0.2); border: 1px solid rgba(16, 185, 129, 0.4); color: #6ee7b7; }
    .arch-sidebar-item.todo { background: rgba(245, 158, 11, 0.2); border: 1px solid rgba(245, 158, 11, 0.4); color: #fcd34d; }
    .arch-sidebar-item.metric { background: rgba(139, 92, 246, 0.2); border: 1px solid rgba(139, 92, 246, 0.4); color: #c4b5fd; }
  </style>
  <div class="arch-title">QinAegis PC GUI 架构</div>
  <div class="arch-subtitle">Phase 1: CLI → Phase 2: TUI → Phase 3: WebView GUI (当前)</div>
  <div class="arch-wrapper">
    <div class="arch-sidebar">
      <div class="arch-sidebar-panel"><div class="arch-sidebar-title">Sidebar</div>
        <div class="arch-sidebar-item done">🏠 Dashboard</div>
        <div class="arch-sidebar-item done">🔍 Explore</div>
        <div class="arch-sidebar-item done">✨ Generate</div>
        <div class="arch-sidebar-item done">▶️ Run Tests</div>
        <div class="arch-sidebar-item done">📊 Reports</div>
        <div class="arch-sidebar-item done">⚙️ Settings</div>
      </div>
      <div class="arch-sidebar-panel"><div class="arch-sidebar-title">Rust Backend</div>
        <div class="arch-sidebar-item done">AppState</div>
        <div class="arch-sidebar-item done">RPC Bridge</div>
        <div class="arch-sidebar-item done">Explorer</div>
        <div class="arch-sidebar-item done">TestExecutor</div>
        <div class="arch-sidebar-item done">LocalStorage</div>
      </div>
    </div>
    <div class="arch-main">
      <div class="arch-layer" style="background: rgba(14, 165, 233, 0.1); border: 1px solid #0ea5e9; box-shadow: 0 0 12px rgba(14, 165, 233, 0.15);">
        <div class="arch-layer-title" style="color: #7dd3fc;">User Interface — React WebView (tao + wry)</div>
        <div class="arch-grid arch-grid-4">
          <div class="arch-box done">Dashboard<br><small>Projects + Stats (wired) + Actions</small></div>
          <div class="arch-box done">ExploreView<br><small>URL + depth → runExplore</small></div>
          <div class="arch-box done">GenerateView<br><small>Requirement → cases</small></div>
          <div class="arch-box done">RunView<br><small>smoke/functional/perf/stress</small></div>
        </div>
        <div class="arch-grid arch-grid-2" style="margin-top: 8px;">
          <div class="arch-box done">ReportView<br><small>Recent Runs + Gate + Export + HTML viewer</small></div>
          <div class="arch-box done">SettingsView<br><small>LLM config + sandbox config</small></div>
        </div>
      </div>
      <div class="arch-layer" style="background: rgba(245, 158, 11, 0.1); border: 1px solid #f59e0b; box-shadow: 0 0 12px rgba(245, 158, 11, 0.15);">
        <div class="arch-layer-title" style="color: #fcd34d;">Rust Core Services (tokio async)</div>
        <div class="arch-grid arch-grid-4">
          <div class="arch-box done">Explorer<br><small>Midscene AI exploration</small></div>
          <div class="arch-box done">TestExecutor<br><small>Parallel test runner</small></div>
          <div class="arch-box done">TestCaseService<br><small>generate_and_save</small></div>
          <div class="arch-box done">Config management<br><small>save/load global</small></div>
        </div>
      </div>
      <div class="arch-layer" style="background: rgba(16, 185, 129, 0.1); border: 1px solid #10b981; box-shadow: 0 0 12px rgba(16, 185, 129, 0.15);">
        <div class="arch-layer-title" style="color: #6ee7b7;">AI + Sandbox Runtime</div>
        <div class="arch-grid arch-grid-4">
          <div class="arch-box done">Midscene<br><small>aiAct/aiQuery/aiAssert</small></div>
          <div class="arch-box done">Playwright<br><small>browser lifecycle + CDP</small></div>
          <div class="arch-box done">Lighthouse CI<br><small>Web Vitals</small></div>
          <div class="arch-box done">k6<br><small>stress test</small></div>
        </div>
      </div>
      <div class="arch-layer" style="background: rgba(236, 72, 153, 0.1); border: 1px solid #ec4899; box-shadow: 0 0 12px rgba(236, 72, 153, 0.15);">
        <div class="arch-layer-title" style="color: #f9a8d4;">Data — Local FS (~/.qinAegis/)</div>
        <div class="arch-grid arch-grid-4">
          <div class="arch-box done">projects/<br><small>config.yaml + spec.md</small></div>
          <div class="arch-box done">cases/draft|approved<br><small>JSON test cases</small></div>
          <div class="arch-box todo">runs/<run-id>/<br><small>report HTML (not wired)</small></div>
          <div class="arch-box done">reports/<br><small>summary.json</small></div>
        </div>
      </div>
    </div>
    <div class="arch-sidebar">
      <div class="arch-sidebar-panel"><div class="arch-sidebar-title">Gap Analysis</div>
        <div class="arch-sidebar-item done">Add Project UI</div>
        <div class="arch-sidebar-item done">Report wire-up</div>
        <div class="arch-sidebar-item done">Gate calculation (E2E done)</div>
        <div class="arch-sidebar-item done">Export UI (JSON download)</div>
        <div class="arch-sidebar-item done">Init wizard</div>
        <div class="arch-sidebar-item done">HTML report viewer</div>
        <div class="arch-sidebar-item done">Self-Healing (✅ done)</div>
      </div>
      <div class="arch-sidebar-panel"><div class="arch-sidebar-title">Features</div>
        <div class="arch-sidebar-item done">Ad-hoc signing</div>
        <div class="arch-sidebar-item done">DMG packaging</div>
        <div class="arch-sidebar-item done">Homebrew Cask</div>
        <div class="arch-sidebar-item done">Quarantine strip</div>
      </div>
      <div class="arch-sidebar-panel"><div class="arch-sidebar-title">Quality Gates</div>
        <div class="arch-sidebar-item metric">E2E Pass Rate</div>
        <div class="arch-sidebar-item metric">Performance</div>
        <div class="arch-sidebar-item metric">Stress</div>
      </div>
    </div>
  </div>
</div>

### 数据流向

```
用户指令 (WebView GUI)
    │
    ▼
项目理解 (accessibility snapshot + DOM/network/console + Midscene 视觉补强)
    │
    ▼
本地写入规格书 (projects/<name>/spec.md)
    │
    ▼
AI 生成测试用例 YAML/JSON → 本地 cases/draft/
    │
    ▼ (人工或 AI Critic 审核 → Approved)
    │
    ▼
沙箱执行 (Playwright + Midscene)
    │
    ├── 冒烟测试 → 结果 + Midscene Report
    ├── 功能测试 → 结果 + Midscene Report
    ├── 性能测试 → Lighthouse JSON
    └── 压力测试 → k6 Summary JSON
    │
    ▼
本地写入测试结果 (projects/<name>/runs/<run-id>/)
    │
    ▼
WebView GUI Dashboard 展示 / qinAegis gate / qinAegis export 导出报告
```

---

## 4. 技术选型

### 4.1 WebView GUI 客户端（自研）

| 技术 | 版本 | 用途 |
|------|------|------|
| Rust | stable | 主语言，编译为原生二进制 |
| tao | 0.17+ | 跨平台窗口管理 |
| wry | 0.20+ | WebView2/webkit 桥接 |
| tokio | 1.x | 异步运行时 |
| reqwest | 0.12+ | HTTP 客户端（LLM API） |
| serde / serde_json | 1.x | 序列化 |
| keyring | macOS Keychain 集成，存储 LLM API token |
| React | 18.x | 前端 UI 框架 |
| Vite | 5.x | 前端构建工具 |

### 4.2 浏览器沙箱（Playwright）

Playwright 原生提供浏览器进程管理，无需 Docker：

```yaml
# playwright.config.ts
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  timeout: 30_000,
  use: {
    trace: 'on-first-retry',  # trace, screenshot, console log
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
```

Playwright 提供的能力：
- 浏览器进程生命周期管理（启动 / 关闭 / 重用）
- CDP WebSocket 连接（`ws://localhost:9222`）用于远程控制
- Session 页面管理（新建 / 截图 / PDF / 打印）
- Trace viewer 录制与回放
- Console、network、accessibility snapshot 采集
- 反检测 + User-Agent 管理

### 4.3 AI 执行引擎（Playwright + Midscene）

执行层采用“结构化优先、视觉补强”的策略：

1. **MCP-style Observer**：优先采集 accessibility snapshot、DOM 摘要、可交互元素、console、network。
2. **Playwright**：负责稳定动作、trace、截图、console/network 证据采集和 fallback。
3. **Midscene.js**：负责复杂 UI 的视觉定位、视觉断言、视觉抽取。
4. **LLM/Vision Model**：只在 explore、生成、视觉断言、失败解释、自动修复建议等环节调用。

内部统一抽象：

```rust
trait BrowserAutomation {
    async fn observe(&self, instruction: &str) -> Result<Observation>;
    async fn act(&self, instruction: &str) -> Result<ActionResult>;
    async fn extract(&self, instruction: &str) -> Result<serde_json::Value>;
    async fn assert(&self, instruction: &str) -> Result<AssertionResult>;
}
```

Midscene.js 通过**视觉方式**定位和操作 UI 元素，不依赖 CSS selector 或 XPath，适合处理语义结构差、视觉状态复杂、传统 selector 不稳定的页面。

**环境变量配置（对接 MiniMax VL）：**

```bash
# MiniMax 视觉模型（推荐）
export MIDSCENE_MODEL_BASE_URL="https://api.minimax.chat/v1"
export MIDSCENE_MODEL_API_KEY="your-minimax-api-key"
export MIDSCENE_MODEL_NAME="MiniMax-VL-01"
export MIDSCENE_MODEL_FAMILY="openai"

# 或：Qwen3-VL（本地 Ollama / 阿里云）
export MIDSCENE_MODEL_BASE_URL="http://localhost:11434/v1"
export MIDSCENE_MODEL_API_KEY="ollama"
export MIDSCENE_MODEL_NAME="qwen3-vl:7b"
export MIDSCENE_MODEL_FAMILY="openai"

# 或：UI-TARS（本地，专为 UI 自动化训练）
export MIDSCENE_MODEL_BASE_URL="http://localhost:8080/v1"
export MIDSCENE_MODEL_API_KEY="local"
export MIDSCENE_MODEL_NAME="ui-tars-7b"
export MIDSCENE_MODEL_FAMILY="openai"
```

**核心 API：**

```typescript
import { PlaywrightAgent } from "@midscene/web/playwright";

const agent = new PlaywrightAgent(page);

// 自然语言操作（不需要 selector）
await agent.aiAct('在搜索框输入"测试关键词"，点击搜索按钮');

// 结构化数据提取
const items = await agent.aiQuery(
  "{title: string, price: number}[], 提取列表中所有商品的标题和价格"
);

// AI 视觉断言
await agent.aiAssert("页面右上角显示用户头像，说明已成功登录");

// 等待条件
await agent.aiWaitFor("加载动画消失，内容区域完全展示");
```

### 4.4 测试用例格式（本地 YAML/JSON）

测试用例存储在本地 `cases/` 目录中，不再写入 Notion。推荐分层：

```text
cases/
  draft/
  approved/
  flaky/
  archived/
```

测试用例以业务意图为中心，执行层再编译成 Midscene/Playwright 可运行计划：

```yaml
# TC-001: 用户登录功能
id: TC-001
status: draft
priority: P0
type: smoke
target:
  url: https://your-app.com/login

tasks:
  - name: 打开登录页
    flow:
      - aiAct: 确认页面显示登录表单，包含邮箱和密码输入框

  - name: 填写凭证
    flow:
      - aiAct: 在邮箱输入框填入 test@example.com
      - aiAct: 在密码输入框填入 password123
      - aiAct: 点击登录按钮

  - name: 验证登录成功
    flow:
      - aiAssert: 页面跳转到 Dashboard，顶部导航显示用户名
      - aiAssert: 不存在任何错误提示或弹窗
```

### 4.5 大模型选型建议（视觉能力对比）

| 模型 | 接入方式 | 视觉能力 | 国内可用 | 推荐场景 |
|------|---------|---------|---------|---------|
| MiniMax-VL-01 | API | ★★★★ | ✅ | 默认推荐，国内直连 |
| Qwen3-VL-7B | 本地 Ollama / 阿里云 | ★★★★ | ✅ | M4 Mac Mini 本地推理 |
| UI-TARS-7B | 本地 | ★★★★★ | ✅ | UI 自动化专用，精度最高 |
| Doubao-1.6-vision | 火山引擎 API | ★★★★ | ✅ | 字节系，Midscene 官方支持 |
| GPT-4o | OpenAI API | ★★★★★ | ❌ 需代理 | 参考基准 |

> ⚠️ **重要**：原方案的 `MiniMax abab6.5-chat` 是纯文本模型，无法处理截图，必须换成 `-VL` 后缀的视觉版本。

---

## 5. Phase 1 · Bootstrap — 配置 + 沙箱搭建

### 5.1 本地配置初始化

TUI 启动检测 `~/.config/qinAegis/config.toml`，若无则引导用户配置：

```
1. 检查 ~/.config/qinAegis/config.toml 是否存在
2. 若不存在，启动首次配置向导
3. 引导用户输入 AI 模型配置（Provider · Base URL · API Key · Model）
4. API Key 加密写入 macOS Keychain（keyring crate）
5. 配置写入 ~/.config/qinAegis/config.toml
6. TUI 显示 "配置完成，可添加第一个项目"
```

**config.toml 格式：**

```toml
[llm]
provider = "minimax"
base_url = "https://api.minimax.chat/v1"
model = "MiniMax-VL-01"
# api_key 存在 macOS Keychain，不写入文件

[sandbox]
playwright_browser = "chromium"  # chromium, firefox, webkit
headless = true

[exploration]
max_depth = 3
max_pages_per_seed = 20
```

### 5.2 项目本地存储初始化

`qinAegis project add` 在本地创建项目目录：

```
~/.qinAegis/projects/<project-name>/
├── config.yaml          # 项目配置（URL、技术栈）
├── spec.md              # explore 结果
├── requirements/        # 需求文档
│   └── <req-id>.md
├── cases/               # 测试用例 JSON
│   └── <case-id>.json
└── reports/             # 测试结果
    └── <run-id>/
        ├── summary.json
        └── <case-id>.html
```

### 5.3 沙箱启动

TUI 在首次运行 `qinAegis run` 时自动启动 Playwright 浏览器：

```rust
// src/sandbox/mod.rs
pub async fn ensure_sandbox_running() -> Result<SandboxHandle> {
    // 检查 Playwright 浏览器是否可用
    check_playwright_installed()?;

    // Playwright 管理浏览器进程生命周期
    let browser = launch_browser(BrowserType::Chromium, headless).await?;
    let cdp_ws = browser.ws_endpoint();

    Ok(SandboxHandle {
        playwright_endpoint: cdp_ws,
    })
}
```

### 5.4 MiniMax VL 配置 TUI 页面

TUI 提供配置向导（`qinAegis config`）：

```
┌─ AI 模型配置 ─────────────────────────────────────┐
│                                                    │
│  Provider:  [ MiniMax ▼ ]                          │
│  Base URL:  https://api.minimax.chat/v1            │
│  API Key:   sk-••••••••••••••••••••                │
│  Model:     MiniMax-VL-01                          │
│                                                    │
│  [ 测试连接 ]  [ 保存 ]                             │
└────────────────────────────────────────────────────┘
```

配置写入 `~/.config/qinAegis/config.toml`：

```toml
[llm]
provider = "minimax"
base_url = "https://api.minimax.chat/v1"
model = "MiniMax-VL-01"
# api_key 存在 macOS Keychain，不写入文件

[sandbox]
playwright_browser = "chromium"
headless = true
```

---

## 6. Phase 2 · AI Core — 视觉驱动执行引擎

### 6.1 项目熟悉流水线（① 探索阶段）

AI 拿到项目 URL 后，用 Midscene.js 驱动 Playwright 进行结构化探索：

```typescript
// sandbox/explorer.ts
import { PlaywrightAgent } from "@midscene/web/playwright";
import { chromium } from "playwright";

export async function exploreProject(projectUrl: string) {
  // 启动 Playwright 浏览器
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const agent = new PlaywrightAgent(page);

  await page.goto(projectUrl);

  // 1. 提取整体页面结构
  const pageStructure = await agent.aiQuery(`
    {
      title: string,
      primaryNavigation: string[],
      mainFeatures: string[],
      authRequired: boolean,
      techStack: string[]
    },
    分析当前页面，提取导航结构和主要功能模块
  `);

  // 2. 爬取所有可见路由
  const routes = await agent.aiQuery(
    "string[], 提取页面中所有内部链接的 href 路径，去重后返回"
  );

  // 3. 逐页截图 + 功能描述
  const pageDetails = [];
  for (const route of routes.slice(0, 20)) {  // 最多探索 20 个页面
    await page.goto(`${projectUrl}${route}`);
    const detail = await agent.aiQuery(`
      {
        path: string,
        purpose: string,
        keyElements: string[],
        forms: string[],
        interactions: string[]
      },
      描述当前页面的功能和关键交互元素
    `);
    pageDetails.push(detail);
  }

  // 4. 写入本地规格书
  await writeToLocalSpec({ pageStructure, routes, pageDetails });

  return { pageStructure, routes, pageDetails };
}
```

### 6.2 测试用例生成（② 生成阶段）

基于项目上下文和需求描述，调用 LLM 生成结构化测试用例：

**Prompt 模板：**

```
你是一名资深 QA 工程师，熟悉 Midscene.js 的 YAML 测试格式。

项目信息：
{project_spec_json}

需求描述：
{requirement_text}

请生成符合以下规范的测试用例列表（JSON 格式）：

[{
  "id": "TC-001",
  "name": "用例标题（简洁）",
  "requirement_id": "REQ-001",
  "type": "smoke|functional|performance|stress",
  "priority": "P0|P1|P2",
  "preconditions": ["前置条件1"],
  "yaml_script": "完整的 Midscene YAML 脚本字符串",
  "expected_result": "期望结果描述",
  "tags": ["login", "auth"]
}]

规则：
1. P0 用例仅覆盖核心路径（注册、登录、主要功能）
2. yaml_script 使用 aiAct / aiAssert / aiQuery API
3. 不得使用任何 CSS selector 或 XPath
4. 每个用例必须有明确的 aiAssert 断言
```

### 6.3 用例审核（③ Critic 阶段）

支持两种审核模式：

**人工审核**：本地用例状态流转

```
draft → reviewed → approved / flaky / archived
```

**AI Critic 审核**（自动化模式）：

```typescript
async function aiCriticReview(testCase: TestCase): Promise<ReviewResult> {
  const response = await llm.chat([{
    role: "user",
    content: `
      审核以下测试用例，评估其完整性、可执行性和覆盖度：
      ${JSON.stringify(testCase)}

      返回 JSON：{
        "approved": boolean,
        "score": 1-10,
        "issues": string[],
        "suggestions": string[]
      }
    `
  }]);

  return JSON.parse(response);
}
```

### 6.4 Midscene Report 集成

每次执行后生成可视化 HTML Report，保存到本地 `runs/<run-id>/`：

```typescript
// Midscene 自动生成 report
// 路径：./midscene_run/report/xxx.html

await localStorage.saveRunArtifact({
  project: "admin-web",
  runId: "20260507-103000",
  name: "midscene-report.html",
  sourcePath: "./midscene_run/report/latest.html"
});
```

Report 内容包含：
- 完整截图序列（每步操作前后）
- AI 决策链（定位推理过程）
- 内置 Playground（可回放和调试）
- 操作耗时统计

---

## 7. Phase 3 · Test Execution — 四类测试

### 7.1 冒烟测试（Smoke Test）

**目标**：快速验证核心路径，P0 用例全部通过。  
**触发时机**：每次部署后自动执行。  
**工具**：Midscene.js + Playwright（Playwright 管理的浏览器进程）

```typescript
// 执行流程
async function runSmokeTests(projectId: string) {
  // 1. 从本地加载 P0 已批准用例
  const cases = await localStorage.listCases(projectId, {
    type: "smoke",
    priority: "P0",
    status: "Approved"
  });

  // 2. 逐用例执行
  const results = [];
  for (const tc of cases) {
    const result = await executeMidsceneYaml(tc.yaml_script);
    results.push({
      case_id: tc.id,
      passed: result.success,
      duration_ms: result.duration,
      report_path: result.reportPath,
      screenshot_url: result.screenshotUrl,
      error_message: result.error
    });
  }

  // 3. 写入本地 reports/
  await localStorage.saveResults(results);

  // 4. TUI 展示摘要
  const passRate = results.filter(r => r.passed).length / results.length;
  return { passRate, results };
}
```

### 7.2 功能测试（Functional Test）

**目标**：需求全覆盖，包括正常流、边界值、异常流。  
**触发时机**：需求变更后、版本发布前。  
**工具**：Midscene.js + Playwright

功能测试用例示例（YAML）：

```yaml
# TC-021: 表单提交边界验证
target:
  url: https://your-app.com/register

tasks:
  - name: 空表单提交验证
    flow:
      - aiAct: 不填写任何内容，直接点击注册按钮
      - aiAssert: 页面显示必填字段错误提示，不跳转

  - name: 邮箱格式验证
    flow:
      - aiAct: 在邮箱输入框输入 "invalid-email"
      - aiAct: 点击注册按钮
      - aiAssert: 显示"邮箱格式不正确"的错误提示

  - name: 密码强度验证
    flow:
      - aiAct: 在密码框输入 "123"（弱密码）
      - aiAssert: 密码强度指示器显示"弱"

  - name: 正常注册流程
    flow:
      - aiAct: 填写有效邮箱 test_${timestamp}@example.com
      - aiAct: 填写密码 Test@123456
      - aiAct: 点击注册按钮
      - aiAssert: 页面跳转至欢迎页或邮箱验证提示页
```

### 7.3 性能测试（Performance Test）

**目标**：测量核心 Web Vitals 指标。  
**工具**：Lighthouse CI（通过 Playwright 运行）

```typescript
async function runPerformanceTest(url: string) {
  // 通过 Playwright 的 chromium 注入 lighthouse
  const { chromium } = require('playwright');
  const lighthouse = require('lighthouse');

  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();

  const report = await lighthouse(url, {
    port: new URL(browser.wsEndpoint()).port,
    output: 'json',
  });

  const metrics = {
    lcp: report.audits["largest-contentful-paint"].numericValue,  // ms
    fcp: report.audits["first-contentful-paint"].numericValue,    // ms
    tti: report.audits["interactive"].numericValue,                // ms
    cls: report.audits["cumulative-layout-shift"].numericValue,
    tbt: report.audits["total-blocking-time"].numericValue,       // ms
    performance_score: report.categories.performance.score * 100
  };

  // 写入本地 reports/
  await localStorage.savePerformanceResult(metrics);

  return metrics;
}
```

**性能基准参考（Google 标准）：**

| 指标 | 良好 | 需改进 | 差 |
|------|------|--------|-----|
| LCP | < 2.5s | 2.5s ~ 4s | > 4s |
| FCP | < 1.8s | 1.8s ~ 3s | > 3s |
| CLS | < 0.1 | 0.1 ~ 0.25 | > 0.25 |
| TBT | < 200ms | 200ms ~ 600ms | > 600ms |

### 7.4 压力测试（Stress Test）

**目标**：验证接口在高并发下的稳定性。  
**工具**：k6（直接运行，不需要 Docker）

**k6 脚本自动生成（AI 根据接口列表生成）：**

```javascript
// k6-script-generated.js（AI 根据 HAR 文件和接口文档自动生成）
import http from "k6/http";
import { check, sleep } from "k6";
import { Rate } from "k6/metrics";

const errorRate = new Rate("errors");

export const options = {
  stages: [
    { duration: "1m", target: 20 },   // 爬坡：1分钟到20并发
    { duration: "3m", target: 50 },   // 压力：3分钟保持50并发
    { duration: "1m", target: 0 },    // 降压
  ],
  thresholds: {
    http_req_duration: ["p(99)<2000"],  // 99% 请求 < 2s
    errors: ["rate<0.05"],              // 错误率 < 5%
  },
};

export default function () {
  // AI 根据项目接口自动填充
  const res = http.post(
    "https://your-app.com/api/login",
    JSON.stringify({ email: "test@test.com", password: "Test@123" }),
    { headers: { "Content-Type": "application/json" } }
  );

  check(res, {
    "status is 200": (r) => r.status === 200,
    "response time < 500ms": (r) => r.timings.duration < 500,
  });

  errorRate.add(res.status !== 200);
  sleep(1);
}
```

**执行命令：**

```bash
k6 run scripts/k6-script-generated.js --out json=results/k6-summary.json
```

**关键指标写入本地运行报告和质量知识库：**

| 指标 | 说明 |
|------|------|
| RPS | 每秒请求数（峰值） |
| P50 / P95 / P99 | 响应时间百分位 |
| Error Rate | 错误率 |
| Max VUs | 最大并发用户数 |
| Throughput | 总吞吐量 |

---

## 8. Phase 4 · 本地数据模型

### 8.1 目录结构

```
~/.qinAegis/
├── config.toml                    # 全局配置（AI 模型 · 沙箱端口）
└── projects/
    └── <project-name>/
         ├── config.yaml           # 项目配置（URL · 技术栈）
         ├── spec.md               # AI 探索生成的规格书
         ├── requirements/
         │   └── <req-id>.md       # 需求文档（Markdown）
         ├── cases/
         │   └── <case-id>.json    # 测试用例（JSON）
         └── reports/
              └── <run-id>/
                   ├── summary.json # 本次运行汇总
                   └── <case-id>.html # 每个用例的详细报告
```

### 8.2 数据模型

#### ProjectConfig (config.yaml)

```yaml
name: <project-name>
url: <target-url>
tech_stack: [react, vite]
created_at: <timestamp>
```

#### TestCase (cases/<id>.json)

```json
{
  "id": "<case-id>",
  "name": "<test-name>",
  "requirement_id": "<req-id>",
  "type": "smoke|full|perf|stress",
  "priority": "P0|P1|P2",
  "status": "Draft|Approved|Rejected",
  "yaml_script": "...",
  "expected_result": "...",
  "tags": ["login"],
  "created_by": "AI|Human",
  "reviewed_by": "AI-Critic|Human",
  "created_at": "<timestamp>"
}
```

#### TestResult (reports/<run-id>/summary.json)

```json
{
  "run_id": "<run-id>",
  "project": "<project-name>",
  "type": "<type>",
  "total": 10,
  "passed": 9,
  "failed": 1,
  "duration_ms": 12345,
  "run_at": "<timestamp>",
  "cases": [
    {
      "case_id": "<id>",
      "status": "Passed|Failed|Error",
      "duration_ms": 1234,
      "report_path": "<case-id>.html"
    }
  ]
}
```

### 8.3 通过率计算（本地）

```rust
// reports/<run-id>/summary.json 加载后直接计算
let pass_rate = (passed as f64 / total as f64) * 100.0;
```

---

## 9. Phase 5 · Distribution — Homebrew Cask 分发

### 9.1 Homebrew Cask

```ruby
# homebrew-tap/Formula/qinAegis.rb
cask "qinaegis" do
  version "0.1.0"
  arch = Hardware::CPU.arm? ? "arm64" : "x86_64"

  url "https://github.com/mbpz/qinAegis/releases/download/v#{version}/QinAegis-#{version}-mac-#{arch == "arm64" ? "arm64" : "x64"}.dmg"
  sha256 "REPLACE_WITH_ACTUAL_SHA256"

  name "QinAegis"
  desc "AI-powered automated testing platform for web projects"
  homepage "https://github.com/mbpz/qinAegis"

  artifact "QinAegis.app", target: "/Applications/QinAegis.app"

  post_install do
    system "xattr", "-r", "-d", "com.apple.quarantine", "#{staged_path}/QinAegis.app"
  end

  uninstall pkgutil: "com.qinaegis.app"

  caveats <<~EOS
    After installation, find QinAegis in your Applications folder.
    Double-click to launch the GUI application.
  EOS
end
```

### 9.2 安装命令

```bash
brew install --cask mbpz/qinAegis/qinAegis
```

### 9.3 GitHub Actions CI/CD

构建流程：
1. 安装 Node.js 20
2. 构建 React UI：`cd crates/web_client/ui && npm install && npm run build`
3. 编译 Rust：`cargo build --release --bin qinAegis-web`
4. 创建 .app bundle（Info.plist + codesign ad-hoc）
5. 打包 DMG：`hdiutil create`
6. 移除 quarantine：`xattr -r -d com.apple.quarantine`
7. 上传到 GitHub Release

详见：`.github/workflows/release.yml`

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ["v*"]

jobs:
  build-macos:
    runs-on: macos-latest
    strategy:
      matrix:
        target:
          - aarch64-apple-darwin
          - x86_64-apple-darwin

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Build
        run: |
          cargo build --release --target ${{ matrix.target }}
          tar -czf qinAegis-${{ matrix.target }}.tar.gz \
            -C target/${{ matrix.target }}/release qinAegis

      - name: Upload Release Asset
        uses: softprops/action-gh-release@v1
        with:
          files: qinAegis-${{ matrix.target }}.tar.gz

  update-homebrew:
    needs: build-macos
    runs-on: ubuntu-latest
    steps:
      - name: Update Homebrew Formula
        run: |
          # 计算 SHA256 并更新 tap 仓库
          ARM_SHA=$(sha256sum qinAegis-aarch64-apple-darwin.tar.gz | cut -d' ' -f1)
          X86_SHA=$(sha256sum qinAegis-x86_64-apple-darwin.tar.gz | cut -d' ' -f1)
          # 更新 homebrew-tap 仓库中的 formula
```

### 9.3 用户安装流程

```bash
# 添加 tap
brew tap yourorg/qinAegis

# 安装
brew install qinAegis

# 初始化（本地配置向导）
qinAegis init

# 添加项目
qinAegis project add --url https://your-app.com --name "My App"

# AI 探索项目
qinAegis explore

# 生成测试用例
qinAegis generate --requirement "用户可以通过邮箱密码登录"

# 执行测试
qinAegis run smoke
qinAegis run full
qinAegis run perf
qinAegis run stress

# 查看结果
qinAegis report

# 导出报告
qinAegis export --project My App --format html
```

---

## 10. 原方案 vs 修正方案对比

| 模块 | 原方案 | 修正方案 | 工作量变化 |
|------|--------|---------|-----------|
| **浏览器沙箱** | 自建 mitmproxy + CDP Bridge + Docker | Playwright 原生浏览器进程管理 | -75% |
| **AI 执行引擎** | 自研 Generator + Evaluator 双 Agent | Playwright + MCP-style Observer + Midscene 视觉补强 | 稳定性提升 |
| **页面操作方式** | 手写 CDP 指令（Page.navigate 等） | 结构化观测优先，必要时 aiAct()/视觉断言 | -90% |
| **大模型** | MiniMax abab6.5（纯文本 ❌） | MiniMax-VL / Qwen3-VL / UI-TARS（视觉 ✅） | 配置修改 |
| **测试用例格式** | JSON + cdp_hints 数组 | YAML 自然语言（Midscene 标准格式） | 简化 |
| **测试报告** | 自研截图上传系统 | Midscene/Playwright Evidence → 本地 runs/ | -80% |
| **性能测试** | Lighthouse（保留） | Lighthouse CI（保留 ✅） | 不变 |
| **压力测试** | k6（保留） | k6（保留 ✅） | 不变 |
| **数据存储** | Notion（云端 ❌） | 本地文件系统（本地 ✅） | 替换 |
| **TUI + 本地存储** | Rust ratatui（自研） | Rust ratatui（自研，保留 ✅） | 不变 |
| **Homebrew 分发** | GitHub Actions 打包（保留） | GitHub Actions 打包（保留 ✅） | 不变 |

**整体工作量节省约 40%，稳定性和可维护性大幅提升。**

---

## 11. 开发里程碑

> 更新日期：2026-05-08

### Week 1–2：基础骨架 ✅

- [x] Rust 项目初始化（cargo workspace）
- [x] ratatui 基础 TUI 框架搭建（导航 · 布局 · 输入）
- [x] 本地配置文件初始化（config.toml 读写）
- [x] 本地存储抽象（Storage trait + LocalStorageInstance）
- [x] qinAegis init 向导（AI 模型配置）

### Week 3–4：沙箱 + Midscene 集成 ✅

- [x] Playwright 浏览器进程管理（启动 · 健康检查 · 关闭）
- [x] Playwright trace、screenshot、console、network 证据采集
- [x] MCP-style Observer 输出 accessibility snapshot、DOM 摘要、console、network
- [x] Midscene.js 集成（aiAct / aiQuery / aiAssert 验证）
- [x] MiniMax VL 视觉模型对接测试
- [x] 本地 Qwen3-VL 备选模型测试（Ollama）

### Week 5–6：AI 探索 + 用例生成 ✅

- [x] 项目探索流水线（爬路由 · 截图 · 结构理解）
- [x] 规格书写入本地 spec/product.md、routes.json、ui-map.json
- [x] 测试用例生成（LLM Prompt 工程 · YAML 输出）
- [x] 测试用例写入本地 cases/draft/
- [x] AI Critic 自动审核逻辑
- [x] reviewed / approved / flaky / archived 状态流转

### Week 7–8：核心测试执行 ✅

- [x] YAML 用例从本地加载 + 解析
- [x] Midscene YAML 执行引擎封装
- [x] 冒烟测试流程端到端打通
- [x] 功能测试流程端到端打通
- [x] Midscene/Playwright evidence 写入本地 runs/

### Week 9–10：性能 + 压测 ✅

- [x] Lighthouse CI 集成（通过 Playwright）
- [x] 性能测试结果解析 + 写入本地 runs/
- [x] k6 压测脚本 AI 生成
- [x] k6 直接执行 + 结果解析
- [x] 压测结果写入本地 runs/
- [x] gate 阈值配置与 CI exit code

### Week 11–12：打磨 + 分发 ✅

- [x] WebView GUI（tao + wry）桌面应用
- [x] React 前端 UI 集成
- [x] 错误处理 · 重试机制 · 日志系统
- [x] Homebrew Cask 编写
- [x] GitHub Actions CI/CD（aarch64 + x86_64 双架构 DMG 打包）
- [x] README + 快速上手文档

---

## 13. 竞品分析与功能差距

> 更新日期：2026-05-13 · 基于 GitHub 开源项目调研

### 13.1 竞品对比矩阵

| 项目 | Stars | 核心AI能力 | 测试定义方式 | 报告形式 | 差异化优势 |
|------|-------|-----------|------------|---------|-----------|
| [browser-use/browser-use](https://github.com/browser-use/browser-use) | 93k | LLM原生浏览器Agent，无Playwright依赖 | 自然语言目标 | CLI + JSON | 通用网页任务编排，Claude/GPT/Gemini任意切换 |
| [stagehand](https://github.com/browserbase/stagehand) | 22k | SDK级浏览器Agent，LLM驱动DOM交互 | 自然语言指令 | CLI + trace viewer | 多语言SDK (Rust/Go/Ruby/PHP/Java/C#)，多浏览器支持 |
| [midscene](https://github.com/web-infra-dev/midscene) | 13k | 视觉AI执行（视觉act/assert/extract） | JavaScript/TS API | 内置HTML报告 | 视觉优先不依赖DOM，内置报告服务器 |
| [karate](https://github.com/karatelabs/karate) | 8.8k | API+UI测试自动化，BDD语法 | Gherkin-like DSL | HTML报告 + Jenkins | API测试一等公民，Mock服务器，压测 |
| [lost-pixel](https://github.com/lost-pixel/lost-pixel) | 1.7k | 视觉回归（Percy/Chromatic开源替代） | Code (TS) + config | Dashboard + CI | 开源自托管视觉回归 |
| [testsigma](https://github.com/testsigmahq/testsigma) | 1.2k | Agentic测试自动化，AI同事 | 自然语言（类英语） | Web dashboard | AI同事协同，Salesforce/SAP等企业集成 |
| [playwright-ai-qa-agent](https://github.com/nirarad/playwright-ai-qa-agent) | 5 | Playwright + Claude，AI自动修复断裂locator | Playwright tests | GitHub Issues + PR | **Self-Healing** broken locators |

### 13.2 qinAegis 差异化定位

**核心优势（竞品中稀缺）**

| 优势 | 说明 | 竞品对比 |
|------|------|---------|
| **桌面GUI** | 双击运行，无终端 | 99%竞品是CLI/SDK，browser-use/stagehand/karate全无GUI |
| **本地优先存储** | `~/.qinAegis/projects/` 完全本地化 | 竞品多为SaaS或CLI输出到云端 |
| **Homebrew分发** | `brew install --cask` 一键装到/Applications | 无直接竞品提供同等体验 |
| **测试资产生命周期** | draft/reviewed/approved/flaky/archived | 无竞品有完整状态机治理 |
| **四类测试合一** | 冒烟+功能+性能+压测，统一gate | karate/lost-pixel各偏重一类 |

**技术架构对比**

| 维度 | qinAegis | browser-use (93k) | stagehand (22k) | midscene (13k) |
|------|----------|-------------------|-----------------|----------------|
| **UI形态** | 桌面GUI (tao+wry) | CLI | SDK (多语言) | SDK + 报告服务器 |
| **浏览器依赖** | Playwright | 自研DOM Agent | Playwright/Puppeteer/Selenium | Playwright |
| **视觉模型** | Midscene (MiniMax/Qwen VL) | GPT-4o/Claude | 多LLM | 多LLM |
| **测试格式** | YAML/JSON (Midscene格式) | 自然语言目标 | 自然语言指令 | JS/TS API |
| **本地存储** | ✅ 本地FS | ❌ | ❌ | ❌ |
| **报告** | HTML + JSON summary | JSON | trace viewer | HTML内置报告 |
| **分发** | Homebrew Cask | pip/npm | 多语言包管理器 | npm |

### 13.3 功能差距与优先级

#### 🔴 高优先级 (差异化关键)

| 差距 | 竞品参考 | 说明 | 状态 |
|------|---------|------|------|
| **Self-Healing Locator** | playwright-ai-qa-agent | 用例跑失败时，AI自动修复断裂的step，保留original YAML不污染approved | ✅ 已实现 |
| **多LLM动态切换** | browser-use, stagehand | 同一次运行中根据页面复杂度动态选择MiniMax-VL/Qwen3-VL | 🔜 待实现 |
| **Action Caching** | stagehand | 相同动作+相同页面结构缓存AI决策结果，复用降低API成本 | ✅ 已实现 (30min TTL, LRU 500条) |
| **Natural Language Preview** | browser-use | 执行前用自然语言展示AI计划动作序列，用户确认后执行 | 🔜 待实现 |

#### 🟡 中优先级 (体验完善)

| 差距 | 竞品参考 | 说明 |
|------|---------|------|
| **视觉回归Dashboard** | lost-pixel, Percy | 完整web UI展示视觉diff，approve/reject视觉变更 |
| **Storybook集成** | Loki | 组件级别视觉测试，dev阶段发现问题 |
| **Accessibility测试** | Argos, Happo | 内置a11y检查，WCAG合规 |
| **多浏览器视觉对比** | reg-suit | 同时跑Chrome/Firefox/Safari，跨浏览器视觉回归 |
| **CI/CD原生集成** | lost-pixel | GitHub Actions first-class，PR评论中展示diff |

#### 🟢 低优先级 (生态扩展)

| 差距 | 竞品参考 | 说明 |
|------|---------|------|
| **企业集成** | Testsigma (Salesforce/SAP) | 预建连接器，适配企业应用 |
| **Record/Replay** | TestPilot变种 | 浏览器插件录制用户操作回放+AI增强 |
| **RAG知识库** | qa-agent-rag-platform | 测试资产长期知识积累 |
| **Contract Testing** | karate | API contract testing |

### 13.4 最值得深入研究的竞品

1. **[playwright-ai-qa-agent](https://github.com/nirarad/playwright-ai-qa-agent)** — Self-Healing直接解决用例维护痛点，架构最简单可集成
2. **[stagehand](https://github.com/browserbase/stagehand)** — Action Caching + 多LLM切换，可借鉴sandbox runtime增强
3. **[lost-pixel](https://github.com/lost-pixel/lost-pixel)** — 视觉回归Dashboard开源，可参考ReportView视觉diff UI
4. **[browser-use](https://github.com/browser-use/browser-use)** — 93k stars，LLM-native Agent架构参考，虽不适合直接集成但理念可借鉴

---

### Week 13+：PC端打磨 + 移动端规划 🏗️

#### PC端功能完善 (当前重点)
- [x] Self-Healing (case失败时LLM自动修复断裂step, 保留original YAML)
- [x] Action Caching (stagehand风格, 缓存AI actions降低API成本)
- [x] Natural Language Preview (执行前显示AI计划动作)
- [x] RunView 项目下拉选择 (从getProjects()填充)
- [x] ReportView 多项目选择 (切换不同项目报告)
- [x] ReviewView 项目选择 (从getProjects()填充)
- [x] Performance/Stress Gate 真实数据 (运行 performance/stress 测试后 lighthouse.json + locust-summary.json 自动写入 run dir)
- [x] ReviewView 案例审核UI (approve/reject/flaky 状态切换 + 状态过滤按钮)

#### 移动端扩展 (Phase 4 规划)

> **注**：CLI 已废弃，所有用户交互通过 PC 客户端（GUI）完成。移动端扩展为 Phase 4 规划。

- [ ] iOS测试 (WebDriverAgent + XCUITest)
- [ ] Android测试 (ADB + UIAutomator2)
- [ ] 跨平台统一报告视图
- [ ] Midscene Mobile SDK 集成

#### 集成扩展
- [x] 集成 OWASP ZAP 安全扫描（文档已完成，CI workflow 调用 zap-baseline.py）
- [x] 集成 Stagehand（文档已完成，Midscene 作为主引擎，Stagehand 作为备选视觉引擎）
- [x] 集成 Testplane 视觉回归（文档已完成，PC 客户端内置视觉测试能力，通过 Midscene 实现）
- [x] Playwright Test Agents 参考（文档已完成，npx playwright test AI 功能已内置）

---

## 12. 目录结构

```
qinAegis/
├── Cargo.toml                    # workspace 配置
├── Cargo.lock
│
├── crates/
│   ├── core/                     # 业务逻辑
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── explorer.rs      # 项目探索
│   │   │   ├── generator.rs     # 用例生成（LLM API）
│   │   │   ├── critic.rs        # AI Critic 审核
│   │   │   ├── executor.rs      # 测试执行
│   │   │   ├── reporter.rs      # 报告解析
│   │   │   ├── llm.rs           # LLM 客户端（MiniMax VL 等）
│   │   │   ├── storage/         # 本地存储抽象
│   │   │   │   ├── trait_def.rs # Storage trait
│   │   │   │   └── local.rs     # 本地文件系统实现
│   │   │   ├── prompts/         # Prompt 模板（i18n）
│   │   │   │   └── i18n.rs
│   │   │   ├── sandbox/         # 沙箱适配器
│   │   │   │   └── adapter.rs   # SandboxAdapter trait
│   │   │   ├── automation/      # 浏览器自动化
│   │   │   │   ├── trait_def.rs # BrowserAutomation trait
│   │   │   │   └── midscene.rs  # Midscene 实现
│   │   │   └── config/          # 配置管理
│   │   │       └── app.rs       # AppConfig
│   │   └── Cargo.toml
│   │
│   ├── web_client/               # 桌面 GUI（tao + wry + React）
│   │   ├── src/
│   │   │   ├── main.rs          # 入口 + RPC handler
│   │   │   └── assets.rs        # 嵌入 React 构建产物
│   │   ├── ui/                  # React 前端源码
│   │   │   ├── package.json     # Vite + React
│   │   │   ├── vite.config.ts
│   │   │   ├── tsconfig.json
│   │   │   └── src/
│   │   │       ├── main.tsx
│   │   │       ├── App.tsx
│   │   │       ├── styles.css
│   │   │       └── components/  # Sidebar, Dashboard, ExploreView...
│   │   └── Cargo.toml
│   │
│   └── sandbox/                  # Node.js 沙箱执行层
│       ├── package.json
│       ├── tsconfig.json
│       ├── src/
│       │   ├── explorer.ts      # Midscene 项目探索
│       │   ├── executor.ts      # Midscene YAML 执行
│       │   ├── lighthouse.ts    # Lighthouse 性能测试
│       │   └── k6-generator.ts  # k6 脚本 AI 生成
│       └── scripts/
│           └── k6-template.js   # k6 脚本模板
│
├── homebrew-tap/
│   └── Formula/
│       └── qinAegis.rb          # Homebrew Cask
│
└── .github/
    └── workflows/
        └── release.yml           # CI/CD 打包发布
```

---

## 附录：本地开发环境搭建

```bash
# 1. 克隆项目
git clone https://github.com/mbpz/qinAegis
cd qinAegis

# 2. 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add aarch64-apple-darwin

# 3. 安装 Node.js 20
# https://nodejs.org/

# 4. 安装 sandbox 依赖
cd crates/sandbox && pnpm install && cd ../..

# 5. 构建 React UI
cd crates/web_client/ui && npm install && npm run build && cd ../..

# 6. 安装 Playwright 浏览器
cd crates/sandbox && pnpm exec playwright install chromium && cd ..

# 7. 运行开发版桌面应用
cargo run -p web_client

# 8. 运行测试
cargo test
cd sandbox && pnpm test
```

---

*文档版本：v0.5（PC GUI 迁移版）*
*最后更新：2026-05-13*
