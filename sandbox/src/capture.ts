import type { Page, BrowserContext } from 'playwright';
import { randomBytes, createHash } from 'node:crypto';
import { open, unlink, readdir, stat, chmod } from 'node:fs/promises';
import { join } from 'node:path';
export interface CapturePolicy {
  schema_version: 1;
  mode: 'off' | 'failure_only' | 'key_steps' | 'full';
  max_artifact_bytes: number;
  max_run_bytes: number;
  max_events: number;
  include_sensitive: boolean;
}
export const captureCapabilities = { protocol_version: 1, modes: ['off', 'failure_only', 'key_steps', 'full'], kinds: ['screenshot', 'trace', 'console', 'network', 'steps'], artifact_transport: 'owned_files_v1' };
export function capturePolicy(value: unknown): CapturePolicy {
  const p = value as CapturePolicy;
  if (!p || typeof p !== 'object' || Array.isArray(p) || Object.keys(p).sort().join(',') !== 'include_sensitive,max_artifact_bytes,max_events,max_run_bytes,mode,schema_version' || p.schema_version !== 1 || !captureCapabilities.modes.includes(p.mode) || !Number.isInteger(p.max_artifact_bytes) || p.max_artifact_bytes < 1024 || p.max_artifact_bytes > 8388608 || !Number.isInteger(p.max_run_bytes) || p.max_run_bytes < p.max_artifact_bytes || p.max_run_bytes > 33554432 || !Number.isInteger(p.max_events) || p.max_events < 1 || p.max_events > 2000 || typeof p.include_sensitive !== 'boolean' || (p.mode !== 'off' && !p.include_sensitive))
    throw new Error('Invalid capture policy or missing sensitive-data authorization');
  return { ...p };
}
type Kind = 'screenshot' | 'trace' | 'console' | 'network' | 'steps';
type Channel = {
  kind: Kind;
  status: 'not_requested' | 'collected' | 'failed' | 'truncated' | 'unsupported' | 'unknown';
  reason: string | null;
  bytes: number;
};
export class Capture {
  private channels: Channel[] = (['screenshot', 'trace', 'console', 'network', 'steps'] as Kind[]).map(kind => ({ kind, status: 'not_requested', reason: null, bytes: 0 }));
  private artifacts: {
    id: string;
    kind: Kind;
    file_name: string;
    bytes: number;
    sha256: string;
  }[] = [];
  private events: Record<'console' | 'network' | 'steps', unknown[]> = { console: [], network: [], steps: [] };
  private eventBytes = { console: 0, network: 0, steps: 0 };
  private started = performance.now();
  private count = 0;
  private total = 0;
  private timer: ReturnType<typeof setInterval> | undefined;
  private traceStop: Promise<void> | undefined;
  private traceActive = false;
  private checking = false;
  private secrets = Object.entries(process.env).filter(([k, v]) => v && /(API_KEY|TOKEN|SECRET|PASSWORD)/i.test(k)).map(([, v]) => v!);
  private safe(text: string) { let s = text; for (const key of this.secrets)
    s = s.split(key).join('[redacted]'); return s.slice(0, 4096); }
  private consoleListener = (msg: any) => this.event('console', { time_ms: this.time(), type: String(msg.type()).slice(0, 32), text: this.safe(msg.text()) });
  private networkListener = (response: any) => { try {
    const u = new URL(response.url());
    if (!['http:', 'https:'].includes(u.protocol))
      return;
    u.username = '';
    u.password = '';
    u.search = '';
    u.hash = '';
    this.event('network', { time_ms: this.time(), method: response.request().method().slice(0, 16), status: response.status(), resource_type: response.request().resourceType().slice(0, 32), url: this.safe(u.toString()) });
  }
  catch {
    this.mark('network', 'failed', 'network metadata unavailable');
  } };
  // This is bounded request metadata, not a HAR: no headers, bodies, or failure text.
  private failedRequestListener = (request: any) => { try {
    const u = new URL(request.url());
    if (!['http:', 'https:'].includes(u.protocol)) return;
    u.username = ''; u.password = ''; u.search = ''; u.hash = '';
    this.event('network', { time_ms: this.time(), method: request.method().slice(0, 16), status: null, failed: true, resource_type: request.resourceType().slice(0, 32), url: this.safe(u.toString()) });
  } catch { this.mark('network', 'failed', 'network metadata unavailable'); } };
  private time() { return Math.max(0, Math.round(performance.now() - this.started)); }
  constructor(readonly policy: CapturePolicy, private page: Page, private context: BrowserContext, private traceDir: string) { }
  private mark(kind: Kind, status: Channel['status'], reason: string | null = null) { const c = this.channels.find(c => c.kind === kind)!; c.status = status; c.reason = reason; }
  private event(kind: 'console' | 'network' | 'steps', value: unknown) { const size = Buffer.byteLength(JSON.stringify(value)); if (this.count >= this.policy.max_events || this.eventBytes[kind] + size + 2 > this.policy.max_artifact_bytes) {
    this.mark(kind, 'truncated', 'event or byte limit');
    return;
  } this.count++; this.eventBytes[kind] += size + 1; this.events[kind].push(value); }
  async start() {
    if (this.policy.mode === 'off')
      return;
    for (const k of ['screenshot', 'console', 'network', 'steps'] as Kind[])
      this.mark(k, 'collected');
    this.page.on('console', this.consoleListener);
    this.page.on('response', this.networkListener);
    this.page.on('requestfailed', this.failedRequestListener);
    if (this.policy.mode === 'full') {
      try {
        await this.context.tracing.start({ screenshots: true, snapshots: true, sources: false });
        this.traceActive = true;
        this.mark('trace', 'collected', 'Active temporary monitoring is best effort; retained artifacts have hard byte limits.');
        this.timer = setInterval(() => { void this.checkTrace(); }, 10);
      }
      catch {
        this.mark('trace', 'failed', 'trace initialization failed');
      }
    }
  }
  private async checkTrace() { if (this.checking || !this.traceActive)
    return; this.checking = true; try {
    const size = await this.directoryBytes(this.traceDir);
    if (size >= Math.min(this.policy.max_artifact_bytes, this.policy.max_run_bytes - this.total) / 2) {
      this.mark('trace', 'truncated', 'active trace temporary byte limit; temporary monitoring is best effort, retained artifacts have hard byte limits');
      this.traceActive = false;
      this.traceStop = this.context.tracing.stop().catch(() => { });
    }
  }
  catch {
    this.mark('trace', 'failed', 'trace monitoring failed');
    this.traceActive = false;
    this.traceStop = this.context.tracing.stop().catch(() => { });
  }
  finally {
    this.checking = false;
  } }
  private async directoryBytes(path: string): Promise<number> { let bytes = 0; for (const entry of await readdir(path, { withFileTypes: true })) {
    const p = join(path, entry.name);
    if (entry.isDirectory())
      bytes += await this.directoryBytes(p);
    else if (entry.isFile())
      bytes += (await stat(p)).size;
  } return bytes; }
  async step(index: number, action: string, status: string, started: number) { if (this.policy.mode === 'off')
    return; this.event('steps', { index, action, status, duration_ms: Math.max(0, Math.round(performance.now() - started)) }); if (this.policy.mode === 'key_steps' || this.policy.mode === 'full')
    await this.screenshot(); }
  private async artifact(kind: Kind, bytes: Buffer, extension: string) { if (bytes.length > this.policy.max_artifact_bytes || bytes.length > this.policy.max_run_bytes - this.total || this.artifacts.length >= this.policy.max_events) {
    this.mark(kind, 'truncated', 'artifact or total byte limit');
    return;
  } const name = `qinaegis-capture-${randomBytes(16).toString('hex')}.${extension}`; let owned = false; try {
    const file = await open(name, 'wx', 0o600);
    owned = true;
    try {
      await file.writeFile(bytes);
    }
    finally {
      await file.close();
    }
    this.total += bytes.length;
    this.channels.find(c => c.kind === kind)!.bytes += bytes.length;
    this.artifacts.push({ id: `artifact-${this.artifacts.length + 1}`, kind, file_name: name, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') });
  }
  catch {
    if (owned)
      await unlink(name).catch(() => { });
    this.mark(kind, 'failed', 'artifact persistence failed');
  } }
  private async screenshot() { try {
    await this.artifact('screenshot', await this.page.screenshot({ timeout: 5000 }), 'png');
  }
  catch {
    this.mark('screenshot', 'failed', 'screenshot unavailable');
  } }
  async finish(passed: boolean) {
    if (this.timer)
      clearInterval(this.timer);
    this.page.off('console', this.consoleListener);
    this.page.off('response', this.networkListener);
    this.page.off('requestfailed', this.failedRequestListener);
    const keep = this.policy.mode !== 'off' && (this.policy.mode !== 'failure_only' || !passed);
    if (keep) {
      if (this.policy.mode === 'failure_only' || (!passed && !this.artifacts.some(a => a.kind === 'screenshot')))
        await this.screenshot();
      for (const kind of ['console', 'network', 'steps'] as const)
        await this.artifact(kind, Buffer.from(JSON.stringify(this.events[kind])), 'json');
    }
    if (this.policy.mode === 'full') {
      while (this.checking)
        await new Promise(r => setTimeout(r, 1));
      await this.checkTrace();
      if (this.traceStop)
        await this.traceStop;
      else if (this.traceActive) {
        const name = join(this.traceDir, `qinaegis-capture-${randomBytes(16).toString('hex')}.zip`);
        try {
          await this.context.tracing.stop({ path: name });
          this.traceActive = false;
          const { readFile } = await import('node:fs/promises');
          const info = await stat(name);
          if (info.size > this.policy.max_artifact_bytes || info.size > this.policy.max_run_bytes - this.total)
            this.mark('trace', 'truncated', 'trace archive byte limit');
          else {
            await chmod(name, 0o600);
            await this.artifact('trace', await readFile(name), 'zip');
          }
        }
        catch {
          this.mark('trace', 'failed', 'trace finalization failed');
        }
        finally {
          await unlink(name).catch(() => { });
        }
      }
    }
    if (!keep) {
      for (const c of this.channels) {
        c.status = 'not_requested';
        c.reason = this.policy.mode === 'failure_only' ? 'successful case: failure-only evidence discarded' : null;
        c.bytes = 0;
      }
    }
    return { schema_version: 1, policy: this.policy, status: this.policy.mode === 'off' ? 'disabled' : this.channels.some(c => ['failed', 'truncated', 'unsupported', 'unknown'].includes(c.status)) ? 'partial' : 'complete', channels: this.channels, artifacts: this.artifacts };
  }
}
