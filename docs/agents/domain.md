# qinAegis 项目上下文

## 项目概述

**qinAegis** 是一款 macOS 本地的 AI 自动化测试平台，基于视觉驱动技术为 Web 项目提供端到端的测试能力。

## 核心价值

- **完全本地沙箱化**：测试运行在 Docker 容器内，与宿主机隔离
- **AI 视觉驱动**：使用视觉大模型（MiniMax-VL、Qwen3-VL）理解页面，生成和执行测试，无需维护 CSS selector
- **本地存储**：所有数据存储在本地文件系统 `~/.qinAegis/`
- **brew 一键安装**：对标 `gh` / `lazygit` 等成熟 CLI 工具体验

## 技术栈

### 核心语言
- **Rust** (stable 2021 edition) - CLI 和核心业务逻辑
- **TypeScript/Node.js** - Midscene.js 沙箱执行层

### 依赖项目
| 项目 | 用途 |
|------|------|
| [web-infra-dev/midscene](https://github.com/web-infra-dev/midscene) | AI 视觉执行引擎（aiAct/aiQuery/aiAssert） |
| [steel-dev/steel-browser](https://github.com/steel-dev/steel-browser) | 浏览器沙箱（CDP） |
| [GoogleChrome/lighthouse](https://github.com/GoogleChrome/lighthouse) | 性能测试 |
| [grafana/k6](https://github.com/grafana/k6) | 压力测试 |
| [ratatui](https://github.com/ratatui/ratatui) | TUI 框架 |

### 关键版本
- Rust: stable 2021 edition
- ratatui: 0.27+
- tokio: 1.x
- reqwest: 0.12+

## 项目结构

```
qinAegis/
├── Cargo.toml                    # workspace 配置
├── crates/
│   ├── cli/                      # TUI 入口 + CLI 命令
│   ├── core/                     # 核心业务逻辑
│   │   └── src/
│   │       ├── config/           # 配置管理
│   │       ├── storage/          # 存储抽象
│   │       ├── automation/       # 浏览器自动化（Midscene）
│   │       ├── sandbox/          # 沙箱适配器
│   │       └── ...
│   └── sandbox/                  # Node.js 执行层（TypeScript）
├── docker/                       # Docker 配置文件
├── Formula/                      # Homebrew Formula
└── .github/workflows/            # CI/CD
```

## 核心模块

### 1. Explorer（探索模块）
- AI 自动探索 Web 项目页面结构
- 生成规格书（spec.md）
- 输出：PageInfo, FormInfo 等结构

### 2. Generator（生成模块）
- 基于需求描述生成测试用例
- 调用 LLM API 生成 Midscene YAML 脚本
- 输出：TestCase 结构

### 3. Critic（审核模块）
- AI 自动审核测试用例质量
- 输出：CriticReview（approved, score, issues）

### 4. Executor（执行模块）
- 解析和执行 Midscene YAML 测试脚本
- 支持冒烟测试、功能测试
- 输出：TestResult

### 5. Performance（性能模块）
- Lighthouse CI 集成
- Web Vitals 指标测量
- 性能回归检测

### 6. Stress（压测模块）
- Locust/k6 压力测试
- 并发用户模拟
- 输出：LocustStats

### 7. Reporter（报告模块）
- 生成 HTML 测试报告
- 本地文件系统存储
- Midscene Report 集成

## 命令行接口

```bash
qinAegis init              # 初始化配置（OAuth2）
qinAegis project add       # 添加项目
qinAegis explore           # AI 探索项目
qinAegis generate          # 生成测试用例
qinAegis run               # 执行测试
qinAegis performance      # 性能测试
qinAegis stress            # 压力测试
qinAegis report            # 查看报告
qinAegis tui               # 启动 TUI 界面
```

## 本地数据存储

```
~/.qinAegis/
├── config.toml                    # 全局配置
└── projects/
    └── <project-name>/
         ├── config.yaml           # 项目配置
         ├── spec.md               # 规格书
         ├── requirements/          # 需求文档
         ├── cases/                # 测试用例
         └── reports/              # 测试报告
```

## 配置说明

### 全局配置 (~/.config/qinAegis/config.toml)

```toml
[llm]
provider = "minimax"
base_url = "https://api.minimax.chat/v1"
model = "MiniMax-VL-01"
# api_key 存储在 macOS Keychain

[sandbox]
compose_file = "~/.config/qinAegis/docker-compose.sandbox.yml"
steel_port = 3333
cdp_port = 9222

[exploration]
max_depth = 3
max_pages_per_seed = 20
```

### 环境变量
- `NOTION_CLIENT_ID` / `NOTION_CLIENT_SECRET` - Notion OAuth
- `MIDSCENE_MODEL_API_KEY` - LLM API Key
- `MIDSCENE_MODEL_BASE_URL` - LLM API Base URL
- `MIDSCENE_MODEL_NAME` - 模型名称

## ADR（架构决策记录）

位于 `docs/adr/` 目录，记录关键架构决策：

- ADR-0001: 浏览器自动化方案选择（steel-browser vs 自研）
- ADR-0002: 存储抽象设计
- ADR-0003: 测试用例服务设计
- ADR-0004: LLM 客户端抽象
- ADR-0005: 沙箱适配器设计
- ADR-0006: 应用配置设计

## 开发相关

### 本地开发
```bash
cargo build --release
cargo test
cargo run -p cli -- help
```

### Sandbox 开发
```bash
cd sandbox
pnpm install
pnpm test
```

### 发布
- Homebrew: `brew install yourorg/qinAegis/qinAegis`
- GitHub Releases: `.github/workflows/release.yml`

## 相关文档

- [USER_GUIDE.md](../USER_GUIDE.md) - 详细用户手册
- [INSTALL.md](../INSTALL.md) - 安装指南
- [qinAegis-platform-roadmap.md](../qinAegis-platform-roadmap.md) - 完整架构文档