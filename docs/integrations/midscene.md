# 历史 Midscene 集成示例

> **归档标记：2026-11-16。以下原文保留用于历史追溯，不是当前产品方案或可直接执行的操作手册。** 2.0 仍以 Playwright + Midscene.js 为视觉驱动测试基础，但下面的 TUI、固定模型推荐和开发路径是旧示例，不是当前安装指引。具体厂商、型号与模型族兼容性需按锁定引擎验证；文本请求成功不能证明视觉执行兼容。

当前 2.0 入口：[产品记忆](../../CONTEXT.md) · [完整产品与实施方案](../plans/local-web.md) · [已验证进度与缺口](../progress/local-web-2.0.md)。

2.0 的核心是可跨项目复用、维护、修订和人工纠正的契约。产品运行形态是本地 HTTP 服务与浏览器 Web 工作台，无桌面客户端打包；模型通过可扩展适配器与路由切换，视觉执行保留 Playwright + Midscene.js，报告采用官方 Allure。引擎按需安装、完整场景编排、任务恢复和经明确授权的无障碍电脑控制仍有待交付或验收，不能因以下提案而标记完成。

旧 TUI、桌面外壳、DMG/Cask、Notion 存储或其它引擎的描述均不能覆盖上述 2.0 方向。需要实际安装和配置时，请使用[安装说明](../../INSTALL_zh.md)；模型和测试页面数据授权以产品记忆为准。

---

## 历史原文

# Midscene Integration

[Midscene](https://midscenejs.com/) is the AI-powered browser automation engine used by qinAegis for:
- **explore**: AI-driven page exploration and information extraction
- **aiQuery**: Query page content using natural language
- **aiAct**: Perform actions on the page using natural language
- **aiAssert**: Make assertions about page state using natural language

## Configuration

Midscene uses environment variables for LLM configuration. These can be set via the TUI Config Form or in your shell.

### Required Environment Variables

```bash
# API Key from your LLM provider
export MIDSCENE_MODEL_API_KEY="your-api-key"

# Base URL for OpenAI-compatible API
export MIDSCENE_MODEL_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"

# Model name (must be a vision-capable model for explore)
export MIDSCENE_MODEL_NAME="qwen3.6-plus"

# Model family (required for Midscene to recognize vision models)
export MIDSCENE_MODEL_FAMILY="qwen3.6"
```

### Supported Model Families

| Family | Models | Notes |
|--------|--------|-------|
| `qwen3.6` | qwen3.6-plus | **Recommended** - Latest VL model with reasoning |
| `qwen3-vl` | qwen-vl-max | Older VL model |
| `doubao-vision` | doubao-vision | Bytedance |
| `gemini` | gemini-pro-vision | Google's vision model |

> **Note**: `qwen3.6` is the model family. The OpenAI-compatible API model name is `qwen3.6-plus`.

## Testing Vision Models with curl

Before using Midscene, verify your API credentials work:

### Aliyun DashScope (qwen3.6-plus)

```bash
# Test text-only model
curl https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "qwen-turbo", "messages": [{"role": "user", "content": "say hello"}]}'

# Test vision model (qwen3.6-plus)
curl https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "qwen3.6-plus", "messages": [{"role": "user", "content": "say hello"}]}'

# Test vision model (qwen-vl-max - older model)
curl https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "qwen-vl-max", "messages": [{"role": "user", "content": "say hello"}]}'
```

Expected text response:
```json
{
  "choices": [{
    "message": {
      "content": "Hello! How can I help you today?",
      "role": "assistant"
    },
    "finish_reason": "stop"
  }]
}
```

## Common Errors

### "Model configuration is incomplete: model name (MIDSCENE_MODEL_NAME) is required"

**Cause**: Missing `MIDSCENE_MODEL_NAME` or `MIDSCENE_MODEL_FAMILY` environment variable.

**Fix**: Set both variables:
```bash
export MIDSCENE_MODEL_NAME="qwen3.6-plus"
export MIDSCENE_MODEL_FAMILY="qwen3.6"
```

### "400 Access denied - Arrearage"

**Cause**: Your LLM provider account has overdue payments.

**Fix**: Log into your provider's console and top up your account.

### "Model not found" or "unknown model"

**Cause**: The model name isn't available via the OpenAI-compatible API.

**Fix**: Use the correct model name. For DashScope, use `qwen3.6-plus` (the API model name), not `qwen3.6` (which is the model family/series name).

## Local Development

### Prerequisites

1. Chrome/Chromium installed (for Playwright browser automation)
2. CDP port 9222 available (or set `CDP_PORT` env var)

### Test Midscene Integration

```bash
cd /Users/jinguo.zeng/dmall/project/qinAegis/sandbox

# Set environment
export MIDSCENE_MODEL_API_KEY="your-key"
export MIDSCENE_MODEL_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
export MIDSCENE_MODEL_NAME="qwen3.6-plus"
export MIDSCENE_MODEL_FAMILY="qwen3.6"
export CDP_PORT=9222

# Run a simple test
npx tsx -e '
import { chromium } from "playwright";
import { PlaywrightAgent } from "@midscene/web/playwright";

const browser = await chromium.launch({ headless: true, args: ["--remote-debugging-port=9222"] });
const page = await browser.newPage();
const agent = new PlaywrightAgent(page);

await page.goto("https://www.baidu.com");
const result = await agent.aiQuery("{title: string}, get the page title");

console.log("Page title:", result.title);
await browser.close();
'
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         qinAegis CLI                        │
│  ┌─────────┐  ┌──────────┐  ┌─────────┐  ┌─────────────┐  │
│  │  TUI    │  │ Explorer │  │Generator│  │ Test Runner │  │
│  └────┬────┘  └────┬─────┘  └────┬────┘  └──────┬──────┘  │
│       │            │              │               │          │
│       └────────────┴──────────────┴───────────────┘          │
│                          │                                   │
│                    JSON-RPC                                   │
│                          │                                   │
└──────────────────────────┼───────────────────────────────────┘
                           │
┌──────────────────────────┼───────────────────────────────────┐
│              Midscene Executor (Node.js)                      │
│  ┌───────────────────────┐│                                   │
│  │  Protocol Handler     ││  stdin/stdout JSON-RPC           │
│  └───────────┬───────────┘│                                   │
│              │            │                                   │
│  ┌───────────▼───────────┐│                                   │
│  │   PlaywrightAgent      │◄──► Browser (Chrome)               │
│  │   (AI-powered)         ││    via CDP                        │
│  └───────────┬───────────┘│                                   │
│              │            │                                   │
│  ┌───────────▼───────────┐│                                   │
│  │   Midscene Core        │◄──► LLM API (OpenAI-compatible)   │
│  │   (AI inference)       ││                                   │
│  └───────────────────────┘│                                   │
└──────────────────────────────────────────────────────────────┘
```

## Resources

- [Midscene Documentation](https://midscenejs.com/)
- [Model Strategy](https://midscenejs.com/model-strategy.html)
- [Playwright Integration](https://midscenejs.com/integrate-with-playwright.html)
