import { Button, Input } from './ui';
import { useState } from 'react';
import { rpc } from '../utils/rpc';
import JobProgress from './JobProgress';

export default function ExploreView() {
  const [url, setUrl] = useState('');
  const [depth, setDepth] = useState<string>('3');
  const [jobId, setJobId] = useState<string | null>(null);

  const handleDepthChange = (val: string) => {
    if (val === '' || /^\d+$/.test(val)) {
      setDepth(val);
    }
  };

  const handleExplore = async () => {
    if (!url) return;
    const depthNum = parseInt(depth, 10) || 3;
    const result = await rpc.runExplore(url, depthNum);
    if (result.ok) {
      setJobId(result.jobId);
    }
  };

  return (
    <div className="view">
      <h2 className="view-title">Explore Project</h2>

      <div className="card">
        <div className="card-title">Project URL</div>
        <div className="input-group">
          <label htmlFor="explore-target-url">Target URL</label>
          <Input
            id="explore-target-url"
            className="input"
            type="text"
            placeholder="https://example.com"
            value={url}
            onValueChange={setUrl}
          />
        </div>
        <div className="form-row">
          <div className="input-group">
            <label htmlFor="explore-depth">Exploration Depth</label>
            <Input
              id="explore-depth"
              className="input"
              type="text"
              min="1"
              max="10"
              value={depth}
              onValueChange={handleDepthChange}
            />
          </div>
        </div>
        <div className="form-actions">
          <Button className="btn btn-primary" onClick={handleExplore} disabled={!url || !!jobId}>
            {jobId ? 'Exploring...' : 'Start Exploration'}
          </Button>
        </div>
      </div>

      <div className="output-header">
        <h3>Output</h3>
      </div>
      <JobProgress jobId={jobId} />
    </div>
  );
}
