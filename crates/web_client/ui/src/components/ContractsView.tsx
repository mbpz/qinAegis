import { Button, Checkbox, Input, Select, TextArea } from './ui';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from './motion/collapsible';
import { FileUpload } from './motion/file-upload';
import { useEffect, useRef, useState } from 'react';
import { rpc, RpcError } from '../utils/rpc';
import type { ContractDraft, ContractsBundle, ContractsSnapshot, ContractValueType } from '../types/contracts';

type Editor = {
  id: string; version: string; intent: string; inputs: string; outputs: string;
  preconditions: string; assertions: string; required_capabilities: string;
};
const blank = (): Editor => ({ id: '', version: '1', intent: '', inputs: '{}', outputs: '{}', preconditions: '', assertions: '', required_capabilities: '' });
const pretty = (value: unknown) => JSON.stringify(value, null, 2);
const canonical = (value: unknown) => JSON.stringify(value, (_, item) => item && typeof item === 'object' && !Array.isArray(item)
  ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b))) : item);
const lines = (value: string) => value.split('\n').map(line => line.trim()).filter(Boolean);
const keyOf = (draft: ContractDraft) => JSON.stringify([draft.id, draft.version]);
const fields: (keyof ContractDraft)[] = ['id', 'version', 'intent', 'inputs', 'outputs', 'preconditions', 'assertions', 'required_capabilities'];

function objectJson(text: string): Record<string, unknown> {
  const value: unknown = JSON.parse(text);
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('JSON must be an object.');
  return value as Record<string, unknown>;
}
function typedMap(text: string): Record<string, ContractValueType> {
  const value = objectJson(text);
  for (const [name, type] of Object.entries(value)) {
    if (!name.trim() || typeof type !== 'string' || !['string', 'number', 'boolean', 'object'].includes(type)) {
      throw new Error('Port names must be nonempty; types must be string, number, boolean, or object.');
    }
  }
  return value as Record<string, ContractValueType>;
}
function definition(editor: Editor): ContractDraft {
  const version = Number(editor.version);
  if (!editor.id.trim() || !editor.intent.trim()) throw new Error('Contract ID and intent are required.');
  if (!Number.isInteger(version) || version < 1 || version > 4294967295) throw new Error('Version must be a positive 32-bit integer.');
  const assertions = lines(editor.assertions);
  if (!assertions.length) throw new Error('At least one assertion is required.');
  return {
    id: editor.id.trim(), version, intent: editor.intent.trim(),
    inputs: typedMap(editor.inputs), outputs: typedMap(editor.outputs),
    preconditions: lines(editor.preconditions), assertions,
    required_capabilities: lines(editor.required_capabilities),
  };
}
function edit(draft: ContractDraft): Editor {
  return {
    ...draft, version: String(draft.version), inputs: pretty(draft.inputs), outputs: pretty(draft.outputs),
    preconditions: draft.preconditions.join('\n'), assertions: draft.assertions.join('\n'), required_capabilities: draft.required_capabilities.join('\n'),
  };
}

