import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ui = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const temp = mkdtempSync(join(tmpdir(), 'qinaegis-rpc-'));
execFileSync(process.execPath, [join(ui, 'node_modules/typescript/bin/tsc'), 'src/utils/rpc.ts', 'src/utils/flow_execution_editor.ts', 'src/utils/generation_evidence.ts', '--target', 'ES2020', '--module', 'ES2022', '--moduleResolution', 'bundler', '--outDir', temp], { cwd: ui });
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


test('lifecycle actions preserve explicit approval and typed migration inputs', async () => {
  const { rpc } = await fresh(); const calls = [];
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) return response({ csrf: 'csrf' });
    const body = JSON.parse(init.body); calls.push([body.method, body.params]);
    return response({id: body.id, ok: true, data: {revision: 12}});
  };
  const operations = [
    ['deprecateContract', {contract_id: 'receipt', version: 1, reason: 'Replaced', expected_revision: 11, approved: true}],
    ['deriveContract', {source_id: 'receipt', source_version: 1, new_id: 'receipt-special', reason: 'New scenario', expected_revision: 11, approved: true}],
    ['migrateContractBindings', {contract_id: 'receipt', from_version: 1, to_version: 2, bindings: [{project: 'alpha', parameters: {count: 2}}], reason: 'Reviewed upgrade', expected_revision: 11, approved: true}],
  ];
  for (const [method, params] of operations) await rpc[method](params);
  assert.deepEqual(calls, operations);
});

test('failed execution evidence remains readable and provenance keeps case identity', async () => {
  const { rpc } = await fresh(); const calls = [];
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) return response({csrf: 'csrf'});
    const body = JSON.parse(init.body); calls.push([body.method, body.params]);
    return response({id: body.id, ok: true, data: body.method === 'getCaseProvenance' ? null : {run_id: 'run-1', status: 'failed', error: 'Engine unavailable'}});
  };
  assert.equal(await rpc.getCaseProvenance('alpha', 'receipt'), null);
  assert.equal((await rpc.getRunEvidence('alpha', 'run-1')).error, 'Engine unavailable');
  assert.deepEqual(calls, [['getCaseProvenance', {project: 'alpha', case_id: 'receipt'}], ['getRunEvidence', {project: 'alpha', run_id: 'run-1'}]]);
});


test('Allure generation is an explicit job and saved HTML uses the selected run', async () => {
  const { rpc } = await fresh(); const calls = [];
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) return response({csrf: 'csrf'});
    const body = JSON.parse(init.body); calls.push([body.method, body.params]);
    const data = body.method === 'generateAllureReport' ? {ok: true, jobId: 'allure-1'} : {ok: true, html: '<html>Allure</html>', version: '3'};
    return response({id: body.id, ok: true, data});
  };
  assert.deepEqual(await rpc.generateAllureReport('alpha', 'run-1'), {ok: true, jobId: 'allure-1'});
  assert.equal((await rpc.getAllureReportHtml('alpha', 'run-1')).version, '3');
  assert.deepEqual(calls, [['generateAllureReport', {project: 'alpha', run_id: 'run-1'}], ['getAllureReportHtml', {project: 'alpha', run_id: 'run-1'}]]);
});

test('configuration recovery, browser download and project model changes retain concurrency and consent', async () => {
  const { rpc } = await fresh(); const calls = [];
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) return response({csrf: 'csrf'});
    const body = JSON.parse(init.body); calls.push([body.method, body.params]);
    return response({id: body.id, ok: true, data: {ok: true}});
  };
  await rpc.setConfig({llm: {model: 'reviewed'}}, 7);
  await rpc.restoreConfigBackup(8);
  await rpc.installRuntimeBrowser();
  await rpc.setProjectModelRoute({project: 'alpha', task: 'generate', route: null, expected_revision: 9});
  await rpc.probeModel('vision-local');
  assert.deepEqual(calls, [
    ['setConfig', {config: {llm: {model: 'reviewed'}}, expected_revision: 7}],
    ['restoreConfigBackup', {expected_revision: 8, approved: true}],
    ['installRuntimeBrowser', {approved: true}],
    ['setProjectModelRoute', {project: 'alpha', task: 'generate', route: null, expected_revision: 9}],
    ['probeModel', {profile_id: 'vision-local'}],
  ]);
});

