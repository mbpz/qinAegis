import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, stat, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { chromium, type BrowserContext, type Page } from 'playwright';
import { Capture, capturePolicy, type CapturePolicy } from './capture.js';
import { runYaml } from './yaml_runner.js';
const policy: CapturePolicy = { schema_version: 1, mode: 'off', max_artifact_bytes: 1048576, max_run_bytes: 2097152, max_events: 100, include_sensitive: false };
test('real local Chromium captures bounded owned evidence without any model calls', async () => {
  const root = await mkdtemp(join(tmpdir(), 'qinaegis-capture-test-'));
  const old = process.cwd();
  const server = createServer((_, res) => { res.setHeader('Content-Type', 'text/html'); res.end('<h1>Local capture fixture</h1>'); });
  await new Promise<void>(r => server.listen(0, '127.0.0.1', r));
  const address = server.address() as {
    port: number;
  };
  process.env.QINAEGIS_CAPTURE_TEST_API_KEY = 'capture-private-key';
  process.chdir(root);
  let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined;
  try {
    browser = await chromium.launch({ headless: true, tracesDir: root, executablePath: process.env.QINAEGIS_CAPTURE_BROWSER_EXECUTABLE });
    for (const mode of ['off', 'failure_only', 'key_steps', 'full'] as const) {
      const context: BrowserContext = await browser.newContext();
      const page: Page = await context.newPage();
      const p = capturePolicy({ ...policy, mode, include_sensitive: mode !== 'off' });
      const capture: Capture = new Capture(p, page, context, root);
      await capture.start();
      await page.goto(`http://127.0.0.1:${address.port}/?credential=private`);
      await page.evaluate(async () => { console.log('capture-private-key'); await fetch('http://127.0.0.1:1/failed?credential=private').catch(() => {}); });
      await capture.step(0, 'aiAssert', 'passed', performance.now());
      const receipt: Awaited<ReturnType<Capture['finish']>> = await capture.finish(mode !== 'failure_only');
      assert.equal(receipt.policy.mode, mode);
      if (mode === 'off')
        assert.deepEqual(receipt.artifacts, []);
      else {
        assert.ok(receipt.artifacts.some(a => a.kind === 'screenshot'));
        assert.ok(receipt.artifacts.some(a => a.kind === 'steps'));
        if (mode === 'key_steps')
          assert.equal(receipt.channels.find(c => c.kind === 'trace')!.status, 'not_requested');
        if (mode === 'full')
          assert.ok(receipt.artifacts.some(a => a.kind === 'trace'));
        assert.ok(receipt.artifacts.reduce((n, a) => n + a.bytes, 0) <= p.max_run_bytes);
        for (const artifact of receipt.artifacts) {
          assert.match(artifact.file_name, /^qinaegis-capture-[a-f0-9]{32}\.(png|zip|json)$/);
          const bytes = await readFile(artifact.file_name);
          assert.ok(bytes.length <= p.max_artifact_bytes);
          assert.equal(bytes.length, artifact.bytes);
          assert.equal(createHash('sha256').update(bytes).digest('hex'), artifact.sha256);
          if (process.platform !== 'win32')
            assert.equal((await stat(artifact.file_name)).mode & 0o777, 0o600);
          if (artifact.kind === 'console' || artifact.kind === 'network') {
            assert.ok(!bytes.toString().includes('capture-private-key'));
            assert.ok(!bytes.toString().includes('credential='));
            const events = JSON.parse(bytes.toString()) as Array<{ time_ms: number; failed?: boolean }>;
            assert.ok(events.every(e => Number.isInteger(e.time_ms) && e.time_ms >= 0));
            if (artifact.kind === 'network') assert.ok(events.some(e => e.failed === true));
          }
        }
      }
      await context.close();
    }
    const failureContext = await browser.newContext();
    const failurePage = await failureContext.newPage();
    failurePage.goto = async () => { throw new Error('original fixture navigation failure'); };
    failurePage.screenshot = async () => { throw new Error('private diagnostic failure'); };
    const failure = await runYaml('- aiAssert: must never reach model', 'failure-fixture', failurePage, `http://127.0.0.1:${address.port}/`, new Capture(capturePolicy({ ...policy, mode: 'failure_only', include_sensitive: true }), failurePage, failureContext, root));
    assert.equal(failure.passed, false);
    assert.equal(failure.error_message, 'original fixture navigation failure');
    assert.equal(failure.screenshot_base64, null);
    assert.equal(failure.capture?.channels.find(c => c.kind === 'screenshot')?.status, 'failed');
    await failureContext.close();
    await browser.close();
    const boundedTrace = await mkdtemp(join(root, 'bounded-trace-'));
    browser = await chromium.launch({ headless: true, tracesDir: boundedTrace, executablePath: process.env.QINAEGIS_CAPTURE_BROWSER_EXECUTABLE });
    const context: BrowserContext = await browser.newContext();
    const page: Page = await context.newPage();
    const capture: Capture = new Capture(capturePolicy({ ...policy, mode: 'full', include_sensitive: true, max_artifact_bytes: 1024, max_run_bytes: 1024, max_events: 1 }), page, context, boundedTrace);
    await capture.start();
    await page.goto(`http://127.0.0.1:${address.port}/`);
    await page.evaluate(() => { console.log('first'); console.log('second'); });
    await new Promise(r => setTimeout(r, 50));
    const bounded = await capture.finish(false);
    assert.equal(bounded.status, 'partial');
    assert.equal(bounded.channels.find(c => c.kind === 'trace')!.status, 'truncated');
    assert.match(bounded.channels.find(c => c.kind === 'trace')!.reason!, /active trace temporary byte limit/);
    assert.ok(bounded.artifacts.reduce((n, a) => n + a.bytes, 0) <= 1024);
    await context.close();
  }
  finally {
    await browser?.close();
    server.close();
    process.chdir(old);
    delete process.env.QINAEGIS_CAPTURE_TEST_API_KEY;
    await rm(root, { recursive: true, force: true });
  }
});