export default function ContractsView({ projects }: { projects: string[] }) {
  const [lifecycleReason, setLifecycleReason] = useState('');
  const [newId, setNewId] = useState('');
  const [targetVersion, setTargetVersion] = useState('');
  const [migrationJson, setMigrationJson] = useState('[]');
  const [lifecycleApproved, setLifecycleApproved] = useState(false);
  const [snapshot, setSnapshot] = useState<ContractsSnapshot | null>(null);
  const [editor, setEditor] = useState<Editor>(blank);
  const [selected, setSelected] = useState('');
  const [compare, setCompare] = useState('');
  const [project, setProject] = useState('');
  const [parameters, setParameters] = useState('{}');
  const [correctionSource, setCorrectionSource] = useState<{ id: string; version: number } | null>(null);
  const [reason, setReason] = useState('');
  const [evidence, setEvidence] = useState('');
  const [publishConfirmed, setPublishConfirmed] = useState(false);
  const [reviewConfirmed, setReviewConfirmed] = useState<string | null>(null);
  const [pendingBundle, setPendingBundle] = useState<ContractsBundle | null>(null);
  const [pendingFileName, setPendingFileName] = useState('');
  const [importReviewRevision, setImportReviewRevision] = useState<number | null>(null);
  const [message, setMessage] = useState('');
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState(false);
  const mounted = useRef(false);
  const locked = useRef(false);
  const requestId = useRef(0);

  useEffect(() => {
    mounted.current = true;
    const id = ++requestId.current;
    locked.current = true;
    setBusy(true);
    rpc.getContracts().then(data => {
      if (mounted.current && requestId.current === id) setSnapshot(data);
    }).catch(error => {
      if (mounted.current && requestId.current === id) { setMessage(String(error)); setFailed(true); }
    }).finally(() => {
      if (mounted.current && requestId.current === id) { locked.current = false; setBusy(false); }
    });
    return () => { mounted.current = false; ++requestId.current; };
  }, []);

  async function perform(action: () => Promise<ContractsSnapshot | void>, success: string) {
    if (locked.current) return;
    locked.current = true;
    const id = ++requestId.current;
    setBusy(true);
    setMessage('');
    setFailed(false);
    setImportReviewRevision(null);
    setLifecycleApproved(false);
    try {
      const data = await action();
      if (!mounted.current || requestId.current !== id) return;
      if (data) setSnapshot(data);
      setPublishConfirmed(false);
      setLifecycleApproved(false);
      setReviewConfirmed(null);
      setMessage(success);
    } catch (error) {
      if (!mounted.current || requestId.current !== id) return;
      const conflict = error instanceof RpcError && /revision|conflict/i.test(`${error.code} ${error.message}`);
      setMessage(`${String(error)}${conflict ? ' Refresh the library before retrying. Your editor and binding inputs are retained; review the refreshed version first.' : ' Your editor inputs are retained.'}`);
      setFailed(true);
    } finally {
      if (mounted.current && requestId.current === id) { locked.current = false; setBusy(false); }
    }
  }
  function mutation(action: (revision: number) => Promise<ContractsSnapshot>, success: string) {
    if (!snapshot) return;
    return perform(() => action(snapshot.revision), success);
  }
  const publications = snapshot?.publications ?? [];
  const publication = publications.find(item => keyOf(item.definition) === selected);
  const deprecated = snapshot?.deprecations?.find(item => item.contract_id === publication?.definition.id && item.version === publication?.definition.version);
  useEffect(() => { setLifecycleApproved(false); }, [selected, snapshot?.revision, lifecycleReason, newId, targetVersion, migrationJson]);
  const comparison = publications.find(item => keyOf(item.definition) === compare);
  const references = snapshot?.bindings.filter(item => item.contract_id === publication?.definition.id) ?? [];
  const pendingDraft = snapshot?.drafts.find(item => item.id === editor.id.trim());
  const editorMatchesDraft = (() => {
    try { return !!pendingDraft && canonical(definition(editor)) === canonical(pendingDraft); } catch { return false; }
  })();
  const update = (field: keyof Editor, value: string) => {
    setEditor(previous => ({ ...previous, [field]: value }));
    setPublishConfirmed(false);
  };
  function selectPublication(key: string) {
    setSelected(key);
    setCompare('');
    const chosen = publications.find(item => keyOf(item.definition) === key);
    if (chosen) {
      const sample = Object.fromEntries(Object.entries(chosen.definition.inputs).map(([name, type]) => [name, type === 'string' ? '' : type === 'number' ? 0 : type === 'boolean' ? false : {}]));
      setParameters(pretty(sample));
    }
  }
  function startCorrection() {
    if (!publication) return;
    const original = publication.definition;
    const latest = Math.max(original.version, ...publications.filter(item => item.definition.id === original.id).map(item => item.definition.version));
    setEditor(edit({ ...original, version: latest + 1 }));
    setCorrectionSource({ id: original.id, version: original.version });
    setReason(''); setEvidence(''); setPublishConfirmed(false);
  }
  async function importFile(file: File) {
    setPendingBundle(null);
    setPendingFileName('');
    setImportReviewRevision(null);
    await perform(async () => {
      if (file.size > 1024 * 1024) throw new Error('Shared package exceeds the 1 MiB limit.');
      const parsed = objectJson(await file.text());
      if (parsed.schema_version !== 1 || !Array.isArray(parsed.contracts)) throw new Error('Expected a schema_version 1 package with a contracts array.');
      if ('bindings' in parsed) throw new Error('Shared packages must not include project bindings.');
      if (!parsed.contracts.length) throw new Error('Shared package contains no contracts to review.');
      if (!parsed.contracts.every(item => item && typeof item === 'object' && !Array.isArray(item) && typeof item.reviewer === 'string' && item.definition && typeof item.definition === 'object' && !Array.isArray(item.definition))) throw new Error('Every shared contract must include a definition and external reviewer.');
      const bundle: ContractsBundle = { schema_version: 1, contracts: parsed.contracts as ContractsBundle['contracts'] };
      if (mounted.current) { setPendingBundle(bundle); setPendingFileName(file.name); }
    }, 'Package loaded for review only. No contracts have been imported.');
  }
  async function approveImport() {
    if (!pendingBundle || !snapshot || importReviewRevision !== snapshot.revision) return;
    const bundle = pendingBundle;
    await mutation(async revision => {
      const data = await rpc.importContracts({ bundle, expected_revision: revision, approved: true });
      if (mounted.current) { setPendingBundle(null); setPendingFileName(''); }
      return data;
    }, 'Reviewed shared contracts imported. No project bindings were imported.');
  }
  async function exportFile() {
    await perform(async () => {
      const data = await rpc.exportContracts();
      if (!mounted.current) return;
      const blob = new Blob([pretty({ schema_version: 1, contracts: data.contracts })], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      try {
        const link = document.createElement('a');
        link.href = url; link.download = 'qinaegis-contracts.json'; link.click();
      } finally { setTimeout(() => URL.revokeObjectURL(url), 1000); }
    }, 'Shared package exported with publications only. No project bindings included.');
  }

  return <div className="view">
    <h2 className="view-title">Contracts</h2>
    <p>Reusable, versioned contracts across projects. Publications are immutable; corrections require human review. Reviewer identity is recorded by the local service.</p>
    {message && <div className="card" role={failed ? 'alert' : 'status'} style={{ borderColor: failed ? 'var(--accent-red)' : 'var(--accent-green)', whiteSpace: 'pre-wrap' }}>{message}</div>}
    <fieldset disabled={busy} style={{ border: 0, padding: 0, minWidth: 0 }}>
      <div className="card">
        <div className="card-title">Library {snapshot && `· revision ${snapshot.revision}`}</div>
        <Button type="button" className="btn" onClick={() => perform(() => rpc.getContracts(), 'Library refreshed. Editor inputs retained.')}>Refresh library</Button>{' '}
        <Button type="button" className="btn" disabled={!snapshot} onClick={exportFile}>Export shared package</Button>
        <FileUpload
          value={[]}
          multiple={false}
          maxFiles={1}
          accept="application/json,.json"
          disabled={!snapshot || busy}
          title="Select a local contract package for review"
          description="JSON, up to 1 MiB; publications only. Selection prepares a local preview. Import requires a separate human approval."
          browseLabel="Choose local JSON"
          labels={{ input: 'Select local contract package for review', queued: 'Selected locally; awaiting review', uploading: 'Reading local selection', success: 'Local preview ready; awaiting review', error: 'Local selection could not be read', limitTitle: 'Local selection limit reached', limitDescription: '{count} of {max} local files selected', retry: 'Read {name} again', remove: 'Discard local selection {name}', progress: '{name} local reading progress' }}
          onFilesAdded={(_, files) => { if (files[0]) void importFile(files[0]); }}
        />
        {pendingBundle && <section aria-label="Shared package review">
          <h3>Review pending import: {pendingFileName}</h3>
          <p>{pendingBundle.contracts.length} publications. Review every complete definition, assertion, capability, and correction record below. The reviewer values in this file are external provenance; they do not mean a local human has reviewed these contracts.</p>
          <Collapsible defaultOpen><CollapsibleTrigger>Complete pending package, including definitions, assertions, and original external reviewers</CollapsibleTrigger><CollapsibleContent><pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{pretty(pendingBundle)}</pre></CollapsibleContent></Collapsible>
          <Checkbox label={"I have personally reviewed all pending definitions and assertions and explicitly approve importing this package into the current library."} checked={!!snapshot && importReviewRevision === snapshot.revision} onCheckedChange={checked => setImportReviewRevision(checked && snapshot ? snapshot.revision : null)} /><br />
          <Button type="button" className="btn btn-primary" disabled={!snapshot || importReviewRevision !== snapshot.revision} onClick={() => void approveImport()}>Import reviewed package</Button>{' '}
          <Button type="button" className="btn" onClick={() => { setPendingBundle(null); setPendingFileName(''); setImportReviewRevision(null); }}>Discard pending package</Button>
          <p>Any operation or failed import clears this approval. After a revision conflict, refresh the library, compare the pending package again, then confirm a new review.</p>
        </section>}
        {!snapshot && <p>{busy ? 'Loading contracts…' : 'Load the library to begin.'}</p>}
        {snapshot && <p>{snapshot.drafts.length} drafts · {publications.length} publications · {snapshot.proposals.filter(item => item.status === 'pending').length} pending proposals</p>}
        {snapshot?.drafts.map(draft => <Button type="button" className="btn" key={draft.id} onClick={() => { setEditor(edit(draft)); setCorrectionSource(null); setPublishConfirmed(false); }}>Edit {draft.id} v{draft.version}</Button>)}
        <div className="input-group"><label htmlFor="contract-publication">Published contract / version</label><Select id="contract-publication" className="input" value={selected} onValueChange={value => selectPublication(value)}><option value="">Select a publication</option>{publications.map(item => <option key={keyOf(item.definition)} value={keyOf(item.definition)}>{item.definition.id} v{item.definition.version}</option>)}</Select></div>
      </div>

      <div className="card">
        <div className="card-title">{correctionSource ? `Correction of ${correctionSource.id} v${correctionSource.version}` : 'Draft editor'}</div>
        <Button type="button" className="btn" onClick={() => { setEditor(blank()); setCorrectionSource(null); setPublishConfirmed(false); }}>New draft</Button>
        <div className="form-row"><div className="input-group"><label htmlFor="contract-id">Contract ID</label><Input id="contract-id" className="input" value={editor.id} onValueChange={value => update('id', value)} /></div><div className="input-group"><label htmlFor="contract-version">Version</label><Input id="contract-version" className="input" type="number" min="1" step="1" value={editor.version} onValueChange={value => update('version', value)} /></div></div>
        <div className="input-group"><label htmlFor="contract-intent">Intent</label><TextArea id="contract-intent" className="input" value={editor.intent} onValueChange={value => update('intent', value)} /></div>
        <p>Inputs / outputs: JSON objects mapping port names to string, number, boolean, or object. Example: {`{"target":"string"}`}</p>
        <div className="form-row">{(['inputs', 'outputs'] as const).map(field => <div className="input-group" key={field}><label htmlFor={`contract-${field}`}>{field}</label><TextArea id={`contract-${field}`} className="input" rows={4} value={editor[field]} onValueChange={value => update(field, value)} /></div>)}</div>
        {(['preconditions', 'assertions', 'required_capabilities'] as const).map(field => <div className="input-group" key={field}><label htmlFor={`contract-${field}`}>{field.replace(/_/g, ' ')} (one per line{field === 'assertions' ? '; required' : ''})</label><TextArea id={`contract-${field}`} className="input" rows={3} value={editor[field]} onValueChange={value => update(field, value)} /></div>)}
        <Button type="button" className="btn btn-primary" disabled={!snapshot} onClick={() => mutation(revision => rpc.saveContractDraft({ draft: definition(editor), expected_revision: revision }), 'Draft validated and saved.')}>Validate & save draft</Button>
        {!correctionSource && <>
          <p>Save the exact draft before publishing. Published versions cannot be edited.</p>
          <Checkbox label={"I have reviewed this saved draft and explicitly approve publication."} checked={publishConfirmed} disabled={!editorMatchesDraft} onCheckedChange={checked => setPublishConfirmed(checked)} /><br />
          <Button type="button" className="btn btn-primary" disabled={!snapshot || !publishConfirmed || !editorMatchesDraft} onClick={() => mutation(revision => rpc.publishContract({ id: editor.id.trim(), expected_revision: revision, approved: true }), 'Contract published with local human approval.')}>Publish reviewed draft</Button>
        </>}
        {correctionSource && <>
          <div className="input-group"><label htmlFor="correction-reason">Correction reason</label><TextArea id="correction-reason" className="input" value={reason} onValueChange={value => setReason(value)} /></div>
          <div className="input-group"><label htmlFor="correction-evidence">Evidence references (one per line; do not include secrets)</label><TextArea id="correction-evidence" className="input" value={evidence} onValueChange={value => setEvidence(value)} /></div>
          <Button type="button" className="btn btn-primary" disabled={!snapshot} onClick={() => mutation(revision => {
            const replacement = definition(editor);
            if (replacement.id !== correctionSource.id || replacement.version <= correctionSource.version) throw new Error('Correction must preserve ID and advance version.');
            if (!reason.trim() || !lines(evidence).length) throw new Error('Correction reason and at least one evidence reference are required.');
            return rpc.proposeContractCorrection({ contract_id: correctionSource.id, version: correctionSource.version, replacement, reason: reason.trim(), evidence: lines(evidence), expected_revision: revision });
          }, 'Correction proposed. A human must approve it before publication.')}>Submit correction proposal</Button>
        </>}
      </div>

      {publication && <>
        <div className="card">
          <div className="card-title">Version history & comparison</div>
          <p>{publication.definition.id} v{publication.definition.version} · reviewed by {publication.reviewer}</p>
          {publication.correction && <p>Corrected v{publication.correction.original_version}: {publication.correction.reason}<br />Evidence: {publication.correction.evidence.join(', ')}</p>}
          <Button type="button" className="btn" onClick={startCorrection}>Propose correction from this version</Button>
          <div className="input-group"><label htmlFor="contract-compare">Compare with another version</label><Select id="contract-compare" className="input" value={compare} onValueChange={value => setCompare(value)}><option value="">Choose version</option>{publications.filter(item => item.definition.id === publication.definition.id && keyOf(item.definition) !== selected).map(item => <option key={keyOf(item.definition)} value={keyOf(item.definition)}>v{item.definition.version}</option>)}</Select></div>
          <div style={{ overflowX: 'auto' }}><table><thead><tr><th>Field</th><th>Selected v{publication.definition.version}</th>{comparison && <th>Compare v{comparison.definition.version}</th>}</tr></thead><tbody>{fields.map(field => <tr key={field}><th>{field}</th><td style={{ verticalAlign: 'top', whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{pretty(publication.definition[field])}</td>{comparison && <td style={{ verticalAlign: 'top', whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', background: pretty(publication.definition[field]) !== pretty(comparison.definition[field]) ? 'var(--bg-tertiary)' : undefined }}>{pretty(comparison.definition[field])}{pretty(publication.definition[field]) !== pretty(comparison.definition[field]) && <strong> (changed)</strong>}</td>}</tr>)}</tbody></table></div>
        </div>
        <div className="card">
          <div className="card-title">Contract lifecycle</div>
          {deprecated && <p role="status">Deprecated: {deprecated.reason} · reviewed by {deprecated.reviewer}. New bindings and generation are blocked; historical evidence remains available.</p>}
          {snapshot?.derivations?.filter(item => item.contract_id === publication.definition.id).map(item => <p key={item.contract_id}>Derived from {item.source_id} v{item.source_version}: {item.reason} · {item.reviewer}</p>)}
          <p>Lifecycle changes preserve published versions and historical runs. Migration changes only selected bindings; generated cases require regeneration and human review.</p>
          <label htmlFor="lifecycle-reason">Reason for lifecycle action</label>
          <TextArea id="lifecycle-reason" value={lifecycleReason} onValueChange={setLifecycleReason} />
          <Checkbox label="I have reviewed the selected version, reason and affected bindings and approve the chosen lifecycle action." checked={lifecycleApproved} onCheckedChange={setLifecycleApproved} />
          <p><Button disabled={!!deprecated || !lifecycleApproved || !lifecycleReason.trim()} onClick={() => mutation(revision => rpc.deprecateContract({contract_id: publication.definition.id, version: publication.definition.version, reason: lifecycleReason.trim(), expected_revision: revision, approved: true}), 'Version deprecated. Historical assets preserved.')}>Deprecate selected version</Button></p>
          <label htmlFor="derive-id">New contract identity</label>
          <Input id="derive-id" value={newId} onValueChange={setNewId} />
          <Button disabled={!lifecycleApproved || !lifecycleReason.trim() || !newId.trim()} onClick={() => mutation(revision => rpc.deriveContract({source_id: publication.definition.id, source_version: publication.definition.version, new_id: newId.trim(), reason: lifecycleReason.trim(), expected_revision: revision, approved: true}), 'Derived v1 draft created. Review and publish separately.')}>Create derived draft</Button>
          <Collapsible><CollapsibleTrigger>Explicit binding migration</CollapsibleTrigger><CollapsibleContent>
            <p>Source: {publication.definition.id} v{publication.definition.version}. Only projects listed below move. Review exact parameters against the target input types.</p>
            <label htmlFor="migration-target">Target publication</label>
            <Select id="migration-target" value={targetVersion} onValueChange={setTargetVersion}><option value="">Choose target version</option>{publications.filter(item => item.definition.id === publication.definition.id && item.definition.version !== publication.definition.version && !snapshot?.deprecations?.some(d => d.contract_id === item.definition.id && d.version === item.definition.version)).map(item => <option key={item.definition.version} value={String(item.definition.version)}>v{item.definition.version}</option>)}</Select>
            <Button onClick={() => setMigrationJson(pretty(references.filter(item => item.contract_version === publication.definition.version).map(item => ({project: item.project, parameters: item.parameters}))))}>Prepare source bindings for review</Button>
            <label htmlFor="migration-bindings">Selected bindings JSON (array of project and parameters)</label>
            <TextArea id="migration-bindings" rows={6} value={migrationJson} onValueChange={setMigrationJson} />
            <pre style={{whiteSpace: 'pre-wrap'}}>Target inputs: {pretty(publications.find(item => item.definition.id === publication.definition.id && String(item.definition.version) === targetVersion)?.definition.inputs ?? {})}</pre>
            <Button disabled={!lifecycleApproved || !lifecycleReason.trim() || !targetVersion} onClick={() => mutation(revision => {
              const values: unknown = JSON.parse(migrationJson);
              if (!Array.isArray(values) || !values.length) throw new Error('Select at least one project binding.');
              const target = publications.find(item => item.definition.id === publication.definition.id && String(item.definition.version) === targetVersion);
              if (!target) throw new Error('Select a target publication.');
              const seen = new Set<string>();
              const bindings = values.map(item => {
                if (!item || typeof item.project !== 'string' || seen.has(item.project) || !references.some(ref => ref.project === item.project && ref.contract_version === publication.definition.version)) throw new Error('Every selected project must have one binding to the source version.');
                seen.add(item.project);
                const parameters = objectJson(JSON.stringify(item.parameters));
                if (Object.keys(parameters).sort().join('\0') !== Object.keys(target.definition.inputs).sort().join('\0')) throw new Error('Parameters must exactly match target inputs.');
                for (const [name, type] of Object.entries(target.definition.inputs)) {
                  const value = parameters[name];
                  if (type === 'object' ? !value || typeof value !== 'object' || Array.isArray(value) : typeof value !== type) throw new Error(`Incorrect parameter type: ${name}`);
                }
                return {project: item.project, parameters};
              });
              return rpc.migrateContractBindings({contract_id: publication.definition.id, from_version: publication.definition.version, to_version: target.definition.version, bindings, reason: lifecycleReason.trim(), expected_revision: revision, approved: true});
            }, 'Selected bindings migrated atomically. Existing cases and runs retain their original sources.')}>Migrate reviewed bindings</Button>
          </CollapsibleContent></Collapsible>
          {snapshot?.migrations?.filter(item => item.contract_id === publication.definition.id).map((item, i) => <p key={i}>Migration v{item.from_version} → v{item.to_version}: {item.projects.join(', ')} · {item.reason} · {item.reviewer}</p>)}
        </div>
        <div className="card">
          <div className="card-title">Project binding & impact references</div>
          <p>Bind immutable {publication.definition.id} v{publication.definition.version}. Corrections do not migrate existing bindings automatically.</p>
          <div className="input-group"><label htmlFor="binding-project">Project</label><Select id="binding-project" className="input" value={project} onValueChange={value => setProject(value)}><option value="">Select project</option>{projects.map(name => <option key={name} value={name}>{name}</option>)}</Select></div>
          <div className="input-group"><label htmlFor="binding-parameters">Typed parameters JSON (exactly the declared input names)</label><TextArea id="binding-parameters" className="input" rows={5} value={parameters} onValueChange={value => setParameters(value)} /></div>
          <Button type="button" className="btn btn-primary" disabled={!project || !snapshot || !!deprecated} onClick={() => mutation(revision => {
            const values = objectJson(parameters);
            const ports = publication.definition.inputs;
            if (Object.keys(values).sort().join('\0') !== Object.keys(ports).sort().join('\0')) throw new Error('Parameters must exactly match declared input names.');
            for (const [name, type] of Object.entries(ports)) {
              const value = values[name];
              if (type === 'object' ? !value || typeof value !== 'object' || Array.isArray(value) : typeof value !== type) throw new Error(`Incorrect parameter type: ${name} must be ${type}.`);
            }
            return rpc.bindContract({ project, contract_id: publication.definition.id, version: publication.definition.version, parameters: values, expected_revision: revision });
          }, 'Project binding saved for the selected contract version.')}>Save project binding</Button>
          <p>{references.length} project references across versions of this contract.</p>
          {references.map(item => <div key={JSON.stringify([item.project, item.contract_id, item.contract_version])}><strong>{item.project} · v{item.contract_version}</strong>{item.contract_version === publication.definition.version && ' · selected version'}<pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{pretty(item.parameters)}</pre></div>)}
        </div>
      </>}

      <div className="card">
        <div className="card-title">Correction proposals</div>
        {!snapshot?.proposals.length && <p>No correction proposals.</p>}
        {snapshot?.proposals.map(proposal => <div key={proposal.id} style={{ borderTop: '1px solid var(--border-color)', padding: '12px 0' }}>
          <strong>{proposal.id} · {proposal.replacement.id} v{proposal.original_version} → v{proposal.replacement.version} · {proposal.status}</strong>
          <p>Reason: {proposal.reason}<br />Evidence: {proposal.evidence.join(', ')}{proposal.reviewer && <><br />Reviewer: {proposal.reviewer}</>}</p>
          <p>Existing references (remain pinned after approval): {snapshot.bindings.filter(item => item.contract_id === proposal.replacement.id).map(item => `${item.project} v${item.contract_version}`).join(', ') || 'None'}</p>
          <Collapsible><CollapsibleTrigger>Original published definition v{proposal.original_version}</CollapsibleTrigger><CollapsibleContent><pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{pretty(publications.find(item => item.definition.id === proposal.replacement.id && item.definition.version === proposal.original_version)?.definition ?? 'Original publication unavailable; refresh before reviewing.')}</pre></CollapsibleContent></Collapsible>
          <Collapsible><CollapsibleTrigger>Review complete replacement definition</CollapsibleTrigger><CollapsibleContent><pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{pretty(proposal.replacement)}</pre></CollapsibleContent></Collapsible>
          {proposal.status === 'pending' && <>
            <Checkbox label={"I have reviewed the reason, evidence, replacement, and affected project references."} checked={reviewConfirmed === proposal.id} onCheckedChange={checked => setReviewConfirmed(checked ? proposal.id : null)} /><br />
            <Button type="button" className="btn btn-primary" disabled={reviewConfirmed !== proposal.id} onClick={() => mutation(revision => rpc.reviewContractCorrection({ proposal_id: proposal.id, decision: 'approve', expected_revision: revision, approved: true }), 'Correction approved and published. Existing bindings retain their versions.')}>Approve & publish correction</Button>{' '}
            <Button type="button" className="btn" disabled={reviewConfirmed !== proposal.id} onClick={() => mutation(revision => rpc.reviewContractCorrection({ proposal_id: proposal.id, decision: 'reject', expected_revision: revision, approved: true }), 'Correction proposal rejected.')}>Reject correction</Button>
          </>}
        </div>)}
      </div>
    </fieldset>
    {busy && <p role="status">Working…</p>}
  </div>;
}
