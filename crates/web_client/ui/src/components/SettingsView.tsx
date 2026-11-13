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

      {!config && <button className="btn btn-primary" onClick={() => { setLoading(true); loadConfig(); }}>Retry loading settings</button>}
      {config && <>
      <div className="card">
        <div className="card-title">AI Model Configuration</div>
        <div className="input-group"><label>Provider name (Chat Completions API compatible)</label><input className="input" value={config.llm.provider || ''} onChange={e => setConfig({ ...config, llm: { ...config.llm, provider: e.target.value } })} /></div>
        <div className="input-group">
          <label>API Key</label>
          <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
            <input
              type={showApiKey ? 'text' : 'password'}
              className="input"
              placeholder={config.llm.api_key_configured ? 'Key stored — leave empty to keep' : 'sk-...'}
              value={config?.llm?.api_key || ''}
              onChange={(e) => setConfig({ ...config!, llm: { ...config!.llm, api_key: e.target.value } })}
              style={{ paddingRight: '40px' }}
            />
            <button
              type="button"
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
              {showApiKey ? '👁' : '👁‍🗨'}
            </button>
          </div>
        </div>
        {config.llm.api_key_configured && <label><input type="checkbox" checked={!!config.llm.clear_api_key} onChange={e => setConfig({ ...config, llm: { ...config.llm, clear_api_key: e.target.checked } })} /> Clear stored primary key</label>}
        <div className="form-row">
          <div className="input-group">
            <label>Base URL</label>
            <input
              className="input"
              type="text"
              placeholder="https://api.minimax.chat/v1"
              value={config?.llm?.base_url || ''}
              onChange={(e) => setConfig({ ...config!, llm: { ...config!.llm, base_url: e.target.value } })}
            />
          </div>
          <div className="input-group">
            <label>Model</label>
            <input
              className="input"
              type="text"
              placeholder="MiniMax-VL-01"
              value={config?.llm?.model || ''}
              onChange={(e) => setConfig({ ...config!, llm: { ...config!.llm, model: e.target.value } })}
            />
          </div>
        </div>
      </div>

      <div className="card">
        <div
          className="card-title"
          style={{ cursor: 'pointer', userSelect: 'none' }}
          onClick={() => setShowSecondary(!showSecondary)}
        >
          {showSecondary ? '▼' : '▶'} Secondary LLM (Optional — for fallback & complex tasks)
        </div>
        {showSecondary && (
          <>
            <div className="input-group">
              <label>Provider</label>
              <input
                className="input"
                type="text"
                placeholder="minimax"
                value={config?.llm?.secondary_provider || ''}
                onChange={(e) => setConfig({ ...config!, llm: { ...config!.llm, secondary_provider: e.target.value } })}
              />
            </div>
            <div className="input-group">
              <label>API Key</label>
              <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
                <input
                  type="password"
                  className="input"
                  placeholder={config.llm.secondary_api_key_configured ? 'Key stored — leave empty to keep' : 'sk-...'}
                  value={config?.llm?.secondary_api_key || ''}
                  onChange={(e) => setConfig({ ...config!, llm: { ...config!.llm, secondary_api_key: e.target.value } })}
                  style={{ paddingRight: '40px' }}
                />
              </div>
            </div>
            <div className="form-row">
              <div className="input-group">
                <label>Base URL</label>
                <input
                  className="input"
                  type="text"
                  placeholder="https://api.minimax.chat/v1"
                  value={config?.llm?.secondary_base_url || ''}
                  onChange={(e) => setConfig({ ...config!, llm: { ...config!.llm, secondary_base_url: e.target.value } })}
                />
              </div>
              <div className="input-group">
                <label>Model</label>
                <input
                  className="input"
                  type="text"
                  placeholder="Qwen3-VL-7B"
                  value={config?.llm?.secondary_model || ''}
                  onChange={(e) => setConfig({ ...config!, llm: { ...config!.llm, secondary_model: e.target.value } })}
                />
              </div>
            </div>
            {config.llm.secondary_api_key_configured && <label><input type="checkbox" checked={!!config.llm.clear_secondary_api_key} onChange={e => setConfig({ ...config, llm: { ...config.llm, clear_secondary_api_key: e.target.checked } })} /> Clear stored secondary key</label>}
            <p style={{ fontSize: '12px', color: 'var(--text-secondary)', margin: '8px 0 0' }}>
              The secondary LLM is used as a fallback when the primary fails, or for complex vision tasks.
              Leave empty to use only the primary provider.
            </p>
          </>
        )}
      </div>

      <div className="card">
        <div className="card-title">Sandbox Configuration</div>
        <div className="input-group">
          <label>CDP Port</label>
          <input
            className="input"
            type="number"
            placeholder="9222"
            value={config?.sandbox?.cdp_port || 9222}
            onChange={(e) => setConfig({ ...config!, sandbox: { ...config!.sandbox, cdp_port: parseInt(e.target.value) || 9222 } })}
          />
        </div>
      </div>

      <div className="form-actions">
        <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
          {saving ? 'Saving...' : 'Save Settings'}
        </button>
      </div>
      </>}
    </div>
  );
}
