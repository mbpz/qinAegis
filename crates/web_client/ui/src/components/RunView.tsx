import { Button, Select } from './ui';
import { MorphingModal } from './motion/morphing-modal';
import { useState, useEffect, useRef } from 'react';
import { rpc } from '../utils/rpc';
import type { TestPlan } from '../types/rpc';
import JobProgress from './JobProgress';

const testTypes = [
  { value: 'smoke', label: 'Smoke Test', desc: 'Core path validation' },
  { value: 'functional', label: 'Functional Test', desc: 'Full feature coverage' },
  { value: 'performance', label: 'Performance Test', desc: 'Web Vitals measurement' },
  { value: 'stress', label: 'Stress Test', desc: 'Load and concurrency' },
];

export default function RunView() {
  const [error, setError] = useState('');
  const [projects, setProjects] = useState<string[]>([]);
  const [project, setProject] = useState('');
  const [testType, setTestType] = useState('smoke');
  const [jobId, setJobId] = useState<string | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [showPreview, setShowPreview] = useState(false);
  const [testPlan, setTestPlan] = useState<TestPlan | null>(null);
  const previewButton = useRef<HTMLButtonElement>(null);

  const closePreview = () => {
    setShowPreview(false);
    requestAnimationFrame(() => previewButton.current?.focus());
  };

  useEffect(() => {
    rpc.getProjects().then(ps => { setProjects(ps); setProject(current => ps.includes(current) ? current : ps[0] ?? ''); }).catch(() => {});
  }, []);

  const handlePreview = async () => {
    setPreviewLoading(true);
    try {
      const result = await rpc.previewTestPlan(project, testType);
      setTestPlan(result);
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
    setError('');
    try {
    const result = await rpc.runTests(project, testType);
    if (result.ok) {
      setJobId(result.jobId);
    }
    } catch (e) { setError(String(e)); }
  };

  return (
    <div className="view">
      <h2 className="view-title">Run Tests</h2>
      {error && <p role="alert">{error}</p>}

      <div className="card">
        <div className="card-title">Test Configuration</div>
        <div className="form-row">
          <div className="input-group">
            <label htmlFor="run-project">Project</label>
            <Select id="run-project" className="input" value={project} onValueChange={setProject}>
                {projects.map((p) => (
                  <option key={p} value={p}>{p}</option>
                ))}
              </Select>
          </div>
          <div className="input-group">
            <label htmlFor="run-type">Test Type</label>
            <Select id="run-type" className="input" value={testType} onValueChange={setTestType}>
              {testTypes.map((t) => (
                <option key={t.value} value={t.value}>{t.label}</option>
              ))}
            </Select>
          </div>
        </div>
        <div className="form-actions">
          <Button ref={previewButton} className="btn btn-secondary" onClick={handlePreview} disabled={previewLoading || !project}>
            {previewLoading ? 'Loading...' : 'Preview Plan'}
          </Button>
          <Button className="btn btn-primary" onClick={handleRun} disabled={!!jobId || !project}>
            {jobId ? 'Running...' : 'Run Tests'}
          </Button>
        </div>
      </div>

      <div className="output-header">
        <h3>Output</h3>
      </div>
      <JobProgress jobId={jobId} />

      <MorphingModal
        viewId={showPreview && testPlan ? 'test-plan-preview' : null}
        onClose={closePreview}
        placement="center"
        ariaLabel="Test Plan Preview"
        className="max-w-2xl max-h-[85vh] overflow-y-auto"
      >
        {testPlan && (
          <>
            <div className="modal-header">
              <h3>Test Plan Preview</h3>
              <Button variant="ghost" aria-label="Close test plan preview" className="modal-close" onClick={closePreview}>×</Button>
            </div>
            <div className="modal-body">
              <p className="plan-summary">{testPlan.plan}</p>
              <div className="plan-cases">
                <strong>{testPlan.case_count} test cases</strong>
                <ul>
                  {testPlan.steps.map((step, i) => (
                    <li key={step.id || i}>{step.id} · {step.name} · {step.type} · {step.priority}</li>
                  ))}
                </ul>
              </div>
            </div>
            <div className="modal-footer">
              <Button className="btn btn-secondary" onClick={closePreview}>Cancel</Button>
              <Button className="btn btn-primary" onClick={handleRun}>Run Tests</Button>
            </div>
          </>
        )}
      </MorphingModal>
    </div>
  );
}
