import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const ui = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const temp = mkdtempSync(join(tmpdir(), 'qinaegis-correction-ui-'));
execFileSync(process.execPath, [join(ui, 'node_modules/typescript/bin/tsc'), 'src/utils/contract_correction.ts', 'src/utils/rpc.ts', '--target', 'ES2020', '--module', 'ES2022', '--moduleResolution', 'bundler', '--outDir', temp], {cwd: ui});
process.on('exit', () => rmSync(temp, {recursive: true, force: true}));
for (const name of ['contract_correction', 'rpc']) writeFileSync(join(temp, `${name}.mjs`), readFileSync(join(temp, `utils/${name}.js`)));
const {correctionInstruction, correctionCases, correctionAttempts, correctionJobEvidence} = await import(pathToFileURL(join(temp, 'contract_correction.mjs')));
const publication = {definition: {id: 'ready', version: 1, intent: 'Ready', inputs: {}, outputs: {}, preconditions: [], assertions: ['Visible'], required_capabilities: ['browser.assert']}, reviewer: 'local-owner', correction: null};
function evidence() {
  const source = {binding: {project: 'alpha', contract_id: 'ready', contract_version: 1, parameters: {}}, publication: structuredClone(publication), original_assertions: ['Visible']};
  return {project: 'alpha', status: 'failed', finished_at: '2026-11-18', cases: [{case_id: 'case', yaml_script: 'tasks: []', provenance: {project: 'alpha', case_id: 'case', yaml_script: 'tasks: []', contracts: {project: 'alpha', contracts: [source]}}}]};
}
test('instructions use UTF-8 byte bounds and preserve user content without repair', () => {
  assert.equal(correctionInstruction('  Check readiness\ncarefully  '), 'Check readiness\ncarefully');
  assert.equal(correctionInstruction('中'.repeat(2730)).length, 2730);
  for (const value of ['', '   ', '中'.repeat(2731), 'x'.repeat(8193)]) assert.throws(() => correctionInstruction(value));
});
test('only selected terminal project evidence with complete exact provenance is selectable', () => {
  for (const status of ['failed', 'completed', 'stopped']) {
    const run = evidence(); run.status = status;
    assert.equal(correctionCases(run, 'alpha', publication).length, 1);
  }
  for (const mutate of [r => r.project = 'beta', r => r.status = 'running', r => r.finished_at = null,
    r => r.cases[0].provenance = null, r => r.cases[0].provenance.case_id = 'other',
    r => r.cases[0].provenance.yaml_script = 'changed',
    r => r.cases[0].provenance.contracts.contracts[0].publication.definition.assertions = ['weakened'],
    r => r.cases[0].provenance.contracts.contracts[0].binding.contract_version = 2,
    r => r.cases[0].provenance.contracts.contracts[0].original_assertions = ['different']]) {
    const run = evidence(); mutate(run); assert.deepEqual(correctionCases(run, 'alpha', publication), []);
  }
});
test('old baseline cannot populate new version observation picker', () => {
  const replacement = structuredClone(publication); replacement.definition.version = 2;
  assert.deepEqual(correctionCases(evidence(), 'alpha', replacement), []);
  const run = evidence(); run.cases[0].provenance.contracts.contracts[0].publication = replacement;
  run.cases[0].provenance.contracts.contracts[0].binding.contract_version = 2;
  assert.equal(correctionCases(run, 'alpha', replacement).length, 1);
});
test('actual correction attempts are bounded and separate from generated case evidence', () => {
  const attempts = [{call: 1, attempt: 1, profile_id: 'primary', outcome: 'rate_limited'}, {call: 1, attempt: 2, profile_id: 'backup', outcome: 'succeeded'}];
  assert.deepEqual(correctionAttempts(attempts), attempts);
  const value = {schema_version: 1, source: 'contract_correction', application_outcome: 'completed', attempts, proposal_id: 'proposal'};
  assert.equal(correctionJobEvidence(value).proposalId, 'proposal');
  for (const invalid of [{...value, source: 'registered_route'}, {...value, application_outcome: 'repair_passed'}, {...value, attempts: [[1, 1, 0, 'succeeded']]}, {...value, attempts: [{...attempts[0], attempt: 2}]}, {...value, attempts: [{...attempts[0], outcome: 'invented'}]}, {...value, attempts: Array(5).fill(attempts[0])}]) assert.equal(correctionJobEvidence(invalid), null);
  assert.deepEqual(correctionJobEvidence({...value, application_outcome: 'cancelled', attempts: []}).attempts, []);
});
test('correction RPCs transmit exact bounded references and explicit observation approval', async () => {
  globalThis.window = {location: {hash: '', pathname: '/', search: ''}, history: {replaceState() {}}};
  const {rpc} = await import(pathToFileURL(join(temp, 'rpc.mjs')));
  const seen = [];
  globalThis.fetch = async (url, init) => {
    const body = url.endsWith('/session') ? {csrf: 'csrf'} : JSON.parse(init.body);
    if (!url.endsWith('/session')) seen.push(body);
    return {ok: true, status: 200, json: async () => url.endsWith('/session') ? body : {id: body.id, ok: true, data: {ok: true, jobId: 'job'}}};
  };
  const request = {project: 'alpha', contract_id: 'ready', version: 1, run_id: 'run', case_id: 'case', expected_revision: 7, instruction: 'Check meaning'};
  await rpc.runContractCorrection(request);
  const observation = {proposal_id: 'proposal', project: 'alpha', run_id: 'new-run', case_id: 'new-case', expected_revision: 8, approved: true};
  await rpc.recordContractCorrectionObservation(observation);
  assert.equal(seen[0].method, 'runContractCorrection'); assert.deepEqual(seen[0].params, request);
  assert.equal(seen[1].method, 'recordContractCorrectionObservation'); assert.deepEqual(seen[1].params, observation);
});
