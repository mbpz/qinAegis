#!/usr/bin/env python3
"""Real-service provenance/lifecycle acceptance with deterministic local fixtures.

The Node JSON-RPC engine is a protocol fixture, not Playwright/Midscene business
execution or an external visual model. No remote AI, browser or real app is used.
"""
import importlib.util
import json
from http.server import BaseHTTPRequestHandler
from pathlib import Path
import shutil
import sys
import tempfile
import threading
import time

loader = importlib.util.spec_from_file_location("local_web_smoke", Path(__file__).with_name("test-local-web.py"))
module = importlib.util.module_from_spec(loader)
loader.loader.exec_module(module)
require = module.require

ENGINE = r'''
import fs from 'node:fs';
import readline from 'node:readline';
import path from 'node:path';
const root = process.env.QINAEGIS_FIXTURE_DIR;
const file = name => path.join(root, name);
for await (const line of readline.createInterface({input: process.stdin})) {
  const req = JSON.parse(line);
  if (req.method === 'shutdown') {
    const fail = fs.existsSync(file('mode')) && fs.readFileSync(file('mode'),'utf8') === 'shutdown-failure';
    console.log(JSON.stringify({id:'0',ok:!fail,data:null,error:fail?'fixture shutdown failed':null}));
    break;
  }
  if (req.method !== 'run_yaml') throw Error('unsupported fixture request');
  fs.appendFileSync(file('requests.jsonl'), JSON.stringify(req.args)+'\n');
  if (fs.existsSync(file('hold'))) {
    fs.writeFileSync(file('arrived'),'1');
    const deadline = Date.now()+10000;
    while (!fs.existsSync(file('release')) && Date.now()<deadline)
      await new Promise(resolve=>setTimeout(resolve,10));
    if (!fs.existsSync(file('release'))) throw Error('fixture deadline');
  }
  const mode = fs.existsSync(file('mode')) ? fs.readFileSync(file('mode'),'utf8') : '';
  const passed = !req.args.target_url.includes('smoke-beta');
  const data = {case_id:mode==='foreign'?'foreign-case':req.args.case_id, passed,
    duration_ms:12,screenshot_base64:null,error_message:passed?null:'fixture assertion failed',
    retry_count:0,healed_yaml_script:null,failed_step:passed?null:'fixture check'};
  console.log(JSON.stringify({id:'0',ok:true,data}));
}
'''


def wait_for(predicate, message):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.025)
    raise module.SmokeFailure(message)


