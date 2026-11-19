# 历史 Testplane 集成提案

> **归档标记：2026-11-16。以下原文保留用于历史追溯，不是当前产品方案或可直接执行的操作手册。** Testplane 是旧的可选视觉回归提案，不是 2.0 已交付的报告或执行能力。下文第三方包、API 与比较未在本次重新验证；2.0 当前报告采用官方 Allure，不能用此文推断已具备视觉基线审核。

当前 2.0 入口：[产品记忆](../../CONTEXT.md) · [完整产品与实施方案](../plans/local-web.md) · [已验证进度与缺口](../progress/local-web-2.0.md)。

2.0 的核心是可跨项目复用、维护、修订和人工纠正的契约。产品运行形态是本地 HTTP 服务与浏览器 Web 工作台，无桌面客户端打包；模型通过可扩展适配器与路由切换，视觉执行保留 Playwright + Midscene.js，报告采用官方 Allure。引擎按需安装、完整场景编排、任务恢复和经明确授权的无障碍电脑控制仍有待交付或验收，不能因以下提案而标记完成。

旧 TUI、桌面外壳、DMG/Cask、Notion 存储或其它引擎的描述均不能覆盖上述 2.0 方向。需要实际安装和配置时，请使用[安装说明](../../INSTALL_zh.md)；模型和测试页面数据授权以产品记忆为准。

---

## 历史原文

# Testplane Integration

## Overview

[Testplane](https://github.com/testplane/testplane) is an open-source visual regression testing framework by [Skypoint](https://www.skypoint.dev/). It specializes in AI-powered visual testing and can complement qinAegis for visual regression scenarios.

## Features

- **AI-Powered Comparison**: Intelligent visual diff detection
- **Component Testing**: Test UI components in isolation
- **Cross-browser Testing**: Run visual tests across browsers
- **CI/CD Integration**: GitHub Actions, GitLab CI, Jenkins
- **Interactive Review**: Web-based diff review tool

## Installation

```bash
npm install -D @testplane/testplane @testplane/plugin-hermione

# Or with pnpm
pnpm add -D @testplane/testplane @testplane/plugin-hermione
```

## Basic Usage

### 1. Configuration

```javascript
// .testplane.js
module.exports = {
  sets: [
    {
      name: 'desktop',
      files: 'tests/**/*.testplane.js',
      browser: 'chromium',
      viewport: { width: 1440, height: 900 },
    },
  ],
  plugins: [
    ['@testplane/plugin-hermione', {
      project: 'your-project-token',
    }],
  ],
  gemini: {
    apiKey: process.env.GEMINI_API_KEY,
    threshold: 0.1, // 10% visual difference threshold
  },
};
```

### 2. Visual Test

```javascript
// tests/example.testplane.js
const { TestplaneTest } = require('@testplane/testplane');

TestplaneTest('Visual regression - homepage', async ({ browser }) => {
  const page = await browser.newPage();
  await page.goto('https://your-app.com');

  // Capture screenshot
  const screenshot = await page.locator('body').screenshot();

  // Compare with baseline
  await expect(screenshot).toMatchImageSnapshot({
    baseline: 'homepage-desktop.png',
    threshold: 0.1,
  });
});
```

## Visual Testing with Testplane

Testplane 可独立于 qinAegis 使用，专注于视觉回归测试。

## Visual Testing Workflow

qinAegis PC 客户端内置视觉测试能力，无需额外集成 Testplane。

1. 在 **Run Tests** 视图选择 `visual` 类型执行视觉测试
2. 首次运行自动生成 baseline 截图
3. 后续运行对比差异，diff% 超过阈值则 gate 失败
4. 在 **Reports** 视图查看视觉差异报告

## Resources

- [Testplane GitHub](https://github.com/testplane/testplane)
- [Testplane Documentation](https://testplane.org/)
