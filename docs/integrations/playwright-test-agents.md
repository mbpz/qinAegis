# Playwright Test Agents Reference

## Overview

[Playwright Test Agents](https://github.com/microsoft/playwright-test-agents) is Microsoft's experimental AI system for autonomous test generation. It provides a reference architecture for AI-driven testing that qinAegis can learn from and integrate with.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Playwright Test Agents                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
│  │   Planner    │───▶│  Generator   │───▶│   Healer    │      │
│  │   Agent      │    │   Agent      │    │   Agent      │      │
│  └──────────────┘    └──────────────┘    └──────────────┘      │
│         │                   │                   │              │
│         ▼                   ▼                   ▼              │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   Playwright Test                        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Trace Viewer / Reporter                     │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Planner Agent

```typescript
// Responsible for understanding requirements and planning test strategy
interface PlannerInput {
  requirements: string;        // Natural language requirements
  pageStructure: AccessibilitySnapshot; // DOM/accessibility tree
  existingTests: string[];     // Already covered scenarios
}

interface PlannerOutput {
  testPlan: TestPlan;
  coverageAnalysis: CoverageAnalysis;
  suggestedTestCases: TestCase[];
}
```

### 2. Generator Agent

```typescript
// Generates Playwright test code from test plan
interface GeneratorInput {
  testPlan: TestPlan;
  pageModel: PageModel;        // UI components, locators
  testFramework: 'jest' | 'mocha' | 'playwright';
}

interface GeneratorOutput {
  testCode: string;
  locators: LocatorMap;
  assertions: Assertion[];
}
```

### 3. Healer Agent

```typescript
// Self-heals broken tests when page structure changes
interface HealerInput {
  brokenTest: BrokenTest;
  errorAnalysis: ErrorAnalysis;
  pageSnapshot: AccessibilitySnapshot;
}

interface HealerOutput {
  healedTest: string;
  explanation: string;
  confidence: number;        // 0-1, how confident the fix is
}
```

## Integration Points for qinAegis

### 1. Planner Integration

```typescript
// crates/sandbox/src/agents/planner.ts

import { Agent } from '@playwright/testagents';

export async function planTests(
  requirements: string,
  pageSnapshot: AccessibilitySnapshot
): Promise<TestPlan> {
  const planner = new Agent({
    model: 'gpt-4o',
    system: PLANNER_SYSTEM_PROMPT,
  });

  return planner.complete({
    input: `Requirements: ${requirements}\n\nPage Structure:\n${pageSnapshot}`,
    schema: testPlanSchema,
  });
}

const PLANNER_SYSTEM_PROMPT = `
You are a test planning agent for web applications.

Given requirements and page structure, create a comprehensive test plan:
1. Identify critical user flows
2. Map to page elements
3. Define success/failure criteria
4. Prioritize test cases

Respond with structured test plan.
`;
```

### 2. Generator Integration

```typescript
// crates/sandbox/src/agents/generator.ts

export async function generateTests(
  testPlan: TestPlan,
  context: GenerationContext
): Promise<GeneratedTest[]> {
  const generator = new Agent({
    model: 'gpt-4o',
    system: GENERATOR_SYSTEM_PROMPT,
  });

  const tests: GeneratedTest[] = [];

  for (const testCase of testPlan.cases) {
    const code = await generator.complete({
      input: `Test Case: ${testCase.description}\n${context}`,
      schema: testCodeSchema,
    });

    tests.push({
      id: generateId(),
      code,
      confidence: testCase.priority,
    });
  }

  return tests;
}
```

### 3. Healer Integration

```typescript
// crates/sandbox/src/agents/healer.ts

export async function healTest(
  brokenTest: string,
  error: TestError,
  pageSnapshot: AccessibilitySnapshot
): Promise<HealedTest> {
  const healer = new Agent({
    model: 'gpt-4o',
    system: HEALER_SYSTEM_PROMPT,
  });

  return healer.complete({
    input: `
Broken Test:
${brokenTest}

Error:
${error.message}

Current Page Structure:
${pageSnapshot}

Analyze the error and provide a fixed test.
`,
    schema: healedTestSchema,
  });
}
```

## qinAegis Adaptation

### Adapt Planner → Generator

qinAegis already has a generator module. Enhancement:

```rust
// crates/core/src/generator.rs

pub async fn generate_with_planner(
    &self,
    requirements: &str,
    page_snapshot: &AccessibilitySnapshot,
) -> Result<Vec<TestCase>> {
    // Use Planner to understand requirements
    let plan = self.planner.create_plan(requirements, page_snapshot).await?;

    // Use Generator to create test cases
    let cases = self.generator.generate_from_plan(&plan).await?;

    // Return draft cases for review
    Ok(cases.into_iter().map(|c| c.to_draft()).collect())
}
```

### Adapt Healer

```rust
// crates/core/src/healer.rs

pub async fn heal_failure(
    &self,
    failed_case: &TestCase,
    error: &str,
    page_snapshot: &AccessibilitySnapshot,
) -> Result<TestCase> {
    // Analyze error and propose fix
    let suggestion = self.analyze_and_suggest(error, page_snapshot).await?;

    // Create new draft case with suggested fixes
    let mut healed = failed_case.clone();
    healed.yaml_script = suggestion.fixed_script;
    healed.status = CaseStatus::Draft;
    healed.metadata.insert("healed_from".to_string(), failed_case.id.clone());
    healed.metadata.insert("healer_confidence".to_string(), suggestion.confidence.to_string());

    Ok(healed)
}
```

## Key Lessons from Playwright Test Agents

1. **Three-Agent Architecture**: Planner → Generator → Healer provides clear separation
2. **Draft Workflow**: All AI-generated content goes to draft, requiring human review
3. **Confidence Scoring**: AI suggestions include confidence levels
4. **Explanation**: Healer's fixes include explanation of what changed and why
5. **Trace Integration**: Full execution trace for debugging failed healings

## Resources

- [Playwright Test Agents GitHub](https://github.com/microsoft/playwright-test-agents)
- [Playwright Documentation](https://playwright.dev/docs/)
- [AI Agents in Playwright](https://github.com/microsoft/playwright/blob/main/docs/debugger-ui.md)