test('interrupted history and incremental log offsets survive the RPC boundary', async () => {
  const { rpc } = await fresh(); const calls = [];
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) return response({csrf: 'csrf'});
    const body = JSON.parse(init.body); calls.push([body.method, body.params]);
    const data = body.method === 'getJobs' ? {jobs: [{jobId: 'old-task', status: 'interrupted'}], history_error: null}
      : {output: 'retained tail', cursor: 70000, truncated: true};
    return response({id: body.id, ok: true, data});
  };
  assert.equal((await rpc.getJobs()).jobs[0].status, 'interrupted');
  assert.deepEqual(await rpc.getJobLog('old-task', 42), {output: 'retained tail', cursor: 70000, truncated: true});
  assert.deepEqual(calls, [['getJobs', {}], ['getJobLog', {jobId: 'old-task', cursor: 42}]]);
});


test('flow execution failures stay readable and rejected admission is never replayed', async () => {
  const {rpc} = await fresh(); const calls = [];
  globalThis.fetch = async (url, init) => {
    if (url.endsWith('/session')) return response({csrf: 'csrf'});
    const body = JSON.parse(init.body); calls.push(body.method);
    if (body.method === 'runFlowExecution') return response({id: body.id, ok: false, error: {message: 'preparation expired'}});
    return response({id: body.id, ok: true, data: {run_id: 'flow-1', status: 'failed', error: 'action failed', facts: {cleanup_error: 'cleanup unconfirmed'}}});
  };
  const record = await rpc.getFlowExecution('flow-1');
  assert.equal(record.error, 'action failed');
  assert.equal(record.facts.cleanup_error, 'cleanup unconfirmed');
  await assert.rejects(rpc.runFlowExecution('expired'), /preparation expired/);
  assert.deepEqual(calls, ['getFlowExecution', 'runFlowExecution']);
});


test('malformed advanced flow JSON cannot become unsafe structured editor children', async () => {
  const helper = join(temp, 'flow-editor.mjs');
  writeFileSync(helper, readFileSync(join(temp, 'utils/flow_execution_editor.js')));
  const {parseExecutionDraft} = await import(pathToFileURL(helper).href);
  const draft = {definition: {schema_version: 2, id: 'flow', title: 'Flow', project: 'alpha', inputs: {}, outputs: {}, steps: [{id: 'case', depends_on: [], operation: {capability: 'case.execute.v1', case: {project: 'alpha', case_id: 'approved', content_revision: 'content:exact', contract: {id: 'receipt', version: 1}}}, condition: null, timeout_ms: 1000, max_attempts: 1}], total_timeout_ms: 5000, cleanup_timeout_ms: 1000, required_permissions: ['browser.read', 'browser.write', 'model.send', 'report.write']}, model_profiles: {case: 'vision'}};
  assert.ok(parseExecutionDraft(JSON.stringify(draft)));
  for (const corrupt of [d => delete d.definition.outputs, d => d.definition.steps[0].operation.case.case_id = {}, d => d.definition.inputs = [], d => d.definition.outputs = {x: {value_type: 'number'}}, d => d.definition.steps[0].operation.case.contract.id = {}, d => d.model_profiles.case = {}, d => d.definition.steps[0].operation = {capability: 'wait.v1', milliseconds: {kind: 'literal', value: {toString: null, valueOf: null}}}]) {
    const bad = structuredClone(draft); corrupt(bad);
    assert.equal(parseExecutionDraft(JSON.stringify(bad)), null);
  }
});


test('cached preparation expiry stays anchored when selected again later', async () => {
  const helper = join(temp, 'flow-editor.mjs');
  writeFileSync(helper, readFileSync(join(temp, 'utils/flow_execution_editor.js')));
  const {preparationDeadline} = await import(pathToFileURL(helper).href);
  const receivedAt = 100000;
  assert.equal(preparationDeadline(receivedAt, 600), 700000);
  const selectedMuchLater = 800000;
  assert.ok(preparationDeadline(receivedAt, 600) < selectedMuchLater);
  assert.equal(preparationDeadline(receivedAt, -1), receivedAt);
});

