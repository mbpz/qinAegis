// sandbox/src/zap_runner.ts
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

export interface ZapAlert {
  name: string;
  risk: string;
  confidence: string;
  url: string;
  description: string;
  solution: string;
}

export interface ZapResult {
  target_url: string;
  scan_type: string;
  alert_count: number;
  high_risk: number;
  medium_risk: number;
  low_risk: number;
  informational_risk: number;
  alerts: ZapAlert[];
  report_path: string | null;
  timestamp: string;
}

export async function runZapScan(
  targetUrl: string,
  outputPath: string,
): Promise<ZapResult> {
  validateUrl(targetUrl);

  const timestamp = new Date().toISOString();
  const scanType = 'baseline';

  // Use zap-baseline.py (passive scan, safe)
  const cmd = `docker run --rm owasp/zap2docker-stable zap-baseline.py -t ${targetUrl} -J ${outputPath} -l WARN`;

  let stderr = '';
  try {
    const { stderr: stderrOutput } = await execAsync(cmd, {
      timeout: 300000,
    });
    stderr = stderrOutput;
  } catch (e) {
    // ZAP exits with non-zero on warnings, check if report was generated
  }

  // Parse ZAP JSON report if it exists
  let alerts: ZapAlert[] = [];
  let highRisk = 0;
  let mediumRisk = 0;
  let lowRisk = 0;
  let informationalRisk = 0;

  try {
    const content = await import('fs').then(fs => fs.promises.readFile(outputPath, 'utf8'));
    const report = JSON.parse(content);
    const siteAlerts = report.site?.[0]?.alerts || [];

    for (const alert of siteAlerts) {
      const risk = alert.riskdesc?.toLowerCase() || 'informational';
      if (risk.includes('high')) highRisk++;
      else if (risk.includes('medium')) mediumRisk++;
      else if (risk.includes('low')) lowRisk++;
      else informationalRisk++;

      alerts.push({
        name: alert.name || 'Unknown',
        risk: alert.risk || 'Informational',
        confidence: alert.confidence || 'Medium',
        url: alert.url || targetUrl,
        description: alert.desc || '',
        solution: alert.solution || '',
      });
    }
  } catch {
    // Report may not exist if scan failed
  }

  return {
    target_url: targetUrl,
    scan_type: scanType,
    alert_count: alerts.length,
    high_risk: highRisk,
    medium_risk: mediumRisk,
    low_risk: lowRisk,
    informational_risk: informationalRisk,
    alerts,
    report_path: outputPath,
    timestamp,
  };
}