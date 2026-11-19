import test from 'node:test';
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { createServer } from 'node:http';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { startTraceViewer, stopTraceViewer } from './trace_viewer.js';

test('official offline viewer loads real action timeline and DOM snapshots without replay', { timeout: 45000 }, async () => {
  const root = await mkdtemp(join(tmpdir(), 'qinaegis-viewer-browser-'));
  let visits = 0;
  const fixture = createServer((_, response) => {
    visits++;
    response.setHeader('Content-Type', 'text/html');
    response.end('<title>Owned trace fixture</title><button id="record" onclick="document.querySelector(\'#result\').textContent=\'Recorded result\'">Record action</button><p id="result">Before action</p>');
  });
  await new Promise<void>(resolve => fixture.listen(0, '127.0.0.1', resolve));
  const fixtureOrigin = `http://127.0.0.1:${(fixture.address() as { port: number }).port}`;
  const browser = await chromium.launch({ headless: true, executablePath: process.env.QINAEGIS_CAPTURE_BROWSER_EXECUTABLE });
  try {
    const recording = await browser.newContext();
    await recording.tracing.start({ screenshots: true, snapshots: true, sources: false });
    const subject = await recording.newPage();
    await subject.goto(fixtureOrigin);
    await subject.getByRole('button', { name: 'Record action' }).click();
    await subject.locator('#result').waitFor({ state: 'visible' });
    assert.equal(await subject.locator('#result').innerText(), 'Recorded result');
    const archive = join(root, 'trace.zip');
    await recording.tracing.stop({ path: archive });
    await recording.close();
    const beforeViewing = visits;
    await new Promise<void>(resolve => fixture.close(() => resolve()));
    const opened = await startTraceViewer((await readFile(archive)).toString('base64'));
    const origin = new URL(opened.url).origin;
    const context = await browser.newContext();
    const external: string[] = [];
    const errors: string[] = [];
    const failedResponses: string[] = [];
    context.on('request', request => {
      if (/^https?:/.test(request.url()) && new URL(request.url()).origin !== origin) external.push(request.url());
    });
    await context.route('**/*', route => {
      const url = new URL(route.request().url());
      return url.origin === origin || ['data:', 'blob:'].includes(url.protocol) ? route.continue() : route.abort();
    });
    const page = await context.newPage();
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    page.on('response', response => { if (response.status() >= 400) failedResponses.push(`${response.status()} ${response.url()}`); });
    await page.goto(opened.url);
    await page.getByText('Click', { exact: true }).waitFor({ timeout: 15000 });
    await page.getByText('Click', { exact: true }).click();
    // Debug-only failure context will identify the pinned official DOM if it changes.
    const body = await page.locator('body').innerText();
    assert.match(body, /Actions/);
    assert.equal(await page.locator('.timeline-view').isVisible(), true);
    assert.ok(await page.locator('.timeline-time').count() > 0);
    assert.match(body, /Record action/);
    assert.ok(await page.locator('iframe').count() > 0, body);
    const snapshots = await Promise.all(page.frames().map(async frame => {
      try { return await frame.locator('body').innerText({ timeout: 1000 }); } catch { return ''; }
    }));
    assert.ok(snapshots.some(text => text.includes('Recorded result')), snapshots.join('\n'));
    assert.equal(await page.evaluate(async () => !!(await navigator.serviceWorker.ready).active), true);
    assert.deepEqual(external, []);
    assert.deepEqual(errors, []);
    assert.deepEqual(failedResponses, []);
    assert.equal(visits, beforeViewing, 'viewing must never replay the recorded navigation or action');
    await context.close();
    await stopTraceViewer();
    await assert.rejects(() => fetch(opened.url));
  } finally {
    await stopTraceViewer();
    await browser.close();
    fixture.close();
    await rm(root, { recursive: true, force: true });
  }
});


test('viewer cannot leak listeners through concurrent startup or stop during startup', async () => {
  const bytes = Buffer.from([0x50, 0x4b, 0x03, 0x04, 1, 2, 3, 4]).toString('base64');
  try {
    const first = startTraceViewer(bytes);
    await assert.rejects(() => startTraceViewer(bytes), /already active/);
    const opened = await first;
    await stopTraceViewer();
    await assert.rejects(() => fetch(opened.url));
    const pending = startTraceViewer(bytes);
    const stopped = assert.rejects(pending, /startup stopped/);
    await stopTraceViewer();
    await stopped;
    // Both lifecycle operations leave the single-viewer slot reusable.
    const fresh = await startTraceViewer(bytes);
    assert.equal((await fetch(fresh.url)).status, 200);
  } finally { await stopTraceViewer(); }
});
