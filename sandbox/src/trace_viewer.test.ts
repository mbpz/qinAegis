import test from 'node:test';
import assert from 'node:assert/strict';
import { startTraceViewer, stopTraceViewer } from './trace_viewer.js';

test('offline viewer serves only token-scoped bundled assets and its immutable ZIP', async () => {
  const bytes = Buffer.from([0x50, 0x4b, 0x03, 0x04, 1, 2, 3, 4]);
  const opened = await startTraceViewer(bytes.toString('base64'));
  const entry = new URL(opened.url), base = new URL('.', entry);
  const trace = new URL(entry.searchParams.get('trace')!);
  try {
    assert.equal(opened.version, '1.59.1');
    assert.equal(opened.expires_in_seconds, 900);
    const html = await fetch(entry);
    assert.equal(html.status, 200);
    assert.match(await html.text(), /<html/);
    assert.match(html.headers.get('content-security-policy')!, /connect-src 'self' data: blob:/);
    const worker = await fetch(new URL('sw.bundle.js', base));
    assert.equal(worker.status, 200);
    assert.match(worker.headers.get('content-type')!, /javascript/);
    assert.deepEqual(Buffer.from(await (await fetch(trace)).arrayBuffer()), bytes);
    const range = await fetch(trace, { headers: { Range: 'bytes=2-5' } });
    assert.equal(range.status, 206);
    assert.equal(range.headers.get('content-range'), 'bytes 2-5/8');
    assert.deepEqual(Buffer.from(await range.arrayBuffer()), bytes.subarray(2, 6));
    for (const url of [new URL('/index.html', base), new URL('file?path=/etc/passwd', base), new URL('trace.zip?path=/etc/passwd', base), new URL('index.html?trace=https://example.invalid/evil.zip', base), new URL('runtime.json', base), new URL('%2e%2e%2fpackage.json', base)]) assert.equal((await fetch(url)).status, 404, String(url));
    assert.equal((await fetch(entry, { headers: { Origin: 'https://example.invalid' } })).status, 404);
    assert.equal((await fetch(entry, { method: 'POST' })).status, 404);
    assert.equal((await fetch(trace, { headers: { Range: 'bytes=0-1000' } })).status, 416);
    await assert.rejects(() => startTraceViewer(bytes.toString('base64')), /already active/);
  } finally { await stopTraceViewer(); }
  await assert.rejects(() => fetch(entry));
});

test('viewer rejects unbounded or non-ZIP payloads before opening a listener', async () => {
  for (const value of ['', 'invalid!!', Buffer.from('not a trace').toString('base64')]) await assert.rejects(() => startTraceViewer(value));
});
