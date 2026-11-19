import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const ui = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const temp = mkdtempSync(join(tmpdir(), 'qinaegis-capture-ui-'));
execFileSync(process.execPath, [join(ui, 'node_modules/typescript/bin/tsc'), 'src/utils/capture.ts', 'src/utils/case_history.ts', '--target', 'ES2020', '--module', 'ES2022', '--moduleResolution', 'bundler', '--outDir', temp], {cwd: ui});
process.on('exit', () => rmSync(temp, {recursive: true, force: true}));
writeFileSync(join(temp, 'capture.mjs'), readFileSync(join(temp, 'utils/capture.js')));
const { validCapturePolicy, localTraceViewerUrl, validateArtifactResponse } = await import(pathToFileURL(join(temp, 'capture.mjs')));
const policy = {schema_version:1,mode:'off',max_artifact_bytes:2097152,max_run_bytes:8388608,max_events:500,include_sensitive:false};
test('capture limits and sensitivity apply independently of business results', () => {
 assert.equal(validCapturePolicy(policy),true);
 for (const patch of [{schema_version:2},{mode:'full'},{max_artifact_bytes:1023},{max_run_bytes:1024},{max_events:2001},{max_events:1.5},{mode:'unknown'}]) assert.equal(validCapturePolicy({...policy,...patch}),false);
 for (const mode of ['failure_only','key_steps','full']) assert.equal(validCapturePolicy({...policy,mode,include_sensitive:true}),true);
});
test('viewer links remain on a separate numeric local HTTP endpoint', () => {
 assert.equal(localTraceViewerUrl('http://127.0.0.1:1234/token/'),'http://127.0.0.1:1234/token/');
 for (const value of ['javascript:alert(1)','https://example.com','http://localhost:1234/','http://user@127.0.0.1:1234/','http://127.0.0.1/']) assert.throws(()=>localTraceViewerUrl(value));
});

test('artifact response must match the selected committed receipt before decode or download', () => {
 const expected = {id:'screenshot-1',kind:'screenshot',file_name:'screenshot-1.png',bytes:3,sha256:'a'.repeat(64)};
 const response = {...expected,base64:'AAAA'};
 assert.doesNotThrow(()=>validateArtifactResponse(expected,response));
 for (const patch of [{kind:'trace'},{file_name:'other.png'},{bytes:4},{bytes:8388609},{bytes:-1},{sha256:'b'.repeat(64)},{base64:'A'.repeat(11184813)}]) assert.throws(()=>validateArtifactResponse(expected,{...response,...patch}));
});

writeFileSync(join(temp, 'case_history.mjs'), readFileSync(join(temp, 'utils/case_history.js')));
const {caseRestoreReason,restorableCaseRevision} = await import(pathToFileURL(join(temp,'case_history.mjs')));
test('restoration requires bounded reason and a complete snapshot of the selected case',()=>{
 assert.equal(caseRestoreReason('Explain the correction'),true);
 assert.equal(caseRestoreReason('  '),false);
 assert.equal(caseRestoreReason('中'.repeat(2731)),false);
 const revision={revision:1,snapshot:{id:'case-1'},digest:'content:abc'};
 assert.equal(restorableCaseRevision(revision,'case-1'),true);
 for(const patch of [{revision:0},{revision:1.5},{snapshot:null},{snapshot:{id:'other'}},{digest:null}])assert.equal(restorableCaseRevision({...revision,...patch},'case-1'),false);
});
