import { validCapturePolicy } from '../utils/capture';
import { useEffect, useRef, useState } from 'react';
import { Button, Checkbox, Input, Select } from './ui';
import { rpc } from '../utils/rpc';
import type { CaptureMode, CapturePolicy, CapturePolicySnapshot } from '../types/capture';

export default function CapturePolicyView() {
  const [saved, setSaved] = useState<CapturePolicySnapshot | null>(null);
  const [draft, setDraft] = useState<CapturePolicy | null>(null);
  const [approved, setApproved] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const request = useRef(0);
  const pending = useRef(false);
  useEffect(() => { void reload(); return () => { ++request.current; pending.current = false; }; }, []);
  async function reload() {
    if (pending.current) return; pending.current = true;
    const token = ++request.current; setBusy(true); setApproved(false);
    try { const value = await rpc.getCapturePolicy(); if (token === request.current) { setSaved(value); setDraft(value.policy); setMessage(''); } }
    catch (error) { if (token === request.current) setMessage(String(error)); }
    finally { if (token === request.current) { pending.current = false; setBusy(false); } }
  }
  function change(patch: Partial<CapturePolicy>) { setDraft(current => current && ({ ...current, ...patch })); setApproved(false); setMessage(''); }
  async function save() {
    if (!saved || !draft || !approved || pending.current) return;
    pending.current = true;
    const token = ++request.current; setBusy(true); setApproved(false);
    try { const value = await rpc.saveCapturePolicy(saved.revision, draft); if (token === request.current) { setSaved(value); setDraft(value.policy); setMessage('Policy saved. Existing submissions retain their frozen policy.'); } }
    catch (error) { if (token === request.current) setMessage(`${error}. Draft retained; reload explicitly before resolving a conflict.`); }
    finally { if (token === request.current) { pending.current = false; setBusy(false); } }
  }
  const valid = validCapturePolicy(draft);
  return <section className="card" aria-label="Evidence capture policy">
    <h3>Evidence capture policy</h3>
    <p>Optional diagnostics are independent of business assertions. Execution submissions and flow preparations freeze this policy. Changes apply to later submissions.</p>
    <p>Off preserves results and provenance. It does not prevent visual models from reading the screen for execution. Screenshots and Trace may contain sensitive business data; complete redaction cannot be guaranteed.</p>
    {draft && <>
      <p>Saved revision: {saved?.revision}</p>
      <Select aria-label="Capture mode" value={draft.mode} disabled={busy} onValueChange={value => change({ mode: value as CaptureMode, include_sensitive: false })}>
        <option value="off">Off</option><option value="failure_only">Failures only</option><option value="key_steps">Key steps</option><option value="full">Full diagnostics</option>
      </Select>
      {(['max_artifact_bytes', 'max_run_bytes', 'max_events'] as const).map(key => <label key={key} style={{ display: 'block', marginTop: 12 }}>{key.replace(/_/g, ' ')}<Input aria-label={key} type="number" disabled={busy} value={draft[key]} onValueChange={value => change({ [key]: Number(value) })} /></label>)}
      <Checkbox checked={draft.include_sensitive} disabled={busy || draft.mode === 'off'} onCheckedChange={value => change({ include_sensitive: value === true })} label="Allow sensitive diagnostic content for this saved policy" />
      <Checkbox checked={approved} disabled={busy} onCheckedChange={value => setApproved(value === true)} label="Confirm this exact capture policy and limits" />
      <Button disabled={!valid || !approved || busy} onClick={() => void save()}>Save capture policy</Button>
    </>}
    <Button variant="secondary" disabled={busy} onClick={() => void reload()}>Reload capture policy</Button>
    {message && <p role="status">{message}</p>}
  </section>;
}
