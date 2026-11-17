// sandbox/src/locust_runner.ts
import { spawn } from 'child_process';
import { fileURLToPath } from 'node:url';
import { readFile, stat, unlink } from 'node:fs/promises';

export interface LocustStats {
  total_requests: number;
  total_failures: number;
  median_response_time: number;
  avg_response_time: number;
  p95_response_time: number;
  p99_response_time: number;
  rps: number;
  duration: number;
}

export interface LocustResult {
  target_url: string;
  stats: LocustStats;
  timestamp: string;
  errors: string[];
}

export async function runLocust(
  targetUrl: string,
  users: number,
  spawnRate: number,
  durationSeconds: number,
): Promise<LocustResult> {
  const url = new URL(targetUrl);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw new Error('Invalid load-test URL');
  for (const [value, limit] of [[users, 1000], [spawnRate, 100], [durationSeconds, 600]]) {
    if (!Number.isInteger(value) || value < 1 || value > limit) throw new Error('Load-test limits exceeded');
  }
  const prefix = `${process.cwd()}/locust-metrics`;
  await unlink(`${prefix}_stats.csv`).catch(error => { if (error.code !== 'ENOENT') throw error; });
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const args = ['-f', fileURLToPath(new URL('../locustfile.py', import.meta.url)), '--headless',
      '--users', String(users), '--spawn-rate', String(spawnRate), '--run-time', `${durationSeconds}s`,
      '--host', targetUrl, '--csv', prefix, '--html', `${process.cwd()}/locust-report.html`];
    const child = spawn('locust', args, { cwd: process.cwd() });
    let stderr = '';
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error('Locust timed out; no verified metrics'));
    }, (durationSeconds + 30) * 1000);
    child.stdout.resume();
    child.stderr.on('data', data => { stderr = (stderr + data.toString()).slice(-65536); });
    child.on('error', () => { clearTimeout(timer); reject(new Error('Locust is unavailable; install its compatible runtime explicitly')); });
    child.on('close', async code => {
      clearTimeout(timer);
      if (code !== 0) { reject(new Error('Locust execution failed; no verified metrics')); return; }
      try {
        const path = `${prefix}_stats.csv`;
        if ((await stat(path)).size > 1024 * 1024) throw new Error('Locust metrics exceed limit');
        const stats = parseLocustStats(await readFile(path, 'utf8'), (Date.now() - started) / 1000);
        resolve({ target_url: targetUrl, stats, timestamp: new Date().toISOString(), errors: stderr.split('\n').filter(line => line.includes('ERROR')) });
      } catch { reject(new Error('Locust required metrics missing or invalid')); }
    });
  });
}

// Locust's documented CSV stats are authoritative; console layouts are not.
export function parseLocustStats(csv: string, duration: number): LocustStats {
  const rows: string[][] = [];
  let row: string[] = [], field = '', quoted = false;
  for (let i = 0; i < csv.length; i++) {
    const char = csv[i];
    if (char === '"') {
      if (quoted && csv[i + 1] === '"') { field += '"'; i++; } else quoted = !quoted;
    } else if (!quoted && (char === ',' || char === '\n')) {
      row.push(field.replace(/\r$/, '')); field = '';
      if (char === '\n') { rows.push(row); row = []; }
    } else field += char;
  }
  if (quoted) throw new Error('Invalid CSV');
  if (field || row.length) { row.push(field.replace(/\r$/, '')); rows.push(row); }
  const headers = rows.shift();
  if (!headers || new Set(headers).size !== headers.length) throw new Error('Invalid CSV header');
  const totals = rows.filter(row => row[headers.indexOf('Name')] === 'Aggregated');
  if (totals.length !== 1) throw new Error('Missing aggregate metrics');
  const number = (name: string): number => {
    const raw = totals[0][headers.indexOf(name)];
    if (raw === undefined || raw.trim() === '') throw new Error('Missing metric');
    const value = Number(raw);
    if (!Number.isFinite(value) || value < 0) throw new Error('Invalid metric');
    return value;
  };
  const stats = {
    total_requests: number('Request Count'), total_failures: number('Failure Count'),
    median_response_time: number('Median Response Time'), avg_response_time: number('Average Response Time'),
    p95_response_time: number('95%'), p99_response_time: number('99%'), rps: number('Requests/s'), duration,
  };
  if (!Number.isInteger(stats.total_requests) || stats.total_requests < 1 || !Number.isInteger(stats.total_failures)
      || stats.total_failures > stats.total_requests || !Number.isFinite(duration) || duration <= 0) throw new Error('Invalid measurement');
  return stats;
}
