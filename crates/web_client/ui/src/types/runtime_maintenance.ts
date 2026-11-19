export interface RuntimeSummary {
  version: string;
  archive_sha256: string;
  source_commit?: string;
  target?: string;
}
export interface RuntimeMaintenance {
  current: RuntimeSummary | null;
  previous: RuntimeSummary | null;
  selection: 'managed' | 'external' | 'developer';
  blocked_reason: string | null;
  unmanaged_current?: boolean;
  unmanaged_previous?: boolean;
  preview?: RuntimePreview | null;
  upload?: {upload_id: string; size: number; received: number; claimed: boolean};
  downloads: {available: boolean; reason: string};
}
export interface RuntimePreview {
  preview_token: string;
  expected_revision: string;
  operation: 'install' | 'rollback';
  candidate: RuntimeSummary;
  current: RuntimeSummary | null;
  previous: RuntimeSummary | null;
  expires_at: string;
  discard_previous_required: boolean;
}

export interface RuntimeDownload {asset_id: string; version: string; name: string; size: number}
