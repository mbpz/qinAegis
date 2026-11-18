#!/usr/bin/env python3
"""Real isolated service acceptance for legacy generation; loopback models only."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

loader = importlib.util.spec_from_file_location('local_smoke', Path(__file__).with_name('test-local-web.py'))
m = importlib.util.module_from_spec(loader)
loader.loader.exec_module(m)
require = m.require


def wait(predicate, message, seconds=10):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(.025)
    raise m.SmokeFailure(message)


def main():
    primary_key = 'legacy-generation-private-primary'
    fallback_key = 'legacy-generation-private-secondary'
    replacement_key = 'legacy-generation-private-replacement'
    calls = []
    arrived, release, returned = threading.Event(), threading.Event(), threading.Event()

    class Model(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_POST(self):
            request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            mode = request['model']
            prompt = json.dumps(request.get('messages'))
            generator = 'yaml_script' in prompt and 'requirement_id' in prompt
            calls.append({'model':mode, 'generator':generator, 'auth':self.headers.get('Authorization'),
                          'max_tokens':request.get('max_tokens'), 'path':self.path})
            status = 429 if mode == 'quota' else 503 if mode == 'unavailable' else 401 if mode == 'denied' or mode == 'critic-denied' and not generator else 200
            if status != 200:
                payload = {'error':{'message':primary_key}}
            else:
                if mode == 'blocked' and generator:
                    arrived.set()
                    release.wait(20)
                    returned.set()
                if generator:
                    cases = [{'id':'receipt', 'name':'Receipt', 'requirement_id':'receipt', 'type':'smoke',
                              'priority':'P1', 'yaml_script':'tasks: []', 'expected_result':'receipt matches input', 'tags':[]}]
                    if mode == 'echo':
                        cases[0]['name'] = primary_key
                    if mode in ('echo-secondary', 'echo-escaped'):
                        cases[0]['yaml_script'] = 'tasks: [] # ' + fallback_key
                    content = 'invalid generated document' if mode == 'invalid-document' else json.dumps(cases)
                    if mode == 'echo-escaped':
                        content = content.replace(fallback_key, ''.join('\\u%04x' % ord(c) for c in fallback_key))
                else:
                    content = 'invalid critic document' if mode == 'critic-invalid' else json.dumps({'score':90, 'issues':[], 'suggestions':[], 'coverage':'receipt'})
                message = {'role':'assistant', 'content':content}
                if mode == 'refusal':
                    message['refusal'] = primary_key
                choice = {'message':message}
                if mode != 'missing-finish':
                    choice['finish_reason'] = 'length' if mode == 'length' else 'stop'
                payload = {'choices':[choice]}
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
    with tempfile.TemporaryDirectory(prefix='qinaegis-legacy-generation-') as directory:
        smoke = m.Smoke(Path(sys.argv[1]).resolve(), directory)
        smoke.env.update({'QINAEGIS_LEGACY_PRIMARY_KEY':primary_key,
                          'QINAEGIS_LEGACY_ENDPOINT':f'http://127.0.0.1:{server.server_port}/v1'})
        smoke.secrets.extend([primary_key, fallback_key, replacement_key])
        history = smoke.data / 'jobs/history.json'
        finished = []
        try:
            def start():
                smoke.cli('start', '--no-open', '--port', str(smoke.port))
                smoke.instance = smoke.read_instance()
                smoke.authenticate()
            start()

            def configure(mode, secondary=True, key='$QINAEGIS_LEGACY_PRIMARY_KEY'):
                # Legacy provider labels historically select Chat Completions wire.
                # They must not be advertised as native Messages capability.
                smoke.rpc('setConfig', {'llm':{'provider':'legacy-provider-label',
                    'base_url':'$QINAEGIS_LEGACY_ENDPOINT', 'api_key':key, 'model':mode,
                    'secondary_provider':'legacy-secondary-label',
                    'secondary_base_url':f'http://127.0.0.1:{server.server_port}/v1',
                    'secondary_api_key':fallback_key if secondary else '',
                    'clear_secondary_api_key':not secondary,
                    'secondary_model':'normal' if secondary else ''},
                    'expected_revision':smoke.rpc('getState')['config_revision']})

            def submit(project):
                smoke.rpc('createProject', {'name':project,'url':f'https://{project}.invalid','tech_stack':[]})
                return smoke.rpc('runGenerate', {'project':project,'requirement':'receipt','use_contracts':False})['jobId']

            def finish(job):
                def terminal():
                    state = smoke.rpc('getJobStatus', {'jobId':job})
                    return state if state['status'] in ('completed','failed','stopped','interrupted') else None
                state = wait(terminal, 'legacy generation did not terminate')
                require(isinstance(state.get('result'), dict) and 'attempts' in state['result'],
                        'legacy generation omitted actual attempt evidence')
                require(state['result']['source']=='legacy_config' and not state['result'].get('truncated'),
                        'legacy evidence has incorrect source or was truncated')
                finished.append((job, state))
                return state

            def outcomes(state):
                return [row[3] for row in state['result']['attempts']]

            for mode, secondary, expected, trace in [
                ('normal',False,'completed',['succeeded','succeeded']),
                ('quota',True,'completed',['rate_limited','succeeded','rate_limited','succeeded']),
                ('unavailable',True,'completed',['service_unavailable','succeeded','service_unavailable','succeeded']),
                ('denied',True,'failed',['rejected']),
                ('refusal',True,'failed',['rejected']),
                ('length',True,'failed',['rejected']),
                ('missing-finish',True,'failed',['rejected']),
                ('invalid-document',True,'failed',['succeeded']),
                ('critic-denied',True,'completed',['succeeded','rejected']),
                ('critic-invalid',True,'completed',['succeeded','succeeded']),
            ]:
                configure(mode, secondary)
                before = len(calls)
                state = finish(submit('legacy-' + mode))
                require(state['status']==expected and outcomes(state)==trace,
                        'legacy model outcome or application state is inaccurate')
                require(len(calls)==before+len(trace), 'legacy request secretly retried or fell back')
                candidates = state['snapshot']['model']['candidates']
                require([c['id'] for c in candidates]==(['legacy-primary','legacy-secondary'] if secondary else ['legacy-primary']),
                        'legacy frozen candidate identities inaccurate')
                require(all(c['adapter']=='openai_compatible' for c in candidates), 'legacy provider label misrepresented actual wire adapter')
                require(all(c['max_tokens']==1024 and c['path']=='/v1/chat/completions' for c in calls[before:]),
                        'legacy token default or actual protocol drifted')
                for call, attempt in zip(calls[before:], state['result']['attempts']):
                    require(call['auth']=='Bearer '+(primary_key if attempt[2]==0 else fallback_key), 'attempt identifies a different credential/candidate')
                if mode.startswith('critic-'):
                    require(state['result']['application_outcome']=='completed_with_unverified_reviews', 'unverified legacy critic claimed verification')
                if expected=='completed':
                    case = json.loads(next((smoke.data/f'projects/legacy-{mode}/cases/draft').glob('*.json')).read_text())
                    model_calls = case['provenance']['model_calls']
                    require(model_calls['generator'] and model_calls['critic'], 'legacy saved source omitted actual generation/critic calls')

            for mode in ('echo','echo-secondary','echo-escaped'):
                configure(mode)
                before = len(calls)
                state = finish(submit('legacy-' + mode))
                require(state['status']=='failed' and 'configured credentials' in state['error'], 'legacy credential echo was accepted')
                require(len(calls)==before+1 and not list((smoke.data/f'projects/legacy-{mode}/cases').rglob('*.json')),
                        'legacy credential echo activated fallback or wrote a case')

            # Hold the single execution slot while the second job is admitted.
            configure('blocked')
            first = submit('legacy-blocking')
            require(arrived.wait(5), 'legacy blocking fixture not reached')
            configure('normal')
            queued = submit('legacy-queued')
            queued_state = smoke.rpc('getJobStatus', {'jobId':queued})
            require(queued_state['status']=='queued', 'legacy job was not queued behind owned work')
            before = len(calls)
            configure('replacement', secondary=False, key=replacement_key)
            release.set()
            finish(first)
            state = finish(queued)
            require(state['status']=='completed' and state['snapshot']==queued_state['snapshot'], 'queued legacy inputs changed after config edit')
            queued_calls = calls[before:]
            require(all(c['model'] in ('blocked','normal') and c['auth']=='Bearer '+primary_key for c in queued_calls),
                    'queued legacy generation resolved credentials or model after admission')
            require(sum(c['model']=='normal' for c in queued_calls)==2, 'queued legacy generator/critic did not use admitted model')

            arrived.clear(); release.clear(); returned.clear()
            configure('blocked')
            job = submit('legacy-cancelled')
            require(arrived.wait(5), 'legacy cancelled fixture not reached')
            durable = next(r for r in json.loads(history.read_text())['records'] if r['id']==job)
            require(durable['result']['attempts']==[[1,1,0,'started']], 'legacy request preceded durable intent')
            smoke.rpc('stopJob', {'jobId':job})
            state = finish(job)
            require(state['status']=='stopped' and outcomes(state)==['cancelled'], 'legacy cancellation lost actual attempt')
            release.set()
            require(returned.wait(5), 'cancelled loopback handler did not finish')
            wait(lambda: not any(t['status']=='cancelling' for t in smoke.rpc('getJobs')['jobs']), 'legacy cancel cleanup did not finish')

            arrived.clear(); release.clear(); returned.clear()
            job = submit('legacy-interrupted')
            require(arrived.wait(5), 'legacy interrupted fixture not reached')
            before = len(calls)
            durable = next(r for r in json.loads(history.read_text())['records'] if r['id']==job)
            require(durable['result']['attempts']==[[1,1,0,'started']], 'legacy crash intent not durable')
            os.kill(smoke.instance['pid'], signal.SIGTERM if os.name=='nt' else signal.SIGKILL)
            time.sleep(.5)
            start()
            state = smoke.rpc('getJobStatus', {'jobId':job})
            require(state['status']=='interrupted' and state['result']['attempts']==[[1,1,0,'started']], 'legacy recovery invented completion')
            require(len(calls)==before, 'legacy recovery replayed a request')
            release.set()
            for job, previous in finished:
                current = smoke.rpc('getJobStatus', {'jobId':job})
                require(current['result']==previous['result'] and current['status']==previous['status'], 'restart altered terminal legacy evidence')
            for root in (smoke.data/'projects', smoke.data/'jobs', smoke.state):
                for path in root.rglob('*'):
                    if path.is_file() and path.suffix in ('.json','.log','.yaml'):
                        require(all(secret.encode() not in path.read_bytes() for secret in smoke.secrets), 'legacy credential leaked into asset/evidence/log')
            print('PASS legacy actual generation/critic/fallback, hard rejection without fallback, token/wire identity, queued freeze, secret rejection, cancel and interrupted no-replay (loopback only)')
        finally:
            release.set()
            smoke.cleanup()
            server.shutdown()
            server.server_close()


if __name__ == '__main__':
    main()
