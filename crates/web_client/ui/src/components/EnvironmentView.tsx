import RuntimeMaintenanceView from './RuntimeMaintenanceView';
import { useRef, useState } from 'react';
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
  const submitting = useRef(false);
  async function start(install: boolean) {
    if (submitting.current || running || (install && !approved)) return;
    submitting.current = true; setRunning(true); setError(''); setFacts(null); setJob(null);
    try {
      const result = install ? await rpc.installRuntimeBrowser() : await rpc.diagnoseRuntime();
      setJob(result.jobId);
    } catch (error) { setError((error as Error).message); setRunning(false); }
    finally { submitting.current = false; }
  }
  return <div className="view">
    <h2>Environment and engines</h2>
    <RuntimeMaintenanceView />
    <p>Inspect the selected runtime and launch a temporary headless Chromium check. No project data or model calls are used.</p>
    {error && <p role="alert">{error}</p>}
    <section className="card">
      <Button disabled={running} onClick={() => void start(false)}>Check runtime and browser</Button>
      <p>The standalone engine includes Node, compiled Playwright + Midscene.js and the official Allure reporter. Chromium is installed separately. The base workbench remains available without an engine.</p>
      <Checkbox label="Allow downloading the matching Chromium browser to the selected runtime cache" checked={approved} onCheckedChange={setApproved} disabled={running} />
      <Button disabled={running || !approved} onClick={() => void start(true)}>Install Chromium</Button>
      <p>Installation can take several minutes. Required operating system libraries are diagnosed separately; this operation does not elevate privileges.</p>
    </section>
    <JobProgress jobId={job} onComplete={result => { setFacts(result as RuntimeFacts); setRunning(false); }} onError={error => {setError(error); setRunning(false);}} />
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
