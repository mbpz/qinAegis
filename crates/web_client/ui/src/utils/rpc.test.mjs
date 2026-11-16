import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ui = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const temp = mkdtempSync(join(tmpdir(), 'qinaegis-rpc-'));
execFileSync(process.execPath, [join(ui, 'node_modules/typescript/bin/tsc'), 'src/utils/rpc.ts', '--target', 'ES2020', '--module', 'ES2022', '--moduleResolution', 'bundler', '--outDir', temp], { cwd: ui });
const compiled = join(temp, 'rpc.mjs');
writeFileSync(compiled, readFileSync(join(temp, 'utils/rpc.js')));
process.on('exit', () => rmSync(temp, { recursive: true, force: true }));
let version = 0;
async function fresh(hash = '') {
  const history = [];
  globalThis.window = { location: { hash, pathname: '/', search: '' }, history: { replaceState: (...args) => {
    history.push(args);
    globalThis.window.location.hash = '';
  } } };
  const module = await import(pathToFileURL(compiled).href + '?v=' + ++version);
  return { ...module, history };
}
const response = (body, ok = true, status = 200) => ({ ok, status, json: async () => body });

test('bootstrap token is cleared and passed only in POST body; concurrent calls share initialization', async () => {
  const { rpc, history } = await fresh('#token=secret');
  const requests = [];
  globalThis.fetch = async (url, init) => {
    requests.push([url, init]);
    if (url.endsWith('/session')) return response({ csrf: 'csrf' });
    const { id, method } = JSON.parse(init.body);
    return response({ id, ok: true, data: method === 'getVersion' ? { version: '2' } : { configured: true } });
  };
  await Promise.all([rpc.getVersion(), rpc.checkConfig()]);
  assert.equal(history.length, 1);
  assert.equal(requests.filter(([url]) => url.endsWith('/session')).length, 1);
  assert.equal(requests[0][1].method, 'POST');
  assert.deepEqual(JSON.parse(requests[0][1].body), { token: 'secret' });
  for (const [url, init] of requests.slice(1)) {
    assert.equal(url, '/api/v2/rpc');
    assert.equal(init.method, 'POST');
    assert.equal(init.headers['X-Qinaegis-CSRF'], 'csrf');
    assert.equal(init.credentials, 'same-origin');
  }
});

test('refresh uses cookie session and failed initialization can retry', async () => {
  const { rpc } = await fresh();
  let attempts = 0;
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) {
      assert.equal(init.method, 'GET');
      if (++attempts === 1) throw new Error('offline');
      return response({ csrf: 'csrf' });
    }
    return response({ id: JSON.parse(init.body).id, ok: true, data: { version: '2' } });
  };
  await assert.rejects(rpc.getVersion(), /offline/);
  assert.deepEqual(await rpc.getVersion(), { version: '2' });
  assert.equal(attempts, 2);
});

test('protocol and nested business failures reject instead of presenting success', async () => {
  const { rpc } = await fresh();
  let mode = 'id';
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) return response({ csrf: 'csrf' });
    const id = JSON.parse(init.body).id;
    if (mode === 'id') return response({ id: 'wrong', ok: true, data: {} });
    if (mode === 'business') return response({ id, ok: true, data: { error: 'missing project' } });
    return response({ id, ok: false, error: { code: 'INVALID_PARAMS', message: 'invalid request' } });
  };
  await assert.rejects(rpc.getVersion(), /Response ID mismatch/);
  mode = 'business';
  await assert.rejects(rpc.getVersion(), /missing project/);
  mode = 'envelope';
  await assert.rejects(rpc.getVersion(), error => error.code === 'INVALID_PARAMS');
});

test('RPC 401 invalidates stale CSRF without replaying requests; other HTTP errors keep the session', async () => {
  const { rpc } = await fresh();
  let sessions = 0;
  let calls = 0;
  const tokens = [];
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) {
      assert.equal(init.method, 'GET');
      return response({ csrf: `csrf-${++sessions}` });
    }
    tokens.push(init.headers['X-Qinaegis-CSRF']);
    calls++;
    if (calls === 1) return response({ error: 'temporarily unavailable' }, false, 503);
    if (calls === 2) return response({ error: 'stale session' }, false, 401);
    return response({ id: JSON.parse(init.body).id, ok: true, data: { version: '2' } });
  };
  await assert.rejects(rpc.getVersion(), /temporarily unavailable/);
  assert.equal(sessions, 1);
  await assert.rejects(rpc.getVersion(), /session expired/);
  assert.equal(calls, 2, 'a rejected request must not be automatically replayed');
  assert.equal(sessions, 1, 'non-401 HTTP error must keep cached CSRF');
  assert.deepEqual(await rpc.getVersion(), { version: '2' });
  assert.equal(sessions, 2);
  assert.deepEqual(tokens, ['csrf-1', 'csrf-1', 'csrf-2']);
});

