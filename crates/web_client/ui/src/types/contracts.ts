export type ContractValueType = 'string' | 'number' | 'boolean' | 'object';
export interface ContractDraft {
  id: string;
  version: number;
  intent: string;
  inputs: Record<string, ContractValueType>;
  outputs: Record<string, ContractValueType>;
  preconditions: string[];
  assertions: string[];
  required_capabilities: string[];
}
export interface PublishedContract {
  definition: ContractDraft;
  reviewer: string;
  correction: { original_version: number; reason: string; evidence: string[] } | null;
}
export interface ContractProposal {
  id: string;
  original_version: number;
  replacement: ContractDraft;
  reason: string;
  evidence: string[];
  status: 'pending' | 'approved' | 'rejected';
  reviewer: string | null;
}
export interface ContractBinding {
  project: string;
  contract_id: string;
  contract_version: number;
  parameters: Record<string, unknown>;
}
export interface ContractsSnapshot {
  schema_version: 1;
  revision: number;
  drafts: ContractDraft[];
  publications: PublishedContract[];
  proposals: ContractProposal[];
  bindings: ContractBinding[];
  deprecations: {contract_id: string; version: number; reason: string; reviewer: string}[];
  derivations: {contract_id: string; source_id: string; source_version: number; reason: string; reviewer: string}[];
  migrations: {contract_id: string; from_version: number; to_version: number; projects: string[]; reason: string; reviewer: string}[];
}
export interface ContractsBundle {
  schema_version: 1;
  contracts: PublishedContract[];
}
export interface ExecutionSnapshot {
  schema_version: 1;
  library_revision: number;
  project: string;
  available_capabilities: string[];
  contracts: {
    binding: ContractBinding;
    publication: PublishedContract;
    original_assertions: string[];
  }[];
}
