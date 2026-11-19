import JobProgress from './JobProgress';
import { Button, Checkbox, Input, Select } from './ui';
import { useEffect, useRef, useState } from 'react';
import { rpc } from '../utils/rpc';
import type { ModelProfile, ModelRegistry } from '../types/models';

const emptyProfile = (): ModelProfile => ({id: '', label: '', adapter: 'openai_compatible', base_url: '', model: '', api_key: '', capabilities: ['text'], enabled: true, allow_remote: false, midscene_family: null});
const tasks = ['generate', 'explore', 'execute', 'correct'];

export default function ModelsView({ onSaved, projects = [] }: { onSaved?: () => void; projects?: string[] }) {
  const [registry, setRegistry] = useState<ModelRegistry | null>(null);
  const [profile, setProfile] = useState<ModelProfile>(emptyProfile);
  const [clearKey, setClearKey] = useState(false);
  const [project, setProject] = useState('');
  const [probeJob, setProbeJob] = useState<string | null>(null);
  const [probeResult, setProbeResult] = useState<unknown>(null);
  const [probing, setProbing] = useState(false);
  const [task, setTask] = useState('generate');
  const [primary, setPrimary] = useState('');
  const [fallbacks, setFallbacks] = useState('');
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const mounted = useRef(false);
  const sequence = useRef(0);
  const routeInitialized = useRef(false);
  const anthropic = profile.adapter === 'anthropic_messages';
  const editingBlocked = busy || probing;
  const [probeProfile, setProbeProfile] = useState('');
  const visualTask = task === 'explore' || task === 'execute';
  useEffect(() => { mounted.current = true; void load(); return () => { mounted.current = false; sequence.current++; lock.current = false; }; }, []);
  async function load() {
    if (lock.current || probing) return;
    lock.current = true;
    setBusy(true);
    const request = ++sequence.current;
    try { const data = await rpc.getModels(); if (mounted.current && request === sequence.current) {
      setRegistry(data); setError('');
      if (!routeInitialized.current) {
        const route = data.routes[task];
        setPrimary(route?.primary || ''); setFallbacks(route?.fallbacks.join(', ') || '');
        routeInitialized.current = true;
      }
    } }
    catch (e) { if (mounted.current && request === sequence.current) setError((e as Error).message); }
    finally { if (mounted.current && request === sequence.current) { lock.current = false; setBusy(false); } }
  }
  async function act(work: () => Promise<ModelRegistry>, success: string, savedProfileId?: string) {
    if (lock.current || probing) return;
    lock.current = true; setBusy(true); setError(''); setMessage('');
    const request = ++sequence.current;
    try { const data = await work(); if (mounted.current && request === sequence.current) {
      setRegistry(data); setMessage(success);
      if (savedProfileId !== undefined) {
        const saved = data.profiles.find(item => item.id === savedProfileId);
        if (saved) setProfile({...saved, api_key: ''});
        setClearKey(false);
      }
      onSaved?.();
    } }
    catch (e) { if (mounted.current && request === sequence.current) setError(`${(e as Error).message}. Refresh the registry before retrying a revision conflict; your form is preserved.`); }
    finally { if (mounted.current && request === sequence.current) { lock.current = false; setBusy(false); } }
  }
  function selectTask(value: string, scope = project) { setTask(value); const route = (scope ? registry?.project_routes?.[scope]?.[value] : registry?.routes[value]) || registry?.routes[value]; setPrimary(route?.primary || ''); setFallbacks(route?.fallbacks.join(', ') || ''); }
  async function probeSaved() {
    if (lock.current || probing || !registry?.profiles.some(p => p.id === profile.id)) return;
    lock.current = true; setProbing(true); setError(''); setProbeResult(null); setProbeJob(null);
    const saved = registry.profiles.find(item => item.id === profile.id)!;
    setProbeProfile(`${saved.id} · ${saved.adapter} · ${saved.model}`);
    const request = ++sequence.current;
    try { const result = await rpc.probeModel(profile.id);
      if (mounted.current && request === sequence.current) setProbeJob(result.jobId);
    } catch (e) { if (mounted.current && request === sequence.current) { setError((e as Error).message); setProbing(false); } }
    finally { if (mounted.current && request === sequence.current) lock.current = false; }
  }
  function chooseProfile(value: ModelProfile) {
    setProfile({...value, api_key: ''}); setClearKey(false);
    setProbeJob(null); setProbeResult(null); setProbeProfile('');
  }
  function chooseAdapter(adapter: string) {
    if (adapter === profile.adapter) return;
    setProfile({...profile, adapter, api_key: '', api_key_configured: false,
      capabilities: adapter === 'anthropic_messages' ? profile.capabilities.filter(cap => cap === 'text' || cap === 'json') : profile.capabilities,
      midscene_family: adapter === 'anthropic_messages' ? null : profile.midscene_family});
    setClearKey(false); setProbeJob(null); setProbeResult(null); setProbeProfile('');
    setMessage('Protocol changed. Enter an API key explicitly before saving; the endpoint and model name are preserved.');
  }
  return <div className="view">
    <h2>Models and routing</h2>
    <p>Register compatible endpoints, then choose a model for each role. Running tasks keep their original selection. Remote processing requires explicit authorization per profile.</p>
    {error && <div className="card" role="alert">{error}</div>}
    {message && <p role="status">{message}</p>}
    <Button type="button" className="btn" disabled={editingBlocked} onClick={() => void load()}>Refresh registry</Button>
    <p>Revision: {registry?.revision ?? 'loading'}</p>
    <section className="card">
      <h3>Model profiles</h3>
      {registry?.profiles.map(p => <Button type="button" className="btn" key={p.id} disabled={editingBlocked} onClick={() => {chooseProfile(p);}}>{p.label} · {p.model}{p.enabled ? '' : ' (disabled)'}</Button>)}
      <Button type="button" className="btn" disabled={editingBlocked} onClick={() => {chooseProfile(emptyProfile());}}>New profile</Button>
      <form onSubmit={e => { e.preventDefault(); if (registry) void act(() => rpc.saveModel({profile, expected_revision: registry.revision, clear_api_key: clearKey}), 'Profile saved.', profile.id); }}>
        <fieldset disabled={editingBlocked || !registry} style={{ border: 0, padding: 0 }}>
        <div className="form-group"><label htmlFor="model-id">Profile ID</label><Input id="model-id" required value={profile.id} onValueChange={value => setProfile({...profile, id: value})} /></div>
        <div className="form-group"><label htmlFor="model-label">Label</label><Input id="model-label" required value={profile.label} onValueChange={value => setProfile({...profile, label: value})} /></div>
        <div className="form-group"><label htmlFor="model-adapter">Protocol adapter</label><Select id="model-adapter" value={profile.adapter} onValueChange={chooseAdapter}><option value="openai_compatible">OpenAI compatible Chat Completions</option><option value="anthropic_messages">Anthropic Messages</option></Select></div>
        {anthropic && <p>Messages supports text and structured JSON for generation and correction. Visual tasks and tools are unavailable. Use a base URL ending in /v1; requests append /messages. Protocol version: 2026-10-21.</p>}
        <div className="form-group"><label htmlFor="model-url">Base URL</label><Input id="model-url" required type="url" placeholder={anthropic ? 'https://api.anthropic.com/v1' : 'https://provider.example/v1'} value={profile.base_url} onValueChange={value => setProfile({...profile, base_url: value})} /></div>
        <div className="form-group"><label htmlFor="model-name">Model name</label><Input id="model-name" required value={profile.model} onValueChange={value => setProfile({...profile, model: value})} /></div>
        <div className="form-group"><label htmlFor="model-key">API key {profile.api_key_configured ? '(saved; blank keeps it)' : ''}</label><Input id="model-key" type="password" autoComplete="new-password" disabled={clearKey} value={profile.api_key} onValueChange={value => setProfile({...profile, api_key: value})} /></div>
        <Checkbox label={"Clear saved key"} checked={clearKey} onCheckedChange={checked => {setClearKey(checked); setProfile({...profile, api_key: ''});}} />
        <fieldset className="my-3 flex flex-wrap gap-x-4 gap-y-2 rounded-xl border border-border p-4"><legend className="px-1">Declared capabilities</legend>{['text', 'vision', 'json', 'tools'].map(cap => <Checkbox key={cap} label={cap} disabled={anthropic && (cap === 'vision' || cap === 'tools')} checked={profile.capabilities.includes(cap)} onCheckedChange={checked => setProfile({...profile, capabilities: checked ? [...profile.capabilities, cap] : profile.capabilities.filter(c => c !== cap)})} />)}</fieldset>
        <div className="form-group"><label htmlFor="model-family">Midscene model family (required for visual tasks)</label><Input id="model-family" disabled={anthropic} placeholder="Use a family supported by your installed engine" value={profile.midscene_family || ''} onValueChange={value => setProfile({...profile, midscene_family: value || null})} /></div>
        <p><Checkbox label={"Enabled"} checked={profile.enabled} onCheckedChange={checked => setProfile({...profile, enabled: checked})} /></p>
        <p><Checkbox label={"Authorize this profile to process task data remotely, including when used as fallback"} checked={profile.allow_remote} onCheckedChange={checked => setProfile({...profile, allow_remote: checked})} /></p>
        <Button type="submit" className="btn btn-primary" disabled={editingBlocked || !registry}>Save profile</Button>
        </fieldset>
      </form>
      <Button type="button" className="btn" disabled={editingBlocked || !registry?.profiles.some(p => p.id === profile.id)} onClick={() => void probeSaved()}>Test saved profile</Button>
      <p>This sends only synthetic text to the saved endpoint. Unsaved form edits are excluded. Text/JSON observations do not certify vision, tools or Midscene compatibility.</p>
      <p aria-live="polite">{probeProfile && `Saved profile tested: ${probeProfile}`}</p>
      <JobProgress jobId={probeJob} onComplete={result => { setProbing(false); setProbeResult(result); }} onError={error => {setProbing(false); setError(error);}} />
      {probeResult !== null && <pre aria-label="Model probe observations">{JSON.stringify(probeResult, null, 2)}</pre>}
    </section>
    <section className="card">
      <h3>Task routes</h3>
      <div className="form-group"><label htmlFor="model-project">Route scope</label><Select id="model-project" value={project} disabled={editingBlocked} onValueChange={value => {setProject(value); selectTask(task, value);}}><option value="">Global defaults</option>{projects.map(p => <option key={p} value={p}>{p}</option>)}</Select></div>
      {project && <p>Project overrides take priority. Without an override this task uses the global default; running tasks keep their frozen route.</p>}
      <ul>{Object.entries((project ? registry?.project_routes?.[project] : registry?.routes) || {}).map(([name, route]) => <li key={name}>{name}: {route.primary}{route.fallbacks.length ? ` → ${route.fallbacks.join(' → ')}` : ''}</li>)}</ul>
      <form onSubmit={e => {
        e.preventDefault();
        if (!registry || editingBlocked) return;
        if (visualTask && registry.profiles.find(p => p.id === primary)?.adapter !== 'openai_compatible') { setError('Visual tasks require an OpenAI compatible profile.'); return; }
        const route = {primary, fallbacks: visualTask ? [] : fallbacks.split(',').map(s => s.trim()).filter(Boolean)};
        const params = {task, route, expected_revision: registry.revision};
        void act(() => project ? rpc.setProjectModelRoute({...params, project}) : rpc.setModelRoute(params), 'Route saved.');
      }}>
        <fieldset disabled={editingBlocked || !registry} style={{ border: 0, padding: 0 }}>
        <div className="form-group"><label htmlFor="model-task">Task role</label><Select id="model-task" value={task} onValueChange={value => selectTask(value)}>{tasks.map(t => <option key={t}>{t}</option>)}</Select></div>
        <div className="form-group"><label htmlFor="model-primary">Primary profile</label><Select id="model-primary" required value={primary} onValueChange={value => setPrimary(value)}><option value="">Select profile</option>{registry?.profiles.map(p => <option key={p.id} value={p.id} disabled={visualTask && (p.adapter !== 'openai_compatible' || !p.enabled || !p.capabilities.includes('vision') || !p.midscene_family)}>{p.label}{visualTask && p.adapter !== 'openai_compatible' ? ' (text protocol only)' : ''}</option>)}</Select></div>
        <div className="form-group"><label htmlFor="model-fallback">Fallback profile IDs, in order (comma separated)</label><Input id="model-fallback" disabled={visualTask} value={visualTask ? '' : fallbacks} onValueChange={value => setFallbacks(value)} /></div>
        {visualTask && <p>Visual explore and execute routes support one primary model only. Automatic fallback is unavailable; the profile must use the OpenAI compatible protocol and declare text and vision capabilities and an engine-supported Midscene family.</p>}
        <Button type="submit" className="btn btn-primary" disabled={editingBlocked || !registry}>Save task route</Button>
        {project && <Button type="button" className="btn" disabled={editingBlocked || !registry?.project_routes?.[project]?.[task]} onClick={() => {if (registry) void act(() => rpc.setProjectModelRoute({project, task, route: null, expected_revision: registry.revision}), 'Override removed; global route will be used.');}}>Use global default</Button>}
        </fieldset>
      </form>
    </section>
  </div>;
}
