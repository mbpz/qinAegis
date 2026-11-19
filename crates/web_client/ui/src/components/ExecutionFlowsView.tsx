import {parseExecutionDraft as parse, preparationDeadline, retryExecutionRead} from '../utils/flow_execution_editor';
import {useEffect, useRef, useState} from 'react';
import {Button, Checkbox, Input, Select, TextArea} from './ui';
import {Collapsible, CollapsibleContent, CollapsibleTrigger} from './motion/collapsible';
import {rpc} from '../utils/rpc';
import type {ExecutionDraft, ExecutionFlows, ExecutionPreparation, ExecutionStep, FlowRun} from '../types/flow_execution';
import type {ModelProfile} from '../types/models';
import type {CaseAsset, TestCase} from '../types/rpc';
const pretty = (v: unknown) => JSON.stringify(v, null, 2);
const active = (s: string) => ['queued', 'running', 'cancelling'].includes(s);

function values(text: string): Record<string, unknown> {
  const v: unknown = JSON.parse(text);
  if (!v || typeof v !== 'object' || Array.isArray(v)) throw new Error('Inputs must be a JSON object');
  return v as Record<string, unknown>;
}
function blank(project: string): ExecutionDraft {
  return {definition: {schema_version: 2, id: 'reviewed-execution', title: 'Reviewed case execution', project, inputs: {}, outputs: {}, steps: [], total_timeout_ms: 120000, cleanup_timeout_ms: 15000, required_permissions: []}, model_profiles: {}};
}
function withSteps(d: ExecutionDraft, steps: ExecutionStep[]): ExecutionDraft {
  return {...d, definition: {...d.definition, steps, required_permissions: steps.some(s => s.operation.capability === 'case.execute.v1') ? ['browser.read', 'browser.write', 'model.send', 'report.write'] : []}};
}
function canMove(steps: ExecutionStep[], index: number, delta: number): boolean {
  const to = index + delta;
  if (to < 0 || to >= steps.length) return false;
  const next = [...steps]; [next[index], next[to]] = [next[to], next[index]];
  return next.every((s, i) => s.depends_on.every(dep => next.findIndex(item => item.id === dep) >= 0 && next.findIndex(item => item.id === dep) < i));
}
export default function ExecutionFlowsView({projects}: {projects: string[]}) {
  const [data, setData] = useState<ExecutionFlows | null>(null);
  const [profiles, setProfiles] = useState<ModelProfile[]>([]);
  const [editor, setEditor] = useState(''); const [saved, setSaved] = useState(''); const [selected, setSelected] = useState('');
  const [project, setProject] = useState(''); const [cases, setCases] = useState<TestCase[]>([]);
  const [asset, setAsset] = useState<CaseAsset | null>(null); const [caseId, setCaseId] = useState(''); const [contract, setContract] = useState(''); const [profile, setProfile] = useState('');
  const [saveApproved, setSaveApproved] = useState(false); const [publishApproved, setPublishApproved] = useState(false); const [prepareApproved, setPrepareApproved] = useState(false); const [runApproved, setRunApproved] = useState(false);
  const [order, setOrder] = useState<string[] | null>(null); const [publication, setPublication] = useState(''); const [inputs, setInputs] = useState('{}');
  const [preparationsReceivedAt, setPreparationsReceivedAt] = useState(0);
  const [preparation, setPreparation] = useState<ExecutionPreparation | null>(null); const [deadline, setDeadline] = useState(0); const [now, setNow] = useState(Date.now());
  const [pollVersion, setPollVersion] = useState(0);
  const [runId, setRunId] = useState(''); const [run, setRun] = useState<FlowRun | null>(null);
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [notice, setNotice] = useState('');
  const mounted = useRef(false); const epoch = useRef(0); const pending = useRef(false);
  const draft = parse(editor); const exactSaved = !!draft && pretty(draft) === saved && draft.definition.id === selected;
  const published = data?.library.publications.find(p => `${p.definition.id}:${p.version}` === publication);
  const models = profiles.filter(p => p.enabled && p.adapter === 'openai_compatible' && p.capabilities.includes('vision') && p.api_key_configured);
  const remaining = Math.max(0, Math.ceil((deadline - now) / 1000));
  let validInputs = true; try {values(inputs);} catch {validInputs = false;}
  function clearPreparation() {setPreparation(null); setDeadline(0); setPrepareApproved(false); setRunApproved(false);}
  function clearReview() {setSaveApproved(false); setPublishApproved(false); setOrder(null); clearPreparation(); setNotice('');}
  function edit(d: ExecutionDraft) {setEditor(pretty(d)); clearReview();}
  async function perform(action: (valid: () => boolean) => Promise<void>) {
    if (pending.current) return;
    const ticket = epoch.current; const valid = () => mounted.current && epoch.current === ticket;
    pending.current = true; setBusy(true); setError(''); setNotice('');
    try {await action(valid);} catch (e) {if (valid()) setError(String(e));}
    finally {if (valid()) {pending.current = false; setBusy(false);}}
  }
  async function reload(valid: () => boolean) {
    const [next, registry] = await Promise.all([rpc.getExecutionFlows(), rpc.getModels()]);
    if (!valid()) return;
    setData(next); setPreparationsReceivedAt(Date.now()); setProfiles(registry.profiles); setEditor(''); setSaved(''); setSelected(''); setPublication(''); clearReview();
    const recent = next.run_history.find(r => active(r.status));
    if (recent) {setRunId(recent.run_id); setPollVersion(v => v + 1);}
  }
  useEffect(() => {
    mounted.current = true; void perform(reload);
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => {mounted.current = false; epoch.current++; pending.current = false; clearInterval(timer);};
  }, []);
  useEffect(() => {
    let stale = false; setCases([]); setAsset(null); setCaseId(''); setContract('');
    if (project) void rpc.getReviewCases(project).then(items => {if (!stale) setCases(items.filter(c => c.status === 'approved'));}).catch(e => {if (!stale) setError(String(e));});
    return () => {stale = true;};
  }, [project]);
  useEffect(() => {
    if (!runId) return;
    let stale = false; let timer: ReturnType<typeof setTimeout> | undefined;
    async function poll(attempt = 0) {
      try {const next = await rpc.getFlowExecution(runId); if (stale) return; setRun(next); if (active(next.status)) timer = setTimeout(() => void poll(), 1000);}
      catch (e) {if (!stale) {
        if (retryExecutionRead(e, attempt)) timer = setTimeout(() => void poll(attempt + 1), 250);
        else setError(`Run status unavailable: ${String(e)}. Refresh its record to retry; execution will not be replayed.`);
      }}
    }
    void poll(); return () => {stale = true; if (timer) clearTimeout(timer);};
  }, [runId, pollVersion]);
  function updateStep(index: number, patch: Partial<ExecutionStep>) {if (draft) edit(withSteps(draft, draft.definition.steps.map((s, i) => i === index ? {...s, ...patch} : s)));}
  function addStep(operation: ExecutionStep['operation']) {
    if (!draft) return;
    let n = 1; while (draft.definition.steps.some(s => s.id === `step-${n}`)) n++;
    const id = `step-${n}`;
    const next = withSteps(draft, [...draft.definition.steps, {id, depends_on: draft.definition.steps.length ? [draft.definition.steps[draft.definition.steps.length - 1].id] : [], operation, condition: null, timeout_ms: operation.capability === 'wait.v1' ? 5000 : 60000, max_attempts: 1}]);
    if (operation.capability === 'case.execute.v1') next.model_profiles[id] = profile;
    edit(next);
  }
  return <div><h2 className="view-title">Execution flows</h2>
    <p>Compose approved cases and explicit waits. Each case keeps its reviewed assertions and runs once with its selected model. Publish and prepare before approving execution.</p>
    {editor && !draft && <p role="alert">Invalid execution JSON structure. Correct it in the advanced editor; saving and publication remain disabled.</p>}{error && <p role="alert">{error} Your editor draft is retained. Reload explicitly discards it.</p>}{notice && <p role="status">{notice}</p>}{busy && <p role="status">Working…</p>}
    <Button disabled={busy} onClick={() => void perform(reload)}>Reload execution flows and discard editor draft</Button>
    {data && <><section className="card"><h3>Compose and review</h3>
      <label htmlFor="execution-project">Execution project</label><Select id="execution-project" value={project} disabled={busy} onValueChange={v => {setProject(v); setSelected(''); setSaved(''); setPublication(''); edit(blank(v));}}><option value="">Select a project</option>{projects.map(p => <option key={p} value={p}>{p}</option>)}</Select>
      <label htmlFor="execution-draft">Saved execution draft</label><Select id="execution-draft" value={selected} disabled={busy} onValueChange={v => {const d = data.library.drafts.find(item => item.definition.id === v); setSelected(v); setSaved(d ? pretty(d) : ''); if (d) {setProject(d.definition.project); edit(d);} else edit(blank(project));}}><option value="">New draft</option>{data.library.drafts.map(d => <option key={d.definition.id} value={d.definition.id}>{d.definition.title} — {d.definition.project}</option>)}</Select>
      {!editor && <Button disabled={busy || !project} onClick={() => edit(blank(project))}>Create execution draft</Button>}
      {draft && <><label htmlFor="execution-id">Flow identity</label><Input id="execution-id" value={draft.definition.id} disabled={busy} onValueChange={v => edit({...draft, definition: {...draft.definition, id: v}})} />
        <label htmlFor="execution-title">Flow title</label><Input id="execution-title" value={draft.definition.title} disabled={busy} onValueChange={v => edit({...draft, definition: {...draft.definition, title: v}})} />
        <p>Project: {draft.definition.project}. Dependencies determine execution order. Move controls preserve those dependencies.</p>
        {draft.definition.steps.map((step, index) => <fieldset key={step.id} disabled={busy} style={{marginBlock: 16}}><legend>{index + 1}. {step.id} · {step.operation.capability === 'wait.v1' ? 'Wait' : 'Approved case'}</legend>
          {step.operation.capability === 'case.execute.v1' && <><p>Case: {step.operation.case?.case_id}; contract: {step.operation.case?.contract?.id} v{step.operation.case?.contract?.version}</p><label htmlFor={`model-${step.id}`}>Model for {step.id}</label><Select id={`model-${step.id}`} value={draft.model_profiles[step.id] ?? ''} disabled={busy} onValueChange={v => edit({...draft, model_profiles: {...draft.model_profiles, [step.id]: v}})}><option value="">Select a visual model</option>{models.map(p => <option key={p.id} value={p.id}>{p.label} — {p.model}</option>)}</Select></>}
          {step.operation.capability === 'wait.v1' && step.operation.milliseconds?.kind === 'literal' && <><label htmlFor={`wait-${step.id}`}>Wait milliseconds</label><Input id={`wait-${step.id}`} type="number" value={String(step.operation.milliseconds.value)} disabled={busy} onValueChange={v => updateStep(index, {operation: {capability: 'wait.v1', milliseconds: {kind: 'literal', value: Number(v)}}})} /></>}
          <label htmlFor={`timeout-${step.id}`}>Step timeout milliseconds</label><Input id={`timeout-${step.id}`} type="number" value={step.timeout_ms} disabled={busy} onValueChange={v => updateStep(index, {timeout_ms: Number(v)})} />
          {step.operation.capability === 'wait.v1' && <><label htmlFor={`attempts-${step.id}`}>Maximum wait attempts (1–5)</label><Input id={`attempts-${step.id}`} type="number" value={step.max_attempts} disabled={busy} onValueChange={v => updateStep(index, {max_attempts: Number(v)})} /></>}
          <p>Dependencies</p>{draft.definition.steps.filter(s => s.id !== step.id).map(dep => <Checkbox key={dep.id} label={`${step.id} depends on ${dep.id}`} checked={step.depends_on.includes(dep.id)} disabled={busy} onCheckedChange={checked => updateStep(index, {depends_on: checked ? [...step.depends_on, dep.id] : step.depends_on.filter(id => id !== dep.id)})} />)}
          {step.condition && <p>Condition: {pretty(step.condition)}</p>}
          {[-1, 1].map(delta => <Button key={delta} disabled={busy || !canMove(draft.definition.steps, index, delta)} onClick={() => {const steps = [...draft.definition.steps]; [steps[index], steps[index + delta]] = [steps[index + delta], steps[index]]; edit(withSteps(draft, steps));}}>Move {step.id} {delta < 0 ? 'up' : 'down'}</Button>)}
          <Button disabled={busy || draft.definition.steps.some(s => s.id !== step.id && (s.depends_on.includes(step.id) || JSON.stringify([s.operation, s.condition]).includes(`"step":"${step.id}"`))) || JSON.stringify(draft.definition.outputs).includes(`"step":"${step.id}"`)} onClick={() => {const model_profiles = {...draft.model_profiles}; delete model_profiles[step.id]; edit({...withSteps(draft, draft.definition.steps.filter(s => s.id !== step.id)), model_profiles});}}>Remove {step.id}</Button>
        </fieldset>)}
        <Button disabled={busy || draft.definition.steps.length >= 64} onClick={() => addStep({capability: 'wait.v1', milliseconds: {kind: 'literal', value: 1000}})}>Add wait step</Button>
        <label htmlFor="execution-approved-case">Approved case to reuse</label><Select id="execution-approved-case" value={caseId} disabled={busy || !project} onValueChange={v => {setCaseId(v); setAsset(null); setContract(''); if (v) void perform(async valid => {const a = await rpc.getCaseAsset(project, v); if (valid() && a.status === 'approved') {setAsset(a); const c = a.provenance?.contracts?.contracts[0]?.publication.definition; setContract(c ? `${c.id}:${c.version}` : '');}});}}><option value="">Select an approved case</option>{cases.map(c => <option key={c.id} value={c.id}>{c.name} — {c.id}</option>)}</Select>
        {asset && <><p>{asset.expected_result}</p><label htmlFor="execution-contract">Reviewed source contract</label><Select id="execution-contract" value={contract} disabled={busy} onValueChange={setContract}><option value="">Select a reviewed contract</option>{asset.provenance?.contracts?.contracts.map(c => <option key={`${c.publication.definition.id}:${c.publication.definition.version}`} value={`${c.publication.definition.id}:${c.publication.definition.version}`}>{c.publication.definition.id} v{c.publication.definition.version}</option>)}</Select></>}
        <label htmlFor="execution-new-model">Model for the new case step</label><Select id="execution-new-model" value={profile} disabled={busy} onValueChange={setProfile}><option value="">Select a visual model</option>{models.map(p => <option key={p.id} value={p.id}>{p.label} — {p.model}</option>)}</Select>
        <Button disabled={busy || !asset || !contract || !profile || draft.definition.project !== project || draft.definition.steps.length >= 64} onClick={() => {const c = asset!.provenance?.contracts?.contracts.find(c => `${c.publication.definition.id}:${c.publication.definition.version}` === contract)?.publication.definition; if (c) addStep({capability: 'case.execute.v1', case: {project, case_id: asset!.case_id, content_revision: asset!.digest, contract: {id: c.id, version: c.version}}});}}>Add reviewed case step</Button>
        <label htmlFor="execution-total-timeout">Total execution timeout milliseconds</label><Input id="execution-total-timeout" type="number" value={draft.definition.total_timeout_ms} disabled={busy} onValueChange={v => edit({...draft, definition: {...draft.definition, total_timeout_ms: Number(v)}})} />
      </>}
      <Collapsible><CollapsibleTrigger>Advanced typed inputs, conditions and outputs</CollapsibleTrigger><CollapsibleContent><p>Edit typed ports and finite comparisons below. Server validation checks cycles, types, permissions and retry safety.</p><label htmlFor="execution-json">Execution draft JSON</label><TextArea id="execution-json" rows={18} value={editor} disabled={busy} onValueChange={v => {setEditor(v); clearReview();}} /></CollapsibleContent></Collapsible>
      <Collapsible><CollapsibleTrigger>Original saved execution draft</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace: 'pre-wrap'}}>{saved || '(not saved)'}</pre></CollapsibleContent></Collapsible>
      <Button disabled={busy || !draft} onClick={() => void perform(async valid => {const r = await rpc.validateExecutionFlowDraft(draft!); if (valid()) {setOrder(r.execution_order); setNotice('Draft validated. No actions were run.');}})}>Validate execution draft</Button>
      {order && <p>Validated execution order: {order.join(' → ')}</p>}
      <Checkbox label="I reviewed this execution draft and approve saving it" checked={saveApproved} disabled={busy || !draft} onCheckedChange={setSaveApproved} />
      <Button disabled={busy || !draft || !saveApproved} onClick={() => void perform(async valid => {setSaveApproved(false); const library = await rpc.saveExecutionFlowDraft(draft!, data.library.revision); if (valid()) {setData({...data, library}); setSelected(draft!.definition.id); setSaved(pretty(draft)); clearReview(); setNotice('Draft saved. Validate the exact saved content before publication.');}})}>Save execution draft</Button>
      <Checkbox label="I reviewed the saved execution flow and approve publishing an immutable version" checked={publishApproved} disabled={busy || !exactSaved || !order} onCheckedChange={setPublishApproved} />
      <Button disabled={busy || !exactSaved || !order || !publishApproved} onClick={() => void perform(async valid => {setPublishApproved(false); const library = await rpc.publishExecutionFlow(selected, data.library.revision); if (valid()) {setData({...data, library}); clearReview(); const latest = library.publications.filter(p => p.definition.id === selected).sort((a,b) => b.version-a.version)[0]; setPublication(latest ? `${selected}:${latest.version}` : ''); setSelected(''); setEditor(''); setSaved(''); setNotice('Immutable version published. Prepare execution below.');}})}>Publish execution flow</Button>
    </section>
    <section className="card"><h3>Prepare and approve execution</h3>
      <label htmlFor="execution-publication">Published execution version</label><Select id="execution-publication" value={publication} disabled={busy} onValueChange={v => {setPublication(v); clearPreparation();}}><option value="">Select a published execution version</option>{data.library.publications.map(p => <option key={`${p.definition.id}:${p.version}`} value={`${p.definition.id}:${p.version}`}>{p.definition.title} v{p.version} — {p.definition.project}</option>)}</Select>
      <label htmlFor="execution-inputs">Execution input values JSON</label><TextArea id="execution-inputs" rows={4} value={inputs} disabled={busy} onValueChange={v => {setInputs(v); clearPreparation();}} />
      {published && <p>Project: {published.definition.project}. Permissions: {published.definition.required_permissions.join(', ') || '(none)'}. Preparation freezes reviewed content and model destinations.</p>}
      <Checkbox label="I approve preparing these inputs with the listed execution permissions and model destinations" checked={prepareApproved} disabled={busy || !published || !validInputs} onCheckedChange={setPrepareApproved} />
      <Button disabled={busy || !published || !validInputs || !prepareApproved} onClick={() => void perform(async valid => {setPrepareApproved(false); const p = await rpc.prepareFlowExecution({id: published!.definition.id, version: published!.version, inputs: values(inputs), permissions: published!.definition.required_permissions, expected_revision: data.library.revision, expected_model_revision: data.model_revision}); if (valid()) {setPreparation(p); setDeadline(preparationDeadline(Date.now(), p.expires_in_seconds)); setRunApproved(false);}})}>Prepare execution</Button>
      {data.preparations.length > 0 && <><label htmlFor="execution-preparation">Unconsumed prepared execution</label><Select id="execution-preparation" value={preparation?.preparation_id ?? ''} disabled={busy} onValueChange={v => {const p = data.preparations.find(p => p.preparation_id === v); setPreparation(p ?? null); setDeadline(p ? preparationDeadline(preparationsReceivedAt, p.expires_in_seconds) : 0); setRunApproved(false);}}><option value="">Select a prepared execution</option>{data.preparations.map(p => <option key={p.preparation_id} value={p.preparation_id}>{p.flow_id} v{p.version} — {p.project}</option>)}</Select></>}
      {preparation && <><Collapsible><CollapsibleTrigger>Review frozen preparation before running</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace: 'pre-wrap'}}>{pretty(preparation)}</pre></CollapsibleContent></Collapsible><p>Target: {preparation.target_url}. Order: {preparation.execution_order.join(' → ')}. Expires in {remaining} seconds.</p>{Object.entries(preparation.models).map(([step, model]) => <p key={step}>{step}: {model.profile_id} / {model.model} → {model.base_url}</p>)}
        <Checkbox label="I approve running this exact prepared execution and sending its test data to the listed models" checked={runApproved} disabled={busy || remaining === 0} onCheckedChange={setRunApproved} />
        <Button disabled={busy || !runApproved || remaining === 0} onClick={() => void perform(async valid => {setRunApproved(false); const r = await rpc.runFlowExecution(preparation.preparation_id); if (valid()) {clearPreparation(); setRun(null); setRunId(r.run_id); setPollVersion(v => v + 1); const summary = {run_id: r.run_id, job_id: r.jobId, flow_id: preparation.flow_id, version: preparation.version, project: preparation.project, status: 'queued', created_at: new Date().toISOString()}; setData({...data, run_history: [summary, ...data.run_history.filter(item => item.run_id !== r.run_id)], preparations: data.preparations.filter(p => p.preparation_id !== preparation.preparation_id)}); setNotice('Execution admitted. Refresh restores its facts without replaying it.');}})}>Run prepared execution</Button>
      </>}
    </section>
    <section className="card"><h3>Execution history and cleanup</h3>
      <label htmlFor="execution-history">Execution record</label><Select id="execution-history" value={runId} disabled={busy} onValueChange={v => {setRunId(v); setRun(null);}}><option value="">Select an execution</option>{data.run_history.map(r => <option key={r.run_id} value={r.run_id}>{r.flow_id} v{r.version} — {r.project} / {r.status}</option>)}</Select>
      <Button disabled={busy || !runId} onClick={() => {setError(''); setPollVersion(v => v + 1);}}>Refresh execution record</Button>
      {run && <><p role="status">{run.flow_id} v{run.version}: {run.status}</p>{run.error && <p role="alert">{run.error}</p>}{active(run.status) && <Button disabled={busy || !run.job_id} onClick={() => void perform(async valid => {if (!run.job_id) return; await rpc.stopJob(run.job_id); if (valid()) setNotice('Cancellation requested. Waiting for owned resources and durable facts.');})}>Cancel execution</Button>}
        {run.facts?.steps.map(step => <div key={step.step_id}><h4>{step.step_id}: {step.status}</h4>{step.reason && <p>{step.reason}</p>}{step.attempts.map(a => <p key={a.attempt}>Attempt {a.attempt}: {a.status} · {a.started_ms}–{a.finished_ms} ms{a.error ? ` · ${a.error}` : ''}</p>)}</div>)}
        {run.facts?.cleanup_error && <p role="alert">Cleanup unconfirmed: {run.facts.cleanup_error}</p>}
        {Object.entries(run.case_runs ?? {}).map(([step, evidence]) => <p key={step}>{step}: case run {evidence.run_id} in {evidence.project} · {evidence.status}. {evidence.allure_results_ready ? 'Allure results ready. Select this case run on the Runs page to generate or open its report.' : 'Allure results unavailable; inspect the execution facts.'}</p>)}
        <Collapsible><CollapsibleTrigger>Frozen execution and private facts</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace: 'pre-wrap'}}>{pretty(run)}</pre></CollapsibleContent></Collapsible>
      </>}
    </section></>}
  </div>;
}
