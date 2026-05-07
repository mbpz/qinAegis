# QinAegis Phase 2 Design: AI-Driven Execution Engine

> Version: v0.1
> Date: 2026-04-24
> Status: Approved

---

## 1. Overview

Phase 2 implements the AI-driven test execution engine: project exploration, test case generation, and AI Critic review. This phase transforms the Phase 1 foundation (sandbox + LLM client) into a working automated testing pipeline.

**Key principles:**
- User-provided seed URLs (not automatic crawling)
- AI drafts → human review → Approved workflow
- AI Critic auto-triggered on every generation
- Fixed exploration depth (user-configurable per run)

---

## 2. Execution Layer: Node.js Subprocess + stdio JSON

**Decision:** Use Node.js subprocess with stdio JSON, NOT mlua embedding.

**Rationale:** Midscene.js requires Playwright and `@midscene/web`, which are npm packages. They cannot be called from Rust via mlua without extensive bindings work. stdio JSON is simple, debuggable, and battle-tested (used by LSP servers, tree-sitter, etc.).

**Architecture:**
```
┌─────────────────────────────────────────────┐
│          Rust Core Task Runner               │
│  (tokio::process::Command + async read/write)│
└──────────────────┬──────────────────────────┘
                   │ stdin/stdout JSON
                   ▼
┌─────────────────────────────────────────────┐
│   Node.js Midscene Process                  │
│   sandbox/src/executor.ts                   │
│   - aiAct() / aiQuery() / aiAssert()       │
│   - Playwright chromium.launch()           │
└──────────────────┬──────────────────────────┘
                   │ CDP WebSocket
                   ▼
┌─────────────────────────────────────────────┐
│   Playwright Browser (local, :9222)         │
└──────────────────┬──────────────────────────┘
                   │ HTTP (reqwest)
                   ▼
┌─────────────────────────────────────────────┐
│   MiniMax-VL API                           │
└─────────────────────────────────────────────┘
```

---

## 3. stdio JSON Protocol

### 3.1 Message Format

**Rust → Node (request):**
```json
{"jsonrpc": "2.0", "id": "1", "method": "aiQuery", "args": ["{title: string}, 分析页面标题"]}
{"jsonrpc": "2.0", "id": "2", "method": "aiAct", "args": ["点击登录按钮"]}
{"jsonrpc": "2.0", "id": "3", "method": "explore", "args": ["https://example.com", 3]}
{"jsonrpc": "2.0", "id": "4", "method": "aiAssert", "args": ["页面显示用户头像"]}
```

**Node → Rust (response):**
```json
{"jsonrpc": "2.0", "id": "1", "ok": true, "data": {"title": "Example Domain"}}
{"jsonrpc": "2.0", "id": "1", "ok": false, "error": "assertion failed: 未找到头像元素"}
{"jsonrpc": "2.0", "id": "3", "ok": true, "data": {"pages": [{"url": "...", "title": "...", "nav": [...], "forms": [...]}]}}
```

### 3.2 Protocol Methods

| Method | Args | Returns |
|---|---|---|
| `aiQuery<T>` | `[prompt: string]` | `T` (typed data extraction) |
| `aiAct` | `[action: string]` | `void` (throws on failure) |
| `aiAssert` | `[assertion: string]` | `void` (throws on failure) |
| `explore` | `[seedUrl: string, maxDepth: number]` | `PageInfo[]` |
| `goto` | `[url: string]` | `void` |
| `screenshot` | `[]` | `string` (base64 PNG) |

### 3.3 Process Lifecycle

- **Spawned once** on first `qinAegis explore` or `qinAegis run`
- **Long-lived**: kept alive between test cases (browser session reused)
- **Restart on error**: process auto-restarts on crash (max 3 retries)
- **Graceful shutdown**: Rust sends `{"method": "shutdown"}` on completion

---

## 4. Project Exploration Pipeline

### 4.1 Flow

