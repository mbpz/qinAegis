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

  // A Docker daemon container outlives a killed executor process. Until the
  // host owns a durable container lease and cancellation path, refuse to scan
  // rather than leak a scan or turn an absent report into zero findings.
  throw new Error('ZAP scanning is unavailable: managed container cancellation is not implemented');
}
