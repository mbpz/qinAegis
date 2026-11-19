#!/usr/bin/env python3
"""Real HTTP flow governance and persisted planning, without executing adapters."""
import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile

loader = importlib.util.spec_from_file_location('local_web_smoke', Path(__file__).with_name('test-local-web.py'))
module = importlib.util.module_from_spec(loader)
loader.loader.exec_module(module)
require = module.require


def report_flow():
    return {'id': 'review-flow', 'title': 'Review planning fixture', 'inputs': {'result': 'object'},
            'outputs': {'report': {'value_type': 'object', 'source': {'kind': 'step_output', 'step': 'archive', 'port': 'report'}}},
            'required_permissions': ['report.write'], 'steps': [
                {'id': 'archive', 'capability': 'report.collect', 'depends_on': ['collect'], 'max_attempts': 1,
                 'inputs': {'result': {'kind': 'step_output', 'step': 'collect', 'port': 'report'}}},
                {'id': 'collect', 'capability': 'report.collect', 'depends_on': [], 'max_attempts': 1,
                 'inputs': {'result': {'kind': 'flow_input', 'name': 'result'}}}]}


def main():
    binary = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix='qinaegis-flow-http-') as directory:
        smoke = module.Smoke(binary, directory)

        def start():
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance()
            smoke.authenticate()

        try:
            start()
            initial = smoke.rpc('getFlows')
            require(initial['library']['revision'] == 0 and not initial['plan_history'], 'unexpected initial flows')
            require(all(not c['runner_available'] for c in initial['capabilities']), 'planning advertises execution')
            smoke.rpc('getFlows', {'path': '/etc/passwd'}, success=False)
            for name in ('flow-alpha', 'flow-beta'):
                smoke.rpc('createProject', {'name': name, 'url': f'https://{name}.invalid', 'tech_stack': []})
            draft = report_flow()
            validation = smoke.rpc('validateFlowDraft', {'draft': draft})
            require(validation['execution_order'] == ['collect', 'archive'], 'unstable dependency ordering')
            for mutate in ('cycle', 'wrong_type', 'unknown_capability', 'missing_permission', 'false_idempotence'):
                invalid = copy.deepcopy(draft)
                if mutate == 'cycle': invalid['steps'][1]['depends_on'] = ['archive']
                elif mutate == 'wrong_type': invalid['steps'][0]['inputs']['result'] = {'kind': 'literal', 'value': 'bad'}
                elif mutate == 'unknown_capability': invalid['steps'][0]['capability'] = 'desktop.click'
                elif mutate == 'missing_permission': invalid['required_permissions'] = []
                else: invalid['steps'][0]['idempotent'] = True
                smoke.rpc('validateFlowDraft', {'draft': invalid}, success=False)
            library = smoke.rpc('saveFlowDraft', {'draft': draft, 'expected_revision': 0})
            smoke.rpc('saveFlowDraft', {'draft': draft, 'expected_revision': 0}, success=False)
            smoke.rpc('publishFlow', {'id': draft['id'], 'expected_revision': library['revision'], 'approved': False}, success=False)
            smoke.rpc('publishFlow', {'id': draft['id'], 'expected_revision': 0, 'approved': True}, success=False)
            library = smoke.rpc('publishFlow', {'id': draft['id'], 'expected_revision': library['revision'], 'approved': True})
            request = {'id': draft['id'], 'version': 1, 'project': 'flow-alpha', 'inputs': {'result': {'passed': True}},
                       'permissions': ['report.write'], 'expected_revision': library['revision'],
                       'expected_model_revision': 0, 'approved': True}
            smoke.rpc('prepareFlowPlan', {**request, 'approved': False}, success=False)
            smoke.rpc('prepareFlowPlan', {**request, 'project': 'missing'}, success=False)
            smoke.rpc('prepareFlowPlan', {**request, 'inputs': {'result': []}}, success=False)
            smoke.rpc('prepareFlowPlan', {**request, 'permissions': []}, success=False)
            smoke.rpc('prepareFlowPlan', {**request, 'project_target': 'https://other.invalid'}, success=False)
            first = smoke.rpc('prepareFlowPlan', request)
            second = smoke.rpc('prepareFlowPlan', {**request, 'project': 'flow-beta'})
            require(first['publication'] == second['publication'] and first['plan_id'] != second['plan_id'], 'reusable flow lost identity')
            require(first['project_target'] != second['project_target'] and not first['executable'], 'project target or planning boundary lost')
            require(first['execution_order'] == ['collect', 'archive'] and first['models'] == {}, 'plan ordering or empty-model behavior incorrect')
            require(smoke.rpc('getFlowPlan', {'plan_id': first['plan_id']}) == first, 'plan read changed evidence')
            smoke.rpc('getFlowPlan', {'plan_id': '../plans'}, success=False)
            changed = copy.deepcopy(draft)
            changed['title'] = 'Later version'
            library = smoke.rpc('saveFlowDraft', {'draft': changed, 'expected_revision': library['revision']})
            library = smoke.rpc('publishFlow', {'id': draft['id'], 'expected_revision': library['revision'], 'approved': True})
            require([p['version'] for p in library['publications']] == [1, 2], 'publication overwritten')
            require(smoke.rpc('getFlowPlan', {'plan_id': first['plan_id']}) == first, 'old plan changed after publication')
            smoke.rpc('prepareFlowPlan', request, success=False)

            model_flow = {'id': 'model-plan', 'title': 'Model planning only', 'inputs': {'requirement': 'string'}, 'outputs': {},
                          'required_permissions': ['model.send'], 'steps': [{'id': 'generate', 'capability': 'model.generate',
                          'depends_on': [], 'max_attempts': 1, 'model_role': 'generate',
                          'inputs': {'requirement': {'kind': 'flow_input', 'name': 'requirement'}}}]}
            library = smoke.rpc('saveFlowDraft', {'draft': model_flow, 'expected_revision': library['revision']})
            library = smoke.rpc('publishFlow', {'id': model_flow['id'], 'expected_revision': library['revision'], 'approved': True})
            model_request = {**request, 'id': model_flow['id'], 'inputs': {'requirement': 'plan without network calls'},
                             'permissions': ['model.send'], 'expected_revision': library['revision']}
            smoke.rpc('prepareFlowPlan', model_request, success=False)
            profile = {'id': 'planned-model', 'label': 'Local fixture', 'adapter': 'openai_compatible',
                       'base_url': 'http://127.0.0.1:9/v1', 'model': 'before', 'api_key': 'flow-plan-private-key',
                       'capabilities': ['text'], 'enabled': True, 'allow_remote': False, 'midscene_family': None}
            smoke.secrets.append(profile['api_key'])
            models = smoke.rpc('saveModel', {'profile': profile, 'expected_revision': 0})
            models = smoke.rpc('setModelRoute', {'task': 'generate', 'route': {'primary': profile['id'], 'fallbacks': []}, 'expected_revision': models['revision']})
            model_request['expected_model_revision'] = models['revision']
            model_plan = smoke.rpc('prepareFlowPlan', model_request)
            require(profile['api_key'] not in json.dumps(model_plan), 'plan leaked key')
            require(model_plan['models']['generate']['candidates'][0]['model'] == 'before', 'model configuration not frozen')
            require(model_plan['models']['generate']['candidates'][0]['base_url'] == profile['base_url'], 'model endpoint not frozen')
            profile['model'] = 'after'
            profile['base_url'] = 'http://127.0.0.1:10/v1'
            models = smoke.rpc('saveModel', {'profile': profile, 'expected_revision': models['revision']})
            require(smoke.rpc('getFlowPlan', {'plan_id': model_plan['plan_id']}) == model_plan, 'old model plan changed')
            smoke.rpc('prepareFlowPlan', model_request, success=False)
            smoke.rpc('runFlow', {'plan_id': first['plan_id']}, success=False)
            require(not smoke.rpc('getJobs')['jobs'], 'planning started an action task')
            smoke.cli('stop')
            start()
            require(smoke.rpc('getFlowPlan', {'plan_id': first['plan_id']}) == first, 'restart changed plan')
            require(len(smoke.rpc('getFlows')['plan_history']) == 3, 'history not persisted')
            print('PASS: HTTP flow CAS, typed validation, immutable publication/plan, two-project reuse, model freeze/secrets and restart; no adapters executed')
        finally:
            smoke.cleanup()


if __name__ == '__main__':
    main()
