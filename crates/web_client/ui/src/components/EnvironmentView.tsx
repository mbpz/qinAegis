import RuntimeMaintenanceView from './RuntimeMaintenanceView';
import { useEffect, useRef, useState } from 'react';
import { Button, Checkbox } from './ui';
import JobProgress from './JobProgress';
import { rpc } from '../utils/rpc';

interface RuntimeFacts {
  ready: boolean;
  runtime_available: boolean;
  node?: string;
  node_supported?: boolean;
  engine_version?: string;
  mode?: string;
  packages?: Record<string, string | null>;
  browser?: { installed: boolean; launched: boolean; version?: string; error?: string };
  repair?: string;
}

export default function EnvironmentView() {
  const [job, setJob] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [approved, setApproved] = useState(false);
  const [facts, setFacts] = useState<RuntimeFacts | null>(null);
  const [error, setError] = useState('');
  const [restoring, setRestoring] = useState(true);
  const submitting = useRef(false);
  const generation = useRef(0);
  const activeJob = useRef<string | null>(null);
  useEffect(() => {
    const current = ++generation.current;
    void rpc.getJobs().then(result => {
      if (current !== generation.current) return;
      const recent = result.jobs.find(item => item.jobType === 'browserInstall' || item.jobType === 'runtimeCheck');
      if (recent) {
        activeJob.current = recent.jobId;
        setJob(recent.jobId);
        setRunning(['queued', 'running', 'cancelling'].includes(recent.status));
      }
      if (result.history_error) setError(result.history_error);
    }).catch(error => {
      if (current === generation.current) setError((error as Error).message);
    }).finally(() => {
      if (current === generation.current) setRestoring(false);
    });
    return () => { generation.current++; activeJob.current = null; };
  }, []);
  async function start(install: boolean) {
    if (submitting.current || restoring || running || (install && !approved)) return;
    const current = ++generation.current;
    activeJob.current = null;
    submitting.current = true; setRunning(true); setError(''); setFacts(null); setJob(null);
    if (install) setApproved(false);
    try {
      const result = install ? await rpc.installRuntimeBrowser() : await rpc.diagnoseRuntime();
      if (current !== generation.current) return;
      activeJob.current = result.jobId;
      setJob(result.jobId);
    } catch (error) { if (current === generation.current) { setError((error as Error).message); setRunning(false); } }
    finally { if (current === generation.current) submitting.current = false; }
  }
  return <div className="view">
    <h2>Environment and engines</h2>
    <RuntimeMaintenanceView />
    <p>Inspect the selected runtime and launch a temporary headless Chromium check. No project data or model calls are used.</p>
    {error && <p role="alert">{error}</p>}
    <section className="card">
      {restoring && <p role="status">Restoring the latest environment task…</p>}
      <Button disabled={restoring || running} onClick={() => void start(false)}>Check runtime and browser</Button>
      <p>The standalone engine includes Node, compiled Playwright + Midscene.js and the official Allure reporter. Chromium is installed separately. The base workbench remains available without an engine.</p>
      <Checkbox label="Allow downloading the matching Chromium browser to the selected runtime cache" checked={approved} onCheckedChange={setApproved} disabled={restoring || running} />
      <Button disabled={restoring || running || !approved} onClick={() => void start(true)}>Install Chromium</Button>
      <p>Installation can take several minutes. Required operating system libraries are diagnosed separately; this operation does not elevate privileges.</p>
      <p>Progress refers to each browser component. After failure or cancellation, inspect the task error and approve a new attempt. Refreshing restores the saved task without replaying installation.</p>
    </section>
    <JobProgress jobId={job} onComplete={result => { if (job === activeJob.current) { setFacts(result as RuntimeFacts); setRunning(false); } }} onError={error => {if (job === activeJob.current) { setError(error); setRunning(false); }}} />
    {facts && <section className="card" aria-live="polite">
      <h3>{facts.ready ? 'Runtime check passed' : 'Runtime needs attention'}</h3>
      <p>Runtime: {facts.runtime_available ? `${facts.engine_version} (${facts.mode})` : 'unavailable'}</p>
      {facts.node && <p>Node: {facts.node} · {facts.node_supported ? 'validated major version' : 'use the bundled Node 24 runtime'}</p>}
      {facts.packages && <ul>{Object.entries(facts.packages).map(([name, version]) => <li key={name}>{name}: {version || 'missing'}</li>)}</ul>}
      {facts.browser && <>
        <p>Chromium installed: {facts.browser.installed ? 'yes' : 'no'}; actual launch: {facts.browser.launched ? 'passed' : 'failed'} {facts.browser.version || ''}</p>
        {facts.browser.error && <p>{facts.browser.error}</p>}
      </>}
      {facts.repair && <p>{facts.repair}</p>}
      <p>These checks establish local dependencies. Real model compatibility and business assertions require separate visual acceptance.</p>
    </section>}
  </div>;
}
