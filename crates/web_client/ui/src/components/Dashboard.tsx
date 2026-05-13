import { useState, useEffect } from 'react';
import { View } from '../App';

interface DashboardProps {
  projects: string[];
  onNavigate: (view: View) => void;
}

interface GateStatus {
  e2e_pass_rate: number | null;
  e2e_pass_rate_display: string;
  performance: string | null;
  stress: string | null;
  has_runs: boolean;
  last_run_passed?: number;
  last_run_total?: number;
}

export default function Dashboard({ projects, onNavigate }: DashboardProps) {
  const [showAddProject, setShowAddProject] = useState(false);
  const [newProjectName, setNewProjectName] = useState('');
  const [newProjectUrl, setNewProjectUrl] = useState('');
  const [addingProject, setAddingProject] = useState(false);
  const [gateStatus, setGateStatus] = useState<GateStatus | null>(null);

  useEffect(() => {
    loadGateStatus();
  }, [projects]);

  const loadGateStatus = async () => {
    if (projects.length === 0) return;
    try {
      const status = await window.getGateStatus(projects[0]);
      setGateStatus(status);
    } catch (e) {
      console.error('Failed to load gate status:', e);
    }
  };

  const handleAddProject = async () => {
    if (!newProjectName.trim()) { alert('Project name is required'); return; }
    if (!newProjectUrl.trim()) { alert('Project URL is required'); return; }
    setAddingProject(true);
    try {
      await window.createProject(newProjectName.trim(), newProjectUrl.trim(), ['react']);
      setShowAddProject(false);
      setNewProjectName('');
      setNewProjectUrl('');
      window.location.reload();
    } catch (e) {
      alert('Failed to create project: ' + e);
    } finally {
      setAddingProject(false);
    }
  };

  return (
    <div className="view">
      <h2 className="view-title">Dashboard</h2>

      <div className="stats-grid">
        <div className="stat-card pass">
          <div className="stat-value">{gateStatus?.last_run_passed ?? '--'}</div>
          <div className="stat-label">Passed</div>
        </div>
        <div className="stat-card fail">
          <div className="stat-value">
            {gateStatus?.has_runs ? (gateStatus.last_run_total ?? 0) - (gateStatus.last_run_passed ?? 0) : '--'}
          </div>
          <div className="stat-label">Failed</div>
        </div>
        <div className="stat-card pending">
          <div className="stat-value">{projects.length}</div>
          <div className="stat-label">Projects</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{gateStatus?.e2e_pass_rate_display ?? '--'}</div>
          <div className="stat-label">Pass Rate</div>
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
          <div className="action-card" onClick={() => onNavigate('reports')}>
            <div className="action-icon">📊</div>
            <div className="action-title">View Reports</div>
            <div className="action-desc">Test reports and quality gates</div>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-title">Projects</div>
        {projects.length === 0 ? (
          <p style={{ color: 'var(--text-secondary)', fontSize: '14px', marginBottom: '12px' }}>
            No projects yet. Add your first project to get started.
          </p>
        ) : (
          <div className="project-list" style={{ marginBottom: '12px' }}>
            {projects.map((p) => (
              <div key={p} className="project-item">
                <span>📁</span>
                <span className="project-name">{p}</span>
              </div>
            ))}
          </div>
        )}
        {!showAddProject ? (
          <button className="btn btn-primary" onClick={() => setShowAddProject(true)}>
            + Add Project
          </button>
        ) : (
          <div className="add-project-form">
            <input
              type="text"
              placeholder="Project name (e.g. my-app)"
              value={newProjectName}
              onChange={(e) => setNewProjectName(e.target.value)}
              className="input"
            />
            <input
              type="text"
              placeholder="Project URL (e.g. https://my-app.com)"
              value={newProjectUrl}
              onChange={(e) => setNewProjectUrl(e.target.value)}
              className="input"
              style={{ marginTop: '8px' }}
            />
            <div className="form-actions" style={{ marginTop: '8px' }}>
              <button className="btn btn-primary" onClick={handleAddProject} disabled={addingProject}>
                {addingProject ? 'Creating...' : 'Create Project'}
              </button>
              <button className="btn btn-secondary" onClick={() => setShowAddProject(false)}>
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
