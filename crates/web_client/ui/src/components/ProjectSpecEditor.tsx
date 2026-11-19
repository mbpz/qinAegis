import { useEffect, useRef, useState } from 'react';
import { Button, Checkbox, TextArea } from './ui';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from './motion/collapsible';
import { rpc, type ProjectSpec } from '../utils/rpc';

interface ProjectSpecEditorProps {
  project: string;
  disabled: boolean;
  onRevision: (revision: string | null) => void;
}

export default function ProjectSpecEditor({ project, disabled, onRevision }: ProjectSpecEditorProps) {
  const [saved, setSaved] = useState<ProjectSpec | null>(null);
  const [draft, setDraft] = useState('');
  const [confirmed, setConfirmed] = useState(false);
  const [useSaved, setUseSaved] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [reload, setReload] = useState(0);
  const mounted = useRef(false);
  const projectRef = useRef(project);
  const requestSequence = useRef(0);
  const saving = useRef(false);
  projectRef.current = project;

  useEffect(() => {
    mounted.current = true;
    saving.current = false;
    const request = ++requestSequence.current;
    const active = () => mounted.current && projectRef.current === project && requestSequence.current === request;
    setSaved(null);
    setDraft('');
    setConfirmed(false);
    setUseSaved(false);
    setError('');
    onRevision(null);
    setLoading(false);
    if (!project) return () => { mounted.current = false; requestSequence.current++; };
    setLoading(true);
    rpc.getProjectSpec(project).then(spec => {
      if (!active()) return;
      if (spec.project !== project) throw new Error('Specification belongs to another project');
      setSaved(spec);
      setDraft(spec.markdown);
    }).catch(reason => {
      if (active()) setError(String(reason));
    }).finally(() => {
      if (active()) setLoading(false);
    });
    return () => {
      mounted.current = false;
      requestSequence.current++;
      saving.current = false;
    };
  }, [project, reload]);

  const current = saved?.project === project ? saved : null;
  const dirty = !!current && draft !== current.markdown;

  async function save() {
    if (!current || saving.current || !mounted.current || projectRef.current !== project) return;
    saving.current = true;
    const request = ++requestSequence.current;
    const active = () => mounted.current && projectRef.current === project && requestSequence.current === request;
    setLoading(true);
    setError('');
    try {
      const result = await rpc.saveProjectSpec(project, current.revision, draft);
      if (!active()) return;
      if (result.project !== project) throw new Error('Specification belongs to another project');
      setSaved(result);
      setConfirmed(false);
      setUseSaved(false);
      onRevision(null);
    } catch (reason) {
      if (active()) {
        setError(String(reason));
        setConfirmed(false);
      }
    } finally {
      if (active()) {
        saving.current = false;
        setLoading(false);
      }
    }
  }

  return <section className="card">
    <h3>Saved project specification</h3>
    <p>One managed spec.md per project, up to 128 KiB. Unsaved text never participates in generation. Reload discards this editor draft.</p>
    {loading && <p role="status">Loading specification…</p>}
    {error && <p role="alert">{error} Your draft is retained until you explicitly reload.</p>}
    <Button disabled={!project || disabled || loading} onClick={() => setReload(value => value + 1)}>
      Reload saved specification
    </Button>
    {current && <>
      <p>{current.exists ? 'Existing specification' : 'No saved specification. Create one below.'}</p>
      <Collapsible>
        <CollapsibleTrigger>Original saved specification</CollapsibleTrigger>
        <CollapsibleContent>
          <pre style={{ whiteSpace: 'pre-wrap' }}>{current.markdown || '(empty)'}</pre>
        </CollapsibleContent>
      </Collapsible>
      <label htmlFor="project-spec-markdown">Specification Markdown</label>
      <TextArea id="project-spec-markdown" rows={8} value={draft} disabled={disabled || loading} onValueChange={value => {
        setDraft(value);
        setConfirmed(false);
        setUseSaved(false);
        onRevision(null);
      }} />
      <Checkbox label="I reviewed this specification and approve saving it" checked={confirmed}
        disabled={disabled || loading} onCheckedChange={setConfirmed} />
      <Button disabled={disabled || loading || !confirmed || !draft.trim() || new TextEncoder().encode(draft).length > 131072} onClick={save}>
        {current.exists ? 'Save specification' : 'Create specification'}
      </Button>
      <Checkbox label="Use this saved specification for generation" checked={useSaved}
        disabled={disabled || loading || dirty || !current.exists || !current.markdown.trim()} onCheckedChange={checked => {
          setUseSaved(checked);
          onRevision(checked ? current.revision : null);
        }} />
      {dirty && <p>Save or reload this draft before selecting the saved specification.</p>}
    </>}
  </section>;
}
