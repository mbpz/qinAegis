// sandbox/src/lighthouse_runner.ts
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

function validateUrl(url: string): void {
  try {
    const parsed = new URL(url);
    if (!['http:', 'https:'].includes(parsed.protocol)) {
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

  // Run lighthouse CI with JSON output
  const cmd = `npx @lhci/cli autorun --url=${url} --output-json=${outputPath} --temporary-public-storage=true`;

  let stderr = '';
  try {
    const { stdout, stderr: stderrOutput } = await execAsync(cmd, {
      cwd: process.cwd(),
      timeout: 120000,
    });
    stderr = stderrOutput;

    // Parse JSON output
    const output = JSON.parse(stdout);

    const metrics: LighthouseMetrics = {
      performance: output.categories?.performance?.score ?? 0,
      accessibility: output.categories?.accessibility?.score ?? 0,
      bestPractices: output.categories?.['best-practices']?.score ?? 0,
      seo: output.categories?.seo?.score ?? 0,
      firstContentfulPaint: output.audits?.['first-contentful-paint']?.numericValue ?? 0,
      largestContentfulPaint: output.audits?.['largest-contentful-paint']?.numericValue ?? 0,
      cumulativeLayoutShift: output.audits?.['cumulative-layout-shift']?.numericValue ?? 0,
      totalBlockingTime: output.audits?.['total-blocking-time']?.numericValue ?? 0,
      speedIndex: output.audits?.['speed-index']?.numericValue ?? 0,
      ttfb: output.audits?.['server-response-time']?.numericValue ?? 0,
    };

    return {
      url,
      score: metrics.performance,
      metrics,
      timestamp,
      report_path: outputPath,
    };
  } catch (e) {
    throw new Error(`Lighthouse failed: ${stderr || e}`);
  }
}