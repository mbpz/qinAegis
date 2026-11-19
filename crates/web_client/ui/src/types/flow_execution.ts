import type { ContractValueType } from './contracts';
import type { InputSource } from './flows';
export interface ReviewedCaseRef {project: string; case_id: string; content_revision: string; contract: {id: string; version: number}}
export type ExecutionOperation = {capability: 'wait.v1'; milliseconds: InputSource} | {capability: 'case.execute.v1'; case: ReviewedCaseRef};
export interface ExecutionStep {id: string; depends_on: string[]; operation: ExecutionOperation; condition: {operator: 'equals' | 'not_equals'; source: InputSource; value_type: ContractValueType; value: unknown} | null; timeout_ms: number; max_attempts: number}
export interface ExecutionDefinition {schema_version: 2; id: string; title: string; project: string; inputs: Record<string, ContractValueType>; outputs: Record<string, {value_type: ContractValueType; source: InputSource}>; steps: ExecutionStep[]; total_timeout_ms: number; cleanup_timeout_ms: number; required_permissions: string[]}
export interface ExecutionDraft {definition: ExecutionDefinition; model_profiles: Record<string, string>}
export interface ExecutionPublication extends ExecutionDraft {version: number; reviewer: string}
export interface ExecutionLibrary {schema_version: 2; revision: number; drafts: ExecutionDraft[]; publications: ExecutionPublication[]}
export interface ExecutionPreparation {preparation_id: string; flow_id: string; version: number; project: string; target_url: string; execution_order: string[]; models: Record<string, {profile_id: string; adapter: string; model: string; base_url: string; midscene_family: string | null}>; expires_in_seconds: number; permissions: string[]; snapshot: unknown}
export interface FlowRunSummary {run_id: string; job_id: string | null; flow_id: string; version: number; project: string; status: string; created_at: string}
export interface ExecutionFacts {schema_version: 2; flow_id: string; status: string; steps: {step_id: string; status: string; reason: string | null; attempts: {attempt: number; started_ms: number; finished_ms: number; status: string; error: string | null; case_result: unknown}[]; outputs: Record<string, unknown>}[]; outputs: Record<string, unknown>; error: string | null; cleanup_error: string | null; cleanup_started_ms: number; finished_ms: number}
export interface FlowRun extends FlowRunSummary {schema_version: 2; preparation: unknown; facts: ExecutionFacts | null; case_runs: Record<string, {run_id: string; project: string; status: string; allure_results_ready: boolean}>; error: string | null; updated_at: string}
export interface ExecutionFlows {library: ExecutionLibrary; model_revision: number; run_history: FlowRunSummary[]; preparations: ExecutionPreparation[]}
