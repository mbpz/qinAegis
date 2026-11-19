import CapturePolicyView from './CapturePolicyView';
import { Button, Checkbox, Input, Loader, Select } from './ui';
import { useState, useEffect, useRef } from 'react';
import { rpc } from '../utils/rpc';
import type { AppConfig } from '../types/rpc';
import type { ConfigMigrationPreview, ConfigMigrationSources, ConfigSecretPolicy } from '../types/config_migration';

const defaultConfig: AppConfig = {
  llm: { provider: 'minimax', api_key: '', base_url: 'https://api.minimax.chat/v1', model: 'MiniMax-M2.7' },
  sandbox: { cdp_port: 9222 },
  exploration: { max_depth: 3, max_pages_per_seed: 20 },
};

export default function SettingsView({ onSaved }: { onSaved?: () => void }) {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [revision, setRevision] = useState<number | null>(null);
  const [backup, setBackup] = useState(false);
  const [restoreApproved, setRestoreApproved] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [showSecondary, setShowSecondary] = useState(false);
  const [message, setMessage] = useState('');
  const [showApiKey, setShowApiKey] = useState(false);
  const [migrationSources, setMigrationSources] = useState<ConfigMigrationSources['sources']>([]);
  const [migrationProjects, setMigrationProjects] = useState<string[]>([]);
  const [migrationProject, setMigrationProject] = useState('');
  const [migrationSource, setMigrationSource] = useState('');
  const [migrationPreview, setMigrationPreview] = useState<ConfigMigrationPreview | null>(null);
  const [secretPolicy, setSecretPolicy] = useState<ConfigSecretPolicy | ''>('');
  const [migrationApproved, setMigrationApproved] = useState(false);
  const [migrationBusy, setMigrationBusy] = useState(false);
  const migrationRequest = useRef(0);
  const migrationLock = useRef(false);

  useEffect(() => {
    loadConfig();
    rpc.getProjects().then(setMigrationProjects).catch(() => {});
    return () => { ++migrationRequest.current; };
  }, []);

  const clearMigrationPreview = () => {
    ++migrationRequest.current;
    setMigrationPreview(null); setSecretPolicy(''); setMigrationApproved(false);
  };
  const loadMigrationSources = async () => {
    if (migrationLock.current) return;
    migrationLock.current = true; setMigrationBusy(true); clearMigrationPreview();
    const request = migrationRequest.current;
    try {
      const result = await rpc.listConfigMigrationSources(migrationProject ? {project:migrationProject} : {});
      if (request === migrationRequest.current) { setMigrationSources(result.sources); setMigrationSource(''); }
    } catch (error) { if (request === migrationRequest.current) setMessage('Migration sources unavailable: ' + error); }
    finally { migrationLock.current = false; setMigrationBusy(false); }
  };
  const previewMigration = async () => {
    if (!migrationSource || migrationLock.current) return;
    migrationLock.current = true; setMigrationBusy(true); clearMigrationPreview();
    const request = migrationRequest.current;
    try {
      const preview = await rpc.previewConfigMigration({source_id:migrationSource, ...(migrationProject ? {project:migrationProject} : {})});
      if (request === migrationRequest.current) setMigrationPreview(preview);
    } catch (error) { if (request === migrationRequest.current) setMessage('Migration preview failed: ' + error); }
    finally { migrationLock.current = false; setMigrationBusy(false); }
  };
  const applyMigration = async () => {
    if (!migrationPreview || !secretPolicy || !migrationApproved || migrationLock.current) return;
    migrationLock.current = true; setMigrationBusy(true);
    try {
      const result = await rpc.applyConfigMigration({preview_token:migrationPreview.preview_token, secret_policy:secretPolicy, approved:true});
      clearMigrationPreview(); await loadConfig(); onSaved?.();
      setMessage(result.unchanged ? 'Migration already matches the saved configuration; no changes made.' : 'Configuration migrated successfully. Source retained; previous target backed up.');
    } catch (error) {
      clearMigrationPreview();
      setMessage('Migration failed or conflicted: ' + error + '. Reload sources and preview again; existing files are retained.');
    } finally { migrationLock.current = false; setMigrationBusy(false); }
  };

  const loadConfig = async () => {
    clearMigrationPreview();
    try {
      const state = await rpc.getState();
      setConfig(state.config || structuredClone(defaultConfig));
      setRevision(state.config_revision);
      setBackup(state.config_backup_available);
      setRestoreApproved(false);
    } catch (e) {
      setMessage('Failed to load settings: ' + (e as Error).message);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    if (!config || revision === null || saving || migrationBusy) return;
    setSaving(true);
    try {
      const result = await rpc.setConfig(config, revision);
      if (result.ok) {
        await loadConfig();
        onSaved?.();
        setMessage('Settings saved successfully!');
        setTimeout(() => setMessage(''), 3000);
      } else {
        setMessage('Failed to save settings');
      }
    } catch (e) {
      console.error('setConfig error:', e);
      setMessage('Failed to save settings: ' + e);
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="view">
        <h2 className="view-title">Settings</h2>
      <CapturePolicyView />
        <div className="loading">
          <Loader />
          <span>Loading configuration...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="view">
      <h2 className="view-title">Settings</h2>
      <CapturePolicyView />

      {message && (
        <div className="card" style={{ borderColor: message.includes('success') ? 'var(--accent-green)' : 'var(--accent-red)' }}>
          {message}
        </div>
      )}

      {!config && <Button type="button" className="btn btn-primary" onClick={() => { setLoading(true); loadConfig(); }}>Retry loading settings</Button>}
      {config && <>
      <div className="card">
        <div className="card-title">AI Model Configuration</div>
        <div className="input-group"><label htmlFor="settings-primary-provider">Provider name (Chat Completions API compatible)</label><Input id="settings-primary-provider" className="input" value={config.llm.provider || ''} onValueChange={value => setConfig({ ...config, llm: { ...config.llm, provider: value } })} /></div>
        <div className="input-group">
          <label htmlFor="settings-primary-api-key">API Key</label>
          <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
            <Input
              id="settings-primary-api-key"
              type={showApiKey ? 'text' : 'password'}
              className="input"
              placeholder={config.llm.api_key_configured ? 'Key stored — leave empty to keep' : 'sk-...'}
              value={config?.llm?.api_key || ''}
              onValueChange={(value) => setConfig({ ...config!, llm: { ...config!.llm, api_key: value } })}
              style={{ paddingRight: '40px' }}
            />
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={showApiKey ? 'Hide primary API key' : 'Show primary API key'}
              aria-pressed={showApiKey}
              aria-controls="settings-primary-api-key"
              onClick={() => setShowApiKey(!showApiKey)}
              style={{
                position: 'absolute',
                right: '8px',
                background: 'none',
                border: 'none',
                cursor: 'pointer',
                padding: '4px',
                color: 'var(--text-secondary)',
              }}
              title={showApiKey ? 'Hide API Key' : 'Show API Key'}
            >
              <span aria-hidden="true">{showApiKey ? '👁' : '👁‍🗨'}</span>
            </Button>
          </div>
        </div>
        {config.llm.api_key_configured && <Checkbox label={"Clear stored primary key"} checked={!!config.llm.clear_api_key} onCheckedChange={checked => setConfig({ ...config, llm: { ...config.llm, clear_api_key: checked } })} />}
        <div className="form-row">
          <div className="input-group">
            <label htmlFor="settings-primary-base-url">Base URL</label>
            <Input
              id="settings-primary-base-url"
              className="input"
              type="text"
              placeholder="https://api.minimax.chat/v1"
              value={config?.llm?.base_url || ''}
              onValueChange={(value) => setConfig({ ...config!, llm: { ...config!.llm, base_url: value } })}
            />
          </div>
          <div className="input-group">
            <label htmlFor="settings-primary-model">Model</label>
            <Input
              id="settings-primary-model"
              className="input"
              type="text"
              placeholder="MiniMax-VL-01"
              value={config?.llm?.model || ''}
              onValueChange={(value) => setConfig({ ...config!, llm: { ...config!.llm, model: value } })}
            />
          </div>
        </div>
      </div>

      <div className="card">
        <Button
          type="button"
          variant="ghost"
          className="card-title h-auto w-full justify-start whitespace-normal p-0 text-left"
          aria-expanded={showSecondary}
          aria-controls="secondary-llm-config"
          onClick={() => setShowSecondary(!showSecondary)}
        >
          {showSecondary ? '▼' : '▶'} Secondary LLM (Optional — for fallback & complex tasks)
        </Button>
        {showSecondary && (
          <div id="secondary-llm-config">
            <div className="input-group">
              <label htmlFor="settings-secondary-provider">Provider</label>
              <Input
                id="settings-secondary-provider"
                className="input"
                type="text"
                placeholder="minimax"
                value={config?.llm?.secondary_provider || ''}
                onValueChange={(value) => setConfig({ ...config!, llm: { ...config!.llm, secondary_provider: value } })}
              />
            </div>
            <div className="input-group">
              <label htmlFor="settings-secondary-api-key">API Key</label>
              <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
                <Input
                  id="settings-secondary-api-key"
                  type="password"
                  className="input"
                  placeholder={config.llm.secondary_api_key_configured ? 'Key stored — leave empty to keep' : 'sk-...'}
                  value={config?.llm?.secondary_api_key || ''}
                  onValueChange={(value) => setConfig({ ...config!, llm: { ...config!.llm, secondary_api_key: value } })}
                  style={{ paddingRight: '40px' }}
                />
              </div>
            </div>
            <div className="form-row">
              <div className="input-group">
                <label htmlFor="settings-secondary-base-url">Base URL</label>
                <Input
                  id="settings-secondary-base-url"
                  className="input"
                  type="text"
                  placeholder="https://api.minimax.chat/v1"
                  value={config?.llm?.secondary_base_url || ''}
                  onValueChange={(value) => setConfig({ ...config!, llm: { ...config!.llm, secondary_base_url: value } })}
                />
              </div>
              <div className="input-group">
                <label htmlFor="settings-secondary-model">Model</label>
                <Input
                  id="settings-secondary-model"
                  className="input"
                  type="text"
                  placeholder="Qwen3-VL-7B"
                  value={config?.llm?.secondary_model || ''}
                  onValueChange={(value) => setConfig({ ...config!, llm: { ...config!.llm, secondary_model: value } })}
                />
              </div>
            </div>
            {config.llm.secondary_api_key_configured && <Checkbox label={"Clear stored secondary key"} checked={!!config.llm.clear_secondary_api_key} onCheckedChange={checked => setConfig({ ...config, llm: { ...config.llm, clear_secondary_api_key: checked } })} />}
            <p style={{ fontSize: '12px', color: 'var(--text-secondary)', margin: '8px 0 0' }}>
              The secondary LLM is used as a fallback when the primary fails, or for complex vision tasks.
              Leave empty to use only the primary provider.
            </p>
          </div>
        )}
      </div>

      <div className="card">
        <div className="card-title">Sandbox Configuration</div>
        <div className="input-group">
          <label htmlFor="settings-cdp-port">CDP Port</label>
          <Input
            id="settings-cdp-port"
            className="input"
            type="number"
            placeholder="9222"
            value={config?.sandbox?.cdp_port || 9222}
            onValueChange={(value) => setConfig({ ...config!, sandbox: { ...config!.sandbox, cdp_port: parseInt(value) || 9222 } })}
          />
        </div>
      </div>

      <div className="card">
        <h3>Configuration recovery</h3>
        <p>Revision: {revision ?? 'unavailable'}. Saving replaces the configuration atomically and keeps the previous valid copy privately on this computer.</p>
        <Button disabled={saving || migrationBusy} onClick={() => void loadConfig()}>Reload saved settings</Button>
        {backup && <>
          <p>Restoring the previous copy also restores its saved credentials. The current copy becomes the recovery backup. Running tasks keep their frozen configuration.</p>
          <Checkbox label="I reviewed the restore scope and approve restoring the previous configuration" checked={restoreApproved} onCheckedChange={setRestoreApproved} disabled={saving || migrationBusy} />
          <Button className="btn" disabled={saving || migrationBusy || !restoreApproved || revision === null} onClick={async () => {
            if (revision === null || saving || migrationBusy || !restoreApproved) return;
            setSaving(true);
            try { await rpc.restoreConfigBackup(revision); await loadConfig(); onSaved?.(); setMessage('Configuration restored successfully.'); }
            catch (error) { setMessage('Restore failed: ' + (error as Error).message); }
            finally { setSaving(false); }
          }}>Restore previous configuration</Button>
        </>}
        {!backup && <p>No previous configuration is available.</p>}
      </div>
      <div className="form-actions">
        <Button type="button" className="btn btn-primary" onClick={handleSave} disabled={saving || migrationBusy || revision === null}>
          {saving ? 'Saving...' : 'Save Settings'}
        </Button>
      </div>
      </>}
      <section className="card" aria-label="Configuration migration">
        <h3>Configuration migration</h3>
        <p>Choose one known configuration source. Migration replaces the saved settings with that source; it does not merge multiple sources or import the model registry. Existing files remain in place, and the previous target is backed up. Unsaved settings in this page will be replaced after migration.</p>
        <p>Legacy TOML without a revision is supported. Unknown fields, invalid files and symlink locations are refused. External project folders are not scanned.</p>
        <label htmlFor="migration-project">Optional managed project override</label>
        <Select id="migration-project" value={migrationProject} disabled={saving || migrationBusy} onValueChange={value => {
          setMigrationProject(value); setMigrationSources([]); setMigrationSource(''); clearMigrationPreview();
        }}>
          <option value="">Global locations only</option>
          {migrationProjects.map(project => <option key={project} value={project}>{project}</option>)}
        </Select>
        <Button disabled={saving || migrationBusy} onClick={() => void loadMigrationSources()}>Find known configuration sources</Button>
        {migrationSources.length > 0 && <>
          <label htmlFor="migration-source">Configuration source</label>
          <Select id="migration-source" value={migrationSource} disabled={saving || migrationBusy} onValueChange={value => { setMigrationSource(value); clearMigrationPreview(); }}>
            <option value="">Choose a source</option>
            {migrationSources.map(source => <option key={source.id} value={source.id} disabled={source.status !== 'ready'}>{source.label} ({source.status})</option>)}
          </Select>
          <p>Missing sources are unavailable. Invalid sources require manual inspection; they will not be overwritten or automatically repaired.</p>
          <Button disabled={saving || migrationBusy || !migrationSource} onClick={() => void previewMigration()}>Preview selected migration</Button>
        </>}
        {migrationPreview && <>
          <h4>Preview: {migrationPreview.source_label}</h4>
          <p>Source revision {migrationPreview.source_revision}; target revision {migrationPreview.target_revision}. {migrationPreview.target_exists ? 'The existing target will be retained as a backup when changed.' : 'No target configuration exists yet.'} Preview expires after {migrationPreview.expires_in_seconds / 60} minutes or when another preview replaces it.</p>
          <h4>Current saved settings (credentials hidden)</h4>
          <pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{JSON.stringify(migrationPreview.target_summary,null,2)}</pre>
          <h4>Selected source (credentials hidden)</h4>
          <pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{JSON.stringify(migrationPreview.source_summary,null,2)}</pre>
          <label htmlFor="migration-secret-policy">Explicit credential choice</label>
          <Select id="migration-secret-policy" value={secretPolicy} disabled={saving || migrationBusy} onValueChange={value => {setSecretPolicy(value as ConfigSecretPolicy | ''); setMigrationApproved(false);}}>
            <option value="">Choose how both credential fields are handled</option>
            <option value="keep_target">Keep both target credentials (empty if no target exists)</option>
            <option value="use_source">Replace both credentials from source (empty source clears target)</option>
          </Select>
          <p>Environment references remain references. No new remote model authorization is granted; model profiles and task routes are unchanged.</p>
          <p>Both choices use the source endpoints. Review those endpoints before choosing which credentials they will receive during later model calls.</p>
          {secretPolicy && <>
            <h4>Resulting settings (credentials hidden)</h4>
            <pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{JSON.stringify(migrationPreview.choices[secretPolicy].summary,null,2)}</pre>
            <p>Changed fields: {migrationPreview.choices[secretPolicy].changed_fields.join(', ') || 'None; an unversioned legacy target may still receive its first revision.'}</p>
            <Checkbox label="I reviewed this source, resulting settings and credential choice; replace the target configuration" checked={migrationApproved} onCheckedChange={setMigrationApproved} disabled={saving || migrationBusy} />
          </>}
          <Button disabled={saving || migrationBusy || !secretPolicy || !migrationApproved} onClick={() => void applyMigration()}>Apply reviewed migration</Button>
          <p>If source or target content changes, reload sources and preview again. Repeating an identical migration does not create another revision.</p>
        </>}
      </section>
    </div>
  );
}