test('lost bootstrap response retries its token once then recovers via cookie after explicit 401', async () => {
  const { rpc, history } = await fresh('#token=consumed');
  const methods = [];
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) {
      methods.push(init.method);
      if (methods.length === 1) throw new Error('response lost after cookie was set');
      if (methods.length === 2) {
        assert.deepEqual(JSON.parse(init.body), { token: 'consumed' });
        return response({ error: 'already consumed' }, false, 401);
      }
      assert.equal(init.body, undefined);
      return response({ csrf: 'cookie-csrf' });
    }
    assert.equal(init.headers['X-Qinaegis-CSRF'], 'cookie-csrf');
    return response({ id: JSON.parse(init.body).id, ok: true, data: { version: '2' } });
  };
  await assert.rejects(rpc.getVersion(), /response lost/);
  await assert.rejects(rpc.getVersion(), /session expired/);
  assert.deepEqual(await rpc.getVersion(), { version: '2' });
  assert.deepEqual(methods, ['POST', 'POST', 'GET']);
  assert.equal(history.length, 2);
});

test('contract governance methods preserve revision, explicit approval, and structured parameters', async () => {
  const { rpc } = await fresh();
  const requests = [];
  const snapshot = { schema_version: 1, revision: 8, drafts: [], publications: [], proposals: [], bindings: [] };
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) return response({ csrf: 'csrf' });
    const body = JSON.parse(init.body);
    requests.push(body);
    return response({ id: body.id, ok: true, data: snapshot });
  };
  const draft = { id: 'submit', version: 2, intent: 'Receipt', inputs: { count: 'number' }, outputs: {}, preconditions: [], assertions: ['Receipt exists'], required_capabilities: [] };
  const bundle = { schema_version: 1, contracts: [] };
  const operations = [
    ['getContracts', undefined],
    ['saveContractDraft', { draft, expected_revision: 7 }],
    ['publishContract', { id: 'submit', expected_revision: 7, approved: true }],
    ['proposeContractCorrection', { contract_id: 'submit', version: 1, replacement: draft, reason: 'Correct assertion', evidence: ['run/observation'], expected_revision: 7 }],
    ['reviewContractCorrection', { proposal_id: 'proposal-1', decision: 'approve', expected_revision: 7, approved: true }],
    ['bindContract', { project: 'project-a', contract_id: 'submit', version: 2, parameters: { count: 3 }, expected_revision: 7 }],
    ['exportContracts', undefined],
    ['importContracts', { bundle, expected_revision: 7, approved: true }],
  ];
  for (const [method, params] of operations) {
    assert.deepEqual(await rpc[method](params), snapshot);
  }
  assert.deepEqual(requests.map(({ method, params }) => [method, params]), operations.map(([method, params]) => [method, params ?? {}]));
});

test('generation pins its project and explicit contract choice; preview uses the same project', async () => {
  const { rpc } = await fresh();
  const requests = [];
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) return response({ csrf: 'csrf' });
    const body = JSON.parse(init.body);
    requests.push([body.method, body.params]);
    return response({ id: body.id, ok: true, data: {} });
  };
  await rpc.getContractExecution('project-a');
  await rpc.runGenerate('Receipt is visible', 'project-a', true);
  await rpc.runGenerate('Receipt is visible', 'project-a', false);
  assert.deepEqual(requests, [
    ['getContractExecution', { project: 'project-a' }],
    ['runGenerate', { requirement: 'Receipt is visible', project: 'project-a', use_contracts: true }],
    ['runGenerate', { requirement: 'Receipt is visible', project: 'project-a', use_contracts: false }],
  ]);
});


test('terminal job failures are pollable status data rather than transport failures', async () => {
  const { rpc } = await fresh();
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) return response({ csrf: 'csrf' });
    return response({ id: JSON.parse(init.body).id, ok: true, data: {jobId: 'job-1', status: 'failed', output: '', error: 'model unavailable'} });
  };
  const status = await rpc.getJobStatus('job-1');
  assert.equal(status.status, 'failed');
  assert.equal(status.error, 'model unavailable');
});

test('project wire responses contain names for listing and metadata for creation', async () => {
  const { rpc } = await fresh();
  const project = { name: 'alpha', url: 'https://alpha.invalid', tech_stack: [], created_at: '2026-11-16T00:00:00Z' };
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) return response({ csrf: 'csrf' });
    const body = JSON.parse(init.body);
    return response({ id: body.id, ok: true, data: body.method === 'getProjects' ? ['alpha'] : project });
  };
  assert.deepEqual(await rpc.getProjects(), ['alpha']);
  assert.deepEqual(await rpc.createProject(project.name, project.url, []), project);
});
