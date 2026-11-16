import { PlaywrightAgent } from '@midscene/web/playwright';
import type { Page } from 'playwright';
import { executeSteps, parseYamlPlan } from './yaml_spec.js';

export interface RunResult {
  case_id: string;
  passed: boolean;
  duration_ms: number;
  screenshot_base64: string | null;
  error_message: string | null;
  report_path: string | null;
  failed_step?: string | null;
}

export async function runYaml(yamlScript: string, caseId: string, page: Page, targetUrl?: string): Promise<RunResult> {
  const start = Date.now();
  let errorMessage: string | null = null;
  let screenshotBase64: string | null = null;
  let lastAttemptedStep: string | null = null;
  try {
    // Validate the complete plan before creating the agent or navigating.
    const plan = parseYamlPlan(yamlScript, targetUrl);
    const agent = new PlaywrightAgent(page);
    lastAttemptedStep = `- goto: ${plan.url}`;
    await page.goto(plan.url);
    await executeSteps(plan, agent, description => { lastAttemptedStep = description; });
    screenshotBase64 = (await page.screenshot()).toString('base64');
  } catch (error) {
    errorMessage = error instanceof Error ? error.message : String(error);
    // Evidence failure cannot replace the original assertion failure.
    if (lastAttemptedStep) {
      try { screenshotBase64 = (await page.screenshot()).toString('base64'); } catch { /* unavailable */ }
    }
  }
  return { case_id: caseId, passed: errorMessage === null, duration_ms: Date.now() - start,
    screenshot_base64: screenshotBase64, error_message: errorMessage, report_path: null,
    failed_step: errorMessage === null ? null : lastAttemptedStep };
}
