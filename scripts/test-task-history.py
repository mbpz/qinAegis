#!/usr/bin/env python3
"""Isolated real-service crash recovery and durable task polling; local model only."""
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
loader = importlib.util.spec_from_file_location("smoke", Path(__file__).with_name("test-local-web.py"))
m = importlib.util.module_from_spec(loader)
loader.loader.exec_module(m)
require = m.require

def main():
    arrived, release = threading.Event(), threading.Event()
    calls=[]
    class Model(BaseHTTPRequestHandler):
        def log_message(self, *_): pass
        def do_POST(self):
            self.rfile.read(int(self.headers['Content-Length']))
            calls.append(True)
            arrived.set()
            release.wait(15)
            data=json.dumps({'choices':[{'message':{'role':'assistant','content':'{"ok":true}'}}]}).encode()
            try:
                self.send_response(200);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
            except (BrokenPipeError,ConnectionResetError): pass
    server=ThreadingHTTPServer(('127.0.0.1',0),Model)
    threading.Thread(target=server.serve_forever,daemon=True).start()
    with tempfile.TemporaryDirectory(prefix='qinaegis-task-history-') as root:
        smoke=m.Smoke(Path(sys.argv[1]).resolve(),root)
        try:
            smoke.cli('start','--no-open','--port',str(smoke.port))
            smoke.instance=smoke.read_instance();smoke.authenticate()
            profile={'id':'local','label':'Local fixture','adapter':'openai_compatible','base_url':f'http://127.0.0.1:{server.server_port}/v1','model':'fixture','api_key':'private-fixture-key','capabilities':['text','json'],'enabled':True,'allow_remote':False,'midscene_family':None}
            smoke.rpc('saveModel',{'profile':profile,'expected_revision':0})
            job=smoke.rpc('probeModel',{'profile_id':'local'})['jobId']
            require(arrived.wait(5),'local model request not reached')
            history=smoke.data/'jobs/history.json'
            require(any(r['id']==job and r['status']=='running' for r in json.loads(history.read_text())['records']),'running not saved before worker')
            require(job in [j['jobId'] for j in smoke.rpc('getJobs')['jobs']],'task list omitted active worker')
            # PID comes from this freshly started, owner-authenticated isolated instance.
            os.kill(smoke.instance['pid'],signal.SIGTERM if os.name=='nt' else signal.SIGKILL)
            time.sleep(.5)
            smoke.cli('start','--no-open','--port',str(smoke.port))
            smoke.instance=smoke.read_instance();smoke.authenticate()
            recovered=smoke.rpc('getJobStatus',{'jobId':job})
            require(recovered['status']=='interrupted','unfinished task not recovered as interrupted')
            require(smoke.rpc('getJobLog',{'jobId':job,'cursor':0})['cursor']==0,'empty recovered cursor invalid')
            require(len(calls)==1,'interrupted operation was replayed')
            release.set()
            next_job=smoke.rpc('probeModel',{'profile_id':'local'})['jobId']
            require(next_job!=job,'task identity reused after restart')
            deadline=time.monotonic()+15
            while time.monotonic()<deadline:
                state=smoke.rpc('getJobStatus',{'jobId':next_job})
                if state['status'] in ('completed','failed','stopped'):
                    break
                time.sleep(.05)
            require(state['status']=='completed','probe fixture did not finish')
            # User-visible status can precede final worker cleanup; wait for durable terminal checkpoint.
            deadline=time.monotonic()+5
            while time.monotonic()<deadline:
                records=json.loads(history.read_text())['records']
                if any(r['id']==next_job and r['status']=='completed' for r in records):break
                time.sleep(.05)
            require(any(r['id']==next_job and r['status']=='completed' for r in records),'terminal task not durable')
            require('private-fixture-key' not in history.read_text(),'model key leaked into task history')
            smoke.cli('stop');smoke.cli('start','--no-open','--port',str(smoke.port))
            smoke.instance=smoke.read_instance();smoke.authenticate()
            require(smoke.rpc('getJobStatus',{'jobId':next_job})['status']=='completed','terminal state lost on restart')
            release.clear();arrived.clear()
            cancelled=smoke.rpc('probeModel',{'profile_id':'local'})['jobId']
            require(arrived.wait(5),'cancel fixture never entered provider')
            smoke.rpc('stopJob',{'jobId':cancelled})
            deadline=time.monotonic()+5
            while time.monotonic()<deadline:
                state=smoke.rpc('getJobStatus',{'jobId':cancelled})
                if state['status']=='stopped':break
                time.sleep(.05)
            require(state['status']=='stopped','cancelled maintenance worker did not exit')
            release.set()
            deadline=time.monotonic()+5
            while time.monotonic()<deadline:
                if any(r['id']==cancelled and r['status']=='stopped' for r in json.loads(history.read_text())['records']):break
                time.sleep(.05)
            require(any(r['id']==cancelled and r['status']=='stopped' for r in json.loads(history.read_text())['records']),'cancel terminal not persisted')
            smoke.cli('stop')
            history.write_text('corrupt-history-fixture')
            smoke.cli('start','--no-open','--port',str(smoke.port),success=False)
            require(history.read_text()=='corrupt-history-fixture','startup overwrote damaged history')
            print('PASS durable admission, crash interrupted recovery, no replay, random IDs, terminal restart, cancellation, corrupt-history refusal and key redaction')
        finally:
            release.set();smoke.cleanup();server.shutdown();server.server_close()
if __name__=='__main__':main()
