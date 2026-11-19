export interface CaseRevision { revision: number; parent_revision: number | null; snapshot: Record<string, unknown> | null; digest: string | null; operation: string; context: { source: string; author: string | null; reason: string }; created_at: string | null }
export interface CaseHistory { schema_version: 1; case_id: string; history_complete: boolean; revisions: CaseRevision[] }

export interface ContractAudit { revision: number; history_complete: boolean; events: { revision: number; parent_revision: number; created_at: string; context: {source: string; author: string | null; reason: string}; before: Record<string, unknown>; after: Record<string, unknown> }[] }
