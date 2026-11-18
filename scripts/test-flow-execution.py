#!/usr/bin/env python3
"""Schema-2 real HTTP governance, waits, queue and deterministic engine fixtures.

Local protocol fixtures verify execution plumbing; they are not real visual AI
or real project acceptance. Every server and model has an isolated local root.
"""
import copy
import importlib.util
import json
import shutil
import threading
from http.server import BaseHTTPRequestHandler
from pathlib import Path
import sys
import tempfile
import time

loader = importlib.util.spec_from_file_location('local_web_smoke', Path(__file__).with_name('test-local-web.py'))
m = importlib.util.module_from_spec(loader)
loader.loader.exec_module(m)
require = m.require

class FlowSmoke(m.Smoke):
    def rpc(self, method, params=None, success=True):
        if not success or method not in ('getFlowExecution', 'getExecutionFlows', 'getJobStatus', 'getJobs', 'getRunEvidence'):
            return super().rpc(method, params, success)
        # A read may briefly share an AssetFile with a worker checkpoint. Only
        # this explicit transient read error is retried; actions are never replayed.
        deadline = time.monotonic() + 2
        while True:
            self.rpc_sequence += 1
            request_id = f'flow-read-{self.rpc_sequence}'
            response, _ = self.json_request('POST', '/api/v2/rpc',
                body={'id':request_id, 'method':method, 'params':params or {}}, headers=self.authorized)
            require(response.get('id') == request_id, 'read response identity differs')
            if response.get('ok') is True:
                require('data' in response, f'RPC {method} omitted data')
                return response['data']
            if 'asset library is busy' in json.dumps(response.get('error', {})) and time.monotonic() < deadline:
                time.sleep(.025)
                continue
            raise m.SmokeFailure(f'RPC {method} returned unexpected read result: {response.get("error")}')


def wait(predicate, message):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(.025)
    raise m.SmokeFailure(message)


def draft(name='wait-flow', milliseconds=25):
    return {'definition': {'schema_version': 2, 'id': name, 'title': 'Reviewed local wait',
        'project': 'smoke-alpha', 'inputs': {'enabled': 'boolean'},
        'outputs': {'elapsed': {'value_type': 'number', 'source': {'kind': 'step_output', 'step': 'pause', 'port': 'elapsed_ms'}}},
        'required_permissions': [], 'steps': [{'id': 'pause', 'depends_on': [],
            'operation': {'capability': 'wait.v1', 'milliseconds': {'kind': 'literal', 'value': milliseconds}},
            'condition': {'operator': 'equals', 'source': {'kind': 'flow_input', 'name': 'enabled'}, 'value_type': 'boolean', 'value': True},
            'timeout_ms': milliseconds + 1000, 'max_attempts': 1}],
        'total_timeout_ms': milliseconds + 2000, 'cleanup_timeout_ms': 500}, 'model_profiles': {}}


ENGINE = r"""
import fs from 'node:fs';
import readline from 'node:readline';
import path from 'node:path';
const root=process.env.QINAEGIS_FIXTURE_DIR;
for await (const line of readline.createInterface({input:process.stdin})) {
 const req=JSON.parse(line);
 if(req.method!=='run_yaml') throw Error('fixture supports run_yaml only');
 fs.appendFileSync(path.join(root,'requests.jsonl'),JSON.stringify({args:req.args,model:process.env.MIDSCENE_MODEL_NAME})+'\n');
 const failed=fs.existsSync(path.join(root,'failed'));
 if(fs.existsSync(path.join(root,'cleanup-damage'))) {
  fs.writeFileSync(path.join(process.cwd(),'.qinaegis-owner'),'damaged-fixture');
  fs.writeFileSync(path.join(root,'owned-workspace'),process.cwd());
  // Hold the damaged workspace until the HTTP harness confirms that the
  // next action is durably queued. A time delay cannot establish admission.
  const release=path.join(root,'cleanup-release');
  const deadline=Date.now()+8000;
  while(!fs.existsSync(release)) {
   if(Date.now()>=deadline) throw Error('cleanup fixture queue-admission handshake timed out');
   await new Promise(resolve=>setTimeout(resolve,20));
  }
 }
 console.log(JSON.stringify({id:'0',ok:true,data:{case_id:req.args.case_id,passed:!failed,duration_ms:12,screenshot_base64:null,error_message:'fixture echo '+process.env.MIDSCENE_MODEL_API_KEY,retry_count:0,healed_yaml_script:null,failed_step:failed?'fixture assertion':null}}));
}
"""