```
用户输入种子URL列表 (e.g., ["https://app.com/login", "https://app.com/dashboard"])
       │
       ▼
Node.js explore(seedUrls, maxDepth=3)
  For each seed URL:
    → page.goto(url)
    → aiQuery<PageInfo> 提取结构 {title, nav, forms, features, links[]}
    → screenshot 保存到 ./exploration/{slug}/shots/{n}.png
    → BFS: for each link (up to maxDepth), repeat
       │
       ▼
规格书 Markdown → Notion Projects Page (body)
       │
       ▼
AI Critic 审核
  → 完整性评分 (1-10)
  → issues[] + suggestions[]
       │
       ▼
Notion Projects Database 更新:
  - spec_page: page ID
  - status: "SpecGenerated"
  - spec_md5: hash (detect changes)
```

### 4.2 PageInfo Schema

```typescript
interface PageInfo {
  url: string;
  title: string;
  primaryNav: string[];
  mainFeatures: string[];
  authRequired: boolean;
  techStack: string[];
  forms: FormInfo[];
  keyElements: string[];
  links: string[];
}

interface FormInfo {
  action: string;
  method: string;
  fields: string[];
}
```

### 4.3 Exploration Output (Markdown)

```markdown
# 项目规格书: MyApp

## 页面结构

### /login
- **标题**: 登录
- **导航**: [注册, 忘记密码]
- **功能**: 邮箱密码登录, OAuth 第三方登录
- **表单**: email (必填), password (必填), remember_me
- **检测技术栈**: React 18, Vite

### /dashboard
- **标题**: 控制台
- **导航**: [概览, 订单, 设置]
- **功能**: 数据概览, 最近订单列表
- **认证**: 需要登录
- **链接**: /orders, /settings, /profile
...
```

---

## 5. Test Case Generation

### 5.1 Flow

```
用户输入需求描述 (e.g., "用户可以通过邮箱密码登录")
       │
       ▼
LLM 生成测试用例 (GPT-4 / MiniMax VL)
  Prompt: 项目规格书 Markdown + 需求描述
  Output: YAML (Midscene format)
       │
       ▼
AI Critic 自动审核
  → score: 1-10
  → issues: string[]
  → suggestions: string[]
       │
       ▼
Notion TestCases Database:
  - yaml_script: YAML 内容
  - status: "Draft"
  - generated_by: "AI"
  - critic_score: N
  - critic_issues: [...]
  - critic_suggestions: [...]
       │
       ▼
人工审核 (Notion UI 或 TUI)
  → Approved: 可以执行
  → Rejected: 标记原因，人工重写或删除
```

### 5.2 Generation Prompt Template

```
你是一名资深 QA 工程师，熟悉 Midscene.js 的 YAML 测试格式。

项目规格书:
{project_spec_markdown}

需求描述:
{requirement_text}

请生成符合以下规范的测试用例列表（JSON 格式）:

[{
  "id": "TC-001",
  "name": "用例标题",
  "requirement_id": "REQ-001",
  "type": "smoke|functional|performance|stress",
  "priority": "P0|P1|P2",
  "yaml_script": "完整的 Midscene YAML 脚本",
  "expected_result": "期望结果",
  "tags": ["login", "auth"]
}]

规则:
1. P0 仅覆盖核心路径
2. yaml_script 使用 aiAct / aiAssert / aiQuery API
3. 不得使用 CSS selector 或 XPath
4. 每个用例必须有明确的 aiAssert 断言
```

### 5.3 YAML Test Case Format

```yaml
# TC-001: 用户登录
target:
  url: https://app.com/login

tasks:
  - name: 验证登录页显示
    flow:
      - aiAssert: 页面显示邮箱和密码输入框

  - name: 正常登录流程
    flow:
      - aiAct: 在邮箱输入框填入 test@example.com
      - aiAct: 在密码输入框填入 Test@123456
      - aiAct: 点击登录按钮
      - aiAssert: 页面跳转到 /dashboard，显示用户名
```

---

## 6. AI Critic Review

### 6.1 Review Logic

