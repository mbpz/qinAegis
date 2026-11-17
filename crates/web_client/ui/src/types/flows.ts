import type { ContractValueType } from './contracts';
export type InputSource = {kind: 'literal'; value: unknown} | {kind: 'flow_input'; name: string} | {kind: 'step_output'; step: string; port: string};
export interface FlowDraft {
  id: string;
  title: string;
  inputs: Record<string, ContractValueType>;
  outputs: Record<string, {value_type: ContractValueType; source: InputSource}>;
  steps: {id: string; capability: string; depends_on: string[]; inputs: Record<string, InputSource>; contract: {id: string; version: number} | null; model_role: string | null; max_attempts: number}[];
  required_permissions: string[];
}
export interface PublishedFlow {definition: FlowDraft; version: number; reviewer: string}
export interface FlowLibrary {schema_version: number; revision: number; drafts: FlowDraft[]; publications: PublishedFlow[]}
export interface CapabilityDescriptor {id: string; inputs: Record<string, ContractValueType>; outputs: Record<string, ContractValueType>; permissions: string[]; idempotent: boolean; planning_supported: boolean; runner_available: boolean}
export interface FlowValidation {execution_order: string[]; capabilities: CapabilityDescriptor[]}
export interface FlowPlanSummary {plan_id: string; project: string; flow_id: string; flow_version: number; executable: false}
export interface FlowPlan extends FlowPlanSummary {[key: string]: unknown}
export interface FlowsResponse {library: FlowLibrary; capabilities: CapabilityDescriptor[]; model_revision: number; plan_history: FlowPlanSummary[]}
export interface PrepareFlowPlan {id: string; version: number; project: string; inputs: Record<string, unknown>; permissions: string[]; expected_revision: number; expected_model_revision: number}
