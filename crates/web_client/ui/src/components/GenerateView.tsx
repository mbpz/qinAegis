import { useState } from 'react';

interface GenerateViewProps {
  output: string;
  onClear: () => void;
}

// Direct RPC call helper
async function rpcCall(method: string, params: object = {}): Promise<any> {
  const paramsStr = encodeURIComponent(JSON.stringify(params));
  const url = 'app://localhost/invoke?method=' + encodeURIComponent(method) + '&params=' + paramsStr;
  const resp = await fetch(url);
  const text = await resp.text();
  return JSON.parse(text);
}

export default function GenerateView({ output, onClear }: GenerateViewProps) {
  const [requirement, setRequirement] = useState('');
  const [loading, setLoading] = useState(false);

  const handleGenerate = async () => {
    if (!requirement) return;
    setLoading(true);
    try {
      await rpcCall('runGenerate', { requirement });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="view">
      <h2 className="view-title">Generate Test Cases</h2>

      <div className="card">
        <div className="card-title">Requirement</div>
        <div className="input-group">
          <label>Test Requirement</label>
          <input
            className="input"
            type="text"
            placeholder="e.g., User login with email and password"
            value={requirement}
            onChange={(e) => setRequirement(e.target.value)}
          />
        </div>
        <div className="form-actions">
          <button className="btn btn-primary" onClick={handleGenerate} disabled={loading || !requirement}>
            {loading ? 'Generating...' : 'Generate Cases'}
          </button>
          <button className="btn btn-secondary" onClick={onClear}>Clear Output</button>
        </div>
      </div>

      <div className="output-header">
        <h3>Output</h3>
      </div>
      <div className="output-log">{output || 'No output yet. Generate test cases to see results.'}</div>
    </div>
  );
}