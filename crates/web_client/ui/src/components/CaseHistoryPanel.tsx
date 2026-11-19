import { caseRestoreReason, restorableCaseRevision } from '../utils/case_history';
import { useEffect, useRef, useState } from 'react';
import { Button, Checkbox, Input, Select } from './ui';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from './motion/collapsible';
import { rpc } from '../utils/rpc';
import type { CaseAsset } from '../types/rpc';
import type { CaseHistory, CaseRevision } from '../types/case_history';

export default function CaseHistoryPanel({ project, asset, onRestored }: { project: string; asset: CaseAsset; onRestored: () => void }) {
  const [history, setHistory] = useState<CaseHistory | null>(null);
  const [selected, setSelected] = useState('');
  const [revision, setRevision] = useState<CaseRevision | null>(null);
  const [reason, setReason] = useState('');
  const [approved, setApproved] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const identity = useRef(0), inspection = useRef(0), pending = useRef(false);
  useEffect(() => {
    const token = ++identity.current; ++inspection.current; pending.current = false;
    setHistory(null); setSelected(''); setRevision(null); setReason(''); setApproved(false); setBusy(false); setError('');
    void rpc.getCaseHistory(project, asset.case_id).then(value => { if (token === identity.current) { if (value.case_id !== asset.case_id) throw new Error('Case history identity mismatch'); setHistory(value); } }).catch(value => { if (token === identity.current) setError(String(value)); });
    return () => { ++identity.current; ++inspection.current; };
  }, [project, asset.case_id, asset.digest]);
  async function inspect(value: string) {
    if (pending.current) return;
    const token = ++inspection.current, current = identity.current;
    setSelected(value); setRevision(null); setApproved(false); setError('');
    if (!value) return;
    try { const result = await rpc.getCaseRevision(project, asset.case_id, Number(value)); if (current === identity.current && token === inspection.current) { if (result.revision !== Number(value)) throw new Error('Case revision identity mismatch'); setRevision(result); } }
    catch (value) { if (current === identity.current && token === inspection.current) setError(String(value)); }
  }
  async function restore() {
    if (pending.current || !approved || !revision || !restorableCaseRevision(revision, asset.case_id) || !caseRestoreReason(reason)) return;
    const current = identity.current; pending.current = true; setBusy(true); setApproved(false); setError('');
    try { const restored = await rpc.restoreCaseRevision(project, asset.case_id, revision.revision, asset.digest, reason); if (current === identity.current) { if (restored.case_id !== asset.case_id || restored.status !== 'draft' || restored.provenance !== null) throw new Error('Restored case did not return an unattributed Draft; inspect the current case'); onRestored(); } }
    catch (value) { if (current === identity.current) setError(`${value}. No approval is inherited. Inspect the current case before resolving a conflict.`); }
    finally { if (current === identity.current) { pending.current = false; setBusy(false); } }
  }
  return <section aria-label="Case revision history" style={{ marginTop: 20 }}><h4>Case revision history</h4>
    <p>Current recorded revision: {asset.revision ?? 'unknown'}. Each restored version creates a new Draft; it does not rewrite history or inherit approval or generated provenance.</p>
    {history && <>
      {!history.history_complete && <p role="status">Earlier history is unknown. The retained baseline does not reconstruct missing authors or changes.</p>}
      <Select aria-label="Historical case revision" value={selected} disabled={busy} onValueChange={value => void inspect(value)}><option value="">Select a revision to inspect</option>{history.revisions.map(item => <option key={item.revision} value={String(item.revision)}>Revision {item.revision} · {item.operation} · {item.context.source}</option>)}</Select>
      {revision && <>
        <p>Revision {revision.revision}; parent {revision.parent_revision ?? 'unknown'} · {revision.operation} · author {revision.context.author ?? 'unknown'} · source {revision.context.source} · recorded {revision.created_at ?? 'unknown'}</p><p>Reason: {revision.context.reason}</p>
        <Collapsible defaultOpen><CollapsibleTrigger>Complete historical snapshot</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{revision.snapshot ? JSON.stringify(revision.snapshot,null,2) : 'No complete snapshot recorded for this revision.'}</pre></CollapsibleContent></Collapsible>
        <Collapsible><CollapsibleTrigger>Current inspected content for comparison</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{JSON.stringify(asset,null,2)}</pre></CollapsibleContent></Collapsible>
        <Input aria-label="Reason for restoring case revision" value={reason} disabled={busy} onValueChange={value => { setReason(value); setApproved(false); }} />
        <Checkbox checked={approved} disabled={busy || !restorableCaseRevision(revision, asset.case_id)} onCheckedChange={value => setApproved(value === true)} label="I compared this historical content with the current case; create a new Draft requiring fresh review and approval" />
        <Button disabled={busy || !approved || !restorableCaseRevision(revision, asset.case_id) || !caseRestoreReason(reason)} onClick={() => void restore()}>Create new Draft from this revision</Button>
      </>}
    </>}
    {error && <p role="alert">{error}</p>}
  </section>;
}
