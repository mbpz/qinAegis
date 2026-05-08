# Stagehand Integration

## Overview

[Stagehand](https://github.com/browserbase/stagehand) is an AI-native browser automation library by [Browserbase](https://www.browserbase.com/). It provides vision-language model powered element detection and interaction, which can complement or serve as an alternative to Midscene.js.

## When to Use Stagehand

| Scenario | Recommended Tool |
|----------|------------------|
| Simple structured actions (click, type, goto) | Playwright directly |
| Complex UI interactions with visual understanding | Stagehand |
| Natural language test generation | Midscene.js |
| Visual regression testing | Midscene.js + Testplane |
| **Hybrid approach** | Stagehand + Midscene |

## Installation

```bash
# Node.js project
npm install stagehand

# Or with pnpm
pnpm add stagehand
```

## Basic Usage

```typescript
import { Stagehand } from 'stagehand';
import { chromium } from 'playwright';

async function main() {
  const browser = await chromium.launch();
  const page = await browser.newPage();

  const stagehand = new Stagehand({
    page,
    model: 'gpt-4o',
    apiKey: process.env.OPENAI_API_KEY,
  });

  await stagehand.init();

  // Navigate and interact using natural language
  await stagehand.act('Go to https://example.com');
  await stagehand.act('Click the "Get Started" button');
  await stagehand.act('Type "test@example.com" into the email field');

  // Assert using natural language
  const success = await stagehand.observe('Success message is visible');

  await stagehand.done();
  await browser.close();
}
```

## Integration with qinAegis

### 1. Add to sandbox package.json

```json
{
  "dependencies": {
    "stagehand": "^1.0.0"
  }
}
```

### 2. Create Stagehand Executor

```typescript
// sandbox/src/stagehand-executor.ts

import { Stagehand } from 'stagehand';
import { chromium } from 'playwright';

interface StagehandConfig {
  model?: 'gpt-4o' | 'gpt-4o-mini' | 'claude-3-5-sonnet';
  apiKey: string;
}

interface StagehandResult {
  success: boolean;
  actions: string[];
  observations: string[];
  error?: string;
}

export async function runWithStagehand(
  url: string,
  instructions: string[],
  config: StagehandConfig
): Promise<StagehandResult> {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();

  const stagehand = new Stagehand({
    page,
    model: config.model || 'gpt-4o-mini',
    apiKey: config.apiKey,
  });

  await stagehand.init();

  const actions: string[] = [];
  const observations: string[] = [];

  try {
    await stagehand.act(`Navigate to ${url}`);
    actions.push(`Navigated to ${url}`);

    for (const instruction of instructions) {
      await stagehand.act(instruction);
      actions.push(instruction);
    }

    // Extract observations
    for (const observation of instructions) {
      const result = await stagehand.observe(observation);
      observations.push(`${observation}: ${result ? 'FOUND' : 'NOT FOUND'}`);
    }

    return { success: true, actions, observations };
  } catch (error) {
    return {
      success: false,
      actions,
      observations,
      error: String(error),
    };
  } finally {
    await stagehand.done();
    await browser.close();
  }
}
```

### 3. Add to Automation Trait

```rust
// crates/core/src/automation/trait_def.rs

#[async_trait]
pub trait BrowserAutomation: Send + Sync {
    // ... existing methods ...

    /// Run a Stagehand script
    async fn run_stagehand(
        &self,
        url: &str,
        instructions: Vec<String>,
        config: StagehandConfig,
    ) -> Result<StagehandResult, AutomationError>;
}
```

## Comparison: Midscene vs Stagehand

| Feature | Midscene.js | Stagehand |
|---------|-------------|-----------|
| **Model Support** | OpenAI, Anthropic, local | OpenAI, Anthropic |
| **Vision Capability** | Yes | Yes |
| **DOM Snapshot** | Accessibility tree | Visual grounding |
| **Test Format** | YAML scripts | TypeScript SDK |
| **Self-Healing** | Basic | Advanced |
| **Debug Mode** | Yes | Yes (Stagehand Lab) |
| **Cost** | API calls | API calls |
| **Open Source** | Yes | Yes |

## Hybrid Approach

Use both tools for different purposes:

```typescript
// Use Midscene for structured E2E tests
import { Midscene } from 'midscene';

// Use Stagehand for exploratory visual testing
import { Stagehand } from 'stagehand';

async function hybridTest() {
  // Run structured E2E with Midscene
  const midscene = new Midscene();
  await midscene.runYaml('tests/smoke-test.yaml');

  // Use Stagehand for visual validation
  const stagehand = new Stagehand({ page: midscene.getPage() });
  await stagehand.observe('Critical UI element is visible');
}
```

## Resources

- [Stagehand GitHub](https://github.com/browserbase/stagehand)
- [Stagehand Documentation](https://docs.browserbase.com/stagehand)
- [Browserbase](https://www.browserbase.com/)
