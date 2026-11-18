const object = (value: unknown): Record<string, unknown> | null => value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : null;
const outcomes: Record<string, string> = {
  started: 'Started locally; provider receipt is unconfirmed',
  succeeded: 'Response received; business validation is separate',
  rate_limited: 'Rate limited', service_unavailable: 'Provider service unavailable',
  transport_failure: 'Transport failed', timed_out: 'Timed out', rejected: 'Request or response rejected', cancelled: 'Local call cancelled',
};
const applicationOutcomes: Record<string, string> = {
  in_progress: 'Generation in progress', completed: 'Generation completed',
  completed_with_unverified_reviews: 'Drafts saved; some model reviews could not be verified',
  failed: 'Generation failed; earlier drafts may have been saved', cancelled: 'Generation cancelled; earlier drafts may have been saved',
};
export interface GenerationAttemptRow {key: string; phase: string; candidate: string; model: string; outcome: string}
export function generationEvidence(result: unknown, snapshot: unknown): {status: string; rows: GenerationAttemptRow[]; legacy: boolean} | null {
  const value = object(result);
  if (!value || value.schema_version !== 1) return null;
  if (value.source === 'legacy_config' && value.actual_attempts === 'not_recorded') return {status: 'Actual provider attempts were not recorded for this legacy route.', rows: [], legacy: true};
  if ((value.source !== 'registered_route' && value.source !== 'legacy_config') || typeof value.application_outcome !== 'string' || !Object.prototype.hasOwnProperty.call(applicationOutcomes, value.application_outcome) || !Array.isArray(value.attempts) || value.attempts.length > 804) return null;
  const model = object(object(snapshot)?.model);
  if (value.source === 'legacy_config' && model?.source !== 'legacy_config') return null;
  if (value.source === 'registered_route' && model?.source === 'legacy_config') return null;
  const candidates = model?.candidates;
  if (!Array.isArray(candidates) || candidates.length < 1 || candidates.length > 4) return null;
  const rows: GenerationAttemptRow[] = [];
  let previousCall = 0, previousAttempt = 0;
  for (const row of value.attempts) {
    if (!Array.isArray(row) || row.length !== 4) return null;
    const [call, attempt, index, outcome] = row;
    if (!Number.isInteger(call) || call < 1 || call > 201 || !Number.isInteger(attempt) || !Number.isInteger(index) || index < 0 || index >= candidates.length || attempt !== index + 1 || typeof outcome !== 'string' || !Object.prototype.hasOwnProperty.call(outcomes, outcome)) return null;
    if (call === previousCall ? attempt !== previousAttempt + 1 : call !== previousCall + 1 || attempt !== 1) return null;
    const candidate = object(candidates[index]);
    if (typeof candidate?.id !== 'string' || typeof candidate?.model !== 'string') return null;
    rows.push({key: `${call}-${attempt}`, phase: call === 1 ? 'Generator' : `Critic ${call - 1}`, candidate: candidate.id, model: candidate.model, outcome: outcomes[outcome]});
    previousCall = call; previousAttempt = attempt;
  }
  return {status: applicationOutcomes[value.application_outcome], rows, legacy: false};
}
