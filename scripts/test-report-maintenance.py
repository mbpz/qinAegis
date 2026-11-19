#!/usr/bin/env python3
"""Real HTTP recovery checks with explicitly seeded interrupted-report fixtures.

No test execution or business/model acceptance is inferred from these fixtures.
"""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile

loader = importlib.util.spec_from_file_location('local_web_smoke', Path(__file__).with_name('test-local-web.py'))
module = importlib.util.module_from_spec(loader)
loader.loader.exec_module(module)
require = module.require


def main():
    with tempfile.TemporaryDirectory(prefix='qinaegis-report-maintenance-') as directory:
        smoke = module.Smoke(Path(sys.argv[1]).resolve(), Path(directory).resolve())
        try:
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance()
            smoke.authenticate()
            project, run = 'report-recovery-fixture', 'interrupted-report'
            smoke.rpc('createProject', {'name': project, 'url': 'http://127.0.0.1:9', 'tech_stack': []})
            root = smoke.data / 'projects' / project / 'reports' / run
            root.mkdir(parents=True)
            record = {'schema_version': 1, 'run_id': run, 'project': project, 'test_type': 'smoke',
                      'status': 'failed', 'started_at': '2026-11-17T00:00:00Z',
                      'finished_at': '2026-11-17T00:00:01Z',
                      'model': {'source': 'legacy_config', 'model': 'explicit-local-fixture'},
                      'engine_version': None,
                      'cases': [{'case_id': 'receipt', 'name': 'Receipt', 'yaml_script': '- aiAssert: receipt',
                                 'target_url': 'http://127.0.0.1:9', 'provenance': None}],
                      'results': [], 'error': 'Explicit recovery fixture; no test execution',
                      'semantic_validation': 'not_verified'}
            run_path = root / 'run.json'
            run_path.write_text(json.dumps(record))
            run_bytes = run_path.read_bytes()
            results = root / 'allure-results'
            results.mkdir()
            (results / 'keep.json').write_text('frozen results')
            complete = root / 'allure-report'
            complete.mkdir()
            (complete / 'index.html').write_text('explicit preserved completed-report fixture')
            (complete / 'qinaegis-reporter.json').write_text(json.dumps({
                'schema_version': 1, 'reporter': 'allure', 'version': 'explicit-fixture',
                'mode': 'awesome-single-file'}))
            stage = root / '.allure-stage-recovery-fixture'
            stage.mkdir()
            (stage / '.qinaegis-report-stage.json').write_text(json.dumps({
                'schema_version': 1, 'kind': 'allure_report_stage', 'project': project,
                'run_id': run, 'stage': stage.name}))
            (stage / 'partial.json').write_text('temporary bytes')
            unknown = root / '.allure-stage-unknown'
            unknown.mkdir()
            (unknown / 'keep.txt').write_text('unverified ownership')
            params = {'project': project, 'run_id': run}
            preview = smoke.rpc('previewReportCleanup', params)
            require(preview['cleanup_available'] and preview['completed_report_preserved'], 'owned residue was not diagnosed')
            require([item['name'] for item in preview['candidates']] == [stage.name], 'unowned residue admitted')
            require([item['name'] for item in preview['manual_review']] == [unknown.name], 'unowned residue not explained')
            cleanup = {**params, 'preview_token': preview['preview_token']}
            smoke.rpc('cleanupReportResidue', cleanup, success=False)
            (stage / 'partial.json').write_text('changed after inspection')
            smoke.rpc('cleanupReportResidue', {**cleanup, 'approved': True}, success=False)
            require(stage.exists(), 'stale preview deleted changed files')
            preview = smoke.rpc('previewReportCleanup', params)
            result = smoke.rpc('cleanupReportResidue', {**params, 'preview_token': preview['preview_token'], 'approved': True})
            require(result['removed'] == [stage.name] and not stage.exists(), 'verified residue not removed')
            smoke.rpc('cleanupReportResidue', {**params, 'preview_token': preview['preview_token'], 'approved': True}, success=False)
            require(run_path.read_bytes() == run_bytes, 'cleanup changed immutable run evidence')
            require((results / 'keep.json').read_text() == 'frozen results', 'cleanup changed result assets')
            require((complete / 'index.html').read_text() == 'explicit preserved completed-report fixture', 'cleanup changed completed report')
            require((unknown / 'keep.txt').is_file(), 'cleanup removed unverified files')
            smoke.rpc('previewReportCleanup', {**params, 'run_id': '../escape'}, success=False)
            after = smoke.rpc('previewReportCleanup', params)
            require(not after['cleanup_available'], 'cleanup remained available without owned residue')
            print('PASS report recovery: preview, confirmation, stale/duplicate rejection, owned cleanup and immutable evidence', flush=True)
        finally:
            smoke.cleanup()


if __name__ == '__main__':
    main()
