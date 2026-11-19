#!/usr/bin/env python3
"""Isolated real HTTP generation evidence; deterministic loopback models only."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler

loader = importlib.util.spec_from_file_location('smoke', Path(__file__).with_name('test-local-web.py'))
m = importlib.util.module_from_spec(loader)
loader.loader.exec_module(m)
require = m.require


def wait(predicate, message, seconds=10):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(.025)
    raise m.SmokeFailure(message)


def main():
    calls = []
    arrived, release = threading.Event(), threading.Event()
    primary_key, fallback_key = 'generation-private-primary-key', 'generation-private-fallback-key'

    class Model(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_POST(self):
            request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            mode = request['model']
            prompt = json.dumps(request['messages'])
            generator = 'yaml_script' in prompt and 'requirement_id' in prompt
            calls.append((mode, generator))
            status = 200
            if mode == 'quota' or (mode == 'denied' or mode == 'critic-denied' and not generator):
                status = 429 if mode == 'quota' else 401
                text = primary_key
            else:
                if mode == 'blocked' and generator:
                    arrived.set()
                    release.wait(15)
                if generator:
                    cases = [{'id':'receipt', 'name':'Receipt', 'requirement_id':'receipt', 'type':'smoke',
                              'priority':'P1', 'yaml_script':'tasks: []', 'expected_result':'receipt matches input', 'tags':[]}]
                    if mode == 'echo-name':
                        cases[0]['name'] = primary_key
                    elif mode == 'echo-yaml':
                        cases[0]['yaml_script'] = 'tasks: [] # ' + fallback_key
                    elif mode in ('echo-later', 'echo-escaped'):
                        cases.append({**cases[0], 'id':'later', 'name':fallback_key})
                    text = 'invalid generated document' if mode == 'invalid-document' else json.dumps(cases)
                    if mode == 'echo-escaped':
                        text = text.replace(fallback_key, ''.join('\\u%04x' % ord(c) for c in fallback_key))
                else:
                    review = {'score':0 if mode == 'valid-zero' else 90, 'issues':[], 'suggestions':[], 'coverage':'receipt'}
                    if mode == 'echo-critic':
                        review['issues'] = [primary_key]
                    text = 'invalid critic document' if mode == 'critic-invalid' else json.dumps(review)
            payload = json.dumps({'error':{'message':text}} if status != 200 else
                                 {'choices':[{'finish_reason':'stop','message':{'role':'assistant','content':text}}]}).encode()
            try:
                self.send_response(status)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)
            except (BrokenPipeError, ConnectionResetError):
                pass

    server = m.LoopbackHTTPServer(('127.0.0.1', 0), Model)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    with tempfile.TemporaryDirectory(prefix='qinaegis-generation-evidence-') as directory:
        smoke = m.Smoke(Path(sys.argv[1]).resolve(), directory)
        smoke.secrets.extend([primary_key, fallback_key])
        try:
            def start():
                smoke.cli('start', '--no-open', '--port', str(smoke.port))
                smoke.instance = smoke.read_instance()
                smoke.authenticate()
            start()
            profile = {'id':'primary','label':'Primary fixture','adapter':'openai_compatible',
                       'base_url':f'http://127.0.0.1:{server.server_port}/v1', 'model':'normal', 'api_key':primary_key,
                       'capabilities':['text'], 'enabled':True, 'allow_remote':False, 'midscene_family':None}
            revision = smoke.rpc('saveModel', {'profile':profile,'expected_revision':0})['revision']
            fallback = {**profile, 'id':'fallback','label':'Approved fixture','api_key':fallback_key}
            revision = smoke.rpc('saveModel', {'profile':fallback,'expected_revision':revision})['revision']
            revision = smoke.rpc('setModelRoute', {'task':'generate','route':{'primary':'primary','fallbacks':['fallback']},'expected_revision':revision})['revision']
            history = smoke.data / 'jobs/history.json'
            finished = []

            def submit(project, mode):
                nonlocal revision
                smoke.rpc('createProject', {'name':project,'url':f'https://{project}.invalid','tech_stack':[]})
                revision = smoke.rpc('saveModel', {'profile':{**profile,'model':mode},'expected_revision':revision})['revision']
                return smoke.rpc('runGenerate', {'project':project,'requirement':'receipt','use_contracts':False})['jobId']

            def finish(job):
                def terminal():
                    state = smoke.rpc('getJobStatus', {'jobId':job})
                    return state if state['status'] in ('completed','failed','stopped','interrupted') else None
                state = wait(terminal, 'generation did not terminate')
                require('attempts' in state['result'], 'generation omitted actual attempt evidence')
                require(not state['result'].get('truncated'), 'generation evidence was silently truncated')
                finished.append((job, state))
                return state

            def outcomes(state):
                return [row[3] for row in state['result']['attempts']]

            before = len(calls)
            state = finish(submit('fallback-success', 'quota'))
            require(state['status']=='completed' and len(calls)==before+4, 'approved generation fallback did not execute twice per phase')
            require(outcomes(state)==['rate_limited','succeeded','rate_limited','succeeded'], 'fallback reasons lost')
            require([row[2] for row in state['result']['attempts']]==[0,1,0,1], 'attempt candidate identity drifted')
            for mode, expected, trace in [('denied','failed',['rejected']),('invalid-document','failed',['succeeded']),
                                          ('critic-denied','completed',['succeeded','rejected']),
                                          ('critic-invalid','completed',['succeeded','succeeded']),('valid-zero','completed',['succeeded','succeeded'])]:
                before = len(calls)
                state = finish(submit(mode, mode))
                require(state['status']==expected and outcomes(state)==trace, 'application and model failure facts conflated')
                require(len(calls)==before+len(trace), 'invalid output activated fallback')
                if mode.startswith('critic-'):
                    require(state['result']['application_outcome']=='completed_with_unverified_reviews', 'unverified critic was shown as verified')
                if mode=='valid-zero':
                    require(state['result']['application_outcome']=='completed', 'valid zero score was mistaken for critic failure')

            for mode in ('echo-name','echo-yaml','echo-later','echo-escaped','echo-critic'):
                before = len(calls)
                state = finish(submit(mode, mode))
                require(state['status']=='failed' and 'configured credentials' in state['error'], 'credential echo was accepted')
                require(len(calls)==before+(2 if mode=='echo-critic' else 1), 'credential echo activated fallback')
                require(not list((smoke.data/f'projects/{mode}/cases').rglob('*.json')), 'credential echo wrote a generated asset')

            job = submit('cancelled-generation', 'blocked')
            require(arrived.wait(5), 'cancelled model request did not reach loopback provider')
            persisted = next(record for record in json.loads(history.read_text())['records'] if record['id']==job)
            require(persisted['result']['attempts']==[[1,1,0,'started']], 'request began before durable attempt intent')
            smoke.rpc('stopJob', {'jobId':job})
            state = finish(job)
            require(state['status']=='stopped' and outcomes(state)==['cancelled'] and state['result']['application_outcome']=='cancelled', 'cancelled actual candidate was lost')
            release.set()

            # Kill only this freshly launched, authenticated isolated instance.
            arrived.clear(); release.clear()
            job = submit('interrupted-generation', 'blocked')
            require(arrived.wait(5), 'crash fixture did not reach provider')
            before = len(calls)
            persisted = next(record for record in json.loads(history.read_text())['records'] if record['id']==job)
            require(persisted['result']['attempts']==[[1,1,0,'started']], 'crash intent not durable')
            os.kill(smoke.instance['pid'], signal.SIGTERM if os.name=='nt' else signal.SIGKILL)
            time.sleep(.5)
            start()
            recovered = smoke.rpc('getJobStatus', {'jobId':job})
            require(recovered['status']=='interrupted' and recovered['result']['attempts']==[[1,1,0,'started']], 'crash invented a completed provider result')
            require(len(calls)==before, 'interrupted generation replayed provider request')
            release.set()
            for job, previous in finished:
                current = smoke.rpc('getJobStatus', {'jobId':job})
                require(current['result']==previous['result'] and current['status']==previous['status'], 'restart changed terminal generation evidence')
            for root in (smoke.data/'projects', smoke.data/'jobs', smoke.state):
                for path in root.rglob('*'):
                    if path.is_file() and path.suffix in ('.json','.log','.yaml'):
                        payload = path.read_bytes()
                        require(all(secret.encode() not in payload for secret in smoke.secrets), 'credential found in generated assets, task facts or service logs')
            print('PASS actual generation failure/fallback/critic/decoded-secret evidence, durable before-request intent, cancellation and crash recovery without replay (loopback only)')
        finally:
            release.set()
            smoke.cleanup()
            server.shutdown()
            server.server_close()


if __name__ == '__main__':
    main()