def persistence_failure(binary):
    """The terminal write AND drop fallback fail, despite confirmed wait cleanup."""
    with tempfile.TemporaryDirectory(prefix='qinaegis-flow-write-fault-') as directory:
        smoke = FlowSmoke(binary, directory)
        path = None
        saved = None
        try:
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance(); smoke.authenticate()
            smoke.rpc('createProject', {'name':'smoke-alpha','url':'https://smoke-alpha.invalid','tech_stack':[]})
            value = draft('write-fault', 1500)
            lib = smoke.rpc('saveExecutionFlowDraft', {'draft':value,'expected_revision':0})
            smoke.rpc('publishExecutionFlow', {'id':'write-fault','expected_revision':lib['revision'],'approved':True})
            def admitted():
                current = smoke.rpc('getExecutionFlows')
                prep = smoke.rpc('prepareFlowExecution', {'id':'write-fault','version':1,'expected_revision':current['library']['revision'],'expected_model_revision':current['model_revision'],'inputs':{'enabled':True},'permissions':[],'approved':True})
                return smoke.rpc('runFlowExecution', {'preparation_id':prep['preparation_id'],'approved':True})
            first = admitted()
            wait(lambda:smoke.rpc('getFlowExecution',{'run_id':first['run_id']})['status']=='running','fault wait did not begin')
            second = admitted()
            path = smoke.data/'flow-executions/runs'/f'{first["run_id"]}.json'
            saved = path.read_bytes()
            # Replace only this isolated record with a directory. AssetFile rejects
            # every checkpoint/final write and fallback read; unrelated JobStore works.
            path.unlink(); path.mkdir()
            def status(job):
                value = smoke.rpc('getJobStatus', {'jobId':job['jobId']})
                return value if value['status'] in ('completed','failed','stopped','interrupted') else None
            require(wait(lambda:status(first),'fault job did not finish')['status']=='failed','terminal persistence failure hidden')
            require(wait(lambda:status(second),'queued job not blocked')['status']=='failed','queue released after both persistence paths failed')
            marker = smoke.data/'flow-executions/resource-quarantine.json'
            require('persistence' in json.loads(marker.read_text())['reason'],'missing durable persistence block reason')
            queued = smoke.rpc('getFlowExecution',{'run_id':second['run_id']})
            require(queued['status']=='interrupted' and queued['facts'] is None,'side effect started after terminal evidence uncertainty')
            path.rmdir(); path.write_bytes(saved); saved = None
            smoke.cli('stop')
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance(); smoke.authenticate()
            recovered = smoke.rpc('getFlowExecution', {'run_id':first['run_id']})
            require(recovered['status']=='interrupted','failed write was replayed on recovery')
            current = smoke.rpc('getExecutionFlows')
            smoke.rpc('prepareFlowExecution', {'id':'write-fault','version':1,'expected_revision':current['library']['revision'],'expected_model_revision':current['model_revision'],'inputs':{'enabled':True},'permissions':[],'approved':True},success=False)
            print('PASS real HTTP terminal + fallback persistence failure blocks queued work and survives restart')
        finally:
            if saved is not None and path is not None and path.is_dir():
                path.rmdir(); path.write_bytes(saved)
            smoke.cleanup()

