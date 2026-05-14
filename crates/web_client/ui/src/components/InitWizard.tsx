import { useState } from 'react';

interface InitWizardProps {
  onComplete: () => void;
}

export default function InitWizard({ onComplete }: InitWizardProps) {
  const [step, setStep] = useState(1);
  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [baseUrl, setBaseUrl] = useState('https://api.minimax.chat/v1');
  const [model, setModel] = useState('MiniMax-VL-01');
  const [cdpPort, setCdpPort] = useState('9222');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');

  const handleSave = async () => {
    if (!apiKey.trim()) { setError('API key is required'); return; }
    setSaving(true);
    setError('');
    try {
      const config = {
        llm: {
          api_key: apiKey.trim(),
          base_url: baseUrl.trim(),
          model: model.trim(),
        },
        sandbox: {
          cdp_port: parseInt(cdpPort, 10) || 9222,
        },
      };
      await window.setConfig(config);
      onComplete();
    } catch (e) {
      setError('Failed to save config: ' + e);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="init-wizard-overlay">
      <div className="init-wizard">
        <div className="init-wizard-header">
          <h1>Welcome to QinAegis</h1>
          <p>Set up your AI model to get started</p>
        </div>

        <div className="init-wizard-step">
          <div className="step-indicator">Step {step} of 2</div>
          {step === 1 && (
            <div className="step-content">
              <h2>AI Model Configuration</h2>
              <div className="form-group">
                <label>API Key *</label>
                <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
                  <input
                    type={showKey ? 'text' : 'password'}
                    className="input"
                    placeholder="sk-..."
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    autoFocus
                    style={{ paddingRight: '40px' }}
                  />
                  <button
                    type="button"
                    onClick={() => setShowKey(!showKey)}
                    style={{
                      position: 'absolute',
                      right: '8px',
                      background: 'none',
                      border: 'none',
                      cursor: 'pointer',
                      color: 'var(--text-secondary)',
                      fontSize: '16px',
                      padding: '4px',
                    }}
                    title={showKey ? 'Hide' : 'Show'}
                  >
                    {showKey ? '🙈' : '👁'}
                  </button>
                </div>
              </div>
              <div className="form-group">
                <label>Base URL</label>
                <input
                  type="text"
                  className="input"
                  placeholder="https://api.minimax.chat/v1"
                  value={baseUrl}
                  onChange={(e) => setBaseUrl(e.target.value)}
                />
              </div>
              <div className="form-group">
                <label>Model</label>
                <input
                  type="text"
                  className="input"
                  placeholder="MiniMax-VL-01"
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                />
              </div>
              <button className="btn btn-primary" onClick={() => setStep(2)} disabled={!apiKey.trim()}>
                Next
              </button>
            </div>
          )}
          {step === 2 && (
            <div className="step-content">
              <h2>Sandbox Configuration</h2>
              <div className="form-group">
                <label>Chrome DevTools Port</label>
                <input
                  type="text"
                  className="input"
                  placeholder="9222"
                  value={cdpPort}
                  onChange={(e) => setCdpPort(e.target.value)}
                />
                <small style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
                  Default: 9222. Used for browser automation.
                </small>
              </div>
              {error && <div className="error-message">{error}</div>}
              <div className="step-actions">
                <button className="btn btn-secondary" onClick={() => setStep(1)}>
                  Back
                </button>
                <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
                  {saving ? 'Saving...' : 'Get Started'}
                </button>
              </div>
            </div>
          )}
        </div>

        <div className="init-wizard-footer">
          <small style={{ color: 'var(--text-secondary)' }}>
            Your API key is stored locally and never sent to any server except the AI model provider.
          </small>
        </div>
      </div>
    </div>
  );
}
