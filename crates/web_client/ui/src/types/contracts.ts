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
