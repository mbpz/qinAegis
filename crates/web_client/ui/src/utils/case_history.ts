import type { CaseRevision } from '../types/case_history';
export function caseRestoreReason(reason: string): boolean { return reason.trim().length > 0 && new TextEncoder().encode(reason).length <= 8192; }
export function restorableCaseRevision(value: CaseRevision | null, caseId: string): boolean {
  return !!value && Number.isSafeInteger(value.revision) && value.revision > 0 && value.snapshot?.id === caseId && typeof value.digest === 'string' && value.digest.length > 0;
}