def main():
    binary = Path(sys.argv[1]).resolve()
    require(shutil.which('node') is not None, 'Node is required for the explicit engine fixture')
    calls = []
    class Model(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass
        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            calls.append(body)
            number = len(calls)
            content = ([{'id': f'receipt-{number}', 'name': 'Receipt', 'requirement_id': 'receipt',
                'type': 'smoke', 'priority': 'P1', 'yaml_script': 'tasks: []',
                'expected_result': 'receipt matches submitted input', 'tags': []}]
                if number % 2 else {'score': 90, 'issues': [], 'suggestions': [], 'coverage': 'receipt'})
            payload = json.dumps({'choices': [{'finish_reason':'stop','message': {'role': 'assistant', 'content': json.dumps(content)}}]}).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
    model = module.LoopbackHTTPServer(('127.0.0.1', 0), Model)
    threading.Thread(target=model.serve_forever, daemon=True).start()
    with tempfile.TemporaryDirectory(prefix='qinaegis-execution-') as directory:
        root = Path(directory)
        smoke = module.Smoke(binary, directory)
        # Cover default runtime lookup beneath the selected data root, without
        # falling back to a real user's engine or relying on a runtime override.
        runtime = smoke.data / 'runtimes/midscene/current'
        runtime.mkdir(parents=True)
        (runtime / 'engine.mjs').write_text(ENGINE)
        manifest = runtime / 'runtime.json'
        manifest.write_text(json.dumps({'schema_version': 1, 'engine': 'midscene',
            'version': 'protocol-fixture-1', 'entry': 'engine.mjs'}))
        smoke.env['QINAEGIS_FIXTURE_DIR'] = str(root)
        projects = smoke.data / 'projects'
        def start():
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance()
            smoke.authenticate()
        def finish(job, expected='completed'):
            def terminal():
                status = smoke.rpc('getJobStatus', {'jobId': job['jobId']})
                return status if status['status'] in ('completed', 'failed', 'stopped') else None
            result = wait_for(terminal, 'fixture job timed out')
            require(result['status'] == expected, 'fixture job returned unexpected terminal state')
            return result
        def run(project, expected='completed'):
            return finish(smoke.rpc('runTests', {'project': project, 'type': 'smoke'}), expected)
        def evidence(project, status):
            return smoke.rpc('getRunEvidence', {'project': project, 'run_id': status['result']['run_id']})
        try:
            start()
            library = smoke.exercise_contracts()
            revision = 0
            for name in ('one', 'two'):
                profile = {'id': name, 'label': name, 'adapter': 'openai_compatible',
                    'base_url': f'http://127.0.0.1:{model.server_port}/v1', 'model': f'model-{name}',
                    'api_key': 'fixture-only-key', 'capabilities': ['text', 'vision'], 'enabled': True,
                    'allow_remote': False, 'midscene_family': 'qwen2.5-vl'}
                smoke.secrets.append(profile['api_key'])
                revision = smoke.rpc('saveModel', {'profile': profile, 'expected_revision': revision})['revision']
            for task in ('generate', 'execute'):
                revision = smoke.rpc('setModelRoute', {'task': task, 'route': {'primary': 'one', 'fallbacks': []},
                    'expected_revision': revision})['revision']
            saved = {}
            for project in ('smoke-alpha', 'smoke-beta'):
                generated = finish(smoke.rpc('runGenerate', {'project': project, 'requirement': 'receipt', 'use_contracts': True}))
                draft = list((projects / project / 'cases' / 'draft').glob('*.json'))
                require(len(draft) == 1, 'generation did not save exactly one project draft')
                case = json.loads(draft[0].read_text())
                provenance = smoke.rpc('getCaseProvenance', {'project': project, 'case_id': case['id']})
                require(provenance['contracts'] == generated['snapshot']['contracts'] and provenance['model'] == generated['snapshot']['model'],
                        'persisted provenance differs from generation snapshot')
                require(provenance['contracts']['contracts'][0]['publication']['definition']['version'] == 1,
                        'generation provenance lost original version')
                smoke.rpc('runTests', {'project': project, 'type': 'smoke'}, success=False)
                for status in ('reviewed', 'approved'):
                    smoke.rpc('updateCaseStatus', {'project': project, 'case_id': case['id'], 'status': status,
                        'expected_digest': smoke.rpc('getCaseAsset', {'project': project, 'case_id': case['id']})['digest']})
                saved[project] = (case, provenance)
            # Lifecycle changes must be explicit and atomic, retaining old case provenance.
            base = {'contract_id': 'receipt', 'version': 1, 'reason': 'use corrected receipt', 'expected_revision': library['revision']}
            smoke.rpc('deprecateContract', base, success=False)
            library = smoke.rpc('deprecateContract', {**base, 'approved': True})
            smoke.rpc('getContractExecution', {'project': 'smoke-alpha'}, success=False)
            smoke.rpc('runGenerate', {'project': 'smoke-alpha', 'requirement': 'receipt', 'use_contracts': True}, success=False)
            migration = {'contract_id': 'receipt', 'from_version': 1, 'to_version': 2,
                'bindings': [{'project': 'smoke-alpha', 'parameters': {'target': 'https://new-alpha.invalid', 'quantity': 3}},
                             {'project': 'smoke-beta', 'parameters': {'target': 3, 'quantity': 'bad'}}],
                'reason': 'reviewed corrected receipt', 'expected_revision': library['revision'], 'approved': True}
            smoke.rpc('migrateContractBindings', migration, success=False)
            require(smoke.rpc('getContracts') == library, 'invalid migration partially changed bindings')
            library = smoke.rpc('migrateContractBindings', {**migration, 'bindings': migration['bindings'][:1]})
            pins = {b['project']: b['contract_version'] for b in library['bindings']}
            require(pins == {'smoke-alpha': 2, 'smoke-beta': 1}, 'migration changed an unselected project')
            smoke.rpc('migrateContractBindings', {**migration, 'bindings': migration['bindings'][:1]}, success=False)
            derived = smoke.rpc('deriveContract', {'source_id': 'receipt', 'source_version': 1, 'new_id': 'derived-receipt',
                'reason': 'reuse as a new business asset', 'expected_revision': library['revision'], 'approved': True})
            require(derived['drafts'][0]['id'] == 'derived-receipt' and derived['drafts'][0]['version'] == 1
                    and len(derived['publications']) == len(library['publications']), 'derivation bypassed draft review')
            revision = smoke.rpc('setModelRoute', {'task': 'execute', 'route': {'primary': 'two', 'fallbacks': []},
                'expected_revision': revision})['revision']
            # Freeze before engine response, then alter disk assets while the fixture waits.
            (root / 'hold').write_text('1')
            job = smoke.rpc('runTests', {'project': 'smoke-alpha', 'type': 'smoke'})
            wait_for(lambda: (root / 'arrived').exists(), 'engine fixture did not receive execution')
            case, provenance = saved['smoke-alpha']
            case_path = projects / 'smoke-alpha' / 'cases' / 'approved' / f"{case['id']}.json"
            mutated = {**case, 'status': 'approved', 'yaml_script': 'tasks: [changed-after-start]'}
            case_path.write_text(json.dumps(mutated))
            (root / 'release').write_text('1')
            alpha_status = finish(job)
            alpha = evidence('smoke-alpha', alpha_status)
            require(alpha['cases'][0]['yaml_script'] == case['yaml_script'] and alpha['cases'][0]['provenance'] == provenance,
                    'active execution reread changed disk assets')
            require(alpha['model']['candidates'][0]['id'] == 'two' and provenance['model']['candidates'][0]['id'] == 'one',
                    'generation and execution model histories were conflated')
            require(alpha['semantic_validation'] == 'not_verified' and alpha['engine_version'] == 'protocol-fixture-1',
                    'protocol fixture misrepresented semantic or real engine validation')
            require(alpha['results'][0]['passed'] is True, 'fixture result not persisted')
            request = json.loads((root / 'requests.jsonl').read_text().splitlines()[0])
            require(request['yaml_script'] == case['yaml_script'], 'engine request used changed case script')
            smoke.rpc('getCaseProvenance', {'project': 'smoke-alpha', 'case_id': case['id']}, success=False)
            count = len((root / 'requests.jsonl').read_text().splitlines())
            smoke.rpc('runTests', {'project': 'smoke-alpha', 'type': 'smoke'}, success=False)
            require(len((root / 'requests.jsonl').read_text().splitlines()) == count, 'tampered case reached engine')
            case_path.write_text(json.dumps({**case, 'status': 'approved'}))
            (root / 'hold').unlink()
            beta = evidence('smoke-beta', run('smoke-beta'))
            require(beta['results'][0]['passed'] is False and beta['results'][0]['error_message'] == 'fixture assertion failed',
                    'failed business result was hidden')
            require(beta['cases'][0]['provenance'] == saved['smoke-beta'][1], 'second project lost historical contract')
            reports = smoke.rpc('getReports', {'project': 'smoke-alpha'})
            require(any(r['run_id'] == alpha['run_id'] for r in reports), 'durable execution missing from report list')
            # Passing case results cannot hide an executor shutdown failure.
            (root / 'mode').write_text('shutdown-failure')
            shutdown_status = run('smoke-alpha', 'failed')
            shutdown_record = evidence('smoke-alpha', shutdown_status)
            require(shutdown_record['status'] == 'failed' and all(r['passed'] for r in shutdown_record['results']),
                    'shutdown fixture did not preserve passing results alongside execution failure')
            gate = smoke.rpc('getGateStatus', {'project': 'smoke-alpha'})
            require(gate['execution_blocked'] is True and gate['execution_status'] == 'failed'
                    and gate['e2e_pass_rate'] is None and gate['last_run_id'] == shutdown_record['run_id'],
                    'executor failure was displayed as a successful quality gate')
            # Foreign result identity cannot turn into a successful run.
            (root / 'mode').write_text('foreign')
            bad = run('smoke-alpha', 'failed')
            require(bad.get('error'), 'invalid engine identity did not produce explicit failure')
            (root / 'mode').unlink()
            # Legacy approved cases remain runnable but never gain invented provenance.
            legacy = {**case, 'id': 'legacy-case', 'status': 'approved', 'test_type': 'functional'}
            legacy.pop('provenance', None)
            (case_path.parent / 'legacy-case.json').write_text(json.dumps(legacy))
            require(smoke.rpc('getCaseProvenance', {'project': 'smoke-alpha', 'case_id': 'legacy-case'}) is None,
                    'legacy case received invented provenance')
            legacy_status = finish(smoke.rpc('runTests', {'project': 'smoke-alpha', 'type': 'functional'}))
            require(evidence('smoke-alpha', legacy_status)['cases'][0]['provenance'] is None,
                    'legacy run received invented provenance')
            # Runtime resolution failure must also leave durable failure evidence.
            manifest.unlink()
            failed_status = run('smoke-alpha', 'failed')
            failed = evidence('smoke-alpha', failed_status)
            require(failed['status'] == 'failed' and failed['error'] and not failed['results'],
                    'engine initialization failure lost durable evidence')
            smoke.cli('stop')
            start()
            require(smoke.rpc('getRunEvidence', {'project': 'smoke-alpha', 'run_id': alpha['run_id']}) == alpha,
                    'service restart changed historical execution evidence')
            require(smoke.rpc('getRunEvidence', {'project': 'smoke-alpha', 'run_id': failed['run_id']}) == failed,
                    'service restart changed failed execution evidence')
            print('PASS real-service provenance, reviewed two-project execution, immutable inputs, lifecycle CAS, durable evidence and failure rejection (local protocol fixtures only)')
        finally:
            (root / 'release').write_text('1')
            for _ in range(100):
                try:
                    smoke.cli('stop')
                    break
                except module.SmokeFailure:
                    time.sleep(0.05)
            smoke.cleanup()
            model.shutdown()
            model.server_close()


if __name__ == '__main__':
    main()
