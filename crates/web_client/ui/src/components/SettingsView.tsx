import { Button, Checkbox, Input } from './ui';
import { useState, useEffect } from 'react';
import { rpc } from '../utils/rpc';
import type { AppConfig } from '../types/rpc';

const defaultConfig: AppConfig = {
  llm: { provider: 'minimax', api_key: '', base_url: 'https://api.minimax.chat/v1', model: 'MiniMax-M2.7' },
  sandbox: { cdp_port: 9222 },
  exploration: { max_depth: 3, max_pages_per_seed: 20 },
};

export default function SettingsView({ onSaved }: { onSaved?: () => void }) {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [showSecondary, setShowSecondary] = useState(false);
  const [message, setMessage] = useState('');
  const [showApiKey, setShowApiKey] = useState(false);

  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      const state = await rpc.getState();
      setConfig(state.config || structuredClone(defaultConfig));
    } catch (e) {
      setMessage('Failed to load settings: ' + (e as Error).message);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    if (!config) return;
    setSaving(true);
    try {
      const result = await rpc.setConfig(config);
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
        <div className="loading">
          <div className="spinner"></div>
          <span>Loading configuration...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="view">
      <h2 className="view-title">Settings</h2>

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

      <div className="form-actions">
        <Button type="button" className="btn btn-primary" onClick={handleSave} disabled={saving}>
          {saving ? 'Saving...' : 'Save Settings'}
        </Button>
      </div>
      </>}
    </div>
  );
}
