import { useEffect, useId, useRef, useState } from 'react';
import { Button, Checkbox, Select, TextArea } from './ui';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from './motion/collapsible';
import { rpc } from '../utils/rpc';
import { correctionCases, correctionInstruction, correctionJobEvidence } from '../utils/contract_correction';
import type { ContractProposal, PublishedContract } from '../types/contracts';
import type { ContractCorrectionSource, CorrectionAttempt, CorrectionLibrary, JobStatus, RunEvidence, RunReport } from '../types/rpc';

const pretty = (value: unknown) => JSON.stringify(value, null, 2);
const active = (status: string) => ['queued', 'running', 'cancelling'].includes(status);
function Attempts({ attempts }: {attempts: CorrectionAttempt[]}) {
  return <ul>{attempts.map(row => <li key={`${row.call}-${row.attempt}`}>Attempt {row.attempt}: {row.profile_id} · {row.outcome}{row.outcome === 'started' && ' (locally admitted; provider receipt unconfirmed)'}</li>)}</ul>;
}
// This view only narrows the picker. The service revalidates immutable facts and CAS.
function EvidencePicker({project, publication, onChange}: {project: string; publication: PublishedContract; onChange: (run: RunEvidence | null, caseId: string) => void}) {
  const id = useId();
  const [reports, setReports] = useState<RunReport[]>([]);
  const [run, setRun] = useState<RunEvidence | null>(null);
  const [runId, setRunId] = useState('');
  const [caseId, setCaseId] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const epoch = useRef(0);
  const changed = useRef(onChange); changed.current = onChange;
  useEffect(() => {
    const token = ++epoch.current;
    setReports([]); setRun(null); setRunId(''); setCaseId(''); setError(''); setLoading(false);
    changed.current(null, '');
    if (project) rpc.getReports(project).then(data => {
      if (epoch.current === token) setReports(data.filter(item => item.has_evidence));
    }).catch(error => { if (epoch.current === token) setError(String(error)); });
    return () => { ++epoch.current; };
  }, [project, publication]);
  async function selectRun(value: string) {
    const token = ++epoch.current;
    setRunId(value); setRun(null); setCaseId(''); setError(''); changed.current(null, '');
    setLoading(!!value);
    if (!value) return;
    try {
      const data = await rpc.getRunEvidence(project, value);
      if (epoch.current !== token) return;
      if (!correctionCases(data, project, publication).length) throw new Error('No terminal case with this exact published contract source is available in this run.');
      setRun(data);
    } catch (error) { if (epoch.current === token) setError(String(error)); }
    finally { if (epoch.current === token) setLoading(false); }
  }
  return <div>
    <label htmlFor={`${id}-run`}>Managed evidence run</label>
    <Select id={`${id}-run`} value={runId} onValueChange={value => void selectRun(value)} disabled={!project}>
      <option value="">Choose a run</option>{reports.map(item => <option key={item.run_id} value={item.run_id}>{item.run_id} · {item.status ?? 'inspect evidence'}</option>)}
    </Select>
    <label htmlFor={`${id}-case`}>Case with exact v{publication.definition.version} provenance</label>
    <Select id={`${id}-case`} value={caseId} disabled={!run} onValueChange={value => {setCaseId(value); changed.current(run, value);}}>
      <option value="">Choose a case</option>{run && correctionCases(run, project, publication).map(item => <option key={item.case_id} value={item.case_id}>{item.case_id} · {item.name}</option>)}
    </Select>
    {loading && <p role="status">Reading managed evidence…</p>}{error && <p role="alert">{error}</p>}
    {run && caseId && <Collapsible><CollapsibleTrigger>Selected frozen facts (not a model conclusion)</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace: 'pre-wrap', overflowWrap: 'anywhere'}}>{pretty({project, run_id: run.run_id, run_status: run.status, case: run.cases.find(item => item.case_id === caseId), result: run.results.filter(item => item.case_id === caseId).map(({screenshot_base64: _image, ...result}) => result)})}</pre></CollapsibleContent></Collapsible>}
  </div>;
}

