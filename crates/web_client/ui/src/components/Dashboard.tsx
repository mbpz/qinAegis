import { View } from '../App';

interface DashboardProps {
  projects: string[];
  onNavigate: (view: View) => void;
}

export default function Dashboard({ projects, onNavigate }: DashboardProps) {
  return (
    <div className="view">
      <h2 className="view-title">Dashboard</h2>

      <div className="stats-grid">
        <div className="stat-card pass">
          <div className="stat-value">0</div>
          <div className="stat-label">Passed</div>
        </div>
        <div className="stat-card fail">
          <div className="stat-value">0</div>
          <div className="stat-label">Failed</div>
        </div>
        <div className="stat-card pending">
          <div className="stat-value">{projects.length}</div>
          <div className="stat-label">Projects</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">0</div>
          <div className="stat-label">Total Runs</div>
        </div>
      </div>

      <div className="card">
        <div className="card-title">Quick Actions</div>
        <div className="action-grid">
          <div className="action-card" onClick={() => onNavigate('explore')}>
            <div className="action-icon">🔍</div>
            <div className="action-title">Explore Project</div>
            <div className="action-desc">AI-powered project discovery</div>
          </div>
          <div className="action-card" onClick={() => onNavigate('generate')}>
            <div className="action-icon">✨</div>
            <div className="action-title">Generate Tests</div>
            <div className="action-desc">Create test cases from requirements</div>
          </div>
          <div className="action-card" onClick={() => onNavigate('run')}>
            <div className="action-icon">▶️</div>
            <div className="action-title">Run Tests</div>
            <div className="action-desc">Execute smoke or full test suite</div>
          </div>
        </div>
      </div>

      {projects.length > 0 && (
        <div className="card">
          <div className="card-title">Projects</div>
          <div className="project-list">
            {projects.map((p) => (
              <div key={p} className="project-item">
                <span>📁</span>
                <span className="project-name">{p}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}