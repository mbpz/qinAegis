// Shared RPC types — single source of truth

export interface RpcRequest<P = object> {
  method: string;
  params?: P;
}

export interface RpcSuccessResponse<T> {
  ok: true;
  data: T;
}

export interface RpcErrorResponse {
  ok: false;
  error: string;
  code?: string;
}

export type RpcResponse<T> = RpcSuccessResponse<T> | RpcErrorResponse;

// Specific response types
export interface VersionResponse {
  version: string;
}

export interface UpdateCheckResponse {
  upToDate: boolean;
  latest: string;
  current: string;
}

export interface ConfigStatusResponse {
  configured: boolean;
}

export interface StateResponse {
  ok: boolean;
  config: AppConfig | null;
  currentView: string;
}

export interface AppConfig {
  llm: {
    api_key: string;
    api_key_configured?: boolean;
    secondary_api_key_configured?: boolean;
    clear_api_key?: boolean;
    clear_secondary_api_key?: boolean;
    base_url: string;
    model: string;
    provider?: string;
    secondary_provider?: string;
    secondary_api_key?: string;
    secondary_base_url?: string;
    secondary_model?: string;
  };
  exploration?: { max_depth: number; max_pages_per_seed: number };
  sandbox: {
    cdp_port: number;
  };
}

export interface Project {
  name: string;
  url: string;
  tech_stack: string[];
}

export interface GateStatus {
  e2e_pass_rate: number | null;
  e2e_pass_rate_display: string;
  performance: string | null;
  stress: string | null;
  has_runs: boolean;
  last_run_passed?: number;
  last_run_total?: number;
}

export interface RunReport {
  run_id: string;
  total: number;
  passed: number;
  failed: number;
}

export interface TestCase {
  id: string;
  name: string;
  status: 'draft' | 'reviewed' | 'approved' | 'flaky' | 'archived';
  priority: string;
  type: string;
  requirement_id?: string;
}

export interface TestPlan {
  plan: string;
  case_count: number;
  steps: string[];
}

export interface ReportHtmlResponse {
  ok: boolean;
  html?: string;
  error?: string;
}

export type JobType = 'explore' | 'generate' | 'testExec';

export type JobStatusValue = 'idle' | 'running' | 'cancelling' | 'completed' | 'failed' | 'stopped';

export interface JobStatus {
  jobId: string;
  jobType: JobType;
  status: JobStatusValue;
  output: string;
  result?: unknown;
  error?: string;
}

export interface ExportResponse {
  ok: boolean;
  project: string;
  reports: Array<{
    run_id: string;
    summary: Record<string, unknown>;
  }>;
  error?: string;
}