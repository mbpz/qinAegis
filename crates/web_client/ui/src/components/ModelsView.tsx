import { Button, Checkbox, Input, Select } from './ui';
import { useEffect, useRef, useState } from 'react';
import { rpc } from '../utils/rpc';
import type { ModelProfile, ModelRegistry } from '../types/models';

const emptyProfile = (): ModelProfile => ({id: '', label: '', adapter: 'openai_compatible', base_url: '', model: '', api_key: '', capabilities: ['text'], enabled: true, allow_remote: false, midscene_family: null});
const tasks = ['generate', 'explore', 'execute', 'correct'];

export default function ModelsView({ onSaved }: { onSaved?: () => void }) {
  const [registry, setRegistry] = useState<ModelRegistry | null>(null);
  const [profile, setProfile] = useState<ModelProfile>(emptyProfile);
  const [clearKey, setClearKey] = useState(false);
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
  const visualTask = task === 'explore' || task === 'execute';
  useEffect(() => { mounted.current = true; void load(); return () => { mounted.current = false; sequence.current++; lock.current = false; }; }, []);
  async function load() {
    if (lock.current) return;
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
    if (lock.current) return;
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
  function selectTask(value: string) { setTask(value); const route = registry?.routes[value]; setPrimary(route?.primary || ''); setFallbacks(route?.fallbacks.join(', ') || ''); }
  return <div className="view">
    <h2>Models and routing</h2>
    <p>Register compatible endpoints, then choose a model for each role. Running tasks keep their original selection. Remote processing requires explicit authorization per profile.</p>
    {error && <div className="card" role="alert">{error}</div>}
    {message && <p role="status">{message}</p>}
    <Button type="button" className="btn" disabled={busy} onClick={() => void load()}>Refresh registry</Button>
    <p>Revision: {registry?.revision ?? 'loading'}</p>
    <section className="card">
      <h3>Model profiles</h3>
      {registry?.profiles.map(p => <Button type="button" className="btn" key={p.id} disabled={busy} onClick={() => {setProfile({...p, api_key: ''}); setClearKey(false);}}>{p.label} · {p.model}{p.enabled ? '' : ' (disabled)'}</Button>)}
      <Button type="button" className="btn" disabled={busy} onClick={() => {setProfile(emptyProfile()); setClearKey(false);}}>New profile</Button>
      <form onSubmit={e => { e.preventDefault(); if (registry) void act(() => rpc.saveModel({profile, expected_revision: registry.revision, clear_api_key: clearKey}), 'Profile saved.', profile.id); }}>
        <fieldset disabled={busy || !registry} style={{ border: 0, padding: 0 }}>
        <div className="form-group"><label htmlFor="model-id">Profile ID</label><Input id="model-id" required value={profile.id} onValueChange={value => setProfile({...profile, id: value})} /></div>
        <div className="form-group"><label htmlFor="model-label">Label</label><Input id="model-label" required value={profile.label} onValueChange={value => setProfile({...profile, label: value})} /></div>
        <p>Adapter: OpenAI compatible Chat Completions</p>
        <div className="form-group"><label htmlFor="model-url">Base URL</label><Input id="model-url" required type="url" placeholder="https://provider.example/v1" value={profile.base_url} onValueChange={value => setProfile({...profile, base_url: value})} /></div>
        <div className="form-group"><label htmlFor="model-name">Model name</label><Input id="model-name" required value={profile.model} onValueChange={value => setProfile({...profile, model: value})} /></div>
        <div className="form-group"><label htmlFor="model-key">API key {profile.api_key_configured ? '(saved; blank keeps it)' : ''}</label><Input id="model-key" type="password" autoComplete="new-password" disabled={clearKey} value={profile.api_key} onValueChange={value => setProfile({...profile, api_key: value})} /></div>
        <Checkbox label={"Clear saved key"} checked={clearKey} onCheckedChange={checked => {setClearKey(checked); setProfile({...profile, api_key: ''});}} />
        <fieldset className="my-3 flex flex-wrap gap-x-4 gap-y-2 rounded-xl border border-border p-4"><legend className="px-1">Declared capabilities</legend>{['text', 'vision', 'json', 'tools'].map(cap => <Checkbox key={cap} label={cap} checked={profile.capabilities.includes(cap)} onCheckedChange={checked => setProfile({...profile, capabilities: checked ? [...profile.capabilities, cap] : profile.capabilities.filter(c => c !== cap)})} />)}</fieldset>
        <div className="form-group"><label htmlFor="model-family">Midscene model family (required for visual tasks)</label><Input id="model-family" placeholder="Use a family supported by your installed engine" value={profile.midscene_family || ''} onValueChange={value => setProfile({...profile, midscene_family: value || null})} /></div>
        <p><Checkbox label={"Enabled"} checked={profile.enabled} onCheckedChange={checked => setProfile({...profile, enabled: checked})} /></p>
        <p><Checkbox label={"Authorize this profile to process task data remotely, including when used as fallback"} checked={profile.allow_remote} onCheckedChange={checked => setProfile({...profile, allow_remote: checked})} /></p>
        <Button type="submit" className="btn btn-primary" disabled={busy || !registry}>Save profile</Button>
        </fieldset>
      </form>
    </section>
    <section className="card">
      <h3>Task routes</h3>
      <ul>{Object.entries(registry?.routes || {}).map(([name, route]) => <li key={name}>{name}: {route.primary}{route.fallbacks.length ? ` → ${route.fallbacks.join(' → ')}` : ''}</li>)}</ul>
      <form onSubmit={e => {e.preventDefault(); if (registry) void act(() => rpc.setModelRoute({task, route: {primary, fallbacks: visualTask ? [] : fallbacks.split(',').map(s => s.trim()).filter(Boolean)}, expected_revision: registry.revision}), 'Route saved.');}}>
        <fieldset disabled={busy || !registry} style={{ border: 0, padding: 0 }}>
        <div className="form-group"><label htmlFor="model-task">Task role</label><Select id="model-task" value={task} onValueChange={value => selectTask(value)}>{tasks.map(t => <option key={t}>{t}</option>)}</Select></div>
        <div className="form-group"><label htmlFor="model-primary">Primary profile</label><Select id="model-primary" required value={primary} onValueChange={value => setPrimary(value)}><option value="">Select profile</option>{registry?.profiles.map(p => <option key={p.id} value={p.id}>{p.label}</option>)}</Select></div>
        <div className="form-group"><label htmlFor="model-fallback">Fallback profile IDs, in order (comma separated)</label><Input id="model-fallback" disabled={visualTask} value={visualTask ? '' : fallbacks} onValueChange={value => setFallbacks(value)} /></div>
        {visualTask && <p>Visual explore and execute routes support one primary model only. Automatic fallback is unavailable; the profile must declare text and vision capabilities and an engine-supported Midscene family.</p>}
        <Button type="submit" className="btn btn-primary" disabled={busy || !registry}>Save task route</Button>
        </fieldset>
      </form>
    </section>
  </div>;
}
