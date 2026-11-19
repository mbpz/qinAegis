import { Button, Input } from './ui';
import { useState, useEffect, useRef } from 'react';
import { View } from '../App';
import { rpc } from '../utils/rpc';
import type { GateStatus } from '../types/rpc';

interface DashboardProps {
  projects: string[];
  onNavigate: (view: View) => void;
  onProjectsChanged: () => Promise<void>;
}

// Shared via ../utils/rpc

export default function Dashboard({ projects, onNavigate, onProjectsChanged }: DashboardProps) {
  const [showAddProject, setShowAddProject] = useState(false);
  const [newProjectName, setNewProjectName] = useState('');
  const [newProjectUrl, setNewProjectUrl] = useState('');
  const [newProjectStack, setNewProjectStack] = useState('');
  const [projectMessage, setProjectMessage] = useState('');
  const [addingProject, setAddingProject] = useState(false);
  const [gateStatus, setGateStatus] = useState<GateStatus | null>(null);
  const mounted = useRef(false);
  const adding = useRef(false);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  useEffect(() => {
    loadGateStatus();
  }, [projects]);

  const loadGateStatus = async () => {
    if (projects.length === 0) return;
    try {
      const status = await rpc.getGateStatus(projects[0]);
      setGateStatus(status);
    } catch (e) {
      console.error('Failed to load gate status:', e);
    }
  };

  const handleAddProject = async () => {
    if (adding.current) return;
    if (!newProjectName.trim()) { setProjectMessage('Project name is required.'); return; }
    try {
      const url = new URL(newProjectUrl.trim());
      if (!['http:', 'https:'].includes(url.protocol)) throw new Error('unsupported protocol');
    } catch { setProjectMessage('Enter a valid http:// or https:// project URL.'); return; }
    adding.current = true;
    setAddingProject(true);
    setProjectMessage('');
    try {
      const result = await rpc.createProject(newProjectName.trim(), newProjectUrl.trim(), newProjectStack.split(',').map(item => item.trim()).filter(Boolean));
      if (result.name !== newProjectName.trim()) throw new Error('Local service did not confirm project creation.');
      await onProjectsChanged();
      if (!mounted.current) return;
      setShowAddProject(false);
      setNewProjectName('');
      setNewProjectUrl('');
      setNewProjectStack('');
      setProjectMessage('Project created.');
    } catch (e) {
      if (mounted.current) setProjectMessage('Failed to create project: ' + String(e));
    } finally {
      adding.current = false;
      if (mounted.current) setAddingProject(false);
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
          {gateStatus?.execution_blocked && <p role="status" style={{fontSize: '13px', color: 'var(--text-secondary)'}}>Execution {gateStatus.execution_status ?? 'blocked'}. The quality gate is blocked even if individual scripts passed. Review execution evidence in Reports.</p>}
        </div>
      </div>

      <div className="card">
        <div className="card-title">Quick Actions</div>
        <div className="action-grid">
          <Button type="button" variant="secondary" className="action-card dashboard-action-card h-auto w-full flex-col whitespace-normal rounded-lg p-5 text-center" onClick={() => onNavigate('explore')}>
            <span className="action-icon" aria-hidden="true">🔍</span>
            <span className="action-title">Explore Project</span>
            <span className="action-desc">AI-powered project discovery</span>
          </Button>
          <Button type="button" variant="secondary" className="action-card dashboard-action-card h-auto w-full flex-col whitespace-normal rounded-lg p-5 text-center" onClick={() => onNavigate('generate')}>
            <span className="action-icon" aria-hidden="true">✨</span>
            <span className="action-title">Generate Tests</span>
            <span className="action-desc">Create test cases from requirements</span>
          </Button>
          <Button type="button" variant="secondary" className="action-card dashboard-action-card h-auto w-full flex-col whitespace-normal rounded-lg p-5 text-center" onClick={() => onNavigate('run')}>
            <span className="action-icon" aria-hidden="true">▶️</span>
            <span className="action-title">Run Tests</span>
            <span className="action-desc">Execute smoke or full test suite</span>
          </Button>
          <Button type="button" variant="secondary" className="action-card dashboard-action-card h-auto w-full flex-col whitespace-normal rounded-lg p-5 text-center" onClick={() => onNavigate('reports')}>
            <span className="action-icon" aria-hidden="true">📊</span>
            <span className="action-title">View Reports</span>
            <span className="action-desc">Test reports and quality gates</span>
          </Button>
        </div>
      </div>

      <div className="card">
        <div className="card-title">Projects</div>
        {projectMessage && <p role="status">{projectMessage}</p>}
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
          <Button type="button" className="btn btn-primary" onClick={() => setShowAddProject(true)}>
            + Add Project
          </Button>
        ) : (
          <div className="add-project-form">
            <label htmlFor="project-name">Project name</label>
            <Input
              id="project-name"
              type="text"
              placeholder="Project name (e.g. my-app)"
              value={newProjectName}
              onValueChange={(value) => setNewProjectName(value)}
              className="input"
              disabled={addingProject}
            />
            <label htmlFor="project-url">Project URL</label>
            <Input
              id="project-url"
              type="text"
              placeholder="Project URL (e.g. https://my-app.com)"
              value={newProjectUrl}
              onValueChange={(value) => setNewProjectUrl(value)}
              className="input"
              style={{ marginTop: '8px' }}
              disabled={addingProject}
            />
            <label htmlFor="project-stack">Technology stack (optional, comma separated)</label>
            <Input id="project-stack" className="input" placeholder="react, typescript" value={newProjectStack} onValueChange={value => setNewProjectStack(value)} disabled={addingProject} />
            <div className="form-actions" style={{ marginTop: '8px' }}>
              <Button type="button" className="btn btn-primary" onClick={handleAddProject} disabled={addingProject}>
                {addingProject ? 'Creating...' : 'Create Project'}
              </Button>
              <Button type="button" className="btn btn-secondary" disabled={addingProject} onClick={() => setShowAddProject(false)}>
                Cancel
              </Button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
