export interface ModelProfile {
  id: string;
  label: string;
  adapter: string;
  base_url: string;
  model: string;
  api_key: string;
  api_key_configured?: boolean;
  capabilities: string[];
  enabled: boolean;
  allow_remote: boolean;
  midscene_family: string | null;
}
export interface TaskModelRoute { primary: string; fallbacks: string[] }
export interface ModelRegistry {
  schema_version: number;
  revision: number;
  profiles: ModelProfile[];
  routes: Record<string, TaskModelRoute>;
  project_routes?: Record<string, Record<string, TaskModelRoute>>;
}
