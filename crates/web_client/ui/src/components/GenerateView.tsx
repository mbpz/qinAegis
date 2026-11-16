import { Button, Checkbox, Select, TextArea } from './ui';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from './motion/collapsible';
import { useEffect, useRef, useState } from 'react';
import { rpc } from '../utils/rpc';
import type { ExecutionSnapshot } from '../types/contracts';
import JobProgress from './JobProgress';

export default function GenerateView({ projects }: { projects: string[] }) {
  const [requirement, setRequirement] = useState('');
  const [project, setProject] = useState('');
  const [useContracts, setUseContracts] = useState(true);
  const [preview, setPreview] = useState<ExecutionSnapshot | null>(null);
  const [previewError, setPreviewError] = useState('');
  const [previewLoading, setPreviewLoading] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const [jobId, setJobId] = useState<string | null>(null);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState('');
  const mounted = useRef(false);
  const locked = useRef(false);
  const activeJob = useRef<string | null>(null);
  const previewRequest = useRef(0);
  const projectExists = projects.includes(project);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  useEffect(() => {
    const request = ++previewRequest.current;
    let cancelled = false;
    setPreview(null);
    setPreviewError('');
    setPreviewLoading(false);
    if (projectExists && useContracts) {
      setPreviewLoading(true);
      rpc.getContractExecution(project).then(snapshot => {
        if (cancelled || !mounted.current || previewRequest.current !== request) return;
        if (snapshot.project !== project || snapshot.schema_version !== 1 || !snapshot.contracts.length) throw new Error('Execution preview is invalid or contains no bound contracts.');
        setPreview(snapshot);
      }).catch(reason => {
        if (!cancelled && mounted.current && previewRequest.current === request) setPreviewError(String(reason));
      }).finally(() => {
        if (!cancelled && mounted.current && previewRequest.current === request) setPreviewLoading(false);
      });
    }
    return () => { cancelled = true; };
  }, [project, projectExists, useContracts, refresh]);

  const previewReady = !!preview && preview.project === project && !previewError && !previewLoading;
  const canGenerate = !!requirement.trim() && projectExists && (!useContracts || previewReady) && !generating;
  const handleGenerate = async () => {
    if (locked.current || !canGenerate) return;
    locked.current = true;
    activeJob.current = null;
    setGenerating(true);
    setError('');
    setJobId(null);
    try {
      const result = await rpc.runGenerate(requirement.trim(), project, useContracts);
      if (!mounted.current) return;
      if (!result.ok || !result.jobId) throw new Error('Local service did not confirm generation.');
      activeJob.current = result.jobId;
      setJobId(result.jobId);
    } catch (reason) {
      locked.current = false;
      if (mounted.current) { setError(String(reason)); setGenerating(false); }
    }
  };
  function finish(message = '') {
    if (!mounted.current || !jobId || activeJob.current !== jobId) return;
    activeJob.current = null;
    locked.current = false;
    setGenerating(false);
    if (message) setError(message);
  }

  return <div className="view">
    <h2 className="view-title">Generate Test Cases</h2>
    <div className="card">
      <div className="card-title">Project & requirement</div>
      <div className="input-group"><label htmlFor="generate-project">Existing project</label><Select id="generate-project" className="input" value={project} disabled={generating} onValueChange={value => setProject(value)}><option value="">Select a project</option>{projects.map(name => <option key={name} value={name}>{name}</option>)}</Select></div>
      {!projects.length && <p>Create a project on the Dashboard before generating tests.</p>}
      <div className="input-group"><label htmlFor="generate-requirement">Test requirement</label><TextArea id="generate-requirement" className="input" rows={3} placeholder="e.g., User login with email and password" value={requirement} disabled={generating} onValueChange={value => setRequirement(value)} /></div>
      <Checkbox label={"Use published contracts bound to this project"} checked={useContracts} disabled={generating} onCheckedChange={checked => setUseContracts(checked)} />
      {useContracts && projectExists && <div>
        <p>Preview pins the current contract versions, inputs, and assertions. Generation prepares a fresh actual snapshot on the service; later edits may change it. This preview makes no model calls.</p>
        <Button type="button" className="btn" disabled={generating || previewLoading} onClick={() => { setPreview(null); setRefresh(value => value + 1); }}>Refresh contract preview</Button>
        {previewLoading && <p role="status">Preparing contract preview…</p>}
        {previewError && <p role="alert">{previewError} Bind published contracts in Contracts and retry the preview.</p>}
        {previewReady && <Collapsible defaultOpen><CollapsibleTrigger>Execution snapshot · library revision {preview!.library_revision}</CollapsibleTrigger><CollapsibleContent><pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{JSON.stringify(preview, null, 2)}</pre></CollapsibleContent></Collapsible>}
      </div>}
      {!useContracts && <p>Generation will use the requirement without project contract assertions.</p>}
      {error && <p role="alert">{error}</p>}
      <div className="form-actions"><Button type="button" className="btn btn-primary" onClick={handleGenerate} disabled={!canGenerate}>{generating ? 'Generating…' : 'Generate Cases'}</Button></div>
    </div>
    <div className="output-header"><h3>Output</h3></div>
    <JobProgress jobId={jobId} onComplete={() => finish()} onError={message => finish(message)} />
  </div>;
}
