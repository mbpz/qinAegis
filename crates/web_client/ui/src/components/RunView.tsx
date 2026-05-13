import { useState } from 'react';

interface RunViewProps {
  output: string;
  onClear: () => void;
}

const testTypes = [
  { value: 'smoke', label: 'Smoke Test', desc: 'Core path validation' },
  { value: 'functional', label: 'Functional Test', desc: 'Full feature coverage' },
  { value: 'performance', label: 'Performance Test', desc: 'Web Vitals measurement' },
  { value: 'stress', label: 'Stress Test', desc: 'Load and concurrency' },
];

export default function RunView({ output, onClear }: RunViewProps) {
  const [project, setProject] = useState('default');
  const [testType, setTestType] = useState('smoke');
  const [loading, setLoading] = useState(false);

  const handleRun = async () => {
    setLoading(true);
    try {
      await window.runTests(project, testType);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="view">
      <h2 className="view-title">Run Tests</h2>

      <div className="card">
        <div className="card-title">Test Configuration</div>
        <div className="form-row">
          <div className="input-group">
            <label>Project</label>
            <input
              className="input"
              type="text"
              value={project}
              onChange={(e) => setProject(e.target.value)}
            />
          </div>
          <div className="input-group">
            <label>Test Type</label>
            <select className="input" value={testType} onChange={(e) => setTestType(e.target.value)}>
              {testTypes.map((t) => (
                <option key={t.value} value={t.value}>{t.label}</option>
              ))}
            </select>
          </div>
        </div>
        <div className="form-actions">
          <button className="btn btn-primary" onClick={handleRun} disabled={loading}>
            {loading ? 'Running...' : 'Run Tests'}
          </button>
          <button className="btn btn-secondary" onClick={onClear}>Clear Output</button>
        </div>
      </div>

      <div className="output-header">
        <h3>Output</h3>
      </div>
      <div className="output-log">{output || 'No output yet. Run tests to see results.'}</div>
    </div>
  );
}