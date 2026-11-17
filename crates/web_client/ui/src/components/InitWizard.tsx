import { Button, Input } from './ui';
import { useState } from 'react';
import { rpc } from '../utils/rpc';
import type { AppConfig } from '../types/rpc';

interface InitWizardProps {
  onComplete: () => void;
}

export default function InitWizard({ onComplete }: InitWizardProps) {
  const [step, setStep] = useState(1);
  const [apiKey, setApiKey] = useState('');
  const [baseUrl, setBaseUrl] = useState('https://api.minimax.chat/v1');
  const [model, setModel] = useState('MiniMax-VL-01');
  const [cdpPort, setCdpPort] = useState('9222');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [showKey, setShowKey] = useState(false);

  const handleSave = async () => {
    if (!apiKey.trim()) { setError('API key is required'); return; }
    setSaving(true);
    setError('');
    try {
      const config: AppConfig = {
        llm: {
          api_key: apiKey.trim(),
          base_url: baseUrl.trim(),
          model: model.trim(),
          secondary_api_key: '',
          secondary_base_url: '',
          secondary_model: '',
        },
        sandbox: {
          cdp_port: parseInt(cdpPort, 10) || 9222,
        },
      };
      const state = await rpc.getState();
      const result = await rpc.setConfig(config, state.config_revision);
      if (result.ok) {
        onComplete();
      } else {
        setError('Failed to save config');
      }
    } catch (e) {
      console.error('setConfig error:', e);
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
                <label htmlFor="wizard-api-key">API Key *</label>
                <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
                  <Input
                    type={showKey ? 'text' : 'password'}
                    className="input"
                    placeholder="sk-..."
                    id="wizard-api-key" value={apiKey}
                    onValueChange={setApiKey}
                    autoFocus
                    style={{ paddingRight: '40px' }}
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    aria-label={showKey ? 'Hide API key' : 'Show API key'}
                    aria-pressed={showKey}
                    onClick={() => setShowKey(!showKey)}
                    style={{
                      position: 'absolute',
                      right: '8px',
                      background: 'none',
                      border: 'none',
                      cursor: 'pointer',
                      padding: '4px',
                      color: 'var(--text-secondary)',
                    }}
                    title={showKey ? 'Hide API Key' : 'Show API Key'}
                  >
                    {showKey ? '👁' : '👁‍🗨'}
                  </Button>
                </div>
              </div>
              <div className="form-group">
                <label htmlFor="wizard-base-url">Base URL</label>
                <Input
                  type="text"
                  className="input"
                  placeholder="https://api.minimax.chat/v1"
                  id="wizard-base-url" value={baseUrl}
                  onValueChange={setBaseUrl}
                />
              </div>
              <div className="form-group">
                <label htmlFor="wizard-model">Model</label>
                <Input
                  type="text"
                  className="input"
                  placeholder="MiniMax-VL-01"
                  id="wizard-model" value={model}
                  onValueChange={setModel}
                />
              </div>
              <Button className="btn btn-primary" onClick={() => setStep(2)} disabled={!apiKey.trim()}>
                Next
              </Button>
            </div>
          )}
          {step === 2 && (
            <div className="step-content">
              <h2>Sandbox Configuration</h2>
              <div className="form-group">
                <label htmlFor="wizard-cdp-port">Chrome DevTools Port</label>
                <Input
                  type="text"
                  className="input"
                  placeholder="9222"
                  id="wizard-cdp-port" value={cdpPort}
                  onValueChange={setCdpPort}
                />
                <small style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
                  Default: 9222. Used for browser automation.
                </small>
              </div>
              {error && <div className="error-message">{error}</div>}
              <div className="step-actions">
                <Button className="btn btn-secondary" onClick={() => setStep(1)}>
                  Back
                </Button>
                <Button className="btn btn-primary" onClick={handleSave} disabled={saving}>
                  {saving ? 'Saving...' : 'Get Started'}
                </Button>
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
