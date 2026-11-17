import type { ExecutionSnapshot } from './contracts';
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
  config_revision: number;
  config_backup_available: boolean;
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
  execution_status?: string | null;
  execution_blocked?: boolean;
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
  status?: string;
  executed?: number;
  has_evidence?: boolean;
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
  steps: {id: string; name: string; priority: string; type: string}[];
}

export interface ReportHtmlResponse {
  ok: boolean;
  html?: string;
  error?: string;
}

export type JobType = 'explore' | 'generate' | 'testExec' | 'allureReport' | 'modelProbe' | 'runtimeCheck' | 'browserInstall';

export type JobStatusValue = 'idle' | 'running' | 'cancelling' | 'completed' | 'failed' | 'stopped' | 'interrupted';

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
export interface CaseProvenance {
  schema_version: number; project: string; case_id: string; yaml_script: string;
  created_at: string; contracts: ExecutionSnapshot | null; model: unknown;
}
export interface RunEvidence {
  schema_version: number; run_id: string; project: string; test_type: string; status: string;
  started_at: string; finished_at: string | null; model: unknown; engine_version: string | null;
  cases: {case_id: string; name: string; yaml_script: string; target_url: string; provenance: CaseProvenance | null}[];
  results: {case_id: string; passed: boolean; duration_ms: number; screenshot_base64: string | null; error_message: string | null}[];
  error: string | null; semantic_validation: 'not_verified';
}

export interface JobStartResult { ok: boolean; jobId: string; }
export interface AllureReportResponse { ok: boolean; html?: string; version?: string; error?: string; }

export interface TaskSummary { jobId: string; jobType: string; status: JobStatusValue; error?: string | null; created_at?: number; updated_at?: number; }

export interface CaseAsset {
  case_id: string; name: string; yaml_script: string; expected_result: string;
  status: TestCase['status']; provenance: CaseProvenance | null; digest: string;
}

export interface ReportCleanupPreview {
  project: string;
  run_id: string;
  run_status: string;
  completed_report_preserved: boolean;
  cleanup_available: boolean;
  preview_token: string | null;
  expires_in_seconds: number;
  candidates: { name: string; kind: string; files: number; bytes: number }[];
  manual_review: { name: string; reason: string }[];
  preserved: string[];
}
