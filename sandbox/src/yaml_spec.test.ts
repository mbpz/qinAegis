import { test } from 'node:test';
import assert from 'node:assert/strict';
import { executeSteps, parseYamlPlan } from './yaml_spec.js';

const target = 'http://127.0.0.1:1234';
test('accepts complete specs and explicit-target step lists', () => {
  const plan = parseYamlPlan('target:\n  url: https://example.test\ntasks:\n  - name: Receipt\n    flow:\n      - aiAssert: Receipt is visible');
  assert.equal(plan.url, 'https://example.test');
  assert.deepEqual(plan.steps, [{ aiAssert: 'Receipt is visible' }]);
  assert.equal(parseYamlPlan('- aiAct: Click submit', target).url, target);
});
test('rejects empty, ambiguous, unsupported, malformed and unbounded plans', () => {
  for (const script of ['', 'tasks: []', 'tasks:\n - name: Empty\n   flow: []', '[]', '{}', 'true', '- aiAct: ""', '- aiAct: click\n  aiAssert: visible', '- wait: 10', '- aiAct: click\n  unknown: x', '- aiAct: [click]', '- aiAct: first\n  aiAct: duplicate', 'tasks: [']) {
    assert.throws(() => parseYamlPlan(script, target), script);
  }
  assert.throws(() => parseYamlPlan('- aiAssert: visible'));
  assert.throws(() => parseYamlPlan('- aiAct: x\n'.repeat(1001), target));
});
test('rejects non-Web, credential-bearing and malformed targets', () => {
  for (const url of ['file:///tmp/private', 'javascript:alert(1)', 'https://user:secret@example.test', 'invalid']) {
    assert.throws(() => parseYamlPlan('- aiAssert: visible', url));
  }
});
test('each action and assertion executes every time without historical-success bypass', async () => {
  const calls: string[] = [];
  const agent = { aiAct: async (s: string) => { calls.push(`act:${s}`); }, aiAssert: async (s: string) => { calls.push(`assert:${s}`); }, aiQuery: async (s: string) => { calls.push(`query:${s}`); } };
  const plan = parseYamlPlan('- aiAct: click\n- aiAssert: visible\n- aiQuery: receipt', target);
  await executeSteps(plan, agent, () => {});
  await executeSteps(plan, agent, () => {});
  assert.deepEqual(calls, ['act:click', 'assert:visible', 'query:receipt', 'act:click', 'assert:visible', 'query:receipt']);
});
test('assertion failure remains a failure and stops later steps', async () => {
  const trace: string[] = [];
  const agent = { aiAct: async () => { throw new Error('must not execute'); }, aiAssert: async () => { throw new Error('receipt mismatch'); }, aiQuery: async () => null };
  const plan = parseYamlPlan('- aiAssert: receipt matches\n- aiAct: submit again', target);
  await assert.rejects(executeSteps(plan, agent, s => trace.push(s)), /receipt mismatch/);
  assert.deepEqual(trace, ['- aiAssert: receipt matches']);
});
