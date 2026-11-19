import { localTraceViewerUrl, validateArtifactResponse } from '../utils/capture';
import { useEffect, useRef, useState } from 'react';
import { Button, Checkbox } from './ui';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from './motion/collapsible';
import { rpc } from '../utils/rpc';
import type { ProcessEvidence as Evidence, TraceViewer } from '../types/capture';

export default function ProcessEvidence({ project, runId }: { project: string; runId: string }) {
  const [evidence, setEvidence] = useState<Evidence | null>(null);
  const [error, setError] = useState('');
  const [preview, setPreview] = useState<{ image?: string; text?: string; label: string } | null>(null);
  const [approved, setApproved] = useState(false);
  const [busy, setBusy] = useState(false);
  const [viewer, setViewer] = useState<TraceViewer | null>(null);
  const viewerRef = useRef<TraceViewer | null>(null);
  const request = useRef(0);
  const action = useRef(0);
  const pending = useRef(false);
  const downloads = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  useEffect(() => {
    const token = ++request.current; ++action.current; pending.current = false;
    setEvidence(null); setError(''); setPreview(null); setApproved(false); setBusy(false); setViewer(null);
    void rpc.readProcessEvidence(project, runId).then(value => { if (token === request.current) { if (value.project !== project || value.run_id !== runId) throw new Error('Process evidence identity mismatch'); setEvidence(value); } }).catch(value => { if (token === request.current) setError(String(value)); });
    return () => { for (const [url, timer] of downloads.current) { clearTimeout(timer); URL.revokeObjectURL(url); } downloads.current.clear(); ++request.current; ++action.current; const old = viewerRef.current; viewerRef.current = null; if (old) void rpc.stopTraceViewer(old.viewer_id).catch(() => {}); };
  }, [project, runId]);
  async function artifact(caseId: string, id: string, download: boolean) {
    if (pending.current) return;
    pending.current = true;
    const identity = request.current, token = ++action.current; setBusy(true); setError('');
    try {
      const value = await rpc.readProcessArtifact(project, runId, caseId, id);
      if (identity !== request.current || token !== action.current) return;
      const expected = evidence?.cases.find(item => item.case_id === caseId)?.receipt?.artifacts.find(item => item.id === id);
      if (!expected) throw new Error('Selected receipt no longer available');
      validateArtifactResponse(expected, value);
      const bytes = Uint8Array.from(atob(value.base64), character => character.charCodeAt(0));
      if (bytes.length !== value.bytes) throw new Error('Artifact byte count mismatch');
      const hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))).map(byte => byte.toString(16).padStart(2, '0')).join('');
      if (identity !== request.current || token !== action.current) return;
      if (hash !== value.sha256) throw new Error('Artifact digest mismatch');
      const image = value.kind === 'screenshot';
      if (download) {
        const url = URL.createObjectURL(new Blob([bytes], { type: image ? 'image/png' : value.kind === 'trace' ? 'application/zip' : 'application/json' }));
        const link = document.createElement('a'); link.href = url; link.download = value.file_name; link.click(); downloads.current.set(url, setTimeout(() => { URL.revokeObjectURL(url); downloads.current.delete(url); }, 1000));
      } else if (image) setPreview({ label: `${caseId} / ${id}`, image: `data:image/png;base64,${value.base64}` });
      else if (['console', 'network', 'steps'].includes(value.kind)) setPreview({ label: `${caseId} / ${id}`, text: JSON.stringify(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)), null, 2) });
      else throw new Error('This artifact requires a download or the isolated Trace viewer');
    } catch (value) { if (identity === request.current && token === action.current) setError(String(value)); }
    finally { if (identity === request.current && token === action.current) { pending.current = false; setBusy(false); } }
  }
  async function openTrace(caseId: string, id: string) {
    if (!approved || pending.current) return;
    pending.current = true;
    const identity = request.current; setBusy(true); setApproved(false); setError('');
    try {
      const previous = viewerRef.current;
      if (previous) { await rpc.stopTraceViewer(previous.viewer_id); if (identity === request.current && viewerRef.current?.viewer_id === previous.viewer_id) { viewerRef.current = null; setViewer(null); } }
      if (identity !== request.current) return;
      const value = await rpc.startTraceViewer(project, runId, caseId, id);
      if (identity !== request.current) { void rpc.stopTraceViewer(value.viewer_id).catch(() => {}); return; }
      try { localTraceViewerUrl(value.url); } catch (error) { void rpc.stopTraceViewer(value.viewer_id).catch(() => {}); throw error; }
      viewerRef.current = value; setViewer(value);
    } catch (value) { if (identity === request.current) setError(`${value}. The original Trace can still be downloaded.`); }
    finally { if (identity === request.current) { pending.current = false; setBusy(false); } }
  }
  async function closeViewer() {
    const current = viewerRef.current; if (!current || pending.current) return;
    pending.current = true;
    const identity = request.current; setBusy(true);
    try { await rpc.stopTraceViewer(current.viewer_id); if (identity === request.current) { viewerRef.current = null; setViewer(null); } }
    catch (value) { if (identity === request.current) setError(String(value)); }
    finally { if (identity === request.current) { pending.current = false; setBusy(false); } }
  }
  return <section className="card" aria-label="Process evidence">
    <h3>Process evidence</h3><p>Diagnostics do not change business results. Viewing does not execute actions, restore a browser session or replay side effects.</p>
    {!evidence && !error && <p role="status">Loading process evidence…</p>}
    {evidence && <>
      <p>Run: {evidence.run_id} · {evidence.status}</p>
      {evidence.policy ? <Collapsible><CollapsibleTrigger>Frozen capture policy · revision {evidence.policy.revision} · {evidence.policy.policy.mode}</CollapsibleTrigger><CollapsibleContent><pre>{JSON.stringify(evidence.policy, null, 2)}</pre></CollapsibleContent></Collapsible> : <p>legacy_unknown: this historical run has no recorded capture policy.</p>}
      <Checkbox checked={approved} disabled={busy} onCheckedChange={value => setApproved(value === true)} label="Allow opening sensitive Trace content in an isolated local viewer" />
      {evidence.cases.map(item => <div key={item.case_id} style={{ marginTop: 16 }}><h4>{item.name} ({item.case_id})</h4>
        {!item.receipt ? <p>No final capture receipt: unknown. Cancellation or interruption does not prove diagnostics were collected.</p> : <>
          <p>Receipt: {item.receipt.status}</p>
          {item.receipt.channels.map(channel => <p key={channel.kind}>{channel.kind}: {channel.status} · {channel.bytes} bytes{channel.reason ? ` · ${channel.reason}` : ''}</p>)}
          {item.receipt.artifacts.map(file => { const availability = item.artifacts.find(candidate => candidate.id === file.id); const available = availability?.available === true;
            return <div key={file.id} style={{ marginTop: 8 }}><p>{file.kind} / {file.id} · {file.bytes} bytes{!available && ` · unavailable: ${availability?.reason || 'not verified'}`}</p>
              <Button variant="secondary" disabled={busy || !available} onClick={() => void artifact(item.case_id, file.id, true)}>Download {file.id}</Button>
              {file.kind === 'trace' ? <Button disabled={busy || !available || !approved} onClick={() => void openTrace(item.case_id, file.id)}>Prepare offline Trace viewer</Button> : <Button disabled={busy || !available} onClick={() => void artifact(item.case_id, file.id, false)}>View {file.id}</Button>}
            </div>; })}
        </>}
      </div>)}
    </>}
    {viewer && <div role="status"><p>Playwright viewer {viewer.version}; expires after {viewer.expires_in_seconds} seconds. This viewer is scoped to the selected Trace.</p><a href={viewer.url} target="_blank" rel="noopener noreferrer">Open offline Trace</a><Button disabled={busy} onClick={() => void closeViewer()}>Close Trace viewer</Button></div>}
    {preview && <Collapsible defaultOpen><CollapsibleTrigger>{preview.label}</CollapsibleTrigger><CollapsibleContent>{preview.image ? <img src={preview.image} alt={preview.label} style={{ maxWidth: '100%' }} /> : <pre style={{ maxHeight: 480, overflow: 'auto', whiteSpace: 'pre-wrap' }}>{preview.text}</pre>}</CollapsibleContent></Collapsible>}
    {error && <p role="alert">{error}</p>}
  </section>;
}
