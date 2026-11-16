import { parse } from 'yaml';

export type YamlStep = { aiAct: string } | { aiAssert: string } | { aiQuery: string };
export interface YamlPlan { url: string; steps: YamlStep[] }
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const text = (value: unknown): value is string => typeof value === 'string' && value.trim().length > 0;
function steps(value: unknown): YamlStep[] {
  if (!Array.isArray(value) || !value.length || value.length > 1000) throw new Error('Flow must contain 1..1000 steps');
  return value.map(step => {
    if (!object(step) || Object.keys(step).length !== 1) throw new Error('Each step must contain exactly one action');
    const key = Object.keys(step)[0];
    if (!['aiAct', 'aiAssert', 'aiQuery'].includes(key) || !text(step[key])) throw new Error('Unsupported or empty action');
    return step as YamlStep;
  });
}
export function parseYamlPlan(source: string, targetUrl?: string): YamlPlan {
  if (!text(source) || source.length > 1024 * 1024) throw new Error('Invalid YAML size');
  const value: unknown = parse(source, { uniqueKeys: true, maxAliasCount: 0 });
  let url: unknown = targetUrl;
  let flow: YamlStep[];
  if (Array.isArray(value)) flow = steps(value);
  else {
    if (!object(value) || Object.keys(value).some(k => !['target', 'tasks'].includes(k))) throw new Error('Expected target/tasks YAML specification');
    if (value.target !== undefined) {
      if (!object(value.target) || Object.keys(value.target).some(k => k !== 'url') || !text(value.target.url)) throw new Error('Invalid target');
      url = value.target.url;
    }
    if (!Array.isArray(value.tasks) || !value.tasks.length || value.tasks.length > 200) throw new Error('Tasks must contain 1..200 entries');
    flow = value.tasks.flatMap(task => {
      if (!object(task) || Object.keys(task).some(k => !['name','flow'].includes(k)) || !text(task.name)) throw new Error('Invalid task');
      return steps(task.flow);
    });
    if (flow.length > 1000) throw new Error('Too many steps');
  }
  if (!text(url)) throw new Error('Target URL is required');
  const parsed = new URL(url);
  if (!['http:', 'https:'].includes(parsed.protocol) || !parsed.hostname || parsed.username || parsed.password) throw new Error('Target must use HTTP(S) without credentials');
  return { url, steps: flow };
}
export interface StepAgent {
  aiAct(prompt: string): Promise<unknown>;
  aiAssert(prompt: string): Promise<unknown>;
  aiQuery(prompt: string): Promise<unknown>;
}
export async function executeSteps(plan: YamlPlan, agent: StepAgent, onStep: (description: string) => void): Promise<void> {
  for (const step of plan.steps) {
    if ('aiAct' in step) { onStep(`- aiAct: ${step.aiAct}`); await agent.aiAct(step.aiAct); }
    else if ('aiAssert' in step) { onStep(`- aiAssert: ${step.aiAssert}`); await agent.aiAssert(step.aiAssert); }
    else { onStep(`- aiQuery: ${step.aiQuery}`); await agent.aiQuery(step.aiQuery); }
  }
}
