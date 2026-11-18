#!/usr/bin/env python3
"""Native Messages and approved cross-protocol routing through the real service.

Only loopback deterministic providers are contacted. This is protocol acceptance,
not paid-provider or visual business acceptance.
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
    requests = []
    fixture_errors = []
    key = 'native-messages-private-fixture-key'

    class Model(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            protocol = 'messages' if self.path == '/v1/messages' else 'completions'
            requests.append((protocol, body))
            if protocol == 'messages':
                if self.headers.get('x-api-key') != key or self.headers.get('anthropic-version') != '2026-10-21' or self.headers.get('Authorization'):
                    fixture_errors.append('wrong native authentication/version')
                if 'response_format' in body or any(m['role'] not in ('user', 'assistant') for m in body['messages']):
                    fixture_errors.append('Chat Completions fields entered native protocol')
            elif self.path != '/v1/chat/completions' or self.headers.get('Authorization') != f'Bearer {key}':
                fixture_errors.append('wrong fallback protocol/authentication')
            model = body['model']
            if model in ('quota', 'denied') or model.startswith('wire-http-'):
                status = int(model.removeprefix('wire-http-')) if model.startswith('wire-http-') else (429 if model == 'quota' else 401)
                self.send_response(status)
                if status == 302:
                    self.send_header('Location', '/v1/chat/completions')
                payload = json.dumps({'error': {'message': key}}).encode()
            else:
                self.send_response(200)
                if model == 'probe':
                    text = json.dumps({'ok': True}) if 'output_config' in body else 'QINAEGIS_OK'
                elif model == 'refusal':
                    text = 'partial output must not pass'
                else:
                    prompt = json.dumps(body['messages']) + body.get('system', '')
                    # The generator asks for cases; the critic receives a YAML
                    # script and asks for a review. Both are real service calls.
                    if 'yaml_script' in prompt and 'requirement_id' in prompt:
                        text = json.dumps([{'id': 'native-receipt', 'name': 'Receipt', 'requirement_id': 'receipt',
                            'type': 'smoke', 'priority': 'P1', 'yaml_script': 'tasks: []',
                            'expected_result': 'receipt matches submitted input', 'tags': []}])
                    else:
                        text = json.dumps({'score': 90, 'issues': [], 'suggestions': [], 'coverage': 'receipt'})
                payload = json.dumps({'type': 'message', 'role': 'assistant',
                    'stop_reason': 'refusal' if model == 'refusal' else 'end_turn',
                    'content': [{'type': 'text', 'text': text}]}
                    if protocol == 'messages' else {'choices': [{'finish_reason': 'stop', 'message': {'role': 'assistant', 'content': text}}]}).encode()
                if model.startswith('wire-'):
                    value = {'choices':[{'finish_reason':'stop','message':{'role':'assistant','content':key}}]}
                    choice = value['choices'][0]
                    if model == 'wire-malformed':
                        payload = ('invalid ' + key).encode()
                    elif model == 'wire-oversize':
                        payload = (key + 'x' * (4 * 1024 * 1024 + 1)).encode()
                    else:
                        if model == 'wire-missing-finish':
                            del choice['finish_reason']
                        elif model == 'wire-null-finish':
                            choice['finish_reason'] = None
                        elif model.startswith('wire-finish-'):
                            choice['finish_reason'] = model.removeprefix('wire-finish-')
                        elif model == 'wire-refusal':
                            choice['message']['refusal'] = key
                        elif model == 'wire-tool':
                            choice['message']['tool_calls'] = [{'id':key}]
                        elif model == 'wire-unknown-content':
                            choice['message']['content'] = [{'type':'unknown','text':key}]
                        elif model == 'wire-unknown-message':
                            choice['message']['unknown'] = key
                        elif model == 'wire-empty':
                            value['choices'] = []
                        payload = json.dumps(value).encode()
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    model = ThreadingHTTPServer(('127.0.0.1', 0), Model)
    thread = threading.Thread(target=model.serve_forever, daemon=True)
    thread.start()
    with tempfile.TemporaryDirectory(prefix='qinaegis-model-protocols-') as directory:
        smoke = module.Smoke(Path(sys.argv[1]).resolve(), directory)
        smoke.secrets.append(key)
        try:
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance()
            smoke.authenticate()
            smoke.exercise_contracts()
            profile = {'id': 'native', 'label': 'Native fixture', 'adapter': 'anthropic_messages',
                'base_url': f'http://127.0.0.1:{model.server_port}/v1', 'model': 'probe', 'api_key': key,
                'capabilities': ['text', 'json'], 'enabled': True, 'allow_remote': False, 'midscene_family': None}
            registry = smoke.rpc('saveModel', {'profile': profile, 'expected_revision': 0})
            require(registry['profiles'][0]['adapter'] == 'anthropic_messages' and registry['profiles'][0]['api_key'] == '',
                    'native profile not saved/redacted')
            for cap in ('vision', 'tools'):
                invalid = {**profile, 'capabilities': ['text', cap]}
                smoke.rpc('saveModel', {'profile': invalid, 'expected_revision': registry['revision']}, success=False)
            smoke.rpc('saveModel', {'profile': {**profile, 'midscene_family': 'claude'}, 'expected_revision': registry['revision']}, success=False)
            smoke.rpc('saveModel', {'profile': {**profile, 'adapter': 'openai_compatible', 'api_key': ''}, 'expected_revision': registry['revision']}, success=False)

            def finish(job):
                deadline = time.monotonic() + 15
                while time.monotonic() < deadline:
                    status = smoke.rpc('getJobStatus', {'jobId': job['jobId']})
                    if status['status'] in ('completed', 'failed', 'stopped'):
                        require(key not in json.dumps(status), 'job exposed credentials/provider body')
                        return status
                    time.sleep(0.02)
                raise module.SmokeFailure('model protocol task timed out')

            observed = finish(smoke.rpc('probeModel', {'profile_id': 'native'}))
            require(observed['status'] == 'completed' and observed['result']['text'] is True
                    and observed['result']['json'] is True and observed['result']['vision'] is None,
                    'native text/JSON probe failed or invented vision observation')
            require(requests[-1][1]['output_config']['format']['type'] == 'json_schema', 'native structured request missing')

            for mode in ('denied', 'refusal'):
                registry = smoke.rpc('saveModel', {'profile': {**profile, 'model': mode}, 'expected_revision': registry['revision']})
                observed = finish(smoke.rpc('probeModel', {'profile_id': 'native'}))
                require(observed['result']['text'] is False and observed['result']['error'], 'rejected/incomplete output passed probe')

            registry = smoke.rpc('saveModel', {'profile': {**profile, 'model': 'cases'}, 'expected_revision': registry['revision']})
            registry = smoke.rpc('setModelRoute', {'task': 'generate', 'route': {'primary': 'native', 'fallbacks': []},
                            'expected_revision': registry['revision']})
            before = len(requests)
            native_generated = finish(smoke.rpc('runGenerate', {'project': 'smoke-beta', 'requirement': 'receipt', 'use_contracts': True}))
            require(native_generated['status'] == 'completed', 'native-primary generation/critic failed')
            native_calls = requests[before:]
            require(len(native_calls) == 2 and all(protocol == 'messages' and body.get('system') for protocol, body in native_calls),
                    'native generation lost protocol/system messages')
            native_saved = list((smoke.data / 'projects/smoke-beta/cases/draft').glob('*.json'))
            require(len(native_saved) == 1, 'native generation did not save one draft')
            native_case = json.loads(native_saved[0].read_text())
            for stage in ('generator', 'critic'):
                require([(a['profile_id'], a['outcome']) for a in native_case['provenance']['model_calls'][stage]]
                        == [('native', 'succeeded')], 'native successful call facts missing')

            registry = smoke.rpc('saveModel', {'profile': {**profile, 'model': 'quota'}, 'expected_revision': registry['revision']})
            fallback = {**profile, 'id': 'fallback', 'label': 'Approved Chat Completions fallback',
                        'adapter': 'openai_compatible', 'model': 'cases', 'capabilities': ['text']}
            registry = smoke.rpc('saveModel', {'profile': fallback, 'expected_revision': registry['revision']})
            registry = smoke.rpc('setModelRoute', {'task': 'generate', 'route': {'primary': 'native', 'fallbacks': ['fallback']},
                            'expected_revision': registry['revision']})
            before = len(requests)
            generated = finish(smoke.rpc('runGenerate', {'project': 'smoke-alpha', 'requirement': 'receipt', 'use_contracts': True}))
            require(generated['status'] == 'completed', 'approved cross-protocol generation failed')
            calls = requests[before:]
            require([protocol for protocol, _ in calls] == ['messages', 'completions', 'messages', 'completions'],
                    'generator/critic bypassed approved protocol order')
            require('receipt matches submitted input' in json.dumps(calls), 'original business assertion lost')
            saved = list((smoke.data / 'projects/smoke-alpha/cases/draft').glob('*.json'))
            require(len(saved) == 1, 'generation did not save exactly one draft')
            case = json.loads(saved[0].read_text())
            require(case['status'] == 'draft' and key not in json.dumps(case), 'generation bypassed review or leaked key')
            for stage in ('generator', 'critic'):
                attempts = case['provenance']['model_calls'][stage]
                require([(a['profile_id'], a['outcome']) for a in attempts] ==
                        [('native', 'transport_failure'), ('fallback', 'succeeded')], 'actual model attempts missing')

            # A hard rejection is terminal, even if an approved fallback exists.
            registry = smoke.rpc('saveModel', {'profile': {**profile, 'model': 'denied'}, 'expected_revision': registry['revision']})
            before = len(requests)
            failed = finish(smoke.rpc('runGenerate', {'project': 'smoke-beta', 'requirement': 'receipt', 'use_contracts': True}))
            require(failed['status'] == 'failed' and len(requests) == before + 1, 'hard rejection triggered fallback')
            require(list((smoke.data / 'projects/smoke-beta/cases/draft').glob('*.json')) == native_saved,
                    'failed generation changed existing native cases')

            # OpenAI-compatible authentication/protocol failures are terminal too.
            # Every mode embeds the key in its body; no body may enter job errors.
            primary = {**fallback, 'id':'open-primary','label':'OpenAI wire primary'}
            registry = smoke.rpc('saveModel', {'profile':primary,'expected_revision':registry['revision']})
            registry = smoke.rpc('setModelRoute', {'task':'generate','route':{'primary':'open-primary','fallbacks':['fallback']},'expected_revision':registry['revision']})
            for mode in ('wire-http-400','wire-http-401','wire-http-403','wire-http-404','wire-http-408','wire-http-422','wire-http-302',
                         'wire-malformed','wire-oversize','wire-missing-finish','wire-null-finish','wire-finish-length','wire-finish-content_filter',
                         'wire-finish-tool_calls','wire-finish-function_call','wire-finish-unknown','wire-refusal','wire-tool','wire-unknown-content','wire-unknown-message','wire-empty'):
                registry = smoke.rpc('saveModel', {'profile':{**primary,'model':mode},'expected_revision':registry['revision']})
                before = len(requests)
                observed = finish(smoke.rpc('runGenerate', {'project':'smoke-beta','requirement':'receipt','use_contracts':True}))
                require(observed['status']=='failed' and len(requests)==before+1, f'{mode} called fallback or passed generation')
                require(key not in json.dumps(observed), f'{mode} exposed response body')
            for status in (429,503):
                registry = smoke.rpc('saveModel', {'profile':{**primary,'model':f'wire-http-{status}'},'expected_revision':registry['revision']})
                project = f'wire-{status}'
                smoke.rpc('createProject', {'name':project,'url':f'https://{project}.invalid','tech_stack':[]})
                before = len(requests)
                observed = finish(smoke.rpc('runGenerate', {'project':project,'requirement':'receipt','use_contracts':False}))
                require(observed['status']=='completed' and len(requests)==before+4, f'{status} did not follow approved two-phase fallback')
                path = next((smoke.data/f'projects/{project}/cases/draft').glob('*.json'))
                provenance = json.loads(path.read_text())['provenance']
                for stage in ('generator','critic'):
                    require([(a['profile_id'],a['outcome']) for a in provenance['model_calls'][stage]] == [('open-primary','transport_failure'),('fallback','succeeded')], 'OpenAI fallback trace differs from actual requests')
                require(key not in path.read_text(), 'OpenAI fallback provenance exposed credential')
            require(not fixture_errors, 'wire fixture detected an invalid native request')
            print('PASS Messages and OpenAI complete-output checks, hard rejection without fallback, bounded approved fallback, actual call counts and redacted provenance')
        finally:
            smoke.cleanup()
            model.shutdown()
            model.server_close()


if __name__ == '__main__':
    main()
