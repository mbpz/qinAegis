# qinAegis 用户手册

> AI 自动化测试平台 — 基于视觉驱动的 Web 项目测试工具

## 目录

1. [安装](#安装)
2. [快速开始](#快速开始)
3. [命令参考](#命令参考)
4. [工作流程](#工作流程)
5. [配置说明](#配置说明)
6. [常见问题](#常见问题)

---

## 安装

### 前置要求

- macOS 12.0+
- **以下二选一**：
  - **方案A（推荐）**: Google Chrome 浏览器（用于无 Docker 沙箱）
  - **方案B**: Docker Desktop（用于 Docker 沙箱）

```bash
# 方案A: 安装 Chrome（如未安装）
brew install --cask google-chrome

# 方案B: 安装 Docker（如未安装）
brew install --cask docker
```

### 通过 Homebrew 安装（推荐）

```bash
brew tap yourorg/qinAegis
brew install qinAegis
```

### 从源码安装

```bash
git clone https://github.com/yourorg/qinAegis.git
cd qinAegis
cargo install --path crates/cli
```

---

## 快速开始

```bash
# 1. 初始化（OAuth2 授权 Notion）
qinAegis init

# 2. 添加项目
qinAegis init-db  # 初始化 Notion Database
qinAegis list-projects  # 查看项目列表

# 3. AI 探索项目
qinAegis explore --url https://your-app.com

# 4. 生成测试用例
qinAegis generate --requirement "用户可以通过邮箱密码登录"

# 5. 执行测试
qinAegis run --project "My App" --test-type smoke

# 6. 性能测试
qinAegis performance --url https://your-app.com

# 7. 压力测试
qinAegis stress --target https://your-app.com --users 100 --duration 60
```

---

## 命令参考

### `qinAegis init`

Notion OAuth2 授权登录。

```bash
qinAegis init
```

会打开浏览器进行 Notion 授权，授权后 token 会安全存储在 macOS Keychain 中。

**环境变量（可选）：**
- `NOTION_CLIENT_ID` — Notion OAuth Client ID
- `NOTION_CLIENT_SECRET` — Notion OAuth Client Secret

---

### `qinAegis init-db`

初始化 Notion Database（Projects、Requirements、TestCases、TestResults 四个 Database）。

```bash
qinAegis init-db
```

---

### `qinAegis list-projects`

列出 Notion 中的所有项目。

```bash
qinAegis list-projects
```

---

### `qinAegis explore`

AI 自动探索 Web 项目，生成规格书。

```bash
qinAegis explore --url https://your-app.com [--depth 3]
```

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--url` | 项目 URL（可多个） | 必填 |
| `--depth` | 探索深度 | 3 |

**输出：**
- 项目页面结构
- 主要功能模块
- 导航路由
- 规格书写入 Notion

---

### `qinAegis generate`

基于需求生成测试用例。

```bash
qinAegis generate --requirement "用户登录功能" [--spec ~/.local/share/qinAegis/exploration/spec.md]
```

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--requirement` | 需求描述 | 必填 |
| `--spec` | 规格书路径 | `~/.local/share/qinAegis/exploration/spec.md` |

---

### `qinAegis run`

执行测试（冒烟测试或功能测试）。

```bash
qinAegis run --project "My App" [--test-type smoke] [--concurrency 4]
```

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--project` | 项目名称 | 必填 |
| `--test-type` | 测试类型 | `smoke` |
| `--concurrency` | 并发数 | 4 |

**测试类型：**
- `smoke` — 冒烟测试（P0 核心用例）
- `functional` — 功能测试（全量用例）

---

### `qinAegis performance`

运行 Lighthouse 性能测试。

```bash
qinAegis performance --url https://your-app.com [--threshold 10]
```

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--url` | 测试 URL | 必填 |
| `--threshold` | 性能回归阈值（%） | 10 |

**输出指标：**
- Performance Score
- LCP (Largest Contentful Paint)
- FCP (First Contentful Paint)
- CLS (Cumulative Layout Shift)
- TBT (Total Blocking Time)

---

### `qinAegis stress`

运行 Locust 压力测试。

```bash
qinAegis stress --target https://your-app.com [--users 100] [--spawn-rate 10] [--duration 60]
```

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--target` | 目标 URL | 必填 |
| `--users` | 并发用户数 | 100 |
| `--spawn-rate` | 用户增长速率 | 10 |
| `--duration` | 持续时间（秒） | 60 |

**输出指标：**
- Total Requests
- Total Failures
- Avg Response Time
- P95 Response Time
- RPS (Requests Per Second)

---

### `qinAegis report`

查看测试报告（TODO）。

```bash
qinAegis report
```

---

### `qinAegis config`

配置管理（TODO）。

```bash
qinAegis config
```

---

## 工作流程

### 完整测试流程

```
1. qinAegis init
   ↓
2. qinAegis init-db
   ↓
3. qinAegis explore --url https://your-app.com
   ↓
4. qinAegis generate --requirement "需求描述"
   ↓
5. 人工/AI 审核测试用例（Notion 中状态 Draft → Approved）
   ↓
6. qinAegis run --project "My App" --test-type smoke
   ↓
7. 查看 Notion TestResults Database
```

### 性能/压力测试流程

```
qinAegis performance --url https://your-app.com
   ↓
qinAegis stress --target https://your-app.com --users 100 --duration 60
   ↓
结果写入 Notion PerformanceResults Database
```

---

## 配置说明

### 配置文件

`~/.config/qinAegis/config.toml`

```toml
[llm]
provider = "minimax"
base_url = "https://api.minimax.chat/v1"
model = "MiniMax-VL-01"
# api_key 存储在 macOS Keychain

[sandbox]
# Playwright 浏览器沙箱（无需 Docker）
cdp_port = 9222
```

### 环境变量

| 变量 | 说明 |
|------|------|
| `NOTION_CLIENT_ID` | Notion OAuth Client ID |
| `NOTION_CLIENT_SECRET` | Notion OAuth Client Secret |
| `MIDSCENE_MODEL_API_KEY` | LLM API Key |
| `MIDSCENE_MODEL_BASE_URL` | LLM API Base URL |

### AI 模型配置

**MiniMax VL（推荐）：**
```bash
export MIDSCENE_MODEL_BASE_URL="https://api.minimax.chat/v1"
export MIDSCENE_MODEL_API_KEY="your-api-key"
export MIDSCENE_MODEL_NAME="MiniMax-VL-01"
export MIDSCENE_MODEL_FAMILY="openai"
```

**Qwen3-VL（本地）：**
```bash
export MIDSCENE_MODEL_BASE_URL="http://localhost:11434/v1"
export MIDSCENE_MODEL_API_KEY="ollama"
export MIDSCENE_MODEL_NAME="qwen3-vl:7b"
export MIDSCENE_MODEL_FAMILY="openai"
```

---

## 常见问题

### Q: Playwright 浏览器未安装

如果浏览器未安装，qinAegis 会自动下载，或手动安装：

```bash
# 安装 Playwright Chromium
cd /Users/jinguo.zeng/dmall/project/qinAegis/sandbox
pnpm exec playwright install chromium

# 检查浏览器状态
qinAegis status
```

### Q: Notion 授权失败

1. 检查 `NOTION_CLIENT_ID` 和 `NOTION_CLIENT_SECRET` 环境变量
2. 确认 Notion Integration 已创建并启用 OAuth
3. 确认 redirect_uri 为 `http://localhost:54321/callback`

### Q: 沙箱启动失败

确保 Playwright 浏览器已安装：

```bash
# 手动安装 Playwright 浏览器
cd /Users/jinguo.zeng/dmall/project/qinAegis/sandbox
pnpm exec playwright install chromium

# 检查浏览器状态
qinAegis status
```

### Q: 性能测试超时

增加超时时间或降低复杂度：
```bash
qinAegis performance --url https://your-app.com --threshold 15
```

---

## 版本信息

- 当前版本：0.1.0
- 构建时间：2026-04-27
