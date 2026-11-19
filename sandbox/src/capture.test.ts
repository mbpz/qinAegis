import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { EventEmitter } from 'node:events';
import type { BrowserContext, Page } from 'playwright';
import { Capture, capturePolicy, type CapturePolicy } from './capture.js';
const policy: CapturePolicy = { schema_version: 1, mode: 'off', max_artifact_bytes: 1048576, max_run_bytes: 2097152, max_events: 100, include_sensitive: false };
test('capture policies reject unknown fields, coercion, limits and missing authorization', () => {
  assert.deepEqual(capturePolicy(policy), policy);
  for (const change of [{ extra: 1 }, { mode: 'unknown' }, { mode: 'full' }, { schema_version: 2 }, { max_events: 0 }, { max_events: 2001 }, { max_artifact_bytes: 1023 }, { max_artifact_bytes: 8388609 }, { max_run_bytes: 33554433 }, { max_run_bytes: 1024 }, { max_events: '1' }, { include_sensitive: 1 }])
    assert.throws(() => capturePolicy({ ...policy, ...change }));
});
test('off and successful failure-only runs never request screenshots or retain artifacts', async () => {
  for (const mode of ['off', 'failure_only'] as const) {
    const page = new EventEmitter() as unknown as import('playwright').Page;
    page.screenshot = async () => { throw new Error('Screenshot must not be requested'); };
    const capture = new Capture(capturePolicy({ ...policy, mode, include_sensitive: mode !== 'off' }), page, {} as import('playwright').BrowserContext, '.');
    await capture.start();
    const receipt = await capture.finish(true);
    assert.deepEqual(receipt.artifacts, []);
    assert.ok(receipt.channels.every(channel => channel.status === 'not_requested'));
  }
});

test('diagnostic failures are bounded receipt facts without leaking their error bodies', async () => {
  const root = await mkdtemp(join(tmpdir(), 'qinaegis-capture-failure-'));
  const previous = process.cwd();
  process.chdir(root);
  try {
    const page = new EventEmitter() as unknown as Page;
    page.screenshot = async () => { throw new Error('private screenshot failure body'); };
    const context = { tracing: { start: async () => { throw new Error('private trace failure body'); } } } as unknown as BrowserContext;
    const capture = new Capture(capturePolicy({ ...policy, mode: 'full', include_sensitive: true }), page, context, root);
    await capture.start();
    await capture.step(0, 'aiAssert', 'failed', performance.now());
    const receipt = await capture.finish(false);
    assert.equal(receipt.status, 'partial');
    assert.equal(receipt.channels.find(c => c.kind === 'screenshot')?.status, 'failed');
    assert.equal(receipt.channels.find(c => c.kind === 'trace')?.status, 'failed');
    assert.ok(!JSON.stringify(receipt).includes('private'));
    assert.ok(receipt.artifacts.every(a => a.kind !== 'screenshot' && a.kind !== 'trace'));
    assert.equal((await readdir(root)).length, receipt.artifacts.length);
  } finally {
    process.chdir(previous);
    await rm(root, { recursive: true, force: true });
  }
});