```typescript
async function aiCriticReview(testCase: TestCase, specMarkdown: string): Promise<ReviewResult> {
  const prompt = `
    审核以下测试用例，评估其完整性和可执行性：

    规格书上下文:
    ${specMarkdown}

    测试用例:
    ${testCase.yaml_script}

    需求:
    ${testCase.requirement_text}

    返回 JSON:
    {
      "score": 1-10,
      "issues": ["问题描述"],
      "suggestions": ["改进建议"],
      "coverage": "P0覆盖率评估"
    }
  `;

  const response = await llm.chat([{ role: "user", content: prompt }]);
  return JSON.parse(response);
}
```

### 6.2 Scoring Criteria

| Score | 含义 |
|---|---|
| 9-10 | 完整覆盖，断言充分，可直接执行 |
| 7-8 | 基本完整，有小问题需要修复 |
| 4-6 | 部分覆盖，需要补充关键场景 |
| 1-3 | 严重缺失，建议重写 |

---

## 7. Module Breakdown

### `crates/core/src/explorer.rs`
- `spawn_midscene_process()` — 启动 Node 子进程
- `explore_project(seed_urls, max_depth)` — 发送 explore 指令，解析 PageInfo[]
- `write_spec_to_notion(page_id, markdown)` — 规格书写入 Notion

### `crates/core/src/generator.rs`
- `generate_test_cases(requirement_text, spec_markdown)` — 调用 LLM 生成 YAML
- `write_cases_to_notion(cases[])` — 批量写入 TestCases Database

### `crates/core/src/critic.rs`
- `ai_review(test_case, spec_markdown)` — AI Critic 审核
- `apply_review_result(page_id, review_result)` — 写回 Notion

### `sandbox/src/executor.ts`
- JSON-RPC 2.0 server over stdio
- Methods: `aiQuery`, `aiAct`, `aiAssert`, `explore`, `goto`, `screenshot`
- Handles CDP connection lifecycle

### `sandbox/src/explorer.ts`
- `exploreProject(seedUrls, maxDepth)` — BFS exploration
- `extractPageInfo()` — aiQuery wrapper returning PageInfo
- Outputs Markdown 规格书 to stdout

---

## 8. Error Handling

| Scenario | Strategy |
|---|---|
| Node process crash | Auto-restart up to 3 times, then surface error |
| CDP connection lost | Restart steel-browser container, reconnect |
| MiniMax API failure | Retry 3× with exponential backoff, then fail test case |
| AI Critic fails | Log error, still save test case as Draft (human review needed) |
| Navigation timeout | aiAct/aiAssert with 30s default timeout, configurable |
| Invalid YAML generated | Log parsing error, mark test case as Draft with error note |

---

## 9. Notion Integration Details

### 9.1 Projects Database Updates

When exploration completes, update the project page:
- `status` → "SpecGenerated"
- `spec_page` → link to the created spec page
- `spec_md5` → MD5 of spec markdown (for change detection)
- `explored_at` → current timestamp

### 9.2 TestCases Database Fields

New fields used in Phase 2:

| Field | Type | Description |
|---|---|---|
| `yaml_script` | Code Block | Midscene YAML |
| `status` | Select | Draft / Approved / Rejected / Deprecated |
| `generated_by` | Select | AI / Human |
| `critic_score` | Number | 1-10 |
| `critic_issues` | Rich Text | AI Critic issues list |
| `critic_suggestions` | Rich Text | AI Critic suggestions |
| `requirement` | Relation | → Requirements |

---

## 10. Configuration

`~/.config/qinAegis/config.toml` additions for Phase 2:

```toml
[exploration]
max_depth = 3
max_pages_per_seed = 20
screenshot_dir = "~/.local/share/qinAegis/exploration"

[generation]
model = "MiniMax-VL-01"
max_tokens = 2048
temperature = 0.7

[critic]
enabled = true
model = "MiniMax-VL-01"
auto_apply = false
```

---

*Last updated: 2026-04-24*
