import { useEffect, useRef, useState } from 'react';
import { Button, Checkbox, Select, TextArea } from './ui';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from './motion/collapsible';
import { rpc } from '../utils/rpc';
import type { FlowDraft, FlowPlan, FlowsResponse, FlowValidation } from '../types/flows';

const pretty = (value: unknown) => JSON.stringify(value, null, 2);
const template: FlowDraft = {
  id: 'review-flow', title: 'Collect reviewed results', inputs: {result: 'object'},
  outputs: {report: {value_type: 'object', source: {kind: 'step_output', step: 'collect-final', port: 'report'}}},
  required_permissions: ['report.write'],
  steps: [
    {id: 'collect-first', capability: 'report.collect', depends_on: [], inputs: {result: {kind: 'flow_input', name: 'result'}}, contract: null, model_role: null, max_attempts: 1},
    {id: 'collect-final', capability: 'report.collect', depends_on: ['collect-first'], inputs: {result: {kind: 'step_output', step: 'collect-first', port: 'report'}}, contract: null, model_role: null, max_attempts: 1},
  ],
};
function objectInput(text: string): Record<string, unknown> {
  const value: unknown = JSON.parse(text);
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Inputs must be a JSON object');
  return value as Record<string, unknown>;
}

export default function FlowsView({projects}: {projects: string[]}) {
  const [data, setData] = useState<FlowsResponse | null>(null);
  const [selected, setSelected] = useState('');
  const [editor, setEditor] = useState('');
  const [saved, setSaved] = useState('');
  const [saveApproved, setSaveApproved] = useState(false);
  const [publishApproved, setPublishApproved] = useState(false);
  const [validation, setValidation] = useState<FlowValidation | null>(null);
  const [publication, setPublication] = useState('');
  const [project, setProject] = useState('');
  const [inputs, setInputs] = useState('{}');
  const [permissionsApproved, setPermissionsApproved] = useState(false);
  const [plan, setPlan] = useState<FlowPlan | null>(null);
  const [historyId, setHistoryId] = useState('');
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const mounted = useRef(false);
  const request = useRef(0);
  const pending = useRef(false);
  const published = data?.library.publications.find(item => `${item.definition.id}:${item.version}` === publication);
  let parsed: FlowDraft | null = null;
  try { parsed = JSON.parse(editor) as FlowDraft; } catch { /* editable JSON */ }
  const exactSaved = !!parsed && pretty(parsed) === saved && parsed.id === selected;
  let validInputs = true;
  try { objectInput(inputs); } catch { validInputs = false; }

  function clearReview() {
    setSaveApproved(false); setPublishApproved(false); setValidation(null); setNotice('');
  }
  function clearPlan() { setPermissionsApproved(false); setPlan(null); setHistoryId(''); }
  async function perform(action: () => Promise<void>) {
    if (pending.current) return;
    pending.current = true;
    setBusy(true); setError(''); setNotice('');
    try { await action(); } catch (reason) { if (mounted.current) setError(String(reason)); }
    finally { pending.current = false; if (mounted.current) setBusy(false); }
  }
  async function reload() {
    const sequence = ++request.current;
    const result = await rpc.getFlows();
    if (!mounted.current || sequence !== request.current) return;
    setData(result); setSelected(''); setEditor(''); setSaved(''); clearReview(); clearPlan(); setPublication('');
  }
  useEffect(() => {
    mounted.current = true;
    const sequence = request.current + 1;
    pending.current = true; setBusy(true);
    void reload().catch(reason => {
      if (mounted.current && request.current === sequence) setError(String(reason));
    }).finally(() => {
      if (mounted.current && request.current === sequence) { pending.current = false; setBusy(false); }
    });
    return () => { mounted.current = false; request.current++; };
  }, []);

  return <div>
    <h2 className="view-title">Flows</h2>
    <p role="note">Planning only. Published flows and project snapshots do not execute actions or grant desktop permissions. Runtime orchestration is not available here.</p>
    {error && <p role="alert">{error} Editor text is retained. Reload explicitly discards it.</p>}
    {notice && <p role="status">{notice}</p>}
    {busy && <p role="status">Working…</p>}
    <Button disabled={busy} onClick={() => void perform(reload)}>Reload flows and discard editor draft</Button>
    {data && <>
      <section className="card">
        <h3>Draft and publication</h3>
        <label htmlFor="flow-draft-select">Saved flow draft</label>
        <Select id="flow-draft-select" value={selected} disabled={busy} onValueChange={id => {
          setError(''); setSelected(id); const draft = data.library.drafts.find(item => item.id === id);
          const text = draft ? pretty(draft) : ''; setEditor(text); setSaved(text); clearReview();
        }}><option value="">New draft</option>{data.library.drafts.map(item => <option key={item.id} value={item.id}>{item.id} — {item.title}</option>)}</Select>
        <Button disabled={busy} onClick={() => {setSelected(''); setEditor(pretty(template)); setSaved(''); clearReview();}}>Load planning template</Button>
        <p>Edit JSON to compose typed inputs, step dependencies and output references. The server checks capabilities, permissions and retry safety.</p>
        <label htmlFor="flow-draft-json">Flow draft JSON</label>
        <TextArea id="flow-draft-json" rows={18} value={editor} disabled={busy} onValueChange={value => {setEditor(value); clearReview();}} />
        <Collapsible><CollapsibleTrigger>Original saved flow draft</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace: 'pre-wrap'}}>{saved || '(not saved)'}</pre></CollapsibleContent></Collapsible>
        <Button disabled={busy || !parsed} onClick={() => void perform(async () => {
          const result = await rpc.validateFlowDraft(parsed!);
          if (mounted.current) {setValidation(result); setNotice('Flow validation passed. No actions were executed.');}
        })}>Validate flow draft</Button>
        {validation && <p>Validated order: {validation.execution_order.join(' → ')}</p>}
        <Checkbox label="I reviewed this flow draft and approve saving it" checked={saveApproved} disabled={busy} onCheckedChange={setSaveApproved} />
        <Button disabled={busy || !parsed || !saveApproved} onClick={() => void perform(async () => {
          const draft = parsed!;
          try {
            const library = await rpc.saveFlowDraft(draft, data.library.revision);
            if (!mounted.current) return;
            setData({...data, library}); setSelected(draft.id); setSaved(pretty(draft)); clearReview(); setNotice('Flow draft saved. Validate the saved content before publication.');
          } finally { if (mounted.current) setSaveApproved(false); }
        })}>Save flow draft</Button>
        {!exactSaved && editor && <p>Save this exact draft before publishing. Published versions cannot be overwritten.</p>}
        <Checkbox label="I reviewed the saved flow and approve publishing a new immutable version" checked={publishApproved} disabled={busy || !exactSaved || !validation} onCheckedChange={setPublishApproved} />
        <Button disabled={busy || !exactSaved || !validation || !publishApproved} onClick={() => void perform(async () => {
          try {
            const library = await rpc.publishFlow(selected, data.library.revision);
            if (mounted.current) {
              setData({...data, library});
              setSelected(''); setEditor(''); setSaved(''); clearReview(); clearPlan();
              const latest = library.publications.filter(item => item.definition.id === selected)
                .sort((a, b) => b.version - a.version)[0];
              setPublication(latest ? `${latest.definition.id}:${latest.version}` : '');
              setNotice('Immutable flow version published. Prepare a project plan below.');
            }
          } finally { if (mounted.current) setPublishApproved(false); }
        })}>Publish flow version</Button>
      </section>
      <section className="card">
        <h3>Project plan</h3>
        <label htmlFor="flow-publication">Published flow version</label>
        <Select id="flow-publication" value={publication} disabled={busy} onValueChange={value => {setPublication(value); clearPlan();}}>
          <option value="">Select a published version</option>{data.library.publications.map(item => <option key={`${item.definition.id}:${item.version}`} value={`${item.definition.id}:${item.version}`}>{item.definition.id} v{item.version}</option>)}
        </Select>
        <label htmlFor="flow-project">Plan project</label>
        <Select id="flow-project" value={project} disabled={busy} onValueChange={value => {setProject(value); clearPlan();}}><option value="">Select an existing project</option>{projects.map(name => <option key={name} value={name}>{name}</option>)}</Select>
        <label htmlFor="flow-inputs">Typed flow inputs JSON</label>
        <TextArea id="flow-inputs" rows={5} value={inputs} disabled={busy} onValueChange={value => {setInputs(value); clearPlan();}} />
        {published && <p>Required planning permissions: {published.definition.required_permissions.join(', ') || '(none)'}. This confirmation grants no execution or OS permissions.</p>}
        <Checkbox label="I approve preparing this project plan with the listed planning permissions" checked={permissionsApproved} disabled={busy || !published || !projects.includes(project)} onCheckedChange={setPermissionsApproved} />
        <Button disabled={busy || !published || !projects.includes(project) || !validInputs || !permissionsApproved} onClick={() => void perform(async () => {
          try {
            const result = await rpc.prepareFlowPlan({id: published!.definition.id, version: published!.version, project, inputs: objectInput(inputs), permissions: published!.definition.required_permissions, expected_revision: data.library.revision, expected_model_revision: data.model_revision});
            if (mounted.current) {setPlan(result); setData({...data, plan_history: [result, ...data.plan_history]}); setNotice('Project snapshot saved. This plan is not executable.');}
          } finally { if (mounted.current) setPermissionsApproved(false); }
        })}>Prepare project plan</Button>
      </section>
      <section className="card">
        <h3>Plan history</h3>
        <label htmlFor="flow-plan-history">Saved project plan</label>
        <Select id="flow-plan-history" value={historyId} disabled={busy} onValueChange={value => {setHistoryId(value); setPlan(null);}}><option value="">Select a saved plan</option>{data.plan_history.map(item => <option key={item.plan_id} value={item.plan_id}>{item.plan_id} — {item.project} / {item.flow_id} v{item.flow_version}</option>)}</Select>
        <Button disabled={busy || !historyId} onClick={() => void perform(async () => {const result = await rpc.getFlowPlan(historyId); if (mounted.current) setPlan(result);})}>View saved plan</Button>
        {plan && <><p>Saved snapshot: {plan.plan_id}. Executable: {String(plan.executable)}.</p><Collapsible><CollapsibleTrigger>Frozen project plan snapshot</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace: 'pre-wrap'}}>{pretty(plan)}</pre></CollapsibleContent></Collapsible></>}
      </section>
      <Collapsible><CollapsibleTrigger>Server capability catalog</CollapsibleTrigger><CollapsibleContent>{data.capabilities.map(capability => <div key={capability.id}><h4>{capability.id}</h4><p>Planning: {String(capability.planning_supported)} · Runner available: {String(capability.runner_available)} · Idempotent: {String(capability.idempotent)}</p><pre style={{whiteSpace: 'pre-wrap'}}>{pretty({inputs: capability.inputs, outputs: capability.outputs, permissions: capability.permissions})}</pre></div>)}</CollapsibleContent></Collapsible>
    </>}
  </div>;
}
