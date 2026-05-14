# qinAegis 项目上下文

## 项目概述

**qinAegis** 是 macOS 本地的 AI 自动化测试平台，基于视觉驱动技术为 Web 项目提供端到端测试能力。

## 核心价值

- **完全本地沙箱化**：Playwright CDP 模式管理独立浏览器进程
- **AI 视觉驱动**：MiniMax-VL、Qwen3-VL 理解页面，生成和执行测试，无需维护 CSS selector
- **本地存储**：所有数据在 `~/.qinAegis/`
- **Homebrew Cask 一键安装**：对标 `gh` / `lazygit` 安装体验

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| **GUI** | wry/tao | WebView 桌面客户端 |
| **前端** | React + TypeScript | Web UI |
| **核心** | Rust | 业务逻辑 |
| **浏览器沙箱** | Playwright | CDP 模式 |
| **AI 引擎** | Midscene.js | 视觉驱动 act/query/assert |
| **性能测试** | Lighthouse | Web Vitals |
| **压力测试** | Locust | 负载测试 |

---

## 核心模块

### 1. Explorer
AI 自动探索 Web 项目页面，生成 `spec.md`。

### 2. Generator
基于需求描述，调用 LLM 生成 Midscene YAML 测试用例。

### 3. Executor
执行 Midscene YAML 脚本，支持 smoke / functional / performance / stress 类型。

### 4. Reporter
生成 HTML 报告，存储到 `~/.qinAegis/projects/{project}/reports/`。

### 5. Quality Gate
汇总 E2E 通过率 + Lighthouse 分数 + Locust RPS，输出 pass/fail。

---

## 数据目录

```
~/.qinAegis/
├── config.toml
└── projects/
    └── {project}/
        ├── config.yaml
        ├── spec/
        ├── cases/   # draft/reviewed/approved/flaky/archived
        └── reports/
            └── {run-id}/
                ├── summary.json
                ├── report.html
                ├── lighthouse.json
                └── locust-summary.json
```

---

## 配置

`~/.config/qinAegis/config.toml`:

```toml
[llm]
base_url = "https://api.minimax.chat/v1"
model = "MiniMax-VL-01"
```

环境变量覆盖：`MIDSCENE_MODEL_API_KEY`, `MIDSCENE_MODEL_BASE_URL`, `MIDSCENE_MODEL_NAME`。

---

## 本地开发

```bash
# 前端
cd crates/web_client/ui && npm install && npm run build && cd ../..

# 二进制
cargo build --release -p qinAegis-web

# 运行
./target/release/qinAegis-web
```

---

## 文档

- [USER_GUIDE.md](../USER_GUIDE.md) - 用户手册
- [CI_INTEGRATION.md](../CI_INTEGRATION.md) - CI/CD 集成
- [qinAegis-platform-roadmap.md](../qinAegis-platform-roadmap.md) - 路线图