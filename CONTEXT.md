# qinAegis 项目上下文

> 本地优先的 AI 质量工程平台 — 面向 Web 项目的测试资产、沙箱执行、失败复盘和质量门禁工作台

---

## 项目概述

qinAegis 是一款运行在 macOS 本地的 **TUI/CLI AI 质量工程平台**，专为前端 Web 项目设计。

**核心特性：**
- 完全本地沙箱化：测试执行在 Docker/Steel Browser 容器内进行
- AI 驱动但可控：结构化页面观测优先，视觉模型处理复杂 UI
- 本地文件系统存储：项目规格书、需求、测试用例、运行结果、质量知识库全在本地
- 测试资产治理：draft / reviewed / approved / flaky / archived
- 失败复盘：截图、trace、console、network、模型摘要统一沉淀
- 质量门禁：E2E、性能、压测统一输出 gate 结果
- brew 一键安装

---

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| **CLI/TUI** | Rust + ratatui | TUI 客户端，跨平台终端界面 |
| **核心逻辑** | Rust + tokio | 异步运行时，业务逻辑处理 |
| **浏览器沙箱** | steel-browser (Docker) | 浏览器会话、页面、CDP 生命周期 |
| **稳定执行** | Playwright | 确定性动作、trace、console、network |
| **结构化观测** | MCP-style observer | accessibility snapshot、DOM、页面状态 |
| **AI 执行引擎** | Midscene.js | 视觉驱动的动作、断言、抽取 |
| **性能测试** | Lighthouse CI | Web Vitals 指标 |
| **压力测试** | k6 | 负载测试 |
| **包管理** | Homebrew | macOS 一键安装 |

---

## 目录结构

```
qinAegis/
├── Cargo.toml              # Workspace 配置
├── Cargo.lock              # 依赖锁定
├── LICENSE                 # MIT License
├── INSTALL.md              # 安装指南
├── README.md               # 项目介绍
├── USER_GUIDE.md           # 用户手册
├── CONTEXT.md              # AI Agent 上下文（此文件）
│
├── crates/
│   ├── cli/                # TUI 入口 + 命令行
│   │   ├── src/
│   │   │   ├── main.rs     # CLI 入口
│   │   │   ├── tui/        # ratatui 组件
│   │   │   └── commands/   # 命令处理 (init/explore/generate/run/...)
│   │
│   ├── core/               # 核心业务逻辑
│   │   └── src/
│   │       ├── explorer.rs    # 项目探索
│   │       ├── generator.rs   # 测试用例生成
│   │       ├── executor.rs    # 测试执行
│   │       ├── reporter.rs    # 报告解析
│   │       ├── critic.rs      # AI Critic 审核
│   │       ├── llm.rs        # LLM 客户端
│   │       ├── storage/      # 本地存储抽象
│   │       ├── automation/   # 浏览器自动化
│   │       └── sandbox/      # 沙箱适配器
│   │
│   └── sandbox/            # Node.js 沙箱执行层
│       ├── package.json
│       └── src/            # TypeScript 代码
│
├── docker/
│   └── docker-compose.sandbox.yml  # 沙箱配置
│
├── Formula/
│   └── qinAegis.rb         # Homebrew Formula
│
└── .github/
    └── workflows/
        └── release.yml     # CI/CD 发布流程
```

---

## 关键模块

### crates/cli/src/main.rs

CLI 命令入口，使用 clap 解析命令行参数。

**主要命令：**
- `qinAegis tui` — 启动交互式 TUI
- `qinAegis init` — 初始化配置
- `qinAegis explore` — AI 探索项目
- `qinAegis generate` — 生成测试用例
- `qinAegis run` — 执行测试
- `qinAegis performance` — 性能测试
- `qinAegis stress` — 压力测试

### crates/core/src/explorer.rs

项目探索模块，使用 Midscene.js 爬取页面结构。

### crates/core/src/generator.rs

测试用例生成模块，调用 LLM 生成 Midscene YAML 格式用例。

### crates/core/src/executor.rs

测试执行模块，运行 Midscene YAML 脚本并收集结果。

### crates/core/src/storage/

本地存储抽象层。

- `trait_def.rs` — Storage trait 定义
- `local.rs` — 本地文件系统实现

### crates/core/src/automation/

浏览器自动化模块。

- `trait_def.rs` — BrowserAutomation trait
- `midscene.rs` — Midscene.js 实现

---

## 命令参考

```bash
# 初始化
qinAegis init                    # 初始化本地配置
qinAegis setup                   # 重新配置

# 项目管理
qinAegis project add --name "MyApp" --url https://myapp.com
qinAegis project list
qinAegis project remove --name "MyApp"

# 测试流程
qinAegis explore --project "MyApp"     # AI 探索
qinAegis generate --project "MyApp" --requirement "用户登录"
qinAegis run --project "MyApp"         # 执行冒烟测试
qinAegis run --project "MyApp" --test-type functional
qinAegis gate --project "MyApp"        # 质量门禁

# 性能测试
qinAegis performance --url https://myapp.com
qinAegis stress --target https://myapp.com --users 100

# 其他
qinAegis config              # 查看配置
qinAegis report              # 查看报告
```

---

## 配置

配置文件位置：`~/.config/qinAegis/config.toml`

```toml
[llm]
provider = "minimax"
base_url = "https://api.minimax.chat/v1"
model = "MiniMax-VL-01"

[sandbox]
steel_port = 3333
cdp_port = 9222
```

---

## AI 模型配置

```bash
# MiniMax VL（推荐）
export MIDSCENE_MODEL_BASE_URL="https://api.minimax.chat/v1"
export MIDSCENE_MODEL_API_KEY="your-api-key"
export MIDSCENE_MODEL_NAME="MiniMax-VL-01"
export MIDSCENE_MODEL_FAMILY="openai"

# Qwen3-VL（本地）
export MIDSCENE_MODEL_BASE_URL="http://localhost:11434/v1"
export MIDSCENE_MODEL_API_KEY="ollama"
export MIDSCENE_MODEL_NAME="qwen3-vl:7b"
```

---

## 本地开发

```bash
# 克隆项目
git clone https://github.com/yourorg/qinAegis.git
cd qinAegis

# 启动沙箱
docker compose -f docker/docker-compose.sandbox.yml up -d

# 运行开发版
cargo run -p qinAegis

# 运行测试
cargo test
```

---

## 版本

当前版本：**v0.1.0** (2026-05)

---

_Last Updated: 2026-05-07_
