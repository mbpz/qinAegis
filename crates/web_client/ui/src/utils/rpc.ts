// Shared RPC client — single source of truth

import type {
  ConfigStatusResponse,
  StateResponse,
  VersionResponse,
  UpdateCheckResponse,
  Project,
  GateStatus,
  RunReport,
  TestCase,
  TestPlan,
  ReportHtmlResponse,
  ExportResponse,
  AppConfig,
} from '../types/rpc';

const DEFAULT_TIMEOUT = 60000;

export class RpcError extends Error {
  constructor(
    message: string,
    public code?: string,
    public method?: string
  ) {
    super(message);
    this.name = 'RpcError';
  }
}

function parseResponse<T>(text: string, method: string): T {
  try {
    const parsed = JSON.parse(text);
    if (!parsed) {
      throw new RpcError('Empty response', 'EMPTY_RESPONSE', method);
    }
    return parsed as T;
  } catch (e) {
    if (e instanceof RpcError) throw e;
    throw new RpcError(`Invalid JSON: ${(e as Error).message}`, 'PARSE_ERROR', method);
  }
}

async function call<T>(method: string, params: object = {}, timeout: number = DEFAULT_TIMEOUT): Promise<T> {
  const paramsStr = encodeURIComponent(JSON.stringify(params));
  const url = 'app://localhost/invoke?method=' + encodeURIComponent(method) + '&params=' + paramsStr;

  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeout);

  try {
    const resp = await fetch(url, { signal: controller.signal });
    clearTimeout(timeoutId);

    if (!resp.ok) {
      throw new RpcError(`HTTP ${resp.status}: ${resp.statusText}`, 'HTTP_ERROR', method);
    }

    const text = await resp.text();
    const result = parseResponse<T>(text, method);

    // Check for RPC-level error
    if (typeof result === 'object' && result !== null && 'ok' in result && result.ok === false) {
      throw new RpcError((result as { error?: string }).error || 'Unknown error', 'RPC_ERROR', method);
    }

    return result;
  } catch (e) {
    if (e instanceof RpcError) throw e;
    if (e instanceof Error && e.name === 'AbortError') {
      throw new RpcError(`Request timeout after ${timeout}ms`, 'TIMEOUT', method);
    }
    throw new RpcError((e as Error).message, 'UNKNOWN', method);
  }
}

// Typed RPC methods
export const rpc = {
  // Config
  async checkConfig(): Promise<ConfigStatusResponse> {
    return call<ConfigStatusResponse>('checkConfig');
  },

  async getState(): Promise<StateResponse> {
    return call<StateResponse>('getState');
  },

  async setConfig(config: AppConfig): Promise<{ ok: boolean }> {
    return call<{ ok: boolean }>('setConfig', { config });
  },

  // Version & Updates
  async getVersion(): Promise<VersionResponse> {
    return call<VersionResponse>('getVersion');
  },

  async checkUpdate(): Promise<UpdateCheckResponse> {
    return call<UpdateCheckResponse>('checkUpdate');
  },

  // Projects
  async getProjects(): Promise<Project[]> {
    return call<Project[]>('getProjects');
  },

  async createProject(name: string, url: string, tech_stack: string[]): Promise<{ ok: boolean }> {
    return call<{ ok: boolean }>('createProject', { name, url, tech_stack });
  },

  // Output
  async getOutput(): Promise<{ output: string }> {
    return call<{ output: string }>('getOutput');
  },

  async clearOutput(): Promise<{ ok: boolean }> {
    return call<{ ok: boolean }>('clearOutput');
  },

  // Explore
  async runExplore(url: string, depth: number): Promise<{ ok: boolean; jobId: string }> {
    return call<{ ok: boolean; jobId: string }>('runExplore', { url, depth });
  },

  // Generate
  async runGenerate(requirement: string, spec?: string): Promise<{ ok: boolean; jobId: string }> {
    return call<{ ok: boolean; jobId: string }>('runGenerate', { requirement, spec });
  },

  // Run Tests
  async runTests(project: string, type: string): Promise<{ ok: boolean; jobId: string }> {
    return call<{ ok: boolean; jobId: string }>('runTests', { project, type });
  },

  async previewTestPlan(project: string, type: string): Promise<TestPlan> {
    return call<TestPlan>('previewTestPlan', { project, type });
  },

  // Reports
  async getReports(project: string): Promise<RunReport[]> {
    return call<RunReport[]>('getReports', { project });
  },

  async getGateStatus(project: string): Promise<GateStatus> {
    return call<GateStatus>('getGateStatus', { project });
  },

  async getReportHtml(project: string, run_id: string): Promise<ReportHtmlResponse> {
    return call<ReportHtmlResponse>('getReportHtml', { project, run_id });
  },

  async exportProject(project: string): Promise<ExportResponse> {
    return call<ExportResponse>('exportProject', { project });
  },

  // Review
  async getReviewCases(project: string): Promise<TestCase[]> {
    return call<TestCase[]>('getReviewCases', { project });
  },

  async updateCaseStatus(project: string, case_id: string, status: string): Promise<{ ok: boolean }> {
    return call<{ ok: boolean }>('updateCaseStatus', { project, case_id, status });
  },
};