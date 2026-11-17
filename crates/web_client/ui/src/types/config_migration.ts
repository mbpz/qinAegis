import type { AppConfig } from './rpc';

export type ConfigSecretPolicy = 'keep_target' | 'use_source';
export interface ConfigMigrationSources {
  sources: { id: string; label: string; status: 'ready' | 'missing' | 'invalid' }[];
}
export interface ConfigMigrationPreview {
  preview_token: string;
  source_id: string;
  source_label: string;
  expires_in_seconds: number;
  source_revision: number;
  target_revision: number;
  target_exists: boolean;
  source_summary: AppConfig;
  target_summary: AppConfig;
  choices: Record<ConfigSecretPolicy, { summary: AppConfig; changed_fields: string[] }>;
}
export interface ConfigMigrationResult { revision: number; unchanged: boolean }