export default function ContractCorrectionPanel({projects, publication, revision, onLibrary}: {projects: string[]; publication: PublishedContract; revision: number; onLibrary: (library: CorrectionLibrary) => void}) {
  const id = useId();
  const [project, setProject] = useState('');
  const [run, setRun] = useState<RunEvidence | null>(null);
  const [caseId, setCaseId] = useState('');
  const [instruction, setInstruction] = useState('');
  const [approved, setApproved] = useState(false);
  const [jobId, setJobId] = useState('');
  const [job, setJob] = useState<JobStatus | null>(null);
  const [error, setError] = useState('');
  const [starting, setStarting] = useState(false);
  const epoch = useRef(0), locked = useRef(false);
  const library = useRef(onLibrary); library.current = onLibrary;
  const reset = () => { ++epoch.current; setApproved(false); setJobId(''); setJob(null); setError(''); setStarting(false); locked.current = false; };
  useEffect(() => () => { ++epoch.current; }, []);
  useEffect(() => {
    if (!jobId) return;
    const token = epoch.current;
    let cancelled = false, timer: ReturnType<typeof setTimeout> | undefined;
    async function poll() {
      try {
        const data = await rpc.getJobStatus(jobId);
        if (cancelled || epoch.current !== token) return;
        setJob(data); setError('');
        if (active(data.status)) timer = setTimeout(poll, 1500);
        else if (data.status === 'completed') {
          const snapshot = await rpc.getContracts();
          if (!cancelled && epoch.current === token) library.current(snapshot);
        }
      } catch (error) {
        if (!cancelled && epoch.current === token) { setError(String(error)); timer = setTimeout(poll, 2500); }
      }
    }
    void poll();
    return () => {cancelled = true; if (timer) clearTimeout(timer);};
  }, [jobId]);
  async function generate() {
    if (locked.current || !run || !caseId || !approved) return;
    let text: string;
    try { text = correctionInstruction(instruction); } catch (error) {setError(String(error)); return;}
    locked.current = true; setStarting(true); setError('');
    const token = ++epoch.current;
    try {
      const started = await rpc.runContractCorrection({project, contract_id: publication.definition.id, version: publication.definition.version, run_id: run.run_id, case_id: caseId, instruction: text, expected_revision: revision});
      if (token === epoch.current) {setJobId(started.jobId); setJob(null); setApproved(false);}
    } catch (error) { if (token === epoch.current) setError(`${String(error)} Refresh the library after a revision conflict, then review again.`); }
    finally { if (token === epoch.current) {setStarting(false); locked.current = false;} }
  }
  const evidence = correctionJobEvidence(job?.result);
  const running = starting || (!!jobId && (!job || active(job.status)));
  return <section className="card" aria-label="Model correction suggestion">
    <div className="card-title">Model correction suggestion · pending human review</div>
    <p>Select one recorded case and explain the suspected contract issue. Uses the explicitly configured correct route and approved fallback chain. It does not publish, migrate bindings or run a repair.</p>
    <label htmlFor={`${id}-project`}>Evidence project</label><Select id={`${id}-project`} value={project} onValueChange={value => {reset(); setProject(value); setRun(null); setCaseId('');}}><option value="">Choose project</option>{projects.map(name => <option key={name} value={name}>{name}</option>)}</Select>
    <EvidencePicker project={project} publication={publication} onChange={(value, selected) => {reset(); setRun(value); setCaseId(selected);}} />
    <label htmlFor={`${id}-instruction`}>Explanation (up to 8192 UTF-8 bytes; do not include credentials)</label>
    <TextArea id={`${id}-instruction`} rows={4} value={instruction} onValueChange={value => {reset(); setInstruction(value);}} />
    <Checkbox label="I authorize sending this selected case’s necessary evidence summary and explanation to the configured correction models. No screenshot or whole report is sent by default." checked={approved} onCheckedChange={setApproved} />
    <p><Button disabled={running || !approved || !run || !caseId || !instruction.trim()} onClick={() => void generate()}>Generate pending suggestion</Button></p>
    <p>Editing inputs detaches this panel from the previous task; it remains available in Tasks. A model response is not proof that the contract is correct or a fix passed.</p>
    {error && <p role="alert">{error}</p>}
    {jobId && <div role="status"><p>Task {jobId}: {job?.status ?? 'starting'} · {evidence?.status ?? 'Actual call evidence not yet available'}</p>
      {!!job?.snapshot && <Collapsible><CollapsibleTrigger>Frozen task input and correction model route</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace: 'pre-wrap', overflowWrap: 'anywhere'}}>{pretty(job.snapshot)}</pre></CollapsibleContent></Collapsible>}
      {evidence && <Attempts attempts={evidence.attempts} />}
      {evidence?.proposalId && <p>Pending proposal: {evidence.proposalId}. Review the full original and replacement below.</p>}
      {job?.error && <p role="alert">{job.error}</p>}{job?.history_error && <p role="alert">{job.history_error}</p>}
      {running && job?.status !== 'cancelling' && <Button onClick={() => {const token = epoch.current; void rpc.stopJob(jobId).then(() => {if (token === epoch.current) setJob(current => current ? {...current, status: 'cancelling'} : current);}).catch(error => {if (token === epoch.current) setError(String(error));});}}>Cancel suggestion</Button>}
    </div>}
  </section>;
}

