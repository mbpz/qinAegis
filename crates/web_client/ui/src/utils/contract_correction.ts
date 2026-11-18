import type { CorrectionAttempt, RunEvidence } from '../types/rpc';
import type { PublishedContract } from '../types/contracts';

export function correctionInstruction(value: string): string {
  const text = value.trim();
  if (!text || new TextEncoder().encode(text).length > 8192) throw new Error('Provide an explanation of 1–8192 UTF-8 bytes.');
  return text;
}
const canonical = (value: unknown): string => JSON.stringify(value, (_, item) => item && typeof item === 'object' && !Array.isArray(item)
  ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b))) : item);
export function correctionCases(run: RunEvidence, project: string, publication: PublishedContract): RunEvidence['cases'] {
  if (run.project !== project || !['completed', 'failed', 'stopped'].includes(run.status) || !run.finished_at) return [];
  return run.cases.filter(item => item.provenance?.project === project && item.provenance.case_id === item.case_id
    && item.provenance.yaml_script === item.yaml_script && item.provenance.contracts?.project === project
    && item.provenance.contracts.contracts.some(source => canonical(source.publication) === canonical(publication)
      && source.binding.project === project && source.binding.contract_id === publication.definition.id
      && source.binding.contract_version === publication.definition.version
      && canonical(source.original_assertions) === canonical(publication.definition.assertions)));
}
export function correctionAttempts(value: unknown): CorrectionAttempt[] | null {
  if (!Array.isArray(value) || value.length > 4) return null;
  const outcomes = ['started', 'succeeded', 'transport_failure', 'timed_out', 'rejected', 'cancelled', 'rate_limited', 'service_unavailable'];
  return value.every((item, i) => item && item.call === 1 && item.attempt === i + 1
    && typeof item.profile_id === 'string' && item.profile_id.length > 0 && item.profile_id.length <= 128
    && outcomes.includes(item.outcome)) ? value as CorrectionAttempt[] : null;
}
export function correctionJobEvidence(value: unknown): {status: string; attempts: CorrectionAttempt[]; proposalId?: string} | null {
  if (!value || typeof value !== 'object') return null;
  const record = value as Record<string, unknown>;
  const attempts = correctionAttempts(record.attempts);
  if (record.schema_version !== 1 || record.source !== 'contract_correction' || !attempts
    || !['in_progress', 'completed', 'failed', 'cancelled'].includes(String(record.application_outcome))
    || (record.proposal_id !== undefined && typeof record.proposal_id !== 'string')) return null;
  return {status: String(record.application_outcome), attempts, proposalId: record.proposal_id as string | undefined};
}
