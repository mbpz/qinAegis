import { useState } from 'react';

interface ExploreViewProps {
  output: string;
  onClear: () => void;
}

export default function ExploreView({ output, onClear }: ExploreViewProps) {
  const [url, setUrl] = useState('');
  const [depth, setDepth] = useState(3);
  const [loading, setLoading] = useState(false);

  const handleExplore = async () => {
    if (!url) return;
    setLoading(true);
    try {
      await window.runExplore(url, depth);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="view">
      <h2 className="view-title">Explore Project</h2>

      <div className="card">
        <div className="card-title">Project URL</div>
        <div className="input-group">
          <label>Target URL</label>
          <input
            className="input"
            type="text"
            placeholder="https://example.com"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
          />
        </div>
        <div className="form-row">
          <div className="input-group">
            <label>Exploration Depth</label>
            <input
              className="input"
              type="number"
              min="1"
              max="10"
              value={depth}
              onChange={(e) => setDepth(parseInt(e.target.value) || 3)}
            />
          </div>
        </div>
        <div className="form-actions">
          <button className="btn btn-primary" onClick={handleExplore} disabled={loading || !url}>
            {loading ? 'Exploring...' : 'Start Exploration'}
          </button>
          <button className="btn btn-secondary" onClick={onClear}>Clear Output</button>
        </div>
      </div>

      <div className="output-header">
        <h3>Output</h3>
      </div>
      <div className="output-log">{output || 'No output yet. Start an exploration to see results.'}</div>
    </div>
  );
}