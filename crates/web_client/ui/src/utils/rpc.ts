import type { FlowDraft, FlowLibrary, FlowValidation, FlowsResponse, FlowPlan, PrepareFlowPlan } from '../types/flows';
export interface ProjectSpec {project: string; exists: boolean; markdown: string; revision: string | null}
// Shared RPC client — single source of truth
import type { ContractDraft, ContractsBundle, ContractsSnapshot, ExecutionSnapshot } from '../types/contracts';
import type { ModelProfile, ModelRegistry, TaskModelRoute } from '../types/models';
import type { ConfigMigrationSources, ConfigMigrationPreview, ConfigMigrationResult, ConfigSecretPolicy } from '../types/config_migration';

import type {
  AllureReportResponse, JobStartResult, CaseProvenance, RunEvidence, CaseAsset,
  ReportCleanupPreview, ConfigStatusResponse,
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
  JobStatus, TaskSummary,
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

let sessionPromise: Promise<string> | undefined;
let nextRequestId = 0;
let pendingToken: string | undefined;

async function request(url: string, init: RequestInit, timeout: number): Promise<unknown> {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeout);
  try {
    const response = await fetch(url, { ...init, credentials: 'same-origin', signal: controller.signal });
    if (response.status === 401) {
      sessionPromise = undefined;
      if (url === '/api/v2/session' && init.method === 'POST') pendingToken = undefined;
      throw new RpcError('Local session expired; retry, or reopen using qinaegis open.', 'HTTP_ERROR');
    }
    const body = await response.json();
    if (!response.ok) {
      throw new RpcError(body?.error?.message || body?.error || `HTTP ${response.status}`, 'HTTP_ERROR');
    }
    return body;
  } finally {
    clearTimeout(timeoutId);
  }
}

function session(): Promise<string> {
  if (!sessionPromise) {
    const fragment = new URLSearchParams(window.location.hash.slice(1));
    const token = fragment.get('token') || pendingToken;
    pendingToken = token || undefined;
    if (token) window.history.replaceState(null, '', window.location.pathname + window.location.search);
    sessionPromise = request('/api/v2/session', token ? {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ token }),
    } : { method: 'GET' }, DEFAULT_TIMEOUT).then(body => {
      const csrf = (body as { csrf?: string }).csrf;
      if (!csrf) throw new RpcError('Local session unavailable; reopen QinAegis using its launch command.', 'SESSION_ERROR');
      pendingToken = undefined;
      return csrf;
    }).catch(error => {
      sessionPromise = undefined;
      throw error;
    });
  }
  return sessionPromise;
}

async function call<T>(method: string, params: object = {}, timeout = DEFAULT_TIMEOUT): Promise<T> {
  try {
    const csrf = await session();
    const id = String(++nextRequestId);
    const body = await request('/api/v2/rpc', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Qinaegis-CSRF': csrf },
      body: JSON.stringify({ id, method, params }),
    }, timeout) as { id: string; ok: boolean; data?: T; error?: { code?: string; message?: string } };
    if (body.id !== id) throw new RpcError('Response ID mismatch', 'PROTOCOL_ERROR', method);
    if (!body.ok) {
      throw new RpcError(body.error?.message || 'Request failed', body.error?.code || 'RPC_ERROR', method);
    }
    const result = body.data;
    if (!['getJobStatus', 'getRunEvidence'].includes(method) && result && typeof result === 'object' && 'error' in result && result.error) {
      throw new RpcError(String(result.error), 'RPC_ERROR', method);
    }
    return result as T;
  } catch (error) {
    if (error instanceof RpcError) throw error;
    if (error instanceof Error && error.name === 'AbortError') {
      throw new RpcError(`Request timeout after ${timeout}ms`, 'TIMEOUT', method);
    }
    throw new RpcError((error as Error).message, 'CONNECTION_ERROR', method);
  }
}

