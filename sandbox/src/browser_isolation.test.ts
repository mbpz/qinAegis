import test from 'node:test';
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:net';
import { createServer as createHttpServer } from 'node:http';

// Opt-in actual-browser acceptance, without a model or external network target.
test('engines ignore inherited CDP and retain independent browsers', { timeout: 60_000 }, async () => {
  const reservation = createServer();
  await new Promise<void>(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = (reservation.address() as { port: number }).port;
  await new Promise<void>((resolve, reject) => reservation.close(error => error ? reject(error) : resolve()));
  const endpoint = `http://127.0.0.1:${port}`;
  const external = await chromium.launch({ headless: true, args: [`--remote-debugging-port=${port}`] });
  const site = createHttpServer((req, res) => {
    if (req.url?.startsWith('/set/')) res.setHeader('Set-Cookie', `session=${req.url.slice(5)}; Path=/`);
    if (req.url === '/check') {
      res.writeHead(302, { Location: `/session/${req.headers.cookie ?? 'empty'}` });
      res.end();
      return;
    }
    res.end('isolated browser fixture');
  });
  const workers: ReturnType<typeof spawn>[] = [];
  const dirs: string[] = [];
  try {
    assert.equal((await fetch(`${endpoint}/json/version`)).ok, true);
    const externalPage = await external.newPage();
    await externalPage.goto('data:text/html,external');
    await new Promise<void>(resolve => site.listen(0, '127.0.0.1', resolve));
    const siteUrl = `http://127.0.0.1:${(site.address() as { port: number }).port}`;
    async function engine() {
      const cwd = await mkdtemp(join(tmpdir(), 'qinaegis-isolation-test-'));
      dirs.push(cwd);
      const child = spawn(process.env.QINAEGIS_TEST_NODE || process.execPath, [process.env.QINAEGIS_TEST_EXECUTOR_ENTRY || fileURLToPath(new URL('./executor.js', import.meta.url))], {
        cwd, env: { ...process.env, CDP_WS_URL: endpoint, CDP_PORT: String(port) }, stdio: ['pipe', 'pipe', 'pipe'],
      });
      workers.push(child);
      child.stderr!.resume();
      const lines = createInterface({ input: child.stdout! })[Symbol.asyncIterator]();
      return async (method: string, args: unknown = {}) => {
        child.stdin!.write(JSON.stringify({ id: 'test', method, args }) + '\n');
        const line = await lines.next();
        assert.equal(line.done, false, 'executor exited before replying');
        return JSON.parse(line.value!);
      };
    }
    const [one, two] = await Promise.all([engine(), engine()]);
    assert.equal((await one('goto', { url: 'data:text/html,one' })).ok, true);
    assert.equal((await two('goto', { url: 'data:text/html,two' })).ok, true);
    assert.equal((await one('currentUrl')).data, 'data:text/html,one');
    assert.equal((await two('currentUrl')).data, 'data:text/html,two');
    assert.equal((await one('goto', { url: `${siteUrl}/set/one` })).ok, true);
    assert.equal((await two('goto', { url: `${siteUrl}/set/two` })).ok, true);
    assert.equal((await one('goto', { url: `${siteUrl}/check` })).ok, true);
    assert.equal((await two('goto', { url: `${siteUrl}/check` })).ok, true);
    assert.match((await one('currentUrl')).data, /session=one$/);
    assert.match((await two('currentUrl')).data, /session=two$/);
    assert.equal((await two('goto', { url: 'data:text/html,two' })).ok, true);
    assert.equal((await one('shutdown')).ok, true);
    assert.equal((await two('currentUrl')).data, 'data:text/html,two');
    assert.equal((await two('explore', { url: 'http://127.0.0.1:1', depth: 0 })).ok, true);
    assert.equal((await two('currentUrl')).data, 'data:text/html,two');
    assert.equal((await two('shutdown')).ok, true);
    assert.equal(external.isConnected(), true);
    assert.equal(externalPage.url(), 'data:text/html,external');
  } finally {
    for (const child of workers) if (child.exitCode === null) child.kill();
    await external.close();
    if (site.listening) await new Promise<void>((resolve, reject) => site.close(error => error ? reject(error) : resolve()));
    await Promise.all(dirs.map(dir => rm(dir, { recursive: true, force: true })));
  }
});
