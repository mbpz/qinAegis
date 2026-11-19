/** Offline official viewer assets, without Playwright's arbitrary-path server. */
import { createServer, type Server } from 'node:http';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { lstat, readdir, readFile } from 'node:fs/promises';
import { randomBytes } from 'node:crypto';

const TTL = 900;
const VERSION = '1.59.1';
let server: Server | undefined;
let starting = false;
let generation = 0;
let expiration: ReturnType<typeof setTimeout> | undefined;
const mime: Record<string, string> = { html: 'text/html; charset=utf-8', js: 'text/javascript; charset=utf-8', css: 'text/css; charset=utf-8', svg: 'image/svg+xml', ttf: 'font/ttf', png: 'image/png', webmanifest: 'application/manifest+json' };

async function assets(): Promise<Map<string, Buffer>> {
  const require = createRequire(import.meta.url);
  const pwRequire = createRequire(require.resolve('playwright/package.json'));
  const packagePath = pwRequire.resolve('playwright-core/package.json');
  if (JSON.parse(await readFile(packagePath, 'utf8')).version !== VERSION) throw new Error('Unsupported offline Trace viewer version');
  const root = join(dirname(packagePath), 'lib/vite/traceViewer');
  const files = new Map<string, Buffer>();
  let total = 0;
  async function visit(path: string, prefix: string) {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      if (!/^[a-zA-Z0-9_.-]+$/.test(entry.name) || entry.isSymbolicLink()) throw new Error('Unsafe viewer asset');
      const target = join(path, entry.name), name = prefix + entry.name;
      const metadata = await lstat(target);
      if (metadata.isDirectory()) await visit(target, name + '/');
      else {
        if (!metadata.isFile() || metadata.size > 8 * 1024 * 1024 || files.size >= 256) throw new Error('Unsupported viewer asset');
        total += metadata.size;
        if (total > 24 * 1024 * 1024) throw new Error('Viewer assets exceed budget');
        const bytes = await readFile(target);
        if (bytes.length !== metadata.size) throw new Error('Viewer asset changed');
        files.set(name, bytes);
      }
    }
  }
  // Runtime dependency installation can legitimately use hardlinked package files;
  // these are trusted distribution assets, never evidence or request-selected files.
  await visit(root, '');
  if (!files.has('index.html') || !files.has('sw.bundle.js')) throw new Error('Offline Trace viewer assets missing');
  return files;
}

export async function stopTraceViewer(): Promise<void> {
  generation++;
  if (expiration) clearTimeout(expiration);
  expiration = undefined;
  const previous = server;
  server = undefined;
  if (previous) await new Promise<void>(resolve => { previous.close(() => resolve()); previous.closeAllConnections(); });
}

export async function startTraceViewer(traceBase64: string): Promise<{ url: string; expires_in_seconds: number; version: string }> {
  if (server || starting) throw new Error('Trace viewer already active');
  starting = true;
  const admittedGeneration = generation;
  try {
    if (typeof traceBase64 !== 'string' || traceBase64.length > Math.ceil(8388608 / 3) * 4 || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(traceBase64)) throw new Error('Invalid Trace payload');
    const trace = Buffer.from(traceBase64, 'base64');
    if (trace.length < 4 || trace.length > 8388608 || trace.toString('base64') !== traceBase64 || trace.readUInt32LE(0) !== 0x04034b50) throw new Error('Invalid Trace archive');
    const files = await assets();
    const prefix = '/' + randomBytes(32).toString('hex') + '/';
    let origin = '';
    const current = createServer((request, response) => {
      const reject = () => { response.writeHead(404); response.end(); };
      if (!['GET', 'HEAD'].includes(request.method ?? '') || request.headers.host !== origin.slice(7) || (request.headers.origin && request.headers.origin !== origin) || request.headers['sec-fetch-site'] === 'cross-site') return reject();
      let url: URL;
      try { url = new URL(request.url ?? '', origin); } catch { return reject(); }
      if (!url.pathname.startsWith(prefix) || /%|\\|\.\./.test(url.pathname)) return reject();
      const name = url.pathname.slice(prefix.length);
      const isTrace = name === 'trace.zip';
      // The server maps an immutable in-memory ZIP and enumerated bundled assets.
      // There is no file/path endpoint, proxy, upload, arbitrary trace or RPC route.
      if (url.search && !(name === 'index.html' && url.searchParams.size === 1 && url.searchParams.get('trace') === origin + prefix + 'trace.zip')) return reject();
      const bytes = isTrace ? trace : files.get(name);
      if (!bytes) return reject();
      const headers: Record<string, string | number> = {
        'Content-Type': isTrace ? 'application/zip' : mime[name.split('.').pop() ?? ''] ?? 'application/octet-stream',
        'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff', 'Referrer-Policy': 'no-referrer',
        'X-Frame-Options': 'SAMEORIGIN',
        'Content-Security-Policy': "default-src 'none'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' data: blob:; worker-src 'self' blob:; frame-src 'self' data: blob:; frame-ancestors 'self'; base-uri 'none'; form-action 'none'",
      };
      let start = 0, end = bytes.length - 1, status = 200;
      if (request.headers.range) {
        const match = /^bytes=(\d+)-(\d*)$/.exec(request.headers.range);
        if (!isTrace || !match) { response.writeHead(416); response.end(); return; }
        start = Number(match[1]); end = match[2] ? Number(match[2]) : end;
        if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end < start || end >= bytes.length) { response.writeHead(416); response.end(); return; }
        status = 206; headers['Content-Range'] = `bytes ${start}-${end}/${bytes.length}`;
      }
      headers['Content-Length'] = end - start + 1;
      if (isTrace) headers['Accept-Ranges'] = 'bytes';
      response.writeHead(status, headers);
      response.end(request.method === 'HEAD' ? undefined : bytes.subarray(start, end + 1));
    });
    await new Promise<void>((resolve, reject) => { current.once('error', reject); current.listen(0, '127.0.0.1', resolve); });
    const address = current.address();
    if (!address || typeof address === 'string') { current.close(); throw new Error('Trace listener unavailable'); }
    origin = `http://127.0.0.1:${address.port}`;
    if (admittedGeneration !== generation) {
      await new Promise<void>(resolve => current.close(() => resolve()));
      throw new Error('Trace viewer startup stopped');
    }
    server = current;
    expiration = setTimeout(() => { void stopTraceViewer().finally(() => process.exit(0)); }, TTL * 1000);
    return { url: origin + prefix + 'index.html?trace=' + encodeURIComponent(origin + prefix + 'trace.zip'), expires_in_seconds: TTL, version: VERSION };
  } finally { starting = false; }
}
