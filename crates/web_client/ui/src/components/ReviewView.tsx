import { useState, useEffect } from 'react';

interface TestCase {
  id: string;
  name: string;
  status: 'draft' | 'reviewed' | 'approved' | 'flaky' | 'archived';
  priority: string;
  type: string;
  requirement_id?: string;
}

const statusLabels: Record<string, string> = {
  draft: 'Draft',
  reviewed: 'Reviewed',
  approved: 'Approved',
  flaky: 'Flaky',
  archived: 'Archived',
};

const statusColors: Record<string, string> = {
  draft: 'var(--accent-yellow)',
  reviewed: 'var(--accent-blue)',
  approved: 'var(--accent-green)',
  flaky: 'var(--accent-red)',
  archived: 'var(--text-secondary)',
};

export default function ReviewView() {
  const [projects, setProjects] = useState<string[]>(['default']);
  const [selectedProject, setSelectedProject] = useState('default');
  const [cases, setCases] = useState<TestCase[]>([]);
  const [loading, setLoading] = useState(true);
  const [filterStatus, setFilterStatus] = useState<string>('all');
  const [updating, setUpdating] = useState<string | null>(null);

  useEffect(() => {
    window.getProjects().then(setProjects).catch(() => {});
  }, []);

  useEffect(() => {
    loadCases();
  }, [selectedProject]);

  const loadCases = async () => {
    setLoading(true);
    try {
      const result = await window.getReviewCases(selectedProject);
      setCases(result || []);
    } catch (e) {
      console.error('Failed to load cases:', e);
    } finally {
      setLoading(false);
    }
  };

  const handleStatusChange = async (caseId: string, newStatus: string) => {
    setUpdating(caseId);
    try {
      await window.updateCaseStatus(selectedProject, caseId, newStatus);
      await loadCases();
    } catch (e) {
      alert('Failed to update case status: ' + e);
    } finally {
      setUpdating(null);
    }
  };

  const filteredCases = filterStatus === 'all'
    ? cases
    : cases.filter(c => c.status === filterStatus);

  if (loading) {
    return (
      <div className="view">
        <h2 className="view-title">Review Test Cases</h2>
        <p style={{ color: 'var(--text-secondary)' }}>Loading...</p>
      </div>
    );
  }

  return (
    <div className="view">
      <h2 className="view-title">Review Test Cases</h2>

      <div className="card">
        <div className="card-title">Project</div>
        <select
          className="input"
          value={selectedProject}
          onChange={(e) => setSelectedProject(e.target.value)}
          style={{ maxWidth: '300px' }}
        >
          {projects.map((p) => (
            <option key={p} value={p}>{p}</option>
          ))}
        </select>
      </div>

      <div className="card">
        <div className="card-title">Filter by Status</div>
        <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
          {['all', 'draft', 'reviewed', 'approved', 'flaky', 'archived'].map((status) => (
            <button
              key={status}
              className={`btn ${filterStatus === status ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setFilterStatus(status)}
            >
              {status === 'all' ? 'All' : statusLabels[status]}
            </button>
          ))}
        </div>
      </div>

      <div className="card">
        <div className="card-title">Test Cases ({filteredCases.length})</div>
        {filteredCases.length === 0 ? (
          <p style={{ color: 'var(--text-secondary)', fontSize: '14px' }}>
            No cases found. Generate test cases first.
          </p>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Status</th>
                <th>Priority</th>
                <th>Type</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {filteredCases.map((tc) => (
                <tr key={tc.id}>
                  <td>{tc.name}</td>
                  <td>
                    <span style={{
                      color: statusColors[tc.status],
                      fontWeight: 600,
                    }}>
                      {statusLabels[tc.status]}
                    </span>
                  </td>
                  <td>{tc.priority}</td>
                  <td>{tc.type}</td>
                  <td>
                    <select
                      className="input"
                      value={tc.status}
                      onChange={(e) => handleStatusChange(tc.id, e.target.value)}
                      disabled={updating === tc.id}
                      style={{ width: 'auto', padding: '4px 8px' }}
                    >
                      <option value="draft">Draft</option>
                      <option value="reviewed">Reviewed</option>
                      <option value="approved">Approved</option>
                      <option value="flaky">Flaky</option>
                      <option value="archived">Archived</option>
                    </select>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}