test('execution polling retries only transient snapshot contention with a finite budget', async () => {
  const helper = join(temp, 'flow-editor.mjs');
  writeFileSync(helper, readFileSync(join(temp, 'utils/flow_execution_editor.js')));
  const {retryExecutionRead} = await import(pathToFileURL(helper).href);
  const busy = new Error('asset library is busy');
  for (let attempt = 0; attempt < 3; attempt++) assert.equal(retryExecutionRead(busy, attempt), true);
  assert.equal(retryExecutionRead(busy, 3), false);
  assert.equal(retryExecutionRead(new Error('session expired'), 0), false);
  assert.equal(retryExecutionRead(new Error('execution not found'), 0), false);
});


test('generation evidence resolves frozen candidate indices and rejects malformed historical attempts', async () => {
  const {generationEvidence} = await import(pathToFileURL(join(temp, 'utils/generation_evidence.js')).href);
  const snapshot = {model:{candidates:[{id:'primary',model:'frozen-one'},{id:'fallback',model:'frozen-two'}]}};
  const value = {schema_version:1,source:'registered_route',application_outcome:'failed',attempts:[[1,1,0,'rate_limited'],[1,2,1,'succeeded'],[2,1,0,'rejected']]};
  const read = generationEvidence(value,snapshot);
  assert.equal(read.rows[1].model,'frozen-two');
  assert.equal(read.rows[1].candidate,'fallback');
  assert.equal(read.rows[2].phase,'Critic 1');
  assert.match(read.status,/failed/);
  assert.match(generationEvidence({...value,application_outcome:'completed_with_unverified_reviews'},snapshot).status,/could not be verified/);
  assert.match(generationEvidence({...value,application_outcome:'cancelled',attempts:[[1,1,0,'cancelled']]},snapshot).rows[0].outcome,/cancelled/);
  for (const row of [[1,1,2,'succeeded'],[1,2,1,'succeeded'],[1,1,0,'unknown'],[1,1,0,{toString:null}],[202,1,0,'started']]) {
    assert.equal(generationEvidence({...value,attempts:[row]},snapshot),null);
  }
  assert.equal(generationEvidence({...value,application_outcome:'toString'},snapshot),null);
  assert.equal(generationEvidence({...value,attempts:[[1,1,0,'toString']]},snapshot),null);
  assert.equal(generationEvidence(value,{model:{candidates:[{id:'primary',model:null}]}}),null);
  assert.equal(generationEvidence({truncated:true},snapshot),null);
  assert.equal(generationEvidence({schema_version:1,source:'legacy_config',actual_attempts:'not_recorded'},null).legacy,true);
});


test('legacy generation evidence displays frozen actual attempts and preserves unrecorded history', async () => {
  const {generationEvidence} = await import(pathToFileURL(join(temp, 'utils/generation_evidence.js')).href);
  const snapshot = {model:{source:'legacy_config',model:'original',candidates:[
    {id:'legacy-primary',adapter:'openai_compatible',model:'original'},
    {id:'legacy-secondary',adapter:'openai_compatible',model:'backup'},
  ]}};
  const value = {schema_version:1,source:'legacy_config',application_outcome:'completed',attempts:[
    [1,1,0,'service_unavailable'],[1,2,1,'succeeded'],[2,1,0,'succeeded'],
  ]};
  const read = generationEvidence(value, snapshot);
  assert.equal(read.legacy,false);
  assert.equal(read.rows[1].candidate,'legacy-secondary');
  assert.equal(read.rows[1].model,'backup');
  assert.match(read.rows[0].outcome,/service unavailable/);
  assert.match(read.status,/completed/);
  assert.equal(generationEvidence(value,{model:{candidates:snapshot.model.candidates}}),null);
  assert.equal(generationEvidence({...value,source:'registered_route'},snapshot),null);
  assert.equal(generationEvidence({...value,source:{toString:null}},snapshot),null);
  assert.equal(generationEvidence({schema_version:1,source:'legacy_config',actual_attempts:'not_recorded'},null).legacy,true);
});
