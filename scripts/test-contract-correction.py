#!/usr/bin/env python3
"""Real-service correction governance acceptance using local protocol fixtures.

The model and JSON-RPC engine are deterministic fixtures. This proves storage,
review and routing behavior, not visual or business-semantic correctness.
"""
import importlib.util
import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import signal
import sys
import tempfile
import threading
import time


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


m = load('local_web', 'test-local-web.py')
e = load('execution_fixture', 'test-contract-execution.py')
require, wait = m.require, e.wait_for


def main():
    calls = []
    primary_key, fallback_key = 'correction-private-primary', 'correction-private-fallback'
    arrived, release, returned = threading.Event(), threading.Event(), threading.Event()

    class Model(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_POST(self):
            request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            prompt = request['messages'][-1]['content']
            try:
                source = json.loads(prompt)
            except json.JSONDecodeError:
                source = {}
            correction = isinstance(source, dict) and 'evidence_token' in source
            calls.append({'model': request['model'], 'source': source, 'correction': correction})
            mode = request['model']
            status = (429 if mode == 'quota' else 401 if mode == 'denied' else 200) if correction else 200
            if status != 200:
                payload = {'error': {'message': primary_key}}
            else:
                if correction:
                    if mode == 'blocked':
                        arrived.set()
                        release.wait(20)
                        returned.set()
                    replacement = {**source['publication']['definition'], 'version': 2}
                    replacement['assertions'] = replacement['assertions'] + ['receipt has a timestamp']
                    output = {'replacement': replacement, 'reason': 'clarify the selected receipt observation',
                              'limitations': ['Protocol fixture cannot establish business correctness.'],
                              'evidence_tokens': [source['evidence_token']]}
                    if mode == 'foreign-token':
                        output['evidence_tokens'] = ['managed-run:other:run:case']
                    elif mode == 'unknown-field':
                        output['unsupported'] = True
                    elif mode in ('echo', 'echo-escaped'):
                        output['reason'] = fallback_key
                    content = 'invalid output' if mode == 'invalid-document' else json.dumps(output)
                    if mode == 'echo-escaped':
                        content = content.replace(fallback_key, ''.join('\\u%04x' % ord(c) for c in fallback_key))
                elif 'yaml_script' in prompt and 'requirement_id' in prompt:
                    content = json.dumps([{'id': f'receipt-{len(calls)}', 'name': 'Receipt', 'requirement_id': 'receipt',
                        'type': 'smoke', 'priority': 'P1', 'yaml_script': 'tasks: []',
                        'expected_result': 'receipt matches submitted input', 'tags': []}])
                else:
                    content = json.dumps({'score': 90, 'issues': [], 'suggestions': [], 'coverage': 'receipt'})
                payload = {'choices': [{'finish_reason': 'stop', 'message': {'role': 'assistant', 'content': content}}]}
            data = json.dumps(payload).encode()
            self.send_response(status)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            try:
                self.wfile.write(data)
            except (BrokenPipeError, ConnectionResetError):
                pass

    server = ThreadingHTTPServer(('127.0.0.1', 0), Model)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    with tempfile.TemporaryDirectory(prefix='qinaegis-correction-') as directory:
        root = Path(directory)
        smoke = m.Smoke(Path(sys.argv[1]).resolve(), directory)
        smoke.secrets.extend([primary_key, fallback_key])
        smoke.env['QINAEGIS_FIXTURE_DIR'] = str(root)
        runtime = smoke.data / 'runtimes/midscene/current'
        runtime.mkdir(parents=True)
        engine = e.ENGINE.replace("const passed = !req.args.target_url.includes('smoke-beta');", "const passed = mode !== 'business-failure';")
        (runtime / 'engine.mjs').write_text(engine)
        (runtime / 'runtime.json').write_text(json.dumps({'schema_version': 1, 'engine': 'midscene',
            'version': 'protocol-fixture-1', 'entry': 'engine.mjs'}))
        (root / 'mode').write_text('business-failure')
        history = smoke.data / 'jobs/history.json'
        project = 'correction-fixture'
        model_revision = 0
        try:
            def start():
                smoke.cli('start', '--no-open', '--port', str(smoke.port))
                smoke.instance = smoke.read_instance()
                smoke.authenticate()

            def finish(job, expected='completed'):
                def terminal():
                    value = smoke.rpc('getJobStatus', {'jobId': job['jobId']})
                    return value if value['status'] in ('completed', 'failed', 'stopped') else None
                value = wait(terminal, 'correction fixture job timed out')
                require(value['status'] == expected, 'correction fixture returned unexpected terminal state: ' + str(value.get('error')))
                return value

            def configure(mode, fallback=False):
                nonlocal model_revision
                for profile_id, model_name, key in [('primary', mode, primary_key), ('fallback', 'normal', fallback_key)]:
                    profile = {'id': profile_id, 'label': profile_id, 'adapter': 'openai_compatible',
                        'base_url': f'http://127.0.0.1:{server.server_port}/v1', 'model': model_name, 'api_key': key,
                        'capabilities': ['text', 'vision'], 'midscene_family': 'qwen2.5-vl', 'enabled': True, 'allow_remote': False}
                    model_revision = smoke.rpc('saveModel', {'profile': profile, 'expected_revision': model_revision})['revision']
                for task in ('generate', 'execute', 'correct'):
                    model_revision = smoke.rpc('setModelRoute', {'task': task,
                        'route': {'primary': 'primary', 'fallbacks': ['fallback'] if task == 'correct' and fallback else []},
                        'expected_revision': model_revision})['revision']

            def library():
                return smoke.rpc('getContracts')

            def generated_case():
                finish(smoke.rpc('runGenerate', {'project': project, 'requirement': 'receipt', 'use_contracts': True}))
                drafts = list((smoke.data / 'projects' / project / 'cases/draft').glob('*.json'))
                require(len(drafts) == 1, 'generation did not create one unreviewed case')
                case = json.loads(drafts[0].read_text())
                for status in ('reviewed', 'approved'):
                    smoke.rpc('updateCaseStatus', {'project': project, 'case_id': case['id'], 'status': status,
                        'expected_digest': smoke.rpc('getCaseAsset', {'project': project, 'case_id': case['id']})['digest']})
                return case

            def run():
                status = finish(smoke.rpc('runTests', {'project': project, 'type': 'smoke'}))
                return smoke.rpc('getRunEvidence', {'project': project, 'run_id': status['result']['run_id']})

            start()
            smoke.rpc('createProject', {'name': project, 'url': 'https://correction-fixture.invalid', 'tech_stack': []})
            draft = {'id': 'receipt', 'version': 1, 'intent': 'submission returns a receipt',
                'inputs': {'target': 'string'}, 'outputs': {}, 'preconditions': [],
                'assertions': ['receipt matches submitted input'], 'required_capabilities': ['browser.interact']}
            smoke.rpc('saveContractDraft', {'draft': draft, 'expected_revision': 0})
            smoke.rpc('publishContract', {'id': 'receipt', 'expected_revision': 1, 'approved': True})
            smoke.rpc('bindContract', {'project': project, 'contract_id': 'receipt', 'version': 1,
                'parameters': {'target': 'https://correction-target.invalid'}, 'expected_revision': 2})
            configure('normal')
            case = generated_case()
            original_run = run()
            require(original_run['results'][0]['passed'] is False, 'fixture did not record selected failed observation')

            def request(**changes):
                value = {'project': project, 'contract_id': 'receipt', 'version': 1, 'run_id': original_run['run_id'],
                    'case_id': case['id'], 'expected_revision': library()['revision'],
                    'instruction': 'Clarify the expected receipt without hiding the original assertion.'}
                return {**value, **changes}

            count = len(calls)
            for changes in ({'case_id': 'foreign-case'}, {'project': 'missing-project'}, {'version': 2}, {'expected_revision': 0}, {'instruction': primary_key}):
                smoke.rpc('runContractCorrection', request(**changes), success=False)
            require(len(calls) == count, 'invalid correction input reached a model')
            for mode in ('invalid-document', 'unknown-field', 'foreign-token', 'echo', 'echo-escaped', 'denied'):
                configure(mode, fallback=True)
                count = len(calls)
                failed = finish(smoke.rpc('runContractCorrection', request()), 'failed')
                require(len(calls) == count + 1 and not library()['proposals'], 'invalid or hard-rejected correction produced a proposal or fallback')
                require(failed['result']['application_outcome'] == 'failed', 'failed correction lacks truthful application outcome')
            configure('quota', fallback=True)
            corrected = finish(smoke.rpc('runContractCorrection', request()))
            proposals = library()['proposals']
            require(len(proposals) == 1 and proposals[0]['status'] == 'pending', 'model bypassed pending human review')
            proposal = proposals[0]
            source = proposal['model_source']
            require(source['input']['publication']['definition'] == draft and source['input']['run_id'] == original_run['run_id']
                and source['input']['case_id'] == case['id'] and source['input']['result']['passed'] is False,
                'correction source lost frozen original assertions or selected execution evidence')
            require([a['profile_id'] for a in source['attempts']] == ['primary', 'fallback']
                and [a['outcome'] for a in source['attempts']] == ['rate_limited', 'succeeded'], 'actual correction fallback evidence differs')
            require(corrected['result']['proposal_id'] == proposal['id'], 'task result does not link the saved proposal')
            prompt = [c['source'] for c in calls if c['correction']][-1]
            require(set(prompt) == {'publication', 'binding', 'instruction', 'script', 'target_url', 'run_status', 'result', 'evidence_token'}
                and 'screenshot_base64' not in prompt['result'], 'model received unselected execution payload')
            smoke.rpc('reviewContractCorrection', {'proposal_id': proposal['id'], 'decision': 'reject',
                'expected_revision': library()['revision'], 'approved': True})
            require(len(library()['publications']) == 1, 'rejected suggestion changed publication')
            configure('blocked')
            arrived.clear(); release.clear(); returned.clear()
            stale = smoke.rpc('runContractCorrection', request())
            wait(arrived.is_set, 'correction fixture did not receive delayed call')
            # A different reviewed governance write wins; no automatic CAS retry.
            smoke.rpc('saveContractDraft', {'draft': {**draft, 'id': 'unrelated'}, 'expected_revision': library()['revision']})
            release.set()
            finish(stale, 'failed')
            require(len(library()['proposals']) == 1, 'stale correction silently rebased its source')
            wait(returned.is_set, 'delayed correction fixture did not return')
            arrived.clear(); release.clear(); returned.clear()
            stopped = smoke.rpc('runContractCorrection', request())
            wait(arrived.is_set, 'cancelled correction did not reach provider')
            durable = next(j for j in json.loads(history.read_text())['records'] if j['id'] == stopped['jobId'])
            require(durable['result']['attempts'][0]['outcome'] == 'started', 'correction call intent not durable before response')
            smoke.rpc('stopJob', {'jobId': stopped['jobId']})
            require(finish(stopped, 'stopped')['result']['application_outcome'] == 'cancelled', 'cancelled correction lost outcome')
            release.set(); wait(returned.is_set, 'cancelled provider fixture did not return')
            require(len(library()['proposals']) == 1, 'cancelled correction published a proposal')
            arrived.clear(); release.clear(); returned.clear()
            interrupted = smoke.rpc('runContractCorrection', request())
            wait(arrived.is_set, 'interrupted correction did not reach provider')
            count = len(calls)
            durable = next(j for j in json.loads(history.read_text())['records'] if j['id'] == interrupted['jobId'])
            require(durable['result']['attempts'][0]['outcome'] == 'started', 'crash lost durable correction intent')
            os.kill(smoke.instance['pid'], signal.SIGTERM if os.name == 'nt' else signal.SIGKILL)
            time.sleep(.5)
            start()
            recovered = smoke.rpc('getJobStatus', {'jobId': interrupted['jobId']})
            require(recovered['status'] == 'interrupted' and recovered['result']['attempts'] == durable['result']['attempts']
                and len(calls) == count, 'restart invented completion or replayed correction')
            release.set(); wait(returned.is_set, 'interrupted provider fixture did not return')
            configure('normal')
            success = finish(smoke.rpc('runContractCorrection', request()))
            proposal = next(p for p in library()['proposals'] if p['id'] == success['result']['proposal_id'])
            review = {'proposal_id': proposal['id'], 'decision': 'approve', 'expected_revision': library()['revision']}
            smoke.rpc('reviewContractCorrection', review, success=False)
            updated = smoke.rpc('reviewContractCorrection', {**review, 'approved': True})
            require(len(updated['publications']) == 2 and updated['bindings'][0]['contract_version'] == 1,
                'human approval changed an old publication or migrated a project implicitly')
            observation = {'proposal_id': proposal['id'], 'project': project, 'run_id': original_run['run_id'],
                'case_id': case['id'], 'expected_revision': updated['revision'], 'approved': True}
            smoke.rpc('recordContractCorrectionObservation', observation, success=False)
            smoke.rpc('bindContract', {'project': project, 'contract_id': 'receipt', 'version': 2,
                'parameters': {'target': 'https://correction-target.invalid'}, 'expected_revision': updated['revision']})
            smoke.rpc('updateCaseStatus', {'project': project, 'case_id': case['id'], 'status': 'archived',
                'expected_digest': smoke.rpc('getCaseAsset', {'project': project, 'case_id': case['id']})['digest']})
            new_case = generated_case()
            (root / 'mode').write_text('normal')
            new_run = run()
            observation.update({'run_id': new_run['run_id'], 'case_id': new_case['id'], 'expected_revision': library()['revision']})
            observed = smoke.rpc('recordContractCorrectionObservation', observation)
            require(len(observed['observations']) == 1 and observed['observations'][0]['outcome'] == 'passed'
                and observed['observations'][0]['contract_version'] == 2 and observed['observations'][0]['semantic_validation'] == 'not_verified',
                'new-version observation invented semantic verification or lost run identity')
            smoke.rpc('recordContractCorrectionObservation', observation, success=False)
            # New task history can disappear; durable model source and observation survive restart.
            terminal_history = smoke.rpc('getJobs')
            smoke.cli('stop')
            start()
            require(library() == observed, 'restart changed durable correction source or observation')
            require(smoke.rpc('getJobs') == terminal_history, 'restart changed terminal correction jobs')
            smoke.cli('stop')
            pruned = json.loads(history.read_text())
            pruned['records'] = []
            history.write_text(json.dumps(pruned))
            start()
            require(library() == observed and smoke.rpc('getJobs')['jobs'] == [],
                'durable model source or observation depended on bounded task history')
            for path in [history, smoke.data / 'contracts/library.json', *smoke.data.glob('projects/**/*.json')]:
                text = path.read_text()
                require(primary_key not in text and fallback_key not in text, 'credential leaked into correction evidence or assets')
            print('PASS real-service selected correction evidence, strict route, secret rejection, durable attempts, fallback, stale CAS, cancel, human review and new-version observation (protocol fixtures only)')
        finally:
            release.set()
            smoke.cleanup()
            server.shutdown()
            server.server_close()


if __name__ == '__main__':
    main()
