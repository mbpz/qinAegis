import { PlaywrightAgent } from '@midscene/web/playwright';
import { Capture } from './capture.js';
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
  capture?: Awaited<ReturnType<Capture['finish']>>;
}
export async function runYaml(yamlScript: string, caseId: string, page: Page, targetUrl?: string, capture?: Capture): Promise<RunResult> {
  const start = Date.now();
  let errorMessage: string | null = null;
  let screenshotBase64: string | null = null;
  let lastAttemptedStep: string | null = null;
  try {
    // Validate the complete plan before creating the agent or navigating.
    const plan = parseYamlPlan(yamlScript, targetUrl);
    await capture?.start();
    const agent = new PlaywrightAgent(page, capture ? { generateReport: false, persistExecutionDump: false, autoPrintReportMsg: false } : undefined);
    lastAttemptedStep = `- goto: ${plan.url}`;
    await page.goto(plan.url);
    if (capture) {
      for (const [index, step] of plan.steps.entries()) {
        const action = Object.keys(step)[0] as 'aiAct' | 'aiAssert' | 'aiQuery';
        lastAttemptedStep = `- ${action}: ${step[action as keyof typeof step]}`;
        const started = performance.now();
        try {
          await agent[action](Object.values(step)[0]);
        }
        catch (error) {
          await capture.step(index, action, 'failed', started);
          throw error;
        }
        await capture.step(index, action, 'passed', started);
      }
    }
    else {
      await executeSteps(plan, agent, description => { lastAttemptedStep = description; });
      screenshotBase64 = (await page.screenshot()).toString('base64');
    }
  }
  catch (error) {
    errorMessage = error instanceof Error ? error.message : String(error);
    // Evidence failure cannot replace the original assertion failure.
    if (lastAttemptedStep && !capture) {
      try {
        screenshotBase64 = (await page.screenshot()).toString('base64');
      }
      catch { /* unavailable */ }
    }
  }
  const receipt = capture ? await capture.finish(errorMessage === null) : undefined;
  return { ...(receipt ? { capture: receipt } : {}), case_id: caseId, passed: errorMessage === null, duration_ms: Date.now() - start,
    screenshot_base64: screenshotBase64, error_message: errorMessage, report_path: null,
    failed_step: errorMessage === null ? null : lastAttemptedStep };
}
