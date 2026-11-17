#!/usr/bin/env python3
"""Real HTTP case editing/review acceptance with a deterministic local model.

The historical run is an explicitly seeded failed-run fixture, not visual execution.
No external model, business target or operating-system control is used.
"""
import importlib.util
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import sys
import tempfile
import threading
import time

loader = importlib.util.spec_from_file_location('local_web_smoke', Path(__file__).with_name('test-local-web.py'))
module = importlib.util.module_from_spec(loader)
loader.loader.exec_module(module)
require = module.require


def main():
    binary = Path(sys.argv[1]).resolve()
    calls = []

    class Model(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            calls.append(body)
            content = ([{'id': 'receipt', 'name': 'Receipt', 'requirement_id': 'receipt',
                         'type': 'smoke', 'priority': 'P1', 'yaml_script': '- aiAssert: original receipt',
                         'expected_result': 'original receipt', 'tags': []}]
                       if len(calls) == 1 else
                       {'score': 90, 'issues': [], 'suggestions': [], 'coverage': 'receipt'})
            payload = json.dumps({'choices': [{'message': {'role': 'assistant', 'content': json.dumps(content)}}]}).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    model = ThreadingHTTPServer(('127.0.0.1', 0), Model)
    threading.Thread(target=model.serve_forever, daemon=True).start()
    with tempfile.TemporaryDirectory(prefix='qinaegis-case-assets-') as directory:
        smoke = module.Smoke(binary, directory)

        def start():
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance()
            smoke.authenticate()

        def finish(job):
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                state = smoke.rpc('getJobStatus', {'jobId': job['jobId']})
                if state['status'] in ('completed', 'failed', 'stopped'):
                    require(state['status'] == 'completed', 'local fixture generation failed')
                    return state
                time.sleep(0.025)
            raise module.SmokeFailure('case generation timed out')

        project = 'case-edit-fixture'
        params = {'project': project, 'case_id': 'receipt'}
        try:
            start()
            smoke.rpc('createProject', {'name': project, 'url': 'http://127.0.0.1:9', 'tech_stack': []})
            profile = {'id': 'fixture', 'label': 'Local fixture', 'adapter': 'openai_compatible',
                       'base_url': f'http://127.0.0.1:{model.server_port}/v1', 'model': 'fixture-model',
                       'api_key': 'case-edit-fixture-key', 'capabilities': ['text'], 'enabled': True,
                       'allow_remote': False, 'midscene_family': None}
            smoke.secrets.append(profile['api_key'])
            state = smoke.rpc('saveModel', {'profile': profile, 'expected_revision': 0})
            smoke.rpc('setModelRoute', {'task': 'generate', 'route': {'primary': 'fixture', 'fallbacks': []},
                                      'expected_revision': state['revision']})
            finish(smoke.rpc('runGenerate', {'project': project, 'requirement': 'receipt', 'use_contracts': False}))
            original = smoke.rpc('getCaseAsset', params)
            require(original['yaml_script'] == '- aiAssert: original receipt' and original['provenance'],
                    'case asset omitted actual script or generated provenance')
            require(original['digest'].startswith('content:'), 'case has no exact-content revision')
            for status in ('reviewed', 'approved'):
                inspected = smoke.rpc('getCaseAsset', params)
                smoke.rpc('updateCaseStatus', {**params, 'status': status}, success=False)
                if status == 'approved':
                    smoke.rpc('updateCaseStatus', {**params, 'status': status, 'expected_digest': original['digest']}, success=False)
                smoke.rpc('updateCaseStatus', {**params, 'status': status, 'expected_digest': inspected['digest']})
            approved = smoke.rpc('getCaseAsset', params)
            require(approved['status'] == 'approved', 'case did not reach approved')
            # Seed an explicit failed-run fixture to test immutable historical bytes.
            history = {'schema_version': 1, 'run_id': 'frozen-edit-fixture', 'project': project,
                       'test_type': 'smoke', 'status': 'failed', 'started_at': '2026-11-17T00:00:00Z',
                       'finished_at': '2026-11-17T00:00:01Z', 'model': approved['provenance']['model'],
                       'engine_version': 'seeded-case-edit-fixture',
                       'cases': [{'case_id': 'receipt', 'name': approved['name'], 'yaml_script': approved['yaml_script'],
                                  'target_url': 'http://127.0.0.1:9', 'provenance': approved['provenance']}],
                       'results': [], 'error': 'explicit seeded historical fixture; no execution',
                       'semantic_validation': 'not_verified'}
            history_path = smoke.data / 'projects' / project / 'reports' / history['run_id'] / 'run.json'
            history_path.parent.mkdir(parents=True)
            history_path.write_text(json.dumps(history))
            history_bytes = history_path.read_bytes()
            edit = {**params, 'expected_digest': approved['digest'], 'name': 'Reviewed manual receipt',
                    'yaml_script': '- aiAssert: corrected receipt', 'expected_result': 'corrected receipt'}
            smoke.rpc('saveCaseAsset', edit, success=False)
            smoke.rpc('saveCaseAsset', {**edit, 'approved': False}, success=False)
            require(smoke.rpc('getCaseAsset', params) == approved, 'unconfirmed edit changed asset')
            saved = smoke.rpc('saveCaseAsset', {**edit, 'approved': True})
            require(saved['status'] == 'draft' and saved['provenance'] is None
                    and saved['expected_result'] == 'corrected receipt', 'manual edit retained approval or stale generation facts')
            smoke.rpc('saveCaseAsset', {**edit, 'approved': True}, success=False)
            smoke.rpc('saveCaseAsset', {**edit, 'expected_digest': saved['digest'], 'yaml_script': 'tasks: []', 'approved': True}, success=False)
            smoke.rpc('saveCaseAsset', {**edit, 'expected_digest': saved['digest'], 'unexpected': 'field', 'approved': True}, success=False)
            require(smoke.rpc('getCaseAsset', params) == saved, 'failed edit overwrote draft')
            def save_unchanged(asset):
                return smoke.rpc('saveCaseAsset', {**params, 'expected_digest': asset['digest'],
                    'name': asset['name'], 'yaml_script': asset['yaml_script'],
                    'expected_result': asset['expected_result'], 'approved': True})

            for _ in range(3):
                saved = save_unchanged(saved)
                require(saved['status'] == 'draft', 'unchanged draft save failed')
                require(len(smoke.rpc('getReviewCases', {'project': project})) == 1, 'unchanged draft save poisoned listing')
            for status in ('reviewed', 'approved'):
                inspected = smoke.rpc('getCaseAsset', params)
                smoke.rpc('updateCaseStatus', {**params, 'status': status}, success=False)
                if status == 'approved':
                    smoke.rpc('updateCaseStatus', {**params, 'status': status, 'expected_digest': original['digest']}, success=False)
                smoke.rpc('updateCaseStatus', {**params, 'status': status, 'expected_digest': inspected['digest']})
            after_review = smoke.rpc('getCaseAsset', params)
            smoke.rpc('updateCaseStatus', {**params, 'status': 'flaky', 'unexpected': True}, success=False)
            smoke.rpc('updateCaseStatus', {**params, 'status': 'unknown'}, success=False)
            require(after_review['expected_result'] == saved['expected_result']
                    and after_review['yaml_script'] == saved['yaml_script'], 'status transition discarded manually edited content')
            # No-op Approved and Archived saves still explicitly reset approval.
            for status in ('approved', 'archived'):
                if status == 'archived':
                    smoke.rpc('updateCaseStatus', {**params, 'status': 'archived'})
                after_review = save_unchanged(smoke.rpc('getCaseAsset', params))
                require(after_review['status'] == 'draft', 'unchanged approved/archived save retained approval')
                for next_status in ('reviewed', 'approved'):
                    inspected = smoke.rpc('getCaseAsset', params)
                    smoke.rpc('updateCaseStatus', {**params, 'status': next_status, 'expected_digest': inspected['digest']})
                after_review = smoke.rpc('getCaseAsset', params)
            require(history_path.read_bytes() == history_bytes, 'editing changed immutable run bytes')
            frozen = smoke.rpc('getRunEvidence', {'project': project, 'run_id': history['run_id']})
            require(frozen['cases'][0]['yaml_script'] == approved['yaml_script'], 'historical run reread changed script')
            smoke.cli('stop')
            start()
            require(smoke.rpc('getCaseAsset', params) == after_review, 'case changed across service restart')
            require(history_path.read_bytes() == history_bytes, 'restart changed frozen run')
            require(len(calls) == 2, 'case editing unexpectedly called a model')
            print('PASS real HTTP case editing: confirmation, exact-content edit/review/approval CAS, approval reset, provenance invalidation, review preservation, immutable run and restart')
        finally:
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
