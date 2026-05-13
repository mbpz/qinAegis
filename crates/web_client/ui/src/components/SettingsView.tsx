import { useState, useEffect } from 'react';

interface ConfigState {
  llm: {
    api_key: string;
    base_url: string;
    model: string;
  };
  sandbox: {
    cdp_port: number;
  };
}

export default function SettingsView() {
  const [config, setConfig] = useState<ConfigState | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState('');

  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      const state = await window.getState();
      if (state?.config) {
        setConfig(state.config);
      }
    } catch (e) {
      console.error('Failed to load config:', e);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    if (!config) return;
    setSaving(true);
    try {
      await window.setConfig(config);
      setMessage('Settings saved successfully!');
      setTimeout(() => setMessage(''), 3000);
    } catch (e) {
      setMessage('Failed to save settings');
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

      <div className="card">
        <div className="card-title">AI Model Configuration</div>
        <div className="input-group">
          <label>API Key</label>
          <input
            className="input"
            type="password"
            placeholder="sk-..."
            value={config?.llm?.api_key || ''}
            onChange={(e) => setConfig({ ...config!, llm: { ...config!.llm, api_key: e.target.value } })}
          />
        </div>
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
    </div>
  );
}