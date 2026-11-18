import type {ExecutionDraft} from '../types/flow_execution';
const record = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
const strings = (v: unknown): v is string[] => Array.isArray(v) && v.every(s => typeof s === 'string');
const kind = (v: unknown) => ['string', 'number', 'boolean', 'object'].includes(String(v));
function source(v: unknown): boolean {
  if (!record(v)) return false;
  return v.kind === 'literal' ? Object.prototype.hasOwnProperty.call(v, 'value') : v.kind === 'flow_input' ? typeof v.name === 'string' : v.kind === 'step_output' && typeof v.step === 'string' && typeof v.port === 'string';
}
// This protects rendering while the server remains the authority for execution.
// Invalid advanced text stays editable and never reaches structured controls.
export function parseExecutionDraft(text: string): ExecutionDraft | null {
  try {
    const value: unknown = JSON.parse(text);
    if (!record(value) || !record(value.definition) || !record(value.model_profiles) || !Object.values(value.model_profiles).every(v => typeof v === 'string')) return null;
    const d = value.definition;
    if (d.schema_version !== 2 || !['id', 'title', 'project'].every(k => typeof d[k] === 'string') || !record(d.inputs) || !Object.values(d.inputs).every(kind)
      || !record(d.outputs) || !Object.values(d.outputs).every(v => record(v) && kind(v.value_type) && source(v.source))
      || !strings(d.required_permissions) || !Number.isFinite(d.total_timeout_ms) || !Number.isFinite(d.cleanup_timeout_ms) || !Array.isArray(d.steps) || d.steps.length > 64) return null;
    const ids = new Set<string>();
    for (const s of d.steps) {
      if (!record(s) || typeof s.id !== 'string' || ids.has(s.id) || !strings(s.depends_on) || !record(s.operation) || !Number.isFinite(s.timeout_ms) || !Number.isFinite(s.max_attempts)) return null;
      ids.add(s.id);
      if (s.condition !== null && (!record(s.condition) || !['equals', 'not_equals'].includes(String(s.condition.operator)) || !kind(s.condition.value_type) || !source(s.condition.source) || !Object.prototype.hasOwnProperty.call(s.condition, 'value'))) return null;
      const op = s.operation;
      if (op.capability === 'wait.v1') {
        if (!source(op.milliseconds) || !record(op.milliseconds)) return null;
        if (op.milliseconds.kind === 'literal' && (typeof op.milliseconds.value !== 'number' || !Number.isFinite(op.milliseconds.value))) return null;
      }
      else if (op.capability === 'case.execute.v1') {
        if (!record(op.case) || !['project', 'case_id', 'content_revision'].every(k => typeof (op.case as Record<string, unknown>)[k] === 'string') || !record(op.case.contract) || typeof op.case.contract.id !== 'string' || !Number.isSafeInteger(op.case.contract.version)) return null;
      } else return null;
    }
    return value as unknown as ExecutionDraft;
  } catch {return null;}
}
export function preparationDeadline(receivedAt: number, remainingSeconds: number): number {
  return receivedAt + Math.max(0, remainingSeconds) * 1000;
}
export function retryExecutionRead(error: unknown, attempt: number): boolean {
  return attempt < 3 && error instanceof Error && error.message.includes('asset library is busy');
}
