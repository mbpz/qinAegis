# QinAegis 用户手册

> AI 自动化测试 PC 客户端 — 基于 Playwright + Midscene.js 视觉驱动测试

## 目录

1. [安装](#安装)
2. [快速开始](#快速开始)
3. [视图参考](#视图参考)
4. [配置说明](#配置说明)
5. [数据目录](#数据目录)
6. [常见问题](#常见问题)

---

## 安装

### Homebrew 安装（推荐）

```bash
brew install --cask mbpz/qinAegis/qinAegis
```

安装完成后在 Applications 文件夹找到 **QinAegis.app**，双击运行。

### 从 DMG 安装

从 [GitHub Releases](https://github.com/mbpz/qinAegis/releases) 下载对应架构的 DMG 文件，挂载后拖入 Applications。

### 从源码构建

```bash
git clone https://github.com/mbpz/qinAegis.git
cd qinAegis

# Node.js UI
cd crates/web_client/ui && npm install && npm run build && cd ../..

# Rust 二进制
cargo build --release -p qinAegis-web
```

产物：`target/release/qinAegis-web`（直接运行，无安装流程）

---

## 快速开始

### 1. 启动应用

双击 QinAegis.app 或运行 `qinAegis-web`。

首次启动显示 **Settings** 视图，要求配置 LLM API。

### 2. 配置 LLM

在 Settings 视图填写：

| 字段 | 说明 | 示例 |
|------|------|------|
| API Key | MiniMax API Key | `eyJ...` |
| Base URL | API 端点 | `https://api.minimax.chat/v1` |
| Model | 模型名称 | `MiniMax-VL-01` |

配置保存后自动生效。

### 3. 创建项目

在 **Dashboard** 点击 `+ New Project`，填写：

- **Project Name**: 项目标识名（如 `my-app`）
- **URL**: 被测 Web 应用地址（如 `https://example.com`）
- **Tech Stack**: 技术栈（可选）

### 4. AI 探索项目

切换到 **Explore** 视图，输入 URL 和探索深度，点击 `Start Explore`。

AI 自动浏览页面，生成规格书（spec.md）存入项目目录。

### 5. 生成测试用例

切换到 **Generate** 视图，输入需求描述，点击 `Generate Cases`。

AI 根据规格书生成 YAML 测试用例，存入 `cases/draft/`。

### 6. 审核测试用例

切换到 **Review** 视图，查看 draft 状态用例，选择状态：`draft → reviewed → approved`（或 `flaky` / `archived`）。

### 7. 执行测试

切换到 **Run Tests** 视图：

1. 选择 **Project** 和 **Test Type**（smoke / functional / performance / stress）
2. 点击 **Preview Plan** 查看 AI 生成的测试计划摘要
3. 点击 **Run Tests** 执行

测试结果写入 `reports/{project}/{run-id}/`。

### 8. 查看报告

切换到 **Reports** 视图，查看历史运行记录和质量门禁（Gate）状态。

---

## 视图参考

### Dashboard

项目列表 + 新建项目入口。显示已配置项目的 URL 和测试统计。

### Explore

| 字段 | 说明 |
|------|------|
| URL | 被探索页面地址 |
| Depth | 递归探索深度（1-5） |

点击 `Start Explore` 后，AI 模拟用户浏览路径，生成 Markdown 规格书。

### Generate

| 字段 | 说明 |
|------|------|
| Requirement | 自然语言需求描述 |
| Spec | 规格书路径（自动读取 explore 结果） |

点击 `Generate Cases` 生成 YAML 测试用例，写入 draft 状态。

### Run Tests

| 字段 | 说明 |
|------|------|
| Project | 项目选择 |
| Test Type | smoke / functional / performance / stress |
| Preview Plan | 查看执行计划摘要后确认执行 |

**Test Type 说明**：
- `smoke` — 冒烟测试，核心路径验证
- `functional` — 功能测试，全量用例
- `performance` — Lighthouse 性能测试，结果写入 `lighthouse.json`
- `stress` — Locust 压力测试，结果写入 `locust-summary.json`

### Review

按状态过滤（draft / reviewed / approved / flaky / archived），支持单条状态切换。

### Reports

| 内容 | 说明 |
|------|------|
| Quality Gate | E2E 通过率 + Performance 分数 + Stress RPS |
| Recent Runs | 历史运行列表，点击查看 HTML 报告 |
| Export | 导出所有报告为 JSON |

### Settings

配置 LLM API Key、Base URL、Model。配置持久化到本地配置文件。

---

## 配置说明

### 配置文件

`~/.config/qinAegis/config.toml`（macOS）

```toml
[llm]
api_key = "your-api-key"
base_url = "https://api.minimax.chat/v1"
model = "MiniMax-VL-01"

[sandbox]
cdp_port = 9222
```

### 环境变量（覆盖配置）

| 变量 | 说明 |
|------|------|
| `MIDSCENE_MODEL_API_KEY` | LLM API Key |
| `MIDSCENE_MODEL_BASE_URL` | API Base URL |
| `MIDSCENE_MODEL_NAME` | 模型名称 |

### Sandbox 配置

默认使用 Playwright 内置 Chrome（CDP 模式），无需 Docker。

如需使用已有 Chrome 实例：设置 `CDP_WS_URL` 环境变量指向 Chrome DevTools WebSocket。

---

## 数据目录

所有数据存储在 `~/.qinAegis/`：

```
~/.qinAegis/
├── config.toml              # 全局配置
├── credentials.json         # 加密凭据（不提交）
└── projects/
    └── {project}/
        ├── config.yaml      # 项目配置
        ├── spec/            # 规格书
        ├── cases/
        │   ├── draft/       # 待审核用例
        │   ├── reviewed/    # 已审核用例
        │   ├── approved/    # 可执行用例
        │   ├── flaky/      # 不稳定用例
        │   └── archived/    # 已归档用例
        ├── reports/
        │   └── {run-id}/
        │       ├── summary.json       # 运行摘要
        │       ├── report.html        # HTML 报告
        │       ├── lighthouse.json   # 性能数据（仅 performance）
        │       └── locust-summary.json # 压测数据（仅 stress）
        └── knowledge/
            └── baseline.json # 性能基线
```

---

## 常见问题

### Q: API Key 无效报错

确认 Settings 中填写的 API Key 有效，且 Base URL 与 Key 类型匹配（MiniMax key 不能用于 OpenAI 兼容端点）。

### Q: Explore/Run 无法启动浏览器

确保 Playwright Chromium 已安装：

```bash
cd /Users/jinguo.zeng/dmall/project/qinAegis/sandbox
npx playwright install chromium
```

### Q: 报告为空

检查 `~/.qinAegis/projects/{project}/reports/` 是否有运行目录。performance/stress 测试需要在 Run Tests 视图中选择对应 Test Type 执行。

### Q: 性能/压测数据显示 `--`

performance 显示 `--` 需要运行 **performance** 测试类型；stress 显示 `--` 需要运行 **stress** 测试类型。smoke/functional 类型不生成这两项数据。

### Q: Self-Healing 未生效

Self-Healing 需要 LLM API Key 配置正确，且 `max_heal_retries > 0`（默认=1）。检查 Settings 配置。

---

## 版本信息

- 当前版本：0.5.4
- 构建时间：2026-05-13