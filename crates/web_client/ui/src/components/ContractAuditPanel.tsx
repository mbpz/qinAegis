import { useEffect, useRef, useState } from 'react';
import { Button } from './ui';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from './motion/collapsible';
import { rpc } from '../utils/rpc';
import type { ContractAudit } from '../types/case_history';

export default function ContractAuditPanel() {
  const [audit, setAudit] = useState<ContractAudit | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const request = useRef(0), pending = useRef(false);
  useEffect(() => { void load(); return () => { ++request.current; pending.current = false; }; }, []);
  async function load() {
    if (pending.current) return; pending.current = true;
    const token = ++request.current; setBusy(true); setError('');
    try { const value = await rpc.getContractAudit(); if (token === request.current) setAudit(value); }
    catch (value) { if (token === request.current) setError(String(value)); }
    finally { if (token === request.current) { pending.current = false; setBusy(false); } }
  }
  return <section className="card" aria-label="Contract change audit"><h3>Contract change audit</h3>
    <p>Changes retain their source, author when known, reason and exact before/after fields. Model suggestions do not replace human review.</p>
    <Button variant="secondary" disabled={busy} onClick={() => void load()}>Refresh contract audit</Button>
    {audit && <>
      <p>Library revision: {audit.revision}</p>
      {!audit.history_complete && <p>Earlier audit history is unknown; missing events and authors have not been reconstructed.</p>}
      {audit.events.length === 0 && <p>No retained change events.</p>}
      {[...audit.events].reverse().map(event => <Collapsible key={event.revision}><CollapsibleTrigger>Revision {event.revision} from {event.parent_revision} · {event.context.source} · {event.created_at}</CollapsibleTrigger><CollapsibleContent><p>Author: {event.context.author ?? 'unknown'}; reason: {event.context.reason}</p><h4>Before</h4><pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{JSON.stringify(event.before,null,2)}</pre><h4>After</h4><pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{JSON.stringify(event.after,null,2)}</pre></CollapsibleContent></Collapsible>)}
    </>}
    {error && <p role="alert">{error}</p>}
  </section>;
}
