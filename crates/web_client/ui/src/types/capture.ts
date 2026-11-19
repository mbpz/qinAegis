export type CaptureMode = 'off' | 'failure_only' | 'key_steps' | 'full';
export interface CapturePolicy { schema_version: 1; mode: CaptureMode; max_artifact_bytes: number; max_run_bytes: number; max_events: number; include_sensitive: boolean }
export interface CapturePolicySnapshot { revision: number; policy: CapturePolicy }
export interface CaptureArtifact { id: string; kind: string; file_name: string; bytes: number; sha256: string }
export interface CaptureReceipt { schema_version: 1; policy: CapturePolicy; status: 'disabled' | 'complete' | 'partial'; channels: { kind: string; status: string; reason: string | null; bytes: number }[]; artifacts: CaptureArtifact[] }
export interface ProcessEvidence { schema_version: 1; project: string; run_id: string; status: string; policy: CapturePolicySnapshot | null; cases: { case_id: string; name: string; receipt: CaptureReceipt | null; artifacts: { id: string; available: boolean; reason?: string }[] }[] }
export interface ProcessArtifact { kind: string; bytes: number; sha256: string; file_name: string; base64: string }
export interface TraceViewer { viewer_id: string; url: string; expires_in_seconds: number; version: string }
