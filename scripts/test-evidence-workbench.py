#!/usr/bin/env python3
"""Isolated browser acceptance; seeded engine is a protocol fixture, not visual AI."""
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(file))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main():
    fixture = load('evidence_fixture', 'test-process-evidence.py')
    with tempfile.TemporaryDirectory(prefix='qinaegis-workbench-') as directory:
        root = Path(directory).resolve()
        smoke = fixture.m.Smoke(Path(sys.argv[1]).resolve(), directory)
        smoke.env['QINAEGIS_FIXTURE_DIR'] = str(root)
        runtime = smoke.data / 'runtimes/midscene/current'
        runtime.mkdir(parents=True)
        (runtime / 'engine.mjs').write_text(fixture.ENGINE)
        (runtime / 'runtime.json').write_text(json.dumps(dict(schema_version=1, engine='midscene', version='explicit-protocol-fixture', entry='engine.mjs')))
        try:
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance()
            smoke.authenticate()
            project, case_id = 'capture-fixture', 'fixture-case'
            smoke.rpc('createProject', dict(name=project, url='https://fixture.invalid', tech_stack=[]))
            case = dict(id=case_id, name='Explicit protocol fixture', requirement_id='fixture', test_type='smoke', priority='P1', created_at='2026-11-19T00:00:00Z', yaml_script='- aiAssert: fixture', expected_result='fixture', tags=[], status='approved')
            path = smoke.data / f'projects/{project}/cases/approved/{case_id}.json'
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(case))
            profile = dict(id='fixture', label='Protocol fixture', adapter='openai_compatible', base_url='https://fixture.invalid/v1', model='fixture', api_key='fixture-key-not-real', capabilities=['text', 'vision'], enabled=True, allow_remote=True, midscene_family='qwen2.5-vl')
            models = smoke.rpc('saveModel', dict(profile=profile, expected_revision=0))
            smoke.rpc('setModelRoute', dict(task='execute', route=dict(primary='fixture', fallbacks=[]), expected_revision=models['revision']))
            policy = smoke.rpc('getCapturePolicy')
            smoke.rpc('saveCapturePolicy', dict(expected_revision=policy['revision'], policy={**policy['policy'], 'mode':'key_steps', 'include_sensitive':True}, approved=True))
            job = smoke.rpc('runTests', dict(project=project, type='smoke'))
            def completed():
                value = smoke.rpc('getJobStatus', job)
                return value if value['status'] in ('completed', 'failed') else None
            result = fixture.wait(completed, 'protocol fixture did not finish')
            fixture.require(result['status'] == 'completed', 'protocol fixture failed')
            run_id = result['result']['run_id']
            evidence = smoke.rpc('readProcessEvidence', dict(project=project, run_id=run_id))
            artifact = next(item for item in evidence['cases'][0]['receipt']['artifacts'] if item['kind']=='console')
            asset = smoke.rpc('getCaseAsset', dict(project=project, case_id=case_id))
            smoke.rpc('saveCaseAsset', dict(project=project, case_id=case_id, expected_digest=asset['digest'], name='Edited protocol fixture', yaml_script='- aiAssert: edited', expected_result='edited', approved=True))
            history = smoke.rpc('getCaseHistory', dict(project=project, case_id=case_id))
            edited_revision = next(item['revision'] for item in history['revisions'] if item.get('snapshot', {}).get('name')=='Edited protocol fixture')
            draft = dict(id='receipt', version=1, intent='submission returns a receipt', inputs={}, outputs={}, preconditions=[], assertions=['receipt is recorded'], required_capabilities=[])
            smoke.rpc('saveContractDraft', dict(draft=draft, expected_revision=0))
            config = dict(edited_revision=edited_revision, origin=smoke.origin, cookie=smoke.authorized['Cookie'].split('=', 1)[1], project=project, case_id=case_id, run_id=run_id, artifact=artifact, artifact_path=str(smoke.data / f'projects/{project}/reports/{run_id}/evidence/{case_id}' / artifact['file_name']), output=str(Path(sys.argv[2] if len(sys.argv)>2 else 'output/playwright/evidence-workbench').resolve()))
            config_path = root / 'browser.json'
            config_path.write_text(json.dumps(config))
            config_path.chmod(0o600)
            subprocess.run([shutil.which('node'), str(Path(__file__).with_suffix('.mjs')), str(config_path)], check=True)
            current = smoke.rpc('getCaseAsset', dict(project=project, case_id=case_id))
            fixture.require(current['status']=='draft' and current['name']==case['name'] and current['provenance'] is None, 'UI restore did not create clean historical Draft')
        finally:
            smoke.cleanup()
            fixture.require(not (smoke.state / 'instance.json').exists(), 'isolated service shutdown was not confirmed')
    fixture.require(not root.exists(), 'isolated data directory was not removed')
    print('PASS isolated browser acceptance and service/data cleanup (explicit protocol fixture; no visual model)')


if __name__ == '__main__':
    main()
