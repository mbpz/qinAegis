import { useState } from 'react';
import { rpc } from '../utils/rpc';
import JobProgress from './JobProgress';

export default function GenerateView() {
  const [requirement, setRequirement] = useState('');
  const [jobId, setJobId] = useState<string | null>(null);

  const handleGenerate = async () => {
    if (!requirement) return;
    const result = await rpc.runGenerate(requirement);
    if (result.ok) {
      setJobId(result.jobId);
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
          <button className="btn btn-primary" onClick={handleGenerate} disabled={!requirement || !!jobId}>
            {jobId ? 'Generating...' : 'Generate Cases'}
          </button>
        </div>
      </div>

      <div className="output-header">
        <h3>Output</h3>
      </div>
      <JobProgress jobId={jobId} />
    </div>
  );
}