# AI 自动化测试平台 qinAegis Roadmap

> 基于开源成熟项目（Midscene.js · steel-browser · Shortest · k6）的修正版本  
> 产物形态：macOS TUI 客户端 · brew 安装 · 完全本地沙箱 · 数据全量维护在 Notion

---

## 目录

1. [项目概述](#1-项目概述)
2. [开源项目选型依据](#2-开源项目选型依据)
3. [整体架构](#3-整体架构)
4. [技术选型](#4-技术选型)
5. [Phase 1 · Bootstrap — 认证 + 沙箱搭建](#5-phase-1--bootstrap--认证--沙箱搭建)
6. [Phase 2 · AI Core — 视觉驱动执行引擎](#6-phase-2--ai-core--视觉驱动执行引擎)
7. [Phase 3 · Test Execution — 四类测试](#7-phase-3--test-execution--四类测试)
8. [Phase 4 · Notion 数据模型](#8-phase-4--notion-数据模型)
9. [Phase 5 · Distribution — Homebrew 分发](#9-phase-5--distribution--homebrew-分发)
10. [原方案 vs 修正方案对比](#10-原方案-vs-修正方案对比)
11. [开发里程碑](#11-开发里程碑)
12. [目录结构](#12-目录结构)

---

## 1. 项目概述

### 产品定位

一款运行在 macOS 本地的 **TUI（Terminal UI）** 自动化测试工具，专为前端 Web 项目设计。核心特性：

- **完全本地沙箱化**：测试执行在 Docker 容器内进行，与宿主机完全隔离
- **AI 驱动**：视觉大模型理解页面、生成测试用例、执行断言，无需维护 CSS selector
- **Notion 作为数据中心**：项目规格书、需求、测试用例、测试结果全部维护在 Notion
- **brew 一键安装**：用户体验对标 gh / lazygit 等成熟 CLI 工具

### 用户使用流程

```
brew install qinAegis
↓
qinAegis init           # OAuth2 授权 Notion，初始化 workspace
↓
qinAegis project add    # 添加 Web 项目（URL + 技术栈）
↓
qinAegis explore        # AI 自动探索项目，生成规格书
↓
qinAegis generate       # 按需求维度生成测试用例 → 写入 Notion
↓
qinAegis run smoke      # 执行冒烟测试
qinAegis run full       # 执行完整功能测试
qinAegis run perf       # 执行性能测试
qinAegis run stress     # 执行压力测试
↓
qinAegis report         # 查看 Notion Dashboard 汇总
```

---

## 2. 开源项目选型依据

调研了以下 GitHub 成熟项目后，对原方案进行了实质性修正：

### 核心依赖项目

| 项目 | Stars 量级 | 用途 | 选用原因 |
|------|-----------|------|---------|
| [web-infra-dev/midscene](https://github.com/web-infra-dev/midscene) | 字节系 · 持续活跃 | AI 视觉执行引擎 | 纯视觉定位、兼容 OpenAI API 格式（可接 MiniMax VL）、内置 Report |
| [steel-dev/steel-browser](https://github.com/steel-dev/steel-browser) | 千星级 | 浏览器沙箱 | 开箱即用 CDP 沙箱，替代手工搭建 mitmproxy + Chrome |
| [antiwork/shortest](https://github.com/antiwork/shortest) | 千星级 | 自然语言测试格式参考 | 用例格式设计（自然语言 → Playwright）值得借鉴 |
| [browserbase/stagehand](https://github.com/browserbase/stagehand) | 千星级 | 备选 AI 浏览器操作 | act() / extract() / agent() CDP 封装，备用方案 |
| grafana/k6 | 万星级 | 压力测试 | 容器内运行，AI 自动生成脚本 |
| GoogleChrome/lighthouse | 万星级 | 性能测试 | LCP / FCP / TTI / CLS 指标 |

### 关键修正：为什么放弃自研执行引擎

原方案计划自研 Generator + Evaluator 双 Agent，并手写 CDP 指令序列。调研后发现：

1. **Midscene.js 已完整解决这个问题**：`aiAct()` 接受自然语言 → 自动生成 CDP 调用 → 执行 → 截图验证，完全覆盖 Generator/Evaluator 的职责
2. **手写 CDP 指令维护成本极高**：`Page.navigate`、`Runtime.evaluate` 等原始 CDP 指令在页面结构变化后会失效，Midscene 基于视觉截图定位，天然抗 DOM 变化
3. **视觉模型是前提**：原方案使用 MiniMax 纯文本模型无法理解截图，必须切换到视觉模型（MiniMax VL / Qwen3-VL / UI-TARS）

---

## 3. 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                   TUI 客户端 (Rust · ratatui)            │
│              brew install qinAegis · 自研               │
└───────────────────────┬─────────────────────────────────┘
                        │ OAuth2
                        ▼
┌─────────────────────────────────────────────────────────┐
│              Notion OAuth2 授权 + 数据层                 │
│   项目 → 需求 → 测试用例 → 测试结果 (四维度 Database)    │
└───────────────────────┬─────────────────────────────────┘
                        │
          ┌─────────────┴─────────────┐
          │                           │
          ▼                           ▼
┌──────────────────┐       ┌──────────────────────────────┐
│  AI 推理层        │       │  沙箱执行层 (Docker)          │
│                  │       │                              │
│  视觉大模型       │       │  steel-browser               │
│  MiniMax VL /   │◄─────►│  (CDP · Playwright · 会话管理)│
│  Qwen3-VL /     │       │                              │
│  UI-TARS        │       │  Midscene.js                 │
│                  │       │  (aiAct / aiQuery / aiAssert)│
│  Midscene SDK   │       │                              │
└──────────────────┘       │  k6 + Lighthouse CI          │
                           │  (性能 · 压测)               │
                           └──────────────────────────────┘
```

### 数据流向

```
用户指令 (TUI)
    │
    ▼
AI 理解项目 (Midscene aiQuery 爬取页面结构 + 截图)
    │
    ▼
Notion 写入规格书
    │
    ▼
AI 生成测试用例 YAML → Notion 测试用例 Database (Draft)
    │
    ▼ (人工或 AI Critic 审核 → Approved)
    │
    ▼
沙箱执行 (steel-browser + Midscene aiAct/aiAssert)
    │
    ├── 冒烟测试 → 结果 + Midscene Report
    ├── 功能测试 → 结果 + Midscene Report
    ├── 性能测试 → Lighthouse JSON
    └── 压力测试 → k6 Summary JSON
    │
    ▼
Notion 测试结果 Database 写入 (通过率 · 耗时 · Report 附件)
    │
    ▼
TUI Dashboard 展示 / Notion Dashboard 视图
```

---

## 4. 技术选型

### 4.1 TUI 客户端（自研）

| 技术 | 版本 | 用途 |
|------|------|------|
| Rust | stable | 主语言，编译为原生二进制 |
| ratatui | 0.27+ | TUI 框架，组件化布局 |
| tokio | 1.x | 异步运行时 |
| reqwest | 0.12+ | HTTP 客户端（Notion API · LLM API） |
| serde / serde_json | 1.x | 序列化 |
| keyring | macOS Keychain 集成，存储 token |
| crossterm | 终端跨平台控制 |

### 4.2 浏览器沙箱（steel-browser）

```yaml
# docker-compose.sandbox.yml
services:
  steel:
    image: ghcr.io/steel-dev/steel-browser:latest
    ports:
      - "3333:3333"    # Steel REST API
      - "9222:9222"    # Chrome CDP WebSocket
    environment:
      STEEL_API_KEY: "local-dev-key"
    volumes:
      - steel-data:/data
    networks: [sandbox]

networks:
  sandbox:
    driver: bridge
    internal: false   # 需要访问被测 Web 项目

volumes:
  steel-data:
```

Steel Browser 提供的能力：
- CDP WebSocket 连接 (`ws://localhost:9222`)
- Session 生命周期管理（创建 / 暂停 / 恢复 / 销毁）
- 内置截图 / PDF / Markdown 转换 API
- 反检测 + User-Agent 管理
- 请求日志 + HAR 录制

### 4.3 AI 执行引擎（Midscene.js）

Midscene.js 通过**纯视觉方式**定位和操作 UI 元素，不依赖 CSS selector 或 XPath。

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

### 4.4 测试用例格式（Midscene YAML）

测试用例存储在 Notion Code Block 中，格式为 Midscene YAML：

```yaml
# TC-001: 用户登录功能
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

## 5. Phase 1 · Bootstrap — 认证 + 沙箱搭建

### 5.1 OAuth2 Notion 登录

TUI 启动检测 `~/.config/qinAegis/config.toml`，若无有效 token 则触发授权：

```
1. 随机端口监听（如 :54321）
2. 打开浏览器 → https://api.notion.com/v1/oauth/authorize?...
3. 用户在 Notion 授权页确认
4. 回调 redirect_uri=http://localhost:54321/callback
5. 换取 access_token + workspace_id
6. 加密写入 macOS Keychain（keyring crate）
7. TUI 显示 "已连接 workspace: xxx"
```

**Notion OAuth2 请求参数：**

```
client_id=YOUR_NOTION_INTEGRATION_ID
redirect_uri=http://localhost:54321/callback
response_type=code
owner=user
```

### 5.2 Notion workspace 初始化

首次登录后，自动在 Notion 创建以下 Database 模板：

```
qinAegis/
├── 📋 Projects          ← 项目维度
├── 📄 Requirements      ← 需求维度（relation → Projects）
├── 🧪 TestCases         ← 测试用例（relation → Requirements）
└── 📊 TestResults       ← 测试结果（relation → TestCases）
```

每个 Database 的 Schema 定义见 [Phase 4](#8-phase-4--notion-数据模型)。

### 5.3 沙箱启动

TUI 在首次运行 `qinAegis run` 时自动拉起沙箱：

```rust
// src/sandbox/mod.rs
pub async fn ensure_sandbox_running() -> Result<SandboxHandle> {
    // 检查 Docker 是否在运行
    check_docker_available()?;

    // 检查 steel-browser 容器状态
    if !is_container_running("qinAegis-sandbox").await? {
        // docker compose -f ~/.config/qinAegis/docker-compose.yml up -d
        start_sandbox().await?;
        wait_for_steel_ready("http://localhost:3333/health").await?;
    }

    Ok(SandboxHandle {
        steel_api: "http://localhost:3333".to_string(),
        cdp_ws: "ws://localhost:9222".to_string(),
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
compose_file = "~/.config/qinAegis/docker-compose.yml"
steel_port = 3333
cdp_port = 9222
```

---

## 6. Phase 2 · AI Core — 视觉驱动执行引擎

### 6.1 项目熟悉流水线（① 探索阶段）

AI 拿到项目 URL 后，用 Midscene.js 驱动 steel-browser 进行结构化探索：

```typescript
// sandbox/explorer.ts
import { PlaywrightAgent } from "@midscene/web/playwright";
import { chromium } from "playwright";

export async function exploreProject(projectUrl: string) {
  // 连接 steel-browser CDP
  const browser = await chromium.connectOverCDP("ws://localhost:9222");
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

  // 4. 写入 Notion 规格书
  await writeToNotionProjectSpec({ pageStructure, routes, pageDetails });

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

**人工审核**：Notion 中 TestCases Database 状态流转

```
Draft → [人工审核] → Approved / Rejected
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

每次执行后生成可视化 HTML Report，上传至 Notion：

```typescript
// Midscene 自动生成 report
// 路径：./midscene_run/report/xxx.html

// 上传到 Notion 作为 Page 附件
await notionClient.pages.update({
  page_id: testResultPageId,
  properties: {
    report_url: {
      url: await uploadToNotionFile("./midscene_run/report/latest.html")
    }
  }
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
**工具**：Midscene.js + Playwright（运行在 steel-browser 沙箱内）

```typescript
// 执行流程
async function runSmokeTests(projectId: string) {
  // 1. 从 Notion 拉取 P0 已批准用例
  const cases = await notion.queryDatabase({
    database_id: TEST_CASES_DB_ID,
    filter: {
      and: [
        { property: "project_id", equals: projectId },
        { property: "type", equals: "smoke" },
        { property: "priority", equals: "P0" },
        { property: "status", equals: "Approved" }
      ]
    }
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

  // 3. 写入 Notion
  await writeTestResults(results);

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
**工具**：Lighthouse CI（运行在沙箱内）

```typescript
async function runPerformanceTest(url: string) {
  // 在沙箱容器内运行 lighthouse
  const result = await docker.exec(
    "qinAegis-sandbox",
    `lighthouse ${url} --output=json --chrome-flags="--headless" --output-path=/tmp/lh-report.json`
  );

  const report = JSON.parse(await docker.readFile("/tmp/lh-report.json"));

  const metrics = {
    lcp: report.audits["largest-contentful-paint"].numericValue,  // ms
    fcp: report.audits["first-contentful-paint"].numericValue,    // ms
    tti: report.audits["interactive"].numericValue,                // ms
    cls: report.audits["cumulative-layout-shift"].numericValue,
    tbt: report.audits["total-blocking-time"].numericValue,       // ms
    performance_score: report.categories.performance.score * 100
  };

  // 写入 Notion TestResults
  await writePerformanceResult(metrics);

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
**工具**：k6（运行在沙箱 Docker 容器内）

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
docker exec qinAegis-sandbox k6 run /scripts/k6-script-generated.js \
  --out json=/results/k6-summary.json
```

**关键指标写入 Notion：**

| 指标 | 说明 |
|------|------|
| RPS | 每秒请求数（峰值） |
| P50 / P95 / P99 | 响应时间百分位 |
| Error Rate | 错误率 |
| Max VUs | 最大并发用户数 |
| Throughput | 总吞吐量 |

---

## 8. Phase 4 · Notion 数据模型

### 8.1 四维度 Database 设计

#### Projects（项目）

| 字段 | 类型 | 说明 |
|------|------|------|
| name | Title | 项目名称 |
| url | URL | 项目访问地址 |
| tech_stack | Multi-select | React / Vue / Next.js 等 |
| status | Select | Active / Archived |
| spec_page | Relation | 关联规格书 Page |
| created_at | Date | 创建时间 |
| total_requirements | Rollup | 需求总数 |
| pass_rate | Formula | 整体通过率 |

#### Requirements（需求）

| 字段 | 类型 | 说明 |
|------|------|------|
| name | Title | 需求标题 |
| project | Relation | → Projects |
| description | Rich Text | 需求详细描述 |
| priority | Select | P0 / P1 / P2 |
| status | Select | Draft / Active / Done |
| test_case_count | Rollup | 关联用例数 |
| pass_rate | Formula | 需求维度通过率 |
| last_run_at | Date | 最近执行时间 |

#### TestCases（测试用例）

| 字段 | 类型 | 说明 |
|------|------|------|
| name | Title | 用例标题 |
| requirement | Relation | → Requirements |
| type | Select | smoke / functional / performance / stress |
| priority | Select | P0 / P1 / P2 |
| status | Select | Draft / Approved / Rejected / Deprecated |
| yaml_script | Code Block | Midscene YAML 脚本 |
| expected_result | Rich Text | 期望结果描述 |
| tags | Multi-select | 功能标签 |
| created_by | Select | AI / Human |
| reviewed_by | Select | AI-Critic / Human |

#### TestResults（测试结果）

| 字段 | 类型 | 说明 |
|------|------|------|
| name | Title | 自动生成（用例名 + 时间戳） |
| test_case | Relation | → TestCases |
| status | Select | Passed / Failed / Skipped / Error |
| duration_ms | Number | 执行耗时（毫秒） |
| run_at | Date | 执行时间 |
| environment | Select | Dev / Staging / Prod |
| report_url | URL | Midscene HTML Report 链接 |
| screenshot_url | Files | 关键截图附件 |
| error_message | Rich Text | 失败原因 |
| retry_count | Number | 重试次数 |
| metrics_json | Code | 性能/压测 JSON 数据 |

### 8.2 通过率计算（Notion Formula）

**Requirements.pass_rate：**

```
// Notion Formula
prop("test_case_count") > 0
? format(
    round(
      divide(
        length(filter(prop("test_results"), current.prop("status") == "Passed")),
        prop("test_case_count")
      ) * 100
    )
  ) + "%"
: "未执行"
```

### 8.3 Dashboard 视图配置

在 Notion 创建以下 Gallery / Table 视图：

**需求健康度视图（Requirements Database 分组）：**
- 按 `priority` 分组
- 显示：名称 · 通过率 · 最近执行时间 · 用例数
- 过滤：status = Active

**失败用例追踪视图（TestResults 筛选）：**
- 过滤：status = Failed
- 排序：run_at 降序
- 显示：用例名 · 失败原因 · Report 链接 · 重试次数

**项目趋势视图（TestResults 按日期归档）：**
- 按 run_at 日期分组
- 显示每日通过率趋势

### 8.4 Notion API 写入示例

```rust
// src/notion/writer.rs
use reqwest::Client;
use serde_json::json;

pub async fn write_test_result(
    client: &Client,
    notion_token: &str,
    result: &TestResult,
) -> Result<()> {
    let body = json!({
        "parent": { "database_id": TEST_RESULTS_DB_ID },
        "properties": {
            "name": {
                "title": [{ "text": { "content": &result.name } }]
            },
            "test_case": {
                "relation": [{ "id": &result.test_case_id }]
            },
            "status": {
                "select": { "name": &result.status }
            },
            "duration_ms": {
                "number": result.duration_ms
            },
            "run_at": {
                "date": { "start": &result.run_at.to_rfc3339() }
            },
            "report_url": {
                "url": &result.report_url
            },
            "error_message": {
                "rich_text": [{
                    "text": { "content": result.error_message.as_deref().unwrap_or("") }
                }]
            }
        }
    });

    client
        .post("https://api.notion.com/v1/pages")
        .bearer_auth(notion_token)
        .header("Notion-Version", "2022-06-28")
        .json(&body)
        .send()
        .await?;

    Ok(())
}
```

---

## 9. Phase 5 · Distribution — Homebrew 分发

### 9.1 Homebrew Formula

```ruby
# Formula/qinAegis.rb
class AiTester < Formula
  desc "AI-powered automated testing TUI for web projects"
  homepage "https://github.com/yourorg/qinAegis"
  version "0.1.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yourorg/qinAegis/releases/download/v0.1.0/qinAegis-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    else
      url "https://github.com/yourorg/qinAegis/releases/download/v0.1.0/qinAegis-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
  end

  depends_on :macos
  depends_on "docker" => :recommended

  def install
    bin.install "qinAegis"

    # 安装 sandbox docker-compose 模板
    (etc/"qinAegis").install "docker-compose.sandbox.yml"
  end

  def post_install
    (var/"log/qinAegis").mkpath
  end

  def caveats
    <<~EOS
      To get started:
        qinAegis init

      Docker is required for sandbox execution:
        brew install --cask docker

      For full documentation:
        https://github.com/yourorg/qinAegis
    EOS
  end

  test do
    system "#{bin}/qinAegis", "--version"
  end
end
```

### 9.2 GitHub Actions CI/CD

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
            -C target/${{ matrix.target }}/release qinAegis \
            docker-compose.sandbox.yml

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

# 初始化（会打开浏览器授权 Notion）
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
```

---

## 10. 原方案 vs 修正方案对比

| 模块 | 原方案 | 修正方案 | 工作量变化 |
|------|--------|---------|-----------|
| **浏览器沙箱** | 自建 mitmproxy + CDP Bridge + Docker | steel-browser（docker pull 开箱即用） | -75% |
| **AI 执行引擎** | 自研 Generator + Evaluator 双 Agent | Midscene.js（成熟生产级，MIT 开源） | -66% |
| **页面操作方式** | 手写 CDP 指令（Page.navigate 等） | aiAct() 自然语言 → 框架自动生成 CDP | -90% |
| **大模型** | MiniMax abab6.5（纯文本 ❌） | MiniMax-VL / Qwen3-VL / UI-TARS（视觉 ✅） | 配置修改 |
| **测试用例格式** | JSON + cdp_hints 数组 | YAML 自然语言（Midscene 标准格式） | 简化 |
| **测试报告** | 自研截图上传系统 | Midscene HTML Report → Notion 附件 | -80% |
| **性能测试** | Lighthouse（保留） | Lighthouse CI（保留 ✅） | 不变 |
| **压力测试** | k6（保留） | k6（保留 ✅） | 不变 |
| **TUI + Notion** | Rust ratatui（自研） | Rust ratatui（自研，保留 ✅） | 不变 |
| **Homebrew 分发** | GitHub Actions 打包（保留） | GitHub Actions 打包（保留 ✅） | 不变 |

**整体工作量节省约 40%，稳定性和可维护性大幅提升。**

---

## 11. 开发里程碑

### Week 1–2：基础骨架

- [ ] Rust 项目初始化（cargo workspace）
- [ ] ratatui 基础 TUI 框架搭建（导航 · 布局 · 输入）
- [ ] OAuth2 Notion 登录流程（本地 HTTP server + 浏览器唤起）
- [ ] Notion API 封装（创建 Database · 写入 Page）
- [ ] Notion workspace 初始化（四个 Database 自动创建）

### Week 3–4：沙箱 + Midscene 集成

- [ ] steel-browser Docker 容器管理（启动 · 健康检查 · 停止）
- [ ] Playwright 通过 CDP 连接 steel-browser
- [ ] Midscene.js 集成（aiAct / aiQuery / aiAssert 验证）
- [ ] MiniMax VL 视觉模型对接测试
- [ ] 本地 Qwen3-VL 备选模型测试（Ollama）

### Week 5–6：AI 探索 + 用例生成

- [ ] 项目探索流水线（爬路由 · 截图 · 结构理解）
- [ ] 规格书写入 Notion Projects Database
- [ ] 测试用例生成（LLM Prompt 工程 · YAML 输出）
- [ ] 测试用例写入 Notion TestCases Database
- [ ] AI Critic 自动审核逻辑

### Week 7–8：核心测试执行

- [ ] YAML 用例从 Notion 拉取 + 解析
- [ ] Midscene YAML 执行引擎封装
- [ ] 冒烟测试流程端到端打通
- [ ] 功能测试流程端到端打通
- [ ] Midscene HTML Report 解析 + 上传 Notion

### Week 9–10：性能 + 压测

- [ ] Lighthouse CI 容器内集成
- [ ] 性能测试结果解析 + 写入 Notion
- [ ] k6 压测脚本 AI 生成
- [ ] k6 容器内执行 + 结果解析
- [ ] 压测结果写入 Notion

### Week 11–12：打磨 + 分发

- [ ] TUI Dashboard（通过率 · 失败用例 · 历史趋势）
- [ ] 错误处理 · 重试机制 · 日志系统
- [ ] Homebrew Formula 编写
- [ ] GitHub Actions CI/CD（aarch64 + x86_64 双架构打包）
- [ ] README + 快速上手文档
- [ ] Beta 测试 + Bug 修复

---

## 12. 目录结构

```
qinAegis/
├── Cargo.toml                    # workspace 配置
├── Cargo.lock
│
├── crates/
│   ├── cli/                      # TUI 入口
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── tui/             # ratatui 组件
│   │   │   │   ├── app.rs
│   │   │   │   ├── dashboard.rs
│   │   │   │   ├── project_list.rs
│   │   │   │   └── config_form.rs
│   │   │   ├── commands/        # 命令处理
│   │   │   │   ├── init.rs      # OAuth2 登录
│   │   │   │   ├── explore.rs   # 项目探索
│   │   │   │   ├── generate.rs  # 用例生成
│   │   │   │   └── run.rs       # 测试执行
│   │   │   └── config.rs        # 配置读写
│   │   └── Cargo.toml
│   │
│   ├── sandbox/                  # 沙箱管理
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── steel.rs         # steel-browser API 封装
│   │   │   ├── docker.rs        # Docker 生命周期
│   │   │   └── health.rs        # 健康检查
│   │   └── Cargo.toml
│   │
│   ├── notion/                   # Notion API 封装
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── auth.rs          # OAuth2 流程
│   │   │   ├── database.rs      # Database CRUD
│   │   │   ├── writer.rs        # 结果写入
│   │   │   └── models.rs        # 数据结构
│   │   └── Cargo.toml
│   │
│   └── core/                     # 业务逻辑
│       ├── src/
│       │   ├── lib.rs
│       │   ├── explorer.rs      # 项目探索（调 Node.js 子进程）
│       │   ├── generator.rs     # 用例生成（LLM API）
│       │   ├── executor.rs      # 测试执行（调 Node.js 子进程）
│       │   ├── reporter.rs      # 报告解析 + 上传
│       │   └── llm.rs           # LLM 客户端（MiniMax VL 等）
│       └── Cargo.toml
│
├── sandbox/                      # Node.js 沙箱执行层
│   ├── package.json
│   ├── tsconfig.json
│   ├── src/
│   │   ├── explorer.ts          # Midscene 项目探索
│   │   ├── executor.ts          # Midscene YAML 执行
│   │   ├── lighthouse.ts        # Lighthouse 性能测试
│   │   └── k6-generator.ts      # k6 脚本 AI 生成
│   └── scripts/
│       └── k6-template.js       # k6 脚本模板
│
├── docker/
│   ├── docker-compose.sandbox.yml
│   └── Dockerfile.sandbox        # 定制沙箱镜像（含 k6 + Lighthouse）
│
├── Formula/
│   └── qinAegis.rb              # Homebrew Formula
│
└── .github/
    └── workflows/
        └── release.yml           # CI/CD 打包发布
```

---

## 附录：本地开发环境搭建

```bash
# 1. 克隆项目
git clone https://github.com/yourorg/qinAegis
cd qinAegis

# 2. 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add aarch64-apple-darwin

# 3. 安装 Node.js 依赖（sandbox 层）
cd sandbox && pnpm install && cd ..

# 4. 启动 steel-browser 沙箱
docker compose -f docker/docker-compose.sandbox.yml up -d

# 5. 配置环境变量（开发用）
cp .env.example .env
# 编辑 .env，填入 MINIMAX_VL_API_KEY 等

# 6. 运行开发版 TUI
cargo run -p cli

# 7. 运行测试
cargo test
cd sandbox && pnpm test
```

---

*文档版本：v0.2（基于开源项目调研修正）*  
*最后更新：2026-04*
