// sandbox/src/locust_runner.ts
import { spawn } from 'child_process';

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
  return new Promise((resolve, reject) => {
    const args = [
      '-f', 'locustfile.py',
      '--headless',
      '--users', users.toString(),
      '--spawn-rate', spawnRate.toString(),
      '--run-time', `${durationSeconds}s`,
      '--target-url', targetUrl,
      '--html', `/tmp/locust_report_${Date.now()}.html`,
    ];

    let stderr = '';
    const locust = spawn('locust', args, { cwd: 'sandbox' });

    locust.stderr.on('data', (data) => {
      stderr += data.toString();
    });

    let stdout = '';
    locust.stdout.on('data', (data) => {
      stdout += data.toString();
    });

    locust.on('close', (code) => {
      if (code !== 0 && !stdout.includes('Running Locust')) {
        reject(new Error(`Locust failed: ${stderr}`));
        return;
      }

      const stats = parseLocustStats(stdout);
      const errors = parseLocustErrors(stderr);

      resolve({
        target_url: targetUrl,
        stats,
        timestamp: new Date().toISOString(),
        errors,
      });
    });

    setTimeout(() => {
      locust.kill();
      const stats = parseLocustStats(stdout);
      resolve({
        target_url: targetUrl,
        stats,
        timestamp: new Date().toISOString(),
        errors: ['Process terminated due to timeout'],
      });
    }, (durationSeconds + 30) * 1000);
  });
}

function parseLocustStats(output: string): LocustStats {
  const match = output.match(/Aggregated\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+([\d.]+)/);
  if (!match) {
    return {
      total_requests: 0,
      total_failures: 0,
      median_response_time: 0,
      avg_response_time: 0,
      p95_response_time: 0,
      p99_response_time: 0,
      rps: 0,
      duration: 0,
    };
  }

  return {
    total_requests: parseInt(match[1]),
    total_failures: parseInt(match[2]),
    median_response_time: parseInt(match[3]),
    avg_response_time: parseInt(match[4]),
    p95_response_time: parseInt(match[5]),
    p99_response_time: 0,
    rps: parseFloat(match[6]),
    duration: 0,
  };
}

function parseLocustErrors(stderr: string): string[] {
  const errors: string[] = [];
  const lines = stderr.split('\n');
  for (const line of lines) {
    if (line.includes('ERROR')) {
      errors.push(line);
    }
  }
  return errors;
}
