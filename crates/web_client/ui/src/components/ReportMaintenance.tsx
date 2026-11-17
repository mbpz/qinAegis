import { useRef, useState } from 'react';
import { Button, Checkbox } from './ui';
import { rpc } from '../utils/rpc';

import type { ReportCleanupPreview } from '../types/rpc';

export default function ReportMaintenance({ project, runId }: { project: string; runId: string }) {
  const [preview, setPreview] = useState<ReportCleanupPreview | null>(null);
  const [approved, setApproved] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const pending = useRef(false);

  async function inspect() {
    if (pending.current) return;
    pending.current = true; setBusy(true); setError(''); setMessage(''); setPreview(null); setApproved(false);
    try { setPreview(await rpc.previewReportCleanup(project, runId)); }
    catch (error) { setError((error as Error).message); }
    finally { pending.current = false; setBusy(false); }
  }

  async function cleanup() {
    if (pending.current || !approved || !preview?.preview_token || !preview.cleanup_available) return;
    pending.current = true; setBusy(true); setError(''); setMessage('');
    const token = preview.preview_token;
    setApproved(false); setPreview(null);
    try {
      const result = await rpc.cleanupReportResidue(project, runId, token);
      setMessage(`Removed ${result.removed_files} temporary files (${result.removed_bytes} bytes). Saved execution evidence and completed reports were preserved.`);
    } catch (error) { setError(`${(error as Error).message}. Inspect again before retrying.`); }
    finally { pending.current = false; setBusy(false); }
  }

  return <section aria-label="Report recovery">
    <h3>Report recovery</h3>
    <p>Inspect temporary files left by interrupted report generation. Cleanup requires a fresh preview and an idle service.</p>
    <Button disabled={busy} onClick={() => void inspect()}>Inspect report leftovers</Button>
    {error && <p role="alert">{error}</p>}
    {message && <p role="status">{message}</p>}
    {preview && <div aria-live="polite">
      <p>Run status: {preview.run_status}. Preserved: {preview.preserved.join(', ')}.</p>
      {preview.candidates.length ? <ul>{preview.candidates.map(candidate => <li key={candidate.name}>{candidate.name}: {candidate.files} files, {candidate.bytes} bytes</li>)}</ul> : <p>No verified temporary files are available for cleanup.</p>}
      {preview.manual_review.length > 0 && <><p>These files require manual review and will be preserved:</p><ul>{preview.manual_review.map(item => <li key={item.name}>{item.name}: {item.reason}</li>)}</ul></>}
      {preview.cleanup_available && <>
        <Checkbox label="Remove only the temporary files listed in this preview" checked={approved} onCheckedChange={setApproved} disabled={busy} />
        <Button disabled={busy || !approved} onClick={() => void cleanup()}>Clean inspected temporary files</Button>
        <p>The preview expires in {preview.expires_in_seconds} seconds. Changed files require another inspection.</p>
      </>}
    </div>}
  </section>;
}
