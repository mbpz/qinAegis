import { Button, Select } from './ui';
import { useState, useEffect, useRef } from 'react';
import { rpc } from '../utils/rpc';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from './motion/collapsible';
import type { CaseProvenance, TestCase } from '../types/rpc';

const statusLabels: Record<string, string> = {
  draft: 'Draft',
  reviewed: 'Reviewed',
  approved: 'Approved',
  flaky: 'Flaky',
  archived: 'Archived',
};

const transitions: Record<TestCase['status'], TestCase['status'][]> = {
  draft: ['reviewed'], reviewed: ['approved', 'draft'],
  approved: ['flaky', 'archived'], flaky: ['approved', 'archived'], archived: [],
};

const statusColors: Record<string, string> = {
  draft: 'var(--accent-yellow)',
  reviewed: 'var(--accent-blue)',
  approved: 'var(--accent-green)',
  flaky: 'var(--accent-red)',
  archived: 'var(--text-secondary)',
};


export default function ReviewView() {
  const [provenance, setProvenance] = useState<CaseProvenance | null>(null);
  const [inspected, setInspected] = useState('');
  const [error, setError] = useState('');
  const request = useRef(0);
  const inspection = useRef(0);
  const [projects, setProjects] = useState<string[]>([]);
  const [selectedProject, setSelectedProject] = useState('');
  const [cases, setCases] = useState<TestCase[]>([]);
  const [loading, setLoading] = useState(true);
  const [filterStatus, setFilterStatus] = useState<string>('all');
  const [updating, setUpdating] = useState<string | null>(null);

  useEffect(() => {
    rpc.getProjects().then(ps => { setProjects(ps); setSelectedProject(current => ps.includes(current) ? current : ps[0] ?? ''); }).catch(() => {});
  }, []);

  useEffect(() => {
    ++inspection.current; setInspected(''); setProvenance(null); setError(''); setUpdating(null);
    loadCases();
    return () => { ++request.current; ++inspection.current; };
  }, [selectedProject]);

  const loadCases = async () => {
    const token = ++request.current;
    if (!selectedProject) { setCases([]); setLoading(false); return; }
    setLoading(true);
    try {
      const result = await rpc.getReviewCases(selectedProject);
      if (token === request.current) setCases(result || []);
    } catch (e) {
      if (token === request.current) setError(String(e));
    } finally {
      if (token === request.current) setLoading(false);
    }
  };

  const handleStatusChange = async (caseId: string, newStatus: string) => {
    const current = cases.find(item => item.id === caseId);
    if (!current || current.status === newStatus) return;
    if (!transitions[current.status].includes(newStatus as TestCase['status'])) { setError('This status transition is not allowed. Review draft cases before approval.'); return; }
    const token = request.current;
    setUpdating(caseId);
    try {
      await rpc.updateCaseStatus(selectedProject, caseId, newStatus);
      if (token === request.current) await loadCases();
    } catch (e) {
      if (token === request.current) setError('Failed to update case status: ' + e);
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
      {error && <p role="alert">{error}</p>}
      {inspected && <div className="card"><h3>Generation source: {inspected}</h3><p>Source snapshots record generator inputs. Contract semantics are not independently verified by a successful script.</p>{provenance ? <Collapsible defaultOpen><CollapsibleTrigger>Original script, contract inputs and model</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace: 'pre-wrap', overflowWrap: 'anywhere'}}>{JSON.stringify(provenance, null, 2)}</pre></CollapsibleContent></Collapsible> : <p>No recorded generation source (legacy or independently authored case). No current contract has been assigned retroactively.</p>}</div>}

      <div className="card">
        <div className="card-title">Project</div>
        <Select aria-label="Review project"
          className="input"
          value={selectedProject}
          onValueChange={setSelectedProject}
          style={{ maxWidth: '300px' }}
        >
          {projects.map((p) => (
            <option key={p} value={p}>{p}</option>
          ))}
        </Select>
      </div>

      <div className="card">
        <div className="card-title">Filter by Status</div>
        <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
          {['all', 'draft', 'reviewed', 'approved', 'flaky', 'archived'].map((status) => (
            <Button
              key={status}
              className={`btn ${filterStatus === status ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setFilterStatus(status)}
            >
              {status === 'all' ? 'All' : statusLabels[status]}
            </Button>
          ))}
        </div>
      </div>

      <div className="card">
        <div className="card-title">Test Cases ({filteredCases.length})</div>
        <p>Inspect source and original assertions before moving Draft → Reviewed → Approved. Only approved cases can execute.</p>
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
                    <Button onClick={async () => {
                      const token = ++inspection.current; setError(''); setInspected(''); setProvenance(null);
                      try { const data = await rpc.getCaseProvenance(selectedProject, tc.id); if (token === inspection.current) { setProvenance(data); setInspected(tc.id); } }
                      catch (e) { if (token === inspection.current) setError(String(e)); }
                    }}>Inspect source</Button>
                    <Select aria-label={`Status for ${tc.id}`}
                      className="input"
                      value={tc.status}
                      onValueChange={(value) => handleStatusChange(tc.id, value)}
                      disabled={updating === tc.id}
                      style={{ width: 'auto', padding: '4px 8px' }}
                    >
                      <option value="draft" disabled={tc.status !== 'draft' && !transitions[tc.status].includes('draft')}>Draft</option>
                      <option value="reviewed" disabled={tc.status !== 'reviewed' && !transitions[tc.status].includes('reviewed')}>Reviewed</option>
                      <option value="approved" disabled={tc.status !== 'approved' && !transitions[tc.status].includes('approved')}>Approved</option>
                      <option value="flaky" disabled={tc.status !== 'flaky' && !transitions[tc.status].includes('flaky')}>Flaky</option>
                      <option value="archived" disabled={tc.status !== 'archived' && !transitions[tc.status].includes('archived')}>Archived</option>
                    </Select>
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