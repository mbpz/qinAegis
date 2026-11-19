import type { CapturePolicy, CaptureArtifact, ProcessArtifact } from '../types/capture';
export function validCapturePolicy(draft: CapturePolicy | null): boolean {
  return !!draft && draft.schema_version === 1 && ['off', 'failure_only', 'key_steps', 'full'].includes(draft.mode) && Number.isInteger(draft.max_artifact_bytes) && draft.max_artifact_bytes >= 1024 && draft.max_artifact_bytes <= 8388608 && Number.isInteger(draft.max_run_bytes) && draft.max_run_bytes >= draft.max_artifact_bytes && draft.max_run_bytes <= 33554432 && Number.isInteger(draft.max_events) && draft.max_events >= 1 && draft.max_events <= 2000 && (draft.mode === 'off' || draft.include_sensitive === true);
}
export function localTraceViewerUrl(value: string): string {
  const url = new URL(value);
  if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || url.username || url.password || !url.port) throw new Error('Invalid local viewer address');
  return value;
}

export function validateArtifactResponse(expected: CaptureArtifact, actual: ProcessArtifact): void {
  if (!Number.isSafeInteger(actual.bytes) || actual.bytes < 0 || actual.bytes > 8388608 || actual.base64.length > 11184812 || expected.kind !== actual.kind || expected.file_name !== actual.file_name || expected.bytes !== actual.bytes || expected.sha256 !== actual.sha256) throw new Error('Artifact does not match the selected committed receipt');
}