export function CorrectionSourceDetails({source}: {source: ContractCorrectionSource}) {
  return <section aria-label="Persisted model suggestion source">
    <p>Model-authored suggestion. Semantic validation: not verified. Human approval and a new-version observation are separate decisions.</p>
    <Collapsible><CollapsibleTrigger>Frozen input facts and explanation</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace: 'pre-wrap', overflowWrap: 'anywhere'}}>{pretty(source.input)}</pre></CollapsibleContent></Collapsible>
    <Collapsible><CollapsibleTrigger>Frozen correction model route</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace: 'pre-wrap', overflowWrap: 'anywhere'}}>{pretty(source.model)}</pre></CollapsibleContent></Collapsible>
    <Attempts attempts={source.attempts} />
    <p>Model-declared limitations (not verified):</p><ul>{source.limitations.map((value, i) => <li key={i}>{value}</li>)}</ul>
    <Collapsible><CollapsibleTrigger>Referenced evidence tokens</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace: 'pre-wrap', overflowWrap: 'anywhere'}}>{pretty(source.evidence_tokens)}</pre></CollapsibleContent></Collapsible>
  </section>;
}

export function CorrectionObservationPanel({proposal, publication, projects, revision, onLibrary}: {proposal: ContractProposal; publication: PublishedContract; projects: string[]; revision: number; onLibrary: (library: CorrectionLibrary) => void}) {
  const id = useId();
  const [project, setProject] = useState(''), [caseId, setCaseId] = useState('');
  const [run, setRun] = useState<RunEvidence | null>(null);
  const [approved, setApproved] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState('');
  const epoch = useRef(0), locked = useRef(false);
  useEffect(() => () => {++epoch.current;}, []);
  const reset = () => {++epoch.current; setApproved(false); setError(''); setBusy(false); locked.current = false;};
  async function record() {
    if (!run || !caseId || !approved || locked.current) return;
    locked.current = true; setBusy(true); const token = ++epoch.current;
    try {
      const snapshot = await rpc.recordContractCorrectionObservation({proposal_id: proposal.id, project, run_id: run.run_id, case_id: caseId, expected_revision: revision, approved: true});
      if (epoch.current === token) onLibrary(snapshot);
    } catch (error) { if (epoch.current === token) {setError(String(error)); setApproved(false);} }
    finally { if (epoch.current === token) {setBusy(false); locked.current = false;} }
  }
  return <section aria-label={`New version observation ${proposal.id}`}>
    <h4>Associate a new v{publication.definition.version} run observation</h4><p>First migrate bindings explicitly, maintain and review cases, then run the new version. Old evidence is rejected. Passed, failed and incomplete observations preserve the original failure and do not prove semantic correctness.</p>
    <label htmlFor={`${id}-project`}>Observation project</label><Select id={`${id}-project`} value={project} onValueChange={value => {reset(); setProject(value); setRun(null); setCaseId('');}}><option value="">Choose project</option>{projects.map(name => <option key={name} value={name}>{name}</option>)}</Select>
    <EvidencePicker project={project} publication={publication} onChange={(value, selected) => {reset(); setRun(value); setCaseId(selected);}} />
    <Checkbox label="I reviewed this exact new-version case and authorize associating its recorded outcome with the approved proposal; semantic validation remains not verified." checked={approved} onCheckedChange={setApproved} />
    <Button disabled={busy || !approved || !run || !caseId} onClick={() => void record()}>Record new-version observation</Button>{error && <p role="alert">{error}</p>}
  </section>;
}