// Typed RPC methods
export const rpc = {
  getFlows(): Promise<FlowsResponse> { return call('getFlows'); },
  saveFlowDraft(draft: FlowDraft, expected_revision: number): Promise<FlowLibrary> { return call('saveFlowDraft', {draft, expected_revision}); },
  validateFlowDraft(draft: FlowDraft): Promise<FlowValidation> { return call('validateFlowDraft', {draft}); },
  publishFlow(id: string, expected_revision: number): Promise<FlowLibrary> { return call('publishFlow', {id, expected_revision, approved: true}); },
  prepareFlowPlan(params: PrepareFlowPlan): Promise<FlowPlan> { return call('prepareFlowPlan', {...params, approved: true}); },
  getFlowPlan(plan_id: string): Promise<FlowPlan> { return call('getFlowPlan', {plan_id}); },
  listConfigMigrationSources(params: {project?: string} = {}): Promise<ConfigMigrationSources> { return call('listConfigMigrationSources', params); },
  previewConfigMigration(params: {source_id: string; project?: string}): Promise<ConfigMigrationPreview> { return call('previewConfigMigration', params); },
  applyConfigMigration(params: {preview_token: string; secret_policy: ConfigSecretPolicy; approved: true}): Promise<ConfigMigrationResult> { return call('applyConfigMigration', params); },
  previewReportCleanup(project: string, run_id: string): Promise<ReportCleanupPreview> { return call('previewReportCleanup', {project, run_id}); },
  cleanupReportResidue(project: string, run_id: string, preview_token: string): Promise<{ok: boolean; removed: string[]; removed_files: number; removed_bytes: number}> { return call('cleanupReportResidue', {project, run_id, preview_token, approved: true}); },
  getCaseAsset(project: string, case_id: string): Promise<CaseAsset> { return call('getCaseAsset', {project, case_id}); },
  saveCaseAsset(params: {project: string; case_id: string; expected_digest: string; name: string; yaml_script: string; expected_result: string; approved: true}): Promise<CaseAsset> { return call('saveCaseAsset', params); },
  diagnoseRuntime(): Promise<JobStartResult> { return call('diagnoseRuntime'); },
  installRuntimeBrowser(): Promise<JobStartResult> { return call('installRuntimeBrowser', {approved: true}); },
  getJobs(): Promise<{jobs: TaskSummary[]; history_error: string | null}> { return call('getJobs'); },
  getJobLog(jobId: string, cursor: number): Promise<{output: string; cursor: number; truncated: boolean}> { return call('getJobLog', {jobId, cursor}); },
  probeModel(profile_id: string): Promise<JobStartResult> { return call('probeModel', {profile_id}); },
  setProjectModelRoute(params: {project: string; task: string; route: TaskModelRoute | null; expected_revision: number}): Promise<ModelRegistry> { return call('setProjectModelRoute', params); },
  restoreConfigBackup(expected_revision: number): Promise<{ok: boolean; revision: number}> { return call('restoreConfigBackup', {expected_revision, approved: true}); },
  getModels(): Promise<ModelRegistry> { return call('getModels'); },
  saveModel(params: { profile: ModelProfile; expected_revision: number; clear_api_key: boolean }): Promise<ModelRegistry> { return call('saveModel', params); },
  setModelRoute(params: { task: string; route: TaskModelRoute; expected_revision: number }): Promise<ModelRegistry> { return call('setModelRoute', params); },
  resolveModelRoute(task: string): Promise<{revision: number; task: string; candidates: {id: string; model: string}[]}> { return call('resolveModelRoute', {task}); },
  deprecateContract(params: {contract_id: string; version: number; reason: string; expected_revision: number; approved: true}): Promise<ContractsSnapshot> { return call('deprecateContract', params); },
  deriveContract(params: {source_id: string; source_version: number; new_id: string; reason: string; expected_revision: number; approved: true}): Promise<ContractsSnapshot> { return call('deriveContract', params); },
  migrateContractBindings(params: {contract_id: string; from_version: number; to_version: number; bindings: {project: string; parameters: Record<string, unknown>}[]; reason: string; expected_revision: number; approved: true}): Promise<ContractsSnapshot> { return call('migrateContractBindings', params); },
  getCaseProvenance(project: string, case_id: string): Promise<CaseProvenance | null> { return call('getCaseProvenance', {project, case_id}); },
  generateAllureReport(project: string, run_id: string): Promise<JobStartResult> { return call('generateAllureReport', {project, run_id}); },
  getAllureReportHtml(project: string, run_id: string): Promise<AllureReportResponse> { return call('getAllureReportHtml', {project, run_id}); },
  getRunEvidence(project: string, run_id: string): Promise<RunEvidence> { return call('getRunEvidence', {project, run_id}); },
  getContracts(): Promise<ContractsSnapshot> {
    return call('getContracts');
  },
  saveContractDraft(params: { draft: ContractDraft; expected_revision: number }): Promise<ContractsSnapshot> {
    return call('saveContractDraft', params);
  },
  publishContract(params: { id: string; expected_revision: number; approved: true }): Promise<ContractsSnapshot> {
    return call('publishContract', params);
  },
  proposeContractCorrection(params: { contract_id: string; version: number; replacement: ContractDraft; reason: string; evidence: string[]; expected_revision: number }): Promise<ContractsSnapshot> {
    return call('proposeContractCorrection', params);
  },
  reviewContractCorrection(params: { proposal_id: string; decision: 'approve' | 'reject'; expected_revision: number; approved: true }): Promise<ContractsSnapshot> {
    return call('reviewContractCorrection', params);
  },
  bindContract(params: { project: string; contract_id: string; version: number; parameters: Record<string, unknown>; expected_revision: number }): Promise<ContractsSnapshot> {
    return call('bindContract', params);
  },
  exportContracts(): Promise<ContractsBundle> {
    return call('exportContracts');
  },
  importContracts(params: { bundle: ContractsBundle; expected_revision: number; approved: true }): Promise<ContractsSnapshot> {
    return call('importContracts', params);
  },
  // Config
  async checkConfig(): Promise<ConfigStatusResponse> {
    return call<ConfigStatusResponse>('checkConfig');
  },

  async getState(): Promise<StateResponse> {
    return call<StateResponse>('getState');
  },

  async setConfig(config: AppConfig, expected_revision: number): Promise<{ ok: boolean }> {
    return call<{ ok: boolean }>('setConfig', { config, expected_revision });
  },

  // Version & Updates
  async getVersion(): Promise<VersionResponse> {
    return call<VersionResponse>('getVersion');
  },

  async checkUpdate(): Promise<UpdateCheckResponse> {
    return call<UpdateCheckResponse>('checkUpdate');
  },

  // Projects
  async getProjects(): Promise<string[]> {
    return call<string[]>('getProjects');
  },

  async createProject(name: string, url: string, tech_stack: string[]): Promise<Project> {
    return call<Project>('createProject', { name, url, tech_stack });
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
  async getContractExecution(project: string): Promise<ExecutionSnapshot> {
    return call<ExecutionSnapshot>('getContractExecution', { project });
  },

  getProjectSpec(project: string): Promise<ProjectSpec> { return call("getProjectSpec", {project}); },
  saveProjectSpec(project: string, expected_revision: string | null, markdown: string): Promise<ProjectSpec> { return call("saveProjectSpec", {project,expected_revision,markdown,approved:true}); },
  async runGenerate(requirement: string, project: string, use_contracts: boolean, spec_revision?: string | null): Promise<{ ok: boolean; jobId: string }> {
    return call<{ ok: boolean; jobId: string }>('runGenerate', { requirement, project, use_contracts, ...(spec_revision !== undefined ? {spec_revision} : {}) });
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

  async updateCaseStatus(project: string, case_id: string, status: string, expected_digest?: string): Promise<{ ok: boolean }> {
    return call<{ ok: boolean }>('updateCaseStatus', { project, case_id, status, expected_digest });
  },

  // Job control
  async getJobStatus(jobId: string): Promise<JobStatus> {
    return call<JobStatus>('getJobStatus', { jobId });
  },

  async stopJob(jobId: string): Promise<{ ok: boolean }> {
    return call<{ ok: boolean }>('stopJob', { jobId });
  },
};
