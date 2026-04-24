import { PlaywrightAgent } from '@midscene/web/playwright';
import { Page } from 'playwright';
// js-yaml is available via pnpm store
import yaml from 'js-yaml';

export interface YamlTask {
  name: string;
  flow: Array<{ aiAct?: string; aiAssert?: string; aiQuery?: string }>;
}

export interface YamlSpec {
  target: { url: string };
  tasks: YamlTask[];
}

export interface RunResult {
  case_id: string;
  passed: boolean;
  duration_ms: number;
  screenshot_base64: string | null;
  error_message: string | null;
  report_path: string | null;
}

export async function runYaml(
  yamlScript: string,
  caseId: string,
  page: Page,
): Promise<RunResult> {
  const start = Date.now();
  let passed = true;
  let errorMessage: string | null = null;
  let screenshotBase64: string | null = null;

  try {
    const spec: YamlSpec = yaml.load(yamlScript) as YamlSpec;
    const agent = new PlaywrightAgent(page);

    await page.goto(spec.target.url);

    for (const task of spec.tasks) {
      for (const step of task.flow) {
        if (step.aiAct) {
          await agent.aiAct(step.aiAct);
        }
        if (step.aiAssert) {
          try {
            await agent.aiAssert(step.aiAssert);
          } catch (e) {
            passed = false;
            errorMessage = String(e);
            screenshotBase64 = (await page.screenshot({ encoding: 'base64' } as any)).toString();
            throw e;
          }
        }
        if (step.aiQuery) {
          await agent.aiQuery(step.aiQuery);
        }
      }
    }

    screenshotBase64 = (await page.screenshot({ encoding: 'base64' } as any)).toString();
  } catch (e) {
    if (!errorMessage) {
      errorMessage = String(e);
      passed = false;
    }
  }

  return {
    case_id: caseId,
    passed,
    duration_ms: Date.now() - start,
    screenshot_base64: screenshotBase64,
    error_message: errorMessage,
    report_path: null,
  };
}
