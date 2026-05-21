# Rust Native Browser Automation

**Spec ID:** 009-rust-native-automation
**Status:** In Progress
**Created:** 2026-05-21

---

## Summary

将 qinAegis 的浏览器自动化层从 `MidsceneAutomation`（通过 JSON-RPC 委托给 Node.js/TypeScript 子进程）**替换为纯 Rust 实现**，消除对 Node.js 运行时的运行时依赖，使 DMG 安装后即可使用，无需用户额外安装 Node.js。

---

## 需求背景

### 问题现状

当前架构依赖一个 Node.js 子进程来执行所有浏览器操作：

```
Rust BrowserAutomation trait
    → MidsceneAutomation (Rust)
        → MidsceneProcess (JSON-RPC stdin/stdout)
            → tsx src/executor.ts (Node.js + TypeScript)
                → playwright (browser control)
                → @midscene/web (AI vision engine)
                → yaml_runner.ts (test execution)
                → lighthouse_runner.ts (performance)
                → locust_runner.ts (stress test)
                → zap_runner.ts (security scan)
```

- DMG 从 Finder 启动时 PATH 不含 Node.js → 缺少 `node` 时报错
- `sandbox/node_modules/` 约 300MB，需 `npm install`
- 用户需手动安装 Node.js 版本管理器（nvm/n/fnm）或 Homebrew
- 每个用户的 Node.js 安装路径不一致

### 目标

- 消除运行时对 Node.js 的依赖
- DMG 安装即用，零额外配置
- 保持或提升现有功能

### 非目标

- 不改变 `BrowserAutomation` trait 接口（向后兼容）
- 不改变 `BfsExplorer` 的探索逻辑（已在 Rust 中实现）
- 不改变 `LlmClient` / `Storage` 等现有 Rust 接口
- Lighthouse / Locust / ZAP 作为外部工具仍以子进程方式运行

---

## 范围

### 包含

- CDP 浏览器启动和生命周期管理（替代 Playwright）
- 页面导航、截图（替代 `page.goto()` / `page.screenshot()`）
- AI 页面分析（替代 `agent.aiQuery()`）
- AI 视觉动作（替代 `agent.aiAct()`）
- AI 视觉断言（替代 `agent.aiAssert()`）
- YAML 测试执行（替代 `yaml_runner.ts`）
- 动作缓存（替代 `action_cache.ts`）
- 移除 `MidsceneProcess` / `JsonRpcRequest` / `JsonRpcResponse`
- 从 `Cargo.toml` 中清理不再需要的依赖

### 不包含

- Lighthouse CI 集成仍通过子进程调用（替换管理代码而非工具本身）
- Locust 集成仍通过子进程调用
- ZAP 安全扫描仍通过 Docker 调用
- `sandbox/` 目录保留但不再运行时依赖（可删除源码引用）

---

## 验收标准

1. `cargo build --release` 编译通过，无 `sandbox/` 运行时依赖
2. Explore 功能正常：输入 URL → 自动打开浏览器 → AI 分析页面 → 输出页面结构
3. Test 执行正常：YAML 测试用例 → 浏览器执行 → 输出 pass/fail
4. Generate 功能正常：AI 分析目标页面 → 生成测试用例
5. DMG 打包后在无 Node.js 的机器上可正常运行 Explore
