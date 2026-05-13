import { useState, useEffect } from 'react';

interface TestPlan {
  plan: string;
  case_count: number;
  steps: string[];
}

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
  const [projects, setProjects] = useState<string[]>(['default']);
  const [project, setProject] = useState('default');
  const [testType, setTestType] = useState('smoke');
  const [loading, setLoading] = useState(false);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [showPreview, setShowPreview] = useState(false);
  const [testPlan, setTestPlan] = useState<TestPlan | null>(null);

  useEffect(() => {
    window.getProjects().then(setProjects).catch(() => {});
  }, []);

  const handlePreview = async () => {
    setPreviewLoading(true);
    try {
      const result = await window.previewTestPlan(project, testType);
      setTestPlan(JSON.parse(result));
      setShowPreview(true);
    } catch {
      setTestPlan({ plan: 'Failed to load preview', case_count: 0, steps: [] });
      setShowPreview(true);
    } finally {
      setPreviewLoading(false);
    }
  };

  const handleRun = async () => {
    setShowPreview(false);
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
            <select className="input" value={project} onChange={(e) => setProject(e.target.value)}>
                {projects.map((p) => (
                  <option key={p} value={p}>{p}</option>
                ))}
              </select>
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
          <button className="btn btn-secondary" onClick={handlePreview} disabled={previewLoading}>
            {previewLoading ? 'Loading...' : 'Preview Plan'}
          </button>
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

      {showPreview && testPlan && (
        <div className="modal-overlay" onClick={() => setShowPreview(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>Test Plan Preview</h3>
              <button className="modal-close" onClick={() => setShowPreview(false)}>×</button>
            </div>
            <div className="modal-body">
              <p className="plan-summary">{testPlan.plan}</p>
              <div className="plan-cases">
                <strong>{testPlan.case_count} test cases</strong>
                <ul>
                  {testPlan.steps.map((step, i) => (
                    <li key={i}>{step}</li>
                  ))}
                </ul>
              </div>
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={() => setShowPreview(false)}>Cancel</button>
              <button className="btn btn-primary" onClick={handleRun}>Run Tests</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}