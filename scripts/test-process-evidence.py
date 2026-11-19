#!/usr/bin/env python3
"""Real-service policy and attachment acceptance using an explicit engine fixture.

This verifies storage/protocol boundaries, not browser or visual business behavior.
The separate sandbox Chromium tests exercise actual Playwright capture/viewing.
"""
import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import time

loader = importlib.util.spec_from_file_location('web_smoke', Path(__file__).with_name('test-local-web.py'))
m = importlib.util.module_from_spec(loader)
loader.loader.exec_module(m)
require = m.require

ENGINE = r'''
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import readline from 'node:readline';
const root=process.env.QINAEGIS_FIXTURE_DIR;
const kinds=['screenshot','trace','console','network','steps'];
const mode=()=>fs.existsSync(path.join(root,'mode'))?fs.readFileSync(path.join(root,'mode'),'utf8'):'';
for await(const line of readline.createInterface({input:process.stdin})) {
 const req=JSON.parse(line);
 if(req.method==='shutdown') {console.log(JSON.stringify({id:'0',ok:true,data:null}));break;}
 if(req.method==='capture_capabilities') {
  console.log(JSON.stringify({id:'0',ok:mode()!=='old',data:{protocol_version:1,modes:['off','failure_only','key_steps','full'],kinds,artifact_transport:'owned_files_v1'}}));continue;
 }
 if(req.method==='trace_viewer_start') {
  // Explicit RPC fault fixture, not an actual Trace renderer.
  console.log(JSON.stringify({id:'0',ok:mode()!=='viewer-refuse',data:{url:'https://external.invalid/not-local',expires_in_seconds:900,version:'1.59.1'}}));continue;
 }
 if(req.method!=='run_yaml') throw Error('unsupported fixture method');
 fs.appendFileSync(path.join(root,'requests.jsonl'),JSON.stringify(req.args)+'\n');
 if(fs.existsSync(path.join(root,'hold'))) {
  fs.writeFileSync(path.join(root,'arrived'),'1');
  while(!fs.existsSync(path.join(root,'release'))) await new Promise(r=>setTimeout(r,10));
 }
 if(mode()==='kill') process.exit(7);
 const policy=req.args.capture_policy;
 const capture={schema_version:1,policy,status:policy.mode==='off'?'disabled':'complete',channels:kinds.map(kind=>({kind,status:'not_requested',reason:null,bytes:0})),artifacts:[]};
 if(policy.mode!=='off') {
  for(const kind of ['screenshot','console','network','steps']) {
   const bytes=kind==='screenshot'?Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=','base64'):Buffer.from(JSON.stringify([{time_ms:1,event:'explicit protocol fixture'}]));
   const name='qinaegis-capture-'+crypto.randomBytes(16).toString('hex')+(kind==='screenshot'?'.png':'.json');
   fs.writeFileSync(name,bytes,{mode:0o600,flag:'wx'});
   capture.artifacts.push({id:kind+'-1',kind,file_name:name,bytes:bytes.length,sha256:crypto.createHash('sha256').update(bytes).digest('hex')});
   Object.assign(capture.channels.find(c=>c.kind===kind),{status:'collected',bytes:bytes.length});
  }
  if(policy.mode==='full') {capture.status='partial';Object.assign(capture.channels[1],{status:'unsupported',reason:'Protocol fixture does not generate a real trace'});}
  if(mode().startsWith('viewer-')) {
   const bytes=Buffer.from('UEsFBgAAAAAAAAAAAAAAAAAAAAAAAA==','base64'); // Empty ZIP protocol fixture.
   const name='qinaegis-capture-'+crypto.randomBytes(16).toString('hex')+'.zip';
   fs.writeFileSync(name,bytes,{mode:0o600,flag:'wx'});
   capture.artifacts.push({id:'trace-1',kind:'trace',file_name:name,bytes:bytes.length,sha256:crypto.createHash('sha256').update(bytes).digest('hex')});
   Object.assign(capture.channels[1],{status:'collected',reason:'Explicit empty ZIP protocol fixture',bytes:bytes.length});
  }
  if(mode()==='digest') capture.artifacts[0].sha256='0'.repeat(64);
  if(mode()==='secret') capture.channels[0].reason=process.env.MIDSCENE_MODEL_API_KEY;
  if(mode()==='invalid') capture.schema_version=999;
 }
 const data={case_id:req.args.case_id,passed:true,duration_ms:2,screenshot_base64:null,error_message:null,retry_count:0,healed_yaml_script:null,failed_step:null,capture};
 console.log(JSON.stringify({id:'0',ok:true,data}));
}
'''


def wait(predicate, message):
    end = time.monotonic() + 15
    while time.monotonic() < end:
        value = predicate()
        if value:
            return value
        time.sleep(.025)
    raise m.SmokeFailure(message)