def main():
    binary = Path(sys.argv[1]).resolve()
    persistence_failure(binary)
    require(shutil.which('node') is not None, 'Node is required for the explicitly labeled protocol fixture')
    calls=[]
    class Model(BaseHTTPRequestHandler):
        def log_message(self,*_): pass
        def do_POST(self):
            calls.append(json.loads(self.rfile.read(int(self.headers['Content-Length']))))
            content=([{'id':'flow-case','name':'Receipt','requirement_id':'receipt','type':'smoke','priority':'P1','yaml_script':'tasks: []','expected_result':'receipt matches submitted input','tags':[]}]
                if len(calls)%2 else {'score':90,'issues':[],'suggestions':[],'coverage':'receipt'})
            payload=json.dumps({'choices':[{'finish_reason':'stop','message':{'role':'assistant','content':json.dumps(content)}}]}).encode()
            self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(payload)));self.end_headers();self.wfile.write(payload)
    model=m.LoopbackHTTPServer(('127.0.0.1',0),Model)
    threading.Thread(target=model.serve_forever,daemon=True).start()
    with tempfile.TemporaryDirectory(prefix='qinaegis-flow-exec-') as directory:
        smoke = FlowSmoke(binary, directory)
        def start():
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance()
            smoke.authenticate()
        def terminal(job):
            state = smoke.rpc('getJobStatus', {'jobId': job['jobId']})
            return state if state['status'] in ('completed', 'failed', 'stopped', 'interrupted') else None
        def finish(job, status='completed'):
            state = wait(lambda: terminal(job), 'flow job timeout')
            require(state['status'] == status, f'expected flow job {status}, received {state["status"]}')
            return smoke.rpc('getFlowExecution', {'run_id': job['run_id']})
        def publish(value):
            rev = smoke.rpc('getExecutionFlows')['library']['revision']
            lib = smoke.rpc('saveExecutionFlowDraft', {'draft': value, 'expected_revision': rev})
            return smoke.rpc('publishExecutionFlow', {'id': value['definition']['id'], 'expected_revision': lib['revision'], 'approved': True})
        def prepare(value, inputs=None):
            current = smoke.rpc('getExecutionFlows')
            version = max(p['version'] for p in current['library']['publications'] if p['definition']['id'] == value['definition']['id'])
            return smoke.rpc('prepareFlowExecution', {'id': value['definition']['id'], 'version': version,
                'expected_revision': current['library']['revision'], 'expected_model_revision': current['model_revision'],
                'inputs': inputs or {'enabled': True}, 'permissions': value['definition']['required_permissions'], 'approved': True})
        def run(prep):
            return smoke.rpc('runFlowExecution', {'preparation_id': prep['preparation_id'], 'approved': True})
        try:
            start()
            smoke.exercise_contracts()
            require(smoke.rpc('getExecutionFlows')['library']['revision'] == 0, 'initial schema-2 library not empty')
            smoke.rpc('getExecutionFlows', {'path': '/etc/passwd'}, success=False)
            value = draft()
            require(smoke.rpc('validateExecutionFlowDraft', {'draft': value})['execution_order'] == ['pause'], 'wait order invalid')
            malformed = copy.deepcopy(value)
            malformed['model_profiles'] = {'pause': 'unneeded'}
            smoke.rpc('validateExecutionFlowDraft', {'draft': malformed}, success=False)
            lib = publish(value)
            smoke.rpc('saveExecutionFlowDraft', {'draft': value, 'expected_revision': 0}, success=False)
            require(lib['publications'][0]['reviewer'] == 'local-owner', 'publication omitted review')
            prep = prepare(value)
            require(prep['snapshot']['publication']['definition'] == value['definition'] and prep['snapshot']['inputs'] == {'enabled': True}, 'review snapshot incomplete')
            smoke.rpc('runFlowExecution', {'preparation_id': prep['preparation_id'], 'approved': False}, success=False)
            job = run(prep)
            evidence = finish(job)
            require(evidence['job_id'] == job['jobId'] and evidence['status'] == 'completed', 'job binding or terminal evidence missing')
            require(evidence['facts']['outputs']['elapsed'] >= 25 and evidence['facts']['cleanup_error'] is None, 'real wait or cleanup evidence missing')
            smoke.rpc('runFlowExecution', {'preparation_id': prep['preparation_id'], 'approved': True}, success=False)
            require(not smoke.rpc('getExecutionFlows')['preparations'], 'consumed preparation retained')
            path = smoke.data / 'flow-executions/runs' / (job['run_id'] + '.json')
            require(path.exists() and json.loads(path.read_text()) == evidence, 'private execution not committed')
            if sys.platform != 'win32': require(path.stat().st_mode & 0o777 == 0o600, 'private execution mode incorrect')
            false_job = run(prepare(value, {'enabled': False}))
            skipped = finish(false_job, 'failed')
            require(skipped['facts']['steps'][0]['status'] == 'skipped' and not skipped['facts']['outputs'], 'skipped output fabricated success')
            stale = prepare(value)
            changed = copy.deepcopy(value)
            changed['definition']['title'] = 'Later reviewed version'
            newer = publish(changed)
            smoke.rpc('runFlowExecution', {'preparation_id': stale['preparation_id'], 'approved': True}, success=False)
            require(newer['publications'][0]['definition'] == value['definition'], 'publication mutated')
            long_value = draft('long-wait', 1500)
            publish(long_value)
            first = run(prepare(long_value))
            wait(lambda: smoke.rpc('getFlowExecution', {'run_id': first['run_id']})['status'] == 'running', 'wait checkpoint not observed')
            queued = run(prepare(long_value))
            require(smoke.rpc('getJobStatus', {'jobId': queued['jobId']})['status'] == 'queued', 'flow bypassed shared queue')
            smoke.rpc('stopJob', {'jobId': first['jobId']})
            cancelled = finish(first, 'stopped')
            require(cancelled['facts']['status'] == 'cancelled' and cancelled['facts']['cleanup_error'] is None, 'cancel facts or cleanup missing')
            finish(queued)
            # Actual reviewed-case admission uses a deterministic local model and engine protocol.
            revision=0
            profiles=[]
            for name in ('one','two'):
                profile={'id':name,'label':name,'adapter':'openai_compatible','base_url':f'http://127.0.0.1:{model.server_port}/v1','model':f'model-{name}-before','api_key':f'flow-secret-{name}-private','capabilities':['text','vision'],'enabled':True,'allow_remote':False,'midscene_family':'qwen2.5-vl'}
                smoke.secrets.append(profile['api_key']);profiles.append(profile)
                revision=smoke.rpc('saveModel',{'profile':profile,'expected_revision':revision})['revision']
            revision=smoke.rpc('setModelRoute',{'task':'generate','route':{'primary':'one','fallbacks':[]},'expected_revision':revision})['revision']
            generation=smoke.rpc('runGenerate',{'project':'smoke-alpha','requirement':'receipt','use_contracts':True})
            generated=wait(lambda:terminal(generation),'generation fixture timeout')
            require(generated['status']=='completed','generation fixture failed')
            case_id='flow-case'
            for status in ('reviewed','approved'):
                smoke.rpc('updateCaseStatus',{'project':'smoke-alpha','case_id':case_id,'status':status,'expected_digest':smoke.rpc('getCaseAsset',{'project':'smoke-alpha','case_id':case_id})['digest']})
            asset=smoke.rpc('getCaseAsset',{'project':'smoke-alpha','case_id':case_id})
            reference={'project':'smoke-alpha','case_id':case_id,'content_revision':asset['digest'],'contract':{'id':'receipt','version':1}}
            case_value=draft('reviewed-case',1)
            case_value['definition']['outputs']={}
            case_value['definition']['steps'][0].update({'operation':{'capability':'case.execute.v1','case':reference},'condition':None,'timeout_ms':10000})
            case_value['definition']['total_timeout_ms']=12000
            case_value['definition']['required_permissions']=['browser.read','browser.write','model.send','report.write']
            case_value['model_profiles']={'pause':'two'}
            publish(case_value)
            # A missing runtime/model must reject before queue admission.
            count=len(smoke.rpc('getJobs')['jobs'])
            current=smoke.rpc('getExecutionFlows')
            req={'id':'reviewed-case','version':1,'expected_revision':current['library']['revision'],'expected_model_revision':current['model_revision'],'inputs':{'enabled':True},'permissions':case_value['definition']['required_permissions'],'approved':True}
            smoke.rpc('prepareFlowExecution',req,success=False)
            require(len(smoke.rpc('getJobs')['jobs'])==count,'missing runtime admitted a worker')
            runtime=smoke.data/'runtimes/midscene/current';runtime.mkdir(parents=True)
            (runtime/'engine.mjs').write_text(ENGINE)
            (runtime/'runtime.json').write_text(json.dumps({'schema_version':1,'engine':'midscene','version':'flow-protocol-fixture','entry':'engine.mjs'}))
            # The runtime fixture uses this inherited test-only control directory.
            # Restart once to pass that directory to the child service.
            smoke.cli('stop');smoke.env['QINAEGIS_FIXTURE_DIR']=str(Path(directory).resolve());start()
            case_prep=prepare(case_value)
            require(case_prep['models']['pause']['profile_id']=='two','explicit selected model lost')
            require(case_prep['snapshot']['cases']['pause']['yaml_script']=='tasks: []','approved script snapshot missing')
            case_job=run(case_prep);case_run=finish(case_job)
            require(case_run['facts']['steps'][0]['attempts'][0]['case_result']['error_message']=='fixture echo [redacted]','engine diagnostic leaked frozen credential')
            case_evidence=case_run['case_runs']['pause']
            actual=smoke.rpc('getRunEvidence',{'project':'smoke-alpha','run_id':case_evidence['run_id']})
            require(actual['cases'][0]['yaml_script']=='tasks: []' and actual['model']['candidates'][0]['id']=='two','case evidence lost reviewed content or explicit model')
            require(case_evidence['allure_results_ready'] and actual['semantic_validation']=='not_verified','Allure bridge or fixture labeling incorrect')
            (Path(directory)/'failed').write_text('1')
            failed=finish(run(prepare(case_value)),'failed')
            require(not failed['facts']['steps'][0]['attempts'][0]['case_result']['passed'],'business assertion failure hidden')
            require(len((Path(directory)/'requests.jsonl').read_text().splitlines())==2,'non-idempotent case retried')
            (Path(directory)/'failed').unlink()
            old=prepare(case_value)
            profiles[1]['model']='model-two-after'
            revision=smoke.rpc('saveModel',{'profile':profiles[1],'expected_revision':revision})['revision']
            smoke.rpc('runFlowExecution',{'preparation_id':old['preparation_id'],'approved':True},success=False)
            # Already admitted queued work retains the model and case that were reviewed.
            profiles[1]['model']='model-two-frozen'
            revision=smoke.rpc('saveModel',{'profile':profiles[1],'expected_revision':revision})['revision']
            blocker=run(prepare(long_value))
            wait(lambda:smoke.rpc('getFlowExecution',{'run_id':blocker['run_id']})['status']=='running','queue blocker not started')
            queued_case=run(prepare(case_value))
            profiles[1]['model']='model-two-after-queue'
            revision=smoke.rpc('saveModel',{'profile':profiles[1],'expected_revision':revision})['revision']
            finish(blocker);frozen_case=finish(queued_case)
            requests=[json.loads(line) for line in (Path(directory)/'requests.jsonl').read_text().splitlines()]
            require(requests[-1]['model']=='model-two-frozen','queued execution reread changed model configuration')
            serialized=json.dumps(smoke.rpc('getExecutionFlows'))+json.dumps(case_run)+json.dumps(actual)+json.dumps(frozen_case)
            require(all(key not in serialized for key in smoke.secrets),'HTTP facts leaked a frozen credential')
            for record_path in (smoke.data/'flow-executions').rglob('*.json'):
                require(all(key not in record_path.read_text() for key in smoke.secrets),'private flow evidence leaked credential')
            smoke.cli('stop')
            start()
            require(smoke.rpc('getFlowExecution', {'run_id': job['run_id']}) == evidence, 'restart changed immutable facts')
            require(not smoke.rpc('getExecutionFlows')['preparations'], 'restart restored memory-only credentials')
            # Real owned-process termination with an invalid workspace marker must quarantine
            # all subsequent queued work, and that quarantine must survive service restart.
            blocked_preparation=prepare(long_value)
            (Path(directory)/'cleanup-damage').write_text('1')
            damaged_job=run(prepare(case_value))
            wait(lambda:(Path(directory)/'owned-workspace').exists(),'damaged workspace fixture not reached')
            try:
                blocked_job=run(blocked_preparation)
                queued=smoke.rpc('getFlowExecution',{'run_id':blocked_job['run_id']})
                require(queued['status']=='queued' and queued['facts'] is None,
                        'cleanup fixture did not hold the execution slot through queue admission')
            finally:
                # Release only after the admission attempt, including failure,
                # so teardown cannot strand the test fixture process.
                (Path(directory)/'cleanup-release').write_text('queue admission inspected')
            damaged=finish(damaged_job,'failed')
            require(damaged['facts']['cleanup_error'],'unconfirmed cleanup was lost')
            blocked=finish(blocked_job,'failed')
            require(blocked['status']=='interrupted' and blocked['facts'] is None,'queued action ran after unconfirmed cleanup')
            smoke.rpc('prepareFlowExecution',{'id':'long-wait','version':1,'expected_revision':smoke.rpc('getExecutionFlows')['library']['revision'],'expected_model_revision':revision,'inputs':{'enabled':True},'permissions':[],'approved':True},success=False)
            smoke.cli('stop');start()
            require((smoke.data/'flow-executions/resource-quarantine.json').exists(),'resource quarantine was not durable')
            smoke.rpc('prepareFlowExecution',{'id':'long-wait','version':1,'expected_revision':smoke.rpc('getExecutionFlows')['library']['revision'],'expected_model_revision':revision,'inputs':{'enabled':True},'permissions':[],'approved':True},success=False)

            require(all(not c['runner_available'] for c in smoke.rpc('getFlows')['capabilities']), 'historical v1 falsely executable')
            print('PASS schema-2 HTTP CAS/review/freeze, waits, cancellation, fixed models, reviewed case RunRecord/Allure, redaction, queue cleanup quarantine and restart (local protocol fixture only)')
        finally:
            smoke.cleanup()
            owned=Path(directory)/'owned-workspace'
            if owned.exists():
                workspace=Path(owned.read_text())
                if workspace.name.startswith('qinaegis-engine-') and workspace.parent.resolve()==Path(tempfile.gettempdir()).resolve() and (workspace/'.qinaegis-owner').read_text()=='damaged-fixture':
                    shutil.rmtree(workspace)
            model.shutdown()


if __name__ == '__main__':
    main()
