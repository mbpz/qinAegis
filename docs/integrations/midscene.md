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
