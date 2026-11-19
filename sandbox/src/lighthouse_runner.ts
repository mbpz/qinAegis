// sandbox/src/lighthouse_runner.ts
import { execFile } from 'child_process';
import { readFile, stat, unlink } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { promisify } from 'util';

const execAsync = promisify(execFile);

function validateUrl(url: string): void {
  try {
    const parsed = new URL(url);
    if (!['http:', 'https:'].includes(parsed.protocol) || parsed.username || parsed.password) {
      throw new Error('Only http/https URLs are allowed');
    }
  } catch {
    throw new Error(`Invalid URL: ${url}`);
  }
}

export interface LighthouseMetrics {
  performance: number;
  accessibility: number;
  bestPractices: number;
  seo: number;
  firstContentfulPaint: number;
  largestContentfulPaint: number;
  cumulativeLayoutShift: number;
  totalBlockingTime: number;
  speedIndex: number;
  ttfb: number;
}

export interface LighthouseResult {
  url: string;
  score: number;
  metrics: LighthouseMetrics;
  timestamp: string;
  report_path: string | null;
}

export async function runLighthouse(
  url: string,
  outputPath: string,
): Promise<LighthouseResult> {
  validateUrl(url);

  const timestamp = new Date().toISOString();

  // Only a preinstalled CLI: no downloads or public report uploads.
  const path = resolve(outputPath);
  if (dirname(path) !== process.cwd()) throw new Error('Report must be inside the owned workspace');
  await unlink(path).catch(error => { if (error.code !== 'ENOENT') throw error; });
  try {
    await execAsync('lighthouse', [url, '--output=json', `--output-path=${path}`, '--chrome-flags=--headless'], {
      cwd: process.cwd(), timeout: 120000, maxBuffer: 1024 * 1024,
    });
    if ((await stat(path)).size > 8 * 1024 * 1024) throw new Error('Lighthouse report exceeds limit');
    const output = JSON.parse(await readFile(path, 'utf8'));
    if (output.runtimeError) throw new Error('Lighthouse did not complete measurement');
    const number = (value: unknown, score = false): number => {
      if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || (score && value > 1)) {
        throw new Error('Lighthouse required metrics missing or invalid');
      }
      return value;
    };
    const metrics: LighthouseMetrics = {
      performance: number(output.categories?.performance?.score, true),
      accessibility: number(output.categories?.accessibility?.score, true),
      bestPractices: number(output.categories?.['best-practices']?.score, true),
      seo: number(output.categories?.seo?.score, true),
      firstContentfulPaint: number(output.audits?.['first-contentful-paint']?.numericValue),
      largestContentfulPaint: number(output.audits?.['largest-contentful-paint']?.numericValue),
      cumulativeLayoutShift: number(output.audits?.['cumulative-layout-shift']?.numericValue),
      totalBlockingTime: number(output.audits?.['total-blocking-time']?.numericValue),
      speedIndex: number(output.audits?.['speed-index']?.numericValue),
      ttfb: number(output.audits?.['server-response-time']?.numericValue),
    };

    return {
      url,
      score: metrics.performance,
      metrics,
      timestamp,
      report_path: outputPath,
    };
  } catch (e) {
    throw new Error(`Lighthouse unavailable or failed: ${e}`);
  }
}
