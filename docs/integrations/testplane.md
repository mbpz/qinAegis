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
