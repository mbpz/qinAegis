import { useEffect, useRef, useState } from 'react';
import { Button, Checkbox, Input, Select } from './ui';
import JobProgress from './JobProgress';
import { rpc } from '../utils/rpc';
import type { RuntimeMaintenance, RuntimePreview, RuntimeSummary, RuntimeDownload } from '../types/runtime_maintenance';

function Summary({label, value}: {label: string; value: RuntimeSummary | null}) {
  return <div><h4>{label}</h4>{value ? <><p>{value.version} {value.target || ''}</p><p className="break-all">SHA-256: {value.archive_sha256}</p>{value.source_commit && <p>Source: {value.source_commit}</p>}</> : <p>None recorded</p>}</div>;
}
export default function RuntimeMaintenanceView() {
  const [now, setNow] = useState(Date.now);
  const [facts, setFacts] = useState<RuntimeMaintenance | null>(null);
  const [file, setFile] = useState<File | null>(null);
  const [hash, setHash] = useState('');
  const [preview, setPreview] = useState<RuntimePreview | null>(null);
  const [approved, setApproved] = useState(false);
  const [discard, setDiscard] = useState(false);
  const [busy, setBusy] = useState(false);
  const [releases, setReleases] = useState<RuntimeDownload[]>([]);
  const [downloadReason, setDownloadReason] = useState('');
  const [asset, setAsset] = useState('');
  const [allowDownload, setAllowDownload] = useState(false);
  const [received, setReceived] = useState(0);
  const [job, setJob] = useState<{id: string; kind: 'prepare' | 'apply' | 'list' | 'discard'; generation: number} | null>(null);
  const [error, setError] = useState('');
  const generation = useRef(0);
  const alive = useRef(true);
  const locked = useRef(false);
  const transfer = useRef<{controller: AbortController; id?: string} | null>(null);
  const valid = (id: number) => alive.current && generation.current === id;
  function invalidate() { generation.current++; setPreview(null); setApproved(false); setDiscard(false); setJob(null); }
  async function reload() {
    const id = ++generation.current;
    setPreview(null); setApproved(false); setDiscard(false); setJob(null);
    try { const result = await rpc.getRuntimeMaintenance(); if (valid(id)) {setFacts(result); setPreview(result.preview || null); setError('');} }
    catch (e) { if (valid(id)) setError((e as Error).message); }
  }
  useEffect(() => {
    alive.current = true; void reload();
    return () => { alive.current = false; generation.current++; transfer.current?.controller.abort(); if (transfer.current?.id) void rpc.abortRuntimeUpload(transfer.current.id).catch(() => {}); };
  }, []);
  useEffect(() => {
    if (!preview) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [preview]);
  const expiry = Number(preview?.expires_at) * 1000;
  const expired = Boolean(preview) && (!Number.isFinite(expiry) || now >= expiry);
  const disabled = busy || Boolean(facts?.blocked_reason);
  async function prepare(rollback: boolean) {
    if (locked.current || disabled || !facts) return;
    if (!rollback && (!file || !/^[a-fA-F0-9]{64}$/.test(hash) || file.size === 0 || file.size > 1024 ** 3)) {
      setError('Choose a nonempty archive up to 1 GiB and enter its trusted 64-character SHA-256.'); return;
    }
    locked.current = true; invalidate(); const id = generation.current;
    setBusy(true); setError(''); setReceived(0);
    try {
      if (rollback) {
        const result = await rpc.prepareRuntimeRollback();
        if (valid(id)) setJob({id: result.jobId, kind: 'prepare', generation: id});
      } else {
        const selected = file!;
        const active = {controller: new AbortController(), id: undefined as string | undefined};
        transfer.current = active;
        const upload = await rpc.beginRuntimeUpload(selected.size, hash.toLowerCase()); active.id = upload.upload_id;
        if (!valid(id)) {await rpc.abortRuntimeUpload(upload.upload_id); return;}
        let offset = upload.received;
        while (offset < selected.size) {
          const result = await rpc.uploadRuntimeChunk(upload.upload_id, offset, selected.slice(offset, offset + 512 * 1024), active.controller.signal);
          if (!valid(id)) return;
          offset = result.received; setReceived(offset);
        }
        active.controller.signal.throwIfAborted();
        const result = await rpc.prepareRuntimeInstall(upload.upload_id);
        transfer.current = null;
        if (valid(id)) setJob({id: result.jobId, kind: 'prepare', generation: id});
      }
    } catch (e) {
      const active = transfer.current; transfer.current = null;
      if (active?.id) await rpc.abortRuntimeUpload(active.id).catch(() => {});
      if (valid(id)) {setError((e as Error).name === 'AbortError' ? 'Upload cancelled.' : (e as Error).message); setBusy(false);}
    } finally { locked.current = false; }
  }
  async function apply() {
    if (locked.current || busy || expired || !preview || !approved || (preview.discard_previous_required && !discard)) return;
    locked.current = true; const id = generation.current; setBusy(true); setError(''); setApproved(false);
    try {
      const result = await rpc.confirmRuntimeChange(preview, discard);
      if (valid(id)) {setPreview(null); setJob({id: result.jobId, kind: 'apply', generation: id});}
    } catch (e) {if (valid(id)) {setError((e as Error).message); setBusy(false); setPreview(null);}}
    finally {locked.current = false;}
  }
  async function additional(action: 'list' | 'download' | 'discard') {
    if (locked.current || busy || (action === 'download' && (!allowDownload || !asset || facts?.upload)) || (action === 'discard' && !preview)) return;
    const pending = preview;
    const token = pending?.preview_token;
    locked.current = true; invalidate(); const id = generation.current;
    setBusy(true); setError(''); setAllowDownload(false);
    if (action === 'list') {setReleases([]); setAsset(''); setDownloadReason('');}
    try {
      const result = action === 'list' ? await rpc.listRuntimeDownloads() : action === 'download' ? await rpc.prepareRuntimeDownload(asset) : await rpc.discardRuntimePreview(token!);
      if (valid(id)) setJob({id: result.jobId, kind: action === 'download' ? 'prepare' : action, generation: id});
    } catch (e) {if (valid(id)) {setError((e as Error).message); setBusy(false); setPreview(pending);}}
    finally {locked.current = false;}
  }
  async function discardUpload() {
    if (locked.current || busy || !facts?.upload) return;
    locked.current = true; setBusy(true); setError('');
    try { await rpc.abortRuntimeUpload(facts.upload.upload_id); await reload(); }
    catch (e) { setError((e as Error).message); }
    finally { locked.current = false; setBusy(false); }
  }
  return <section className="card">
    <h3>Install, upgrade and roll back the engine</h3>
    <p>Import a standalone runtime archive and a SHA-256 obtained from a trusted distribution channel. This transfer sends no project data or model requests. Chromium remains a separate explicit installation.</p>
    <Button disabled={busy} onClick={() => void reload()}>Refresh installed versions</Button>
    {error && <p role="alert">{error}</p>}
    {facts && <>
      {facts.selection !== 'managed' && <p role="status">The selected engine is an {facts.selection} override. Changing the managed installation does not change that selection.</p>}
      {facts.blocked_reason && <p role="alert">{facts.blocked_reason}</p>}
      {facts.upload && <div role="status">
        <p>Retained upload: {facts.upload.received.toLocaleString()} / {facts.upload.size.toLocaleString()} bytes. {facts.upload.claimed ? 'Verification owns this upload.' : 'Discard it before choosing another archive.'}</p>
        <Button disabled={busy || facts.upload.claimed} onClick={() => void discardUpload()}>Discard retained upload</Button>
      </div>}
      {facts.unmanaged_current && <p role="status">The current runtime has no native verification receipt. Import its exact original archive and trusted SHA-256 to adopt it before upgrading.</p>}
      {facts.unmanaged_previous && <p role="status">An existing previous runtime has no native verification receipt. Preserve that directory and resolve it before importing; maintenance will refuse to overwrite it.</p>}
      <Summary label="Current managed runtime" value={facts.current} />
      <Summary label="Previous managed runtime" value={facts.previous} />
      {!facts.downloads.available && <p>{facts.downloads.reason}</p>}
    </>}
    <section aria-label="Trusted runtime downloads">
      <h4>Published runtimes</h4>
      <p>Check the trusted distribution channel explicitly. Only compatible published assets can be selected; no custom download URL is accepted.</p>
      <Button disabled={disabled || !facts || Boolean(preview)} onClick={() => void additional('list')}>Check available stable runtimes</Button>
      {downloadReason && <p role="status">{downloadReason}</p>}
      {releases.length > 0 && <>
        <Select aria-label="Published runtime" value={asset} disabled={disabled || Boolean(preview)} onValueChange={value => {invalidate(); setAsset(value); setAllowDownload(false);}}>
          <option value="">Select a runtime</option>
          {releases.map(item => <option key={item.asset_id} value={item.asset_id}>{item.version} · {item.name} · {item.size.toLocaleString()} bytes</option>)}
        </Select>
        <Checkbox label="Allow downloading this selected runtime from the trusted distribution channel" checked={allowDownload} onCheckedChange={setAllowDownload} disabled={disabled || !asset || Boolean(preview)} />
        <Button disabled={disabled || !asset || !allowDownload || Boolean(preview) || Boolean(facts?.upload)} onClick={() => void additional('download')}>Download and verify selected runtime</Button>
      </>}
    </section>
    <Input label="Runtime archive (.tar.gz)" type="file" accept=".tar.gz,application/gzip" value="" disabled={disabled || Boolean(preview)}
      onInput={event => {invalidate(); setFile(event.currentTarget.files?.[0] || null); setReceived(0);}} />
    {file && <p>Selected: {file.name} ({file.size.toLocaleString()} bytes)</p>}
    <Input label="Trusted archive SHA-256" value={hash} disabled={disabled || Boolean(preview)} onValueChange={value => {invalidate(); setHash(value.trim());}} />
    <Button disabled={disabled || Boolean(preview) || Boolean(facts?.upload) || !facts || !file || !/^[a-fA-F0-9]{64}$/.test(hash)} onClick={() => void prepare(false)}>Upload and verify archive</Button>
    <Button disabled={disabled || Boolean(preview) || !facts?.previous} onClick={() => void prepare(true)}>Prepare rollback</Button>
    {transfer.current && <div role="status"><p>Uploaded {received.toLocaleString()} / {file?.size.toLocaleString()} bytes</p><Button onClick={() => transfer.current?.controller.abort()}>Cancel upload</Button></div>}
    {job && <JobProgress key={job.id} jobId={job.id} onComplete={result => {
      if (!valid(job.generation)) return;
      setBusy(false);
      if (job.kind === 'prepare') {setPreview(result as RuntimePreview); setApproved(false); setDiscard(false);}
      else if (job.kind === 'list') {const listing = result as {releases: RuntimeDownload[]; reason?: string}; setReleases(listing.releases); setDownloadReason(listing.reason || (listing.releases.length ? '' : 'No compatible stable runtime is published.'));}
      else {setPreview(null); void reload();}
    }} onError={message => {if (valid(job.generation)) {setError(message); setBusy(false); setPreview(null); void rpc.getRuntimeMaintenance().then(value => {if (valid(job.generation)) setFacts(value);}).catch(() => {});}}} />}
    {preview && <section aria-label="Runtime change preview">
      <h4>{preview.operation === 'rollback' ? 'Review rollback' : 'Review installation'}</h4>
      <Button disabled={busy} onClick={() => void additional('discard')}>Discard pending preview</Button>
      <Summary label="Verified candidate" value={preview.candidate} />
      <p>Preview expires: {Number.isFinite(expiry) ? new Date(expiry).toLocaleString() : 'invalid expiry'}. Changed installation state requires a fresh preview.</p>
      {expired && <p role="alert">This preview has expired. Discard it and prepare a new preview.</p>}
      {preview.discard_previous_required && <Checkbox label="Allow discarding the older previous runtime after this change succeeds" checked={discard} onCheckedChange={setDiscard} disabled={busy} />}
      <Checkbox label="Approve this exact runtime change" checked={approved} onCheckedChange={setApproved} disabled={busy} />
      <Button disabled={busy || expired || !approved || (preview.discard_previous_required && !discard)} onClick={() => void apply()}>Confirm {preview.operation === 'rollback' ? 'rollback' : 'installation'}</Button>
    </section>}
  </section>;
}
