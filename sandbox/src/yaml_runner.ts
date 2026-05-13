import { PlaywrightAgent } from '@midscene/web/playwright';
import { Page } from 'playwright';
// js-yaml is available via pnpm store
import yaml from 'js-yaml';
import { globalActionCache } from './action_cache.js';

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
  cache_hits?: number;
}

export async function runYaml(
  yamlScript: string,
  caseId: string,
  page: Page,
  targetUrl?: string,
): Promise<RunResult> {
  const start = Date.now();
  let passed = true;
  let errorMessage: string | null = null;
  let screenshotBase64: string | null = null;
  let cacheHits = 0;

  try {
    // Try parsing as full YAML spec first
    let spec: YamlSpec | null = null;
    try {
      spec = yaml.load(yamlScript) as YamlSpec;
    } catch {
      // Not a valid YAML object, treat as step list
    }

    const agent = new PlaywrightAgent(page);

    // Use target URL from spec or passed parameter
    const gotoUrl = spec?.target?.url || targetUrl;
    if (!gotoUrl) {
      throw new Error('No target URL provided');
    }
    await page.goto(gotoUrl);

    // Handle step list format (array of strings like "- aiAct: ...")
    if (Array.isArray(yaml.load(yamlScript))) {
      const steps = yaml.load(yamlScript) as Array<{ aiAct?: string; aiAssert?: string; aiQuery?: string }>;
      for (const step of steps) {
        if (step.aiAct) {
          // Check action cache
          const cached = await globalActionCache.get(page, 'act:' + step.aiAct);
          if (cached !== null) {
            cacheHits++;
          } else {
            await agent.aiAct(step.aiAct);
            await globalActionCache.set(page, 'act:' + step.aiAct, true);
          }
        }
        if (step.aiAssert) {
          // Check action cache
          const cached = await globalActionCache.get(page, 'assert:' + step.aiAssert);
          if (cached !== null) {
            cacheHits++;
          } else {
            try {
              await agent.aiAssert(step.aiAssert);
              await globalActionCache.set(page, 'assert:' + step.aiAssert, true);
            } catch (e) {
              passed = false;
              errorMessage = String(e);
              screenshotBase64 = (await page.screenshot({ encoding: 'base64' } as any)).toString();
              throw e;
            }
          }
        }
        if (step.aiQuery) {
          // Check action cache
          const cached = await globalActionCache.get(page, 'query:' + step.aiQuery);
          if (cached !== null) {
            cacheHits++;
          } else {
            const result = await agent.aiQuery(step.aiQuery);
            await globalActionCache.set(page, 'query:' + step.aiQuery, result);
          }
        }
      }
    } else if (spec) {
      // Full spec format with tasks
      for (const task of spec.tasks) {
        for (const step of task.flow) {
          if (step.aiAct) {
            const cached = await globalActionCache.get(page, 'act:' + step.aiAct);
            if (cached !== null) {
              cacheHits++;
            } else {
              await agent.aiAct(step.aiAct);
              await globalActionCache.set(page, 'act:' + step.aiAct, true);
            }
          }
          if (step.aiAssert) {
            const cached = await globalActionCache.get(page, 'assert:' + step.aiAssert);
            if (cached !== null) {
              cacheHits++;
            } else {
              try {
                await agent.aiAssert(step.aiAssert);
                await globalActionCache.set(page, 'assert:' + step.aiAssert, true);
              } catch (e) {
                passed = false;
                errorMessage = String(e);
                screenshotBase64 = (await page.screenshot({ encoding: 'base64' } as any)).toString();
                throw e;
              }
            }
          }
          if (step.aiQuery) {
            const cached = await globalActionCache.get(page, 'query:' + step.aiQuery);
            if (cached !== null) {
              cacheHits++;
            } else {
              const result = await agent.aiQuery(step.aiQuery);
              await globalActionCache.set(page, 'query:' + step.aiQuery, result);
            }
          }
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
    cache_hits: cacheHits,
  };
}
