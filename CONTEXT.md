# qinAegis 项目上下文

> PC 桌面客户端 — wry/tao WebView GUI，基于 Playwright + Midscene.js 视觉驱动测试

---

## 项目概述

qinAegis 是运行在 macOS 本地的 **AI 质量工程平台**，面向 Web 项目。

**核心特性：**
- 浏览器沙箱隔离：Playwright 管理独立浏览器进程
- AI 驱动：结构化页面观测 + 视觉模型
- 本地存储：项目规格书、测试用例、运行结果全在 `~/.qinAegis/`
- 测试资产治理：draft / reviewed / approved / flaky / archived
- 质量门禁：E2E + Performance + Stress 统一 gate

---

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| **GUI** | Rust + wry/tao | WebView 桌面客户端 |
| **前端** | React + TypeScript | Web UI |
| **核心逻辑** | Rust | 业务逻辑 |
| **浏览器沙箱** | Playwright | CDP 模式浏览器 |
| **AI 执行引擎** | Midscene.js | 视觉驱动动作/断言/抽取 |
| **性能测试** | Lighthouse | Web Vitals 指标 |
| **压力测试** | Locust | 负载测试 |
| **包管理** | Homebrew Cask | macOS 一键安装 |

---

## 目录结构

```
qinAegis/
├── Cargo.toml
├── crates/
│   ├── core/               # 核心业务逻辑
│   │   └── src/
│   │       ├── explorer.rs    # 项目探索
│   │       ├── generator.rs   # 测试用例生成
│   │       ├── executor.rs    # 测试执行
│   │       ├── reporter.rs    # 报告解析
│   │       ├── llm.rs         # LLM 客户端
│   │       └── storage/       # 本地存储
│   │
│   ├── sandbox/            # Node.js 沙箱执行层
│   │   ├── package.json
│   │   └── src/
│   │
│   └── web_client/         # PC 桌面客户端（GUI）
│       ├── src/main.rs     # wry/tao WebView 入口
│       └── ui/             # React 前端
│
├── homebrew/
│   └── Formula/qinaegis.rb  # Homebrew Cask
│
└── .github/
    └── workflows/
        ├── release.yml          # Release + DMG 打包
        └── publish-homebrew-tap.yml  # Homebrew 发布
```

---

## 关键模块

### crates/web_client/src/main.rs

PC 客户端入口，wry/tao WebView 框架。

### crates/core/src/explorer.rs

项目探索，Midscene.js 爬取页面结构，生成 `spec.md`。

### crates/core/src/generator.rs

测试用例生成，调用 LLM 生成 Midscene YAML 格式用例。

### crates/core/src/executor.rs

测试执行，运行 Midscene YAML 脚本并收集结果。

---

## 数据目录

```
~/.qinAegis/
├── config.toml              # 全局配置
└── projects/
    └── {project}/
        ├── config.yaml      # 项目配置
        ├── spec/            # 规格书
        ├── cases/           # 测试用例（draft/reviewed/approved/flaky/archived）
        └── reports/         # 运行报告
            └── {run-id}/
                ├── summary.json
                ├── report.html
                ├── lighthouse.json  # performance
                └── locust-summary.json  # stress
```

---

## 本地开发

```bash
# Node.js 前端
cd crates/web_client/ui && npm install && npm run build && cd ../..

# Rust 二进制
cargo build --release -p qinAegis-web

# 直接运行
./target/release/qinAegis-web
```

---

## 版本

当前版本：**v0.5.4** (2026-05)

---

_Last Updated: 2026-05-14_