import CaseHistoryPanel from './CaseHistoryPanel';
import { Button, Checkbox, Input, Select, TextArea } from './ui';
import { useState, useEffect, useRef } from 'react';
import { rpc } from '../utils/rpc';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from './motion/collapsible';
import type { CaseAsset, TestCase } from '../types/rpc';

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
  const [editing, setEditing] = useState<CaseAsset | null>(null);
  const [editApproved, setEditApproved] = useState(false);
  const [editSaving, setEditSaving] = useState(false);
  const editLock = useRef(false);
  const [reviewAsset, setReviewAsset] = useState<CaseAsset | null>(null);
  const [originalEdit, setOriginalEdit] = useState<CaseAsset | null>(null);
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
    ++inspection.current; setEditing(null); setEditApproved(false); setReviewAsset(null); setError(''); setUpdating(null);
    loadCases();
    return () => { ++request.current; ++inspection.current; };
  }, [selectedProject]);

  const loadCases = async () => {
    ++inspection.current; setReviewAsset(null); setEditApproved(false);
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
    if ((newStatus === 'reviewed' || newStatus === 'approved') && reviewAsset?.case_id !== caseId) {
      setError('Inspect the complete current case before review or approval.'); return;
    }
    const token = request.current;
    setUpdating(caseId);
    try {
      await rpc.updateCaseStatus(selectedProject, caseId, newStatus, reviewAsset?.case_id === caseId ? reviewAsset.digest : undefined);
      if (token === request.current) { setUpdating(null); await loadCases(); }
    } catch (e) {
      if (token === request.current) { setReviewAsset(null); setError('Failed to update case status: ' + e + '. Inspect the current case again.'); }
    } finally {
      if (token === request.current) setUpdating(null);
    }
  };

  async function openEditor(caseId: string) {
    if (editLock.current) return;
    const token = ++inspection.current; setError(''); setEditing(null); setOriginalEdit(null); setReviewAsset(null); setEditApproved(false);
    try { const asset = await rpc.getCaseAsset(selectedProject, caseId);
      if (token === inspection.current) {setEditing(asset); setOriginalEdit(asset);}
    } catch (e) { if (token === inspection.current) setError(String(e)); }
  }
  async function saveEdit() {
    if (!editing || !editApproved || editLock.current) return;
    editLock.current = true; setEditSaving(true); setError('');
    const token = inspection.current;
    try {
      const saved = await rpc.saveCaseAsset({project:selectedProject, case_id:editing.case_id,
        expected_digest:editing.digest, name:editing.name, yaml_script:editing.yaml_script,
        expected_result:editing.expected_result, approved:true});
      if (token === inspection.current) {setEditing(saved); setOriginalEdit(saved); setEditApproved(false); setReviewAsset(null); await loadCases();}
    } catch (e) {if (token === inspection.current) setError(`${String(e)}. Your edit is preserved. On a content revision conflict, reload the case and reconcile your changes.`);}
    finally {editLock.current = false; setEditSaving(false);}
  }

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
      {reviewAsset && <section className="card" aria-label="Inspected case version"><h3>Inspected case: {reviewAsset.case_id}</h3>
        <p>Name: {reviewAsset.name}. Status: {statusLabels[reviewAsset.status]}. Review and approval apply only to this loaded content revision. Inspect again after any status change or refresh.</p>
        <h4>Complete script and assertions</h4><pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{reviewAsset.yaml_script}</pre>
        <h4>Expected result</h4><pre style={{whiteSpace:'pre-wrap'}}>{reviewAsset.expected_result || '(No separate expected result recorded)'}</pre>
        <Collapsible><CollapsibleTrigger>Generation source, contract inputs and model</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{reviewAsset.provenance ? JSON.stringify(reviewAsset.provenance,null,2) : 'No recorded generation source. No current contract has been assigned retroactively.'}</pre></CollapsibleContent></Collapsible>
        <p>Source snapshots record generator inputs. Successful execution alone does not independently verify contract semantics.</p>
        <CaseHistoryPanel key={`${selectedProject}/${reviewAsset.case_id}/${reviewAsset.digest}`} project={selectedProject} asset={reviewAsset} onRestored={() => { ++inspection.current; setReviewAsset(null); setEditing(null); setEditApproved(false); void loadCases(); }} />
      </section>}

      {editing && <section className="card" aria-label="Case editor">
        <h3>Edit case: {editing.case_id}</h3>
        <p>Current status: {statusLabels[editing.status]}. Save creates a Draft requiring fresh human review and approval. Previous generated provenance is removed; historical runs retain their original facts.</p>
        <p>Content revision was captured when this case was loaded. Concurrent edits or status changes will reject this save.</p>
        {originalEdit && <Collapsible defaultOpen><CollapsibleTrigger>Original content for comparison</CollapsibleTrigger><CollapsibleContent>
          <p>Original name: {originalEdit.name}</p><h4>Original complete script and assertions</h4><pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{originalEdit.yaml_script}</pre>
          <h4>Original expected result</h4><pre style={{whiteSpace:'pre-wrap'}}>{originalEdit.expected_result || '(No separate expected result recorded)'}</pre>
        </CollapsibleContent></Collapsible>}
        <form onSubmit={event => {event.preventDefault(); void saveEdit();}}>
          <fieldset disabled={editSaving} style={{border:0,padding:0}}>
            <label htmlFor="edit-case-name">Name</label><Input id="edit-case-name" required value={editing.name} onValueChange={name => {setEditing({...editing,name}); setEditApproved(false); setReviewAsset(null);}} />
            <label htmlFor="edit-case-script">Complete YAML script and assertions</label><TextArea id="edit-case-script" value={editing.yaml_script} rows={10} onValueChange={yaml_script => {setEditing({...editing,yaml_script}); setEditApproved(false); setReviewAsset(null);}} />
            <label htmlFor="edit-case-expected">Expected result (legacy assets may have no separate text)</label><TextArea id="edit-case-expected" value={editing.expected_result} rows={3} onValueChange={expected_result => {setEditing({...editing,expected_result}); setEditApproved(false); setReviewAsset(null);}} />
            <Checkbox label="I reviewed these actions and business assertions; save as Draft and discard the old generation attribution." checked={editApproved} onCheckedChange={setEditApproved} />
            <Button type="submit" disabled={!editApproved || editSaving}>Save as Draft</Button>
            <Button type="button" disabled={editSaving} onClick={() => {++inspection.current; setEditing(null); setEditApproved(false);}}>Close editor</Button>
          </fieldset>
        </form>
      </section>}

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
                    <Button disabled={editSaving || updating !== null} onClick={() => void openEditor(tc.id)}>Edit case</Button>
                    <Button disabled={editSaving || updating !== null} onClick={async () => {
                      const token = ++inspection.current; setError(''); setReviewAsset(null); setEditing(null); setOriginalEdit(null); setEditApproved(false);
                      try { const asset = await rpc.getCaseAsset(selectedProject, tc.id); if (token === inspection.current) setReviewAsset(asset); }
                      catch (e) { if (token === inspection.current) setError(String(e)); }
                    }}>Inspect source</Button>
                    <Select aria-label={`Status for ${tc.id}`}
                      className="input"
                      value={tc.status}
                      onValueChange={(value) => handleStatusChange(tc.id, value)}
                      disabled={updating !== null || editSaving}
                      style={{ width: 'auto', padding: '4px 8px' }}
                    >
                      <option value="draft" disabled={tc.status !== 'draft' && !transitions[tc.status].includes('draft')}>Draft</option>
                      <option value="reviewed" disabled={tc.status !== 'reviewed' && (!transitions[tc.status].includes('reviewed') || reviewAsset?.case_id !== tc.id)}>Reviewed</option>
                      <option value="approved" disabled={tc.status !== 'approved' && (!transitions[tc.status].includes('approved') || reviewAsset?.case_id !== tc.id)}>Approved</option>
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