def main():
    require(shutil.which('node'), 'Node is required for the explicit protocol fixture')
    with tempfile.TemporaryDirectory(prefix='qinaegis-process-') as directory:
        root = Path(directory).resolve()
        smoke = m.Smoke(Path(sys.argv[1]).resolve(), directory)
        smoke.env['QINAEGIS_FIXTURE_DIR'] = str(root)
        runtime = smoke.data / 'runtimes/midscene/current'
        runtime.mkdir(parents=True)
        (runtime/'engine.mjs').write_text(ENGINE)
        (runtime/'runtime.json').write_text(json.dumps({'schema_version':1,'engine':'midscene','version':'capture-protocol-fixture','entry':'engine.mjs'}))
        def start():
            smoke.cli('start','--no-open','--port',str(smoke.port))
            smoke.instance=smoke.read_instance();smoke.authenticate()
        def terminal(job):
            value=smoke.rpc('getJobStatus',{'jobId':job['jobId']})
            return value if value['status'] in ('completed','failed','stopped','interrupted') else None
        def finish(job, expected='completed'):
            value=wait(lambda:terminal(job),'capture fixture job timed out')
            require(value['status']==expected, f'expected {expected}, received {value["status"]}')
            return value
        def run():
            return smoke.rpc('runTests',{'project':'capture-fixture','type':'smoke'})
        def evidence(job):
            return smoke.rpc('getRunEvidence',{'project':'capture-fixture','run_id':job['result']['run_id']})
        def save(snapshot, **changes):
            return smoke.rpc('saveCapturePolicy',{'expected_revision':snapshot['revision'],'policy':{**snapshot['policy'],**changes},'approved':True})
        try:
            start()
            smoke.rpc('createProject',{'name':'capture-fixture','url':'https://capture-fixture.invalid','tech_stack':[]})
            case={'id':'fixture-case','name':'Explicit protocol fixture','requirement_id':'fixture','test_type':'smoke','priority':'P1','created_at':'2026-11-19T00:00:00Z','yaml_script':'- aiAssert: fixture','expected_result':'fixture','tags':[],'status':'approved'}
            case_path=smoke.data/'projects/capture-fixture/cases/approved/fixture-case.json'
            case_path.parent.mkdir(parents=True,exist_ok=True);case_path.write_text(json.dumps(case))
            profile={'id':'fixture','label':'Protocol fixture','adapter':'openai_compatible','base_url':'https://capture-model.invalid/v1','model':'fixture','api_key':'fixture-key-not-real','capabilities':['text','vision'],'enabled':True,'allow_remote':True,'midscene_family':'qwen2.5-vl'}
            smoke.secrets.append(profile['api_key'])
            models=smoke.rpc('saveModel',{'profile':profile,'expected_revision':0})
            smoke.rpc('setModelRoute',{'task':'execute','route':{'primary':'fixture','fallbacks':[]},'expected_revision':models['revision']})
            policy=smoke.rpc('getCapturePolicy')
            require(policy['revision']==0 and policy['policy']['mode']=='off','fresh capture is not off')
            for changes in ({'mode':'invented'},{'mode':'full'},{'max_artifact_bytes':1},{'surprise':True}):
                smoke.rpc('saveCapturePolicy',{'expected_revision':0,'policy':{**policy['policy'],**changes},'approved':True},success=False)
            smoke.rpc('saveCapturePolicy',{'expected_revision':0,'policy':policy['policy'],'approved':False},success=False)
            first=evidence(finish(run()))
            require(first['capture_policy']==policy and first['results'][0]['capture']['status']=='disabled','off run lost actual receipt')
            policy=save(policy,mode='key_steps',include_sensitive=True)
            smoke.rpc('saveCapturePolicy',{'expected_revision':0,'policy':policy['policy'],'approved':True},success=False)
            (root/'hold').write_text('1')
            job=run();wait(lambda:(root/'arrived').exists(),'capture fixture did not start')
            frozen=policy
            policy=save(policy,mode='off',include_sensitive=False)
            (root/'release').write_text('1')
            current=evidence(finish(job))
            require(current['capture_policy']==frozen and current['results'][0]['capture']['policy']==frozen['policy'],'active policy was not frozen')
            for name in ('hold','arrived','release'): (root/name).unlink()
            args={'project':'capture-fixture','run_id':current['run_id']}
            view=smoke.rpc('readProcessEvidence',args)
            require(view['policy']==frozen and all(a['available'] for a in view['cases'][0]['artifacts']),'committed attachments unavailable')
            artifact=current['results'][0]['capture']['artifacts'][0]
            selected={**args,'case_id':'fixture-case','artifact_id':artifact['id']}
            downloaded=smoke.rpc('readProcessArtifact',selected)
            payload=base64.b64decode(downloaded['base64'],validate=True)
            require(hashlib.sha256(payload).hexdigest()==artifact['sha256'] and len(payload)==artifact['bytes'],'download differs from committed receipt')
            for altered in ({'project':'foreign'},{'case_id':'foreign'},{'artifact_id':'../escape'},{'path':'/etc/passwd'}):
                smoke.rpc('readProcessArtifact',{**selected,**altered},success=False)
            smoke.rpc('startTraceViewer',{**selected,'approved':False},success=False)
            smoke.rpc('startTraceViewer',{**selected,'approved':True},success=False)
            path=smoke.data/'projects/capture-fixture/reports'/current['run_id']/'evidence/fixture-case'/artifact['file_name']
            original=path.read_bytes();path.write_bytes(b'x'*len(original))
            smoke.rpc('readProcessArtifact',selected,success=False)
            require(view!=smoke.rpc('readProcessEvidence',args),'tampered attachment still available')
            path.unlink()
            if os.name!='nt':
                external=root/'external.json';external.write_bytes(original);path.symlink_to(external)
                smoke.rpc('readProcessArtifact',selected,success=False);path.unlink()
            path.write_bytes(original)
            # Attachment failure is explicit and does not rewrite the business verdict.
            policy=save(policy,mode='full',include_sensitive=True)
            (root/'mode').write_text('digest')
            failed=evidence(finish(run()))
            require(failed['results'][0]['passed'] is True and failed['results'][0]['capture']['status']=='partial' and failed['results'][0]['capture']['channels'][0]['status']=='failed','invalid attachment digest was hidden or changed the business verdict')
            for bad in ('secret','invalid'):
                (root/'mode').write_text(bad)
                partial=evidence(finish(run()))
                require(partial['results'][0]['passed'] and partial['results'][0]['capture']['status']=='partial'
                        and all(c['status']=='unknown' for c in partial['results'][0]['capture']['channels']),
                        'invalid or credential-bearing receipt changed business result or was accepted')
                require(profile['api_key'] not in json.dumps(partial), 'receipt metadata persisted a credential')
            (root/'mode').write_text('kill')
            interrupted=evidence(finish(run(),'failed'))
            require(interrupted['results'][0]['capture']['status']=='partial'
                    and all(c['status']=='unknown' for c in interrupted['results'][0]['capture']['channels']),
                    'engine exit invented completed evidence')
            # Cancellation retains explicit unknown evidence; no browser was involved.
            (root/'mode').unlink()
            (root/'hold').write_text('1')
            cancel_job=run();wait(lambda:(root/'arrived').exists(),'cancellation fixture did not start')
            smoke.rpc('stopJob',{'jobId':cancel_job['jobId']})
            cancelled=evidence(finish(cancel_job,'stopped'))
            require(cancelled['capture_policy']==policy and all(r['capture']['status']=='partial' for r in cancelled['results']),
                    'cancelled run invented complete evidence')
            for name in ('hold','arrived'): (root/name).unlink()
            # Startup refusal/invalid endpoint must terminate the actual owned Node
            # process and release its durable viewer intent, preserving saved results.
            for failure in ('viewer-refuse','viewer-foreign-url'):
                (root/'mode').write_text(failure)
                viewer_record=evidence(finish(run()))
                smoke.rpc('startTraceViewer',{'project':'capture-fixture','run_id':viewer_record['run_id'],
                    'case_id':'fixture-case','artifact_id':'trace-1','approved':True},success=False)
                journal=smoke.data/'capture/viewer-owners.json'
                require(json.loads(journal.read_text())['owners']==[], 'failed viewer lost confirmed cleanup ownership')
                require(smoke.rpc('getRunEvidence',{'project':'capture-fixture','run_id':viewer_record['run_id']})==viewer_record,
                        'viewer startup error changed saved business evidence')
            # Capability refusal occurs before any run_yaml action.
            (root/'mode').write_text('old')
            count=len((root/'requests.jsonl').read_text().splitlines())
            finish(run(),'failed')
            require(len((root/'requests.jsonl').read_text().splitlines())==count,'unsupported engine ran actions')
            (root/'mode').unlink()
            smoke.cli('stop');start()
            require(smoke.rpc('getRunEvidence',args)==current,'restart changed frozen capture evidence')
            require(smoke.rpc('readProcessArtifact',selected)==downloaded,'restart changed attachment bytes')
            require(smoke.rpc('getCapturePolicy')==policy,'restart changed capture policy')
            # A persisted unconfirmed owner after restart quarantines engine actions;
            # reading saved evidence stays available. This is an explicit journal fixture.
            smoke.cli('stop')
            (smoke.data/'capture/viewer-owners.json').write_text(json.dumps({'schema_version':1,'owners':['viewer-explicit-fixture']}))
            start()
            smoke.rpc('runTests',{'project':'capture-fixture','type':'smoke'},success=False)
            require(smoke.rpc('getRunEvidence',args)==current, 'owner quarantine prevented saved evidence access')
            require(json.loads((smoke.data/'capture/viewer-owners.json').read_text())['owners']==['viewer-explicit-fixture'],
                    'restart discarded unconfirmed ownership')
            print('PASS real HTTP capture CAS, admission snapshot, owned artifact migration, digest/tamper/path checks, legacy capability refusal and restart (protocol fixture only)')
        finally:
            (root/'release').write_text('1')
            smoke.cleanup()


if __name__=='__main__':
    main()
