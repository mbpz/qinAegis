#!/usr/bin/env python3
"""Real HTTP service + official Allure engine acceptance, with seeded run evidence.

Usage: python3 scripts/test-allure.py /absolute/path/to/qinaegis
Prerequisites: Node and the project's sandbox dependencies installed and built
(`pnpm install --frozen-lockfile` and `pnpm build` in sandbox). This script does
not install software, invoke a model, open a browser or execute business tests.
Fixtures test report conversion/rendering only; they are not visual-test proof.
"""
import importlib.util
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import time

loader = importlib.util.spec_from_file_location('local_web_smoke', Path(__file__).with_name('test-local-web.py'))
module = importlib.util.module_from_spec(loader)
loader.loader.exec_module(module)
require = module.require


def run_record(run_id, status, outcomes):
    cases = [{'case_id': 'receipt', 'name': 'Receipt fixture', 'yaml_script': 'tasks: []',
              'target_url': 'https://example.invalid', 'provenance': None}]
    results = [{'case_id': 'receipt', 'passed': passed, 'duration_ms': 12,
                'screenshot_base64': None, 'error_message': None if passed else 'Fixture assertion failed',
                'retry_count': 0, 'healed_yaml_script': None, 'failed_step': None if passed else 'Fixture assertion'}
               for passed in outcomes]
    return {'schema_version': 1, 'run_id': run_id, 'project': 'allure-demo', 'test_type': 'smoke',
            'status': status, 'started_at': '2026-11-16T10:00:00+00:00',
            'finished_at': '2026-11-16T10:00:01+00:00',
            'model': {'source': 'legacy_config', 'model': 'report-fixture'},
            'engine_version': 'report-fixture', 'cases': cases, 'results': results,
            'error': 'Fixture lifecycle failure' if status == 'failed' else None,
            'semantic_validation': 'not_verified'}


def main():
    repo = Path(__file__).resolve().parent.parent
    sandbox = repo / 'sandbox'
    require((sandbox / 'node_modules' / 'allure').is_dir() and (sandbox / 'dist' / 'executor.js').is_file(),
            'Build the declared sandbox dependencies before Allure acceptance')
    binary = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix='qinaegis-allure-') as directory:
        smoke = module.Smoke(binary, directory)
        smoke.env.pop('QINAEGIS_RUNTIME_DIR', None)
        smoke.env['QINAEGIS_DEV_SANDBOX'] = str(sandbox)
        reports = smoke.home / '.qinAegis/projects/allure-demo/reports'
        started = False
        originals = {}
        def start():
            nonlocal started
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            started = True
            smoke.instance = smoke.read_instance()
            smoke.authenticate()
        def stop():
            nonlocal started
            smoke.cli('stop')
            started = False
        def generate(run_id, expected='completed'):
            job = smoke.rpc('generateAllureReport', {'project': 'allure-demo', 'run_id': run_id})
            deadline = time.monotonic() + 140
            while time.monotonic() < deadline:
                status = smoke.rpc('getJobStatus', {'jobId': job['jobId']})
                if status['status'] in ('completed', 'failed', 'stopped'):
                    require(status['status'] == expected, 'Allure generation returned unexpected terminal status')
                    require((reports / run_id / 'run.json').read_bytes() == originals[run_id],
                            'Report rendering changed immutable execution evidence')
                    return status
                time.sleep(0.1)
            raise module.SmokeFailure('Allure generation did not finish within its timeout')
        def report_route(run_id, expected_html):
            path = f'/api/v2/allure/allure-demo/{run_id}'
            cookie = {'Cookie': smoke.authorized['Cookie']}
            payload, raw_headers = smoke.request('GET', path, headers=cookie)
            headers = {key.lower(): value for key, value in raw_headers.items()}
            require(payload.decode() == expected_html, 'Navigated Allure document differs from stored HTML')
            require(headers.get('content-type', '').startswith('text/html'), 'Report route omitted HTML type')
            csp = headers.get('content-security-policy', '')
            directives = {part.strip().split(' ', 1)[0]: part.strip().split(' ', 1)[1]
                          for part in csp.split(';') if len(part.strip().split(' ', 1)) == 2}
            require(directives.get('sandbox') == 'allow-scripts' and 'allow-same-origin' not in csp,
                    'Report route did not isolate the script document origin')
            require(directives.get('default-src') == "'none'" and directives.get('connect-src') == 'blob: data:'
                    and directives.get('frame-ancestors') == "'self'" and 'http:' not in csp and 'https:' not in csp,
                    'Report route CSP permits external network or framing')
            require(headers.get('x-frame-options') == 'SAMEORIGIN' and headers.get('cache-control') == 'no-store',
                    'Report route lacks same-origin framing or private cache policy')
            smoke.request('GET', path, expected=401)
            smoke.request('GET', '/api/v2/allure/bad!/run', headers=cookie, expected=400)
            smoke.request('GET', '/api/v2/allure/allure-demo/missing-run', headers=cookie, expected=404)
            smoke.request('GET', path, headers={**cookie, 'Origin': 'https://outside.invalid'}, expected=403)
            smoke.request('GET', path, headers={**cookie, 'Sec-Fetch-Site': 'cross-site'}, expected=403)
            _, raw_main_headers = smoke.request('GET', '/')
            main_headers = {key.lower(): value for key, value in raw_main_headers.items()}
            main_csp = main_headers.get('content-security-policy', '')
            require("script-src 'self';" in main_csp and "script-src 'unsafe-inline'" not in main_csp,
                    'Workbench script policy was weakened for report rendering')
        def html(run_id):
            report = smoke.rpc('getAllureReportHtml', {'project': 'allure-demo', 'run_id': run_id})
            require(report.get('version') == '3.33.0', 'Allure HTML reporter version differs from the pinned runtime')
            return report['html']
        try:
            start()
            smoke.rpc('createProject', {'name': 'allure-demo', 'url': 'https://example.invalid', 'tech_stack': []})
            fixtures = [('run-pass', 'completed', [True], 'passed'),
                        ('run-assertion', 'completed', [False], 'failed'),
                        ('run-lifecycle', 'failed', [True], 'passed'),
                        ('run-stopped', 'stopped', [], 'skipped')]
            for run_id, status, outcomes, case_status in fixtures + [('run-missing', 'completed', [True], 'passed')]:
                root = reports / run_id
                root.mkdir(parents=True)
                record = root / 'run.json'
                record.write_text(json.dumps(run_record(run_id, status, outcomes)))
                if os.name != 'nt':
                    root.chmod(0o700)
                    record.chmod(0o600)
                originals[run_id] = record.read_bytes()
            saved_html = {}
            for run_id, status, _, expected in fixtures:
                generate(run_id)
                root = reports / run_id
                result_files = list((root / 'allure-results').glob('*-result.json'))
                results = [json.loads(path.read_text()) for path in result_files]
                case = next(value for value in results if value['name'] == 'Receipt fixture')
                require(case['status'] == expected and case['steps'] == [], 'Allure case status/step mapping is incorrect')
                if status == 'failed':
                    require(any(value['name'] == 'Execution lifecycle' and value['status'] == 'broken' for value in results),
                            'Lifecycle failure disappeared behind passing business cases')
                require(len(case['attachments']) >= 3, 'Allure omitted frozen source/script/model attachments')
                for attachment in case['attachments']:
                    source = attachment['source']
                    require(Path(source).name == source and (root / 'allure-results' / source).is_file(),
                            'Allure attachment points outside its result directory or is missing')
                page = html(run_id)
                require('allure-single-file-assets' in page and len(page) > 10000, 'Official Allure self-contained HTML missing')
                require('Content-Security-Policy' in page and "default-src 'none'" in page and "connect-src blob: data:" in page,
                        'Allure HTML lacks its offline network restrictions')
                require(not re.search(r'googletagmanager|function gtag\(\)|const baseEl =|<base\b', page),
                        'Allure HTML contains tracking or document base bootstraps')
                require(not re.search(r'<(?:script|link)\b[^>]*(?:src|href)=[\"\']https?://', page, re.I),
                        'Allure HTML depends on external assets')
                if os.name != 'nt':
                    require(all(path.stat().st_mode & 0o077 == 0 for path in root.rglob('*') if path.is_file()),
                            'Allure report/evidence files are not private')
                report_route(run_id, page)
                saved_html[run_id] = page
                generate(run_id)
                require(html(run_id) == page, 'Repeated report generation changed a complete report')
            stop()
            start()
            for run_id, page in saved_html.items():
                require(html(run_id) == page, 'Restart lost or changed the generated Allure report')
                report_route(run_id, page)
            stop()
            smoke.env.pop('QINAEGIS_DEV_SANDBOX', None)
            smoke.env['QINAEGIS_RUNTIME_DIR'] = str(Path(directory) / 'missing-runtime')
            start()
            failed = generate('run-missing', 'failed')
            require(bool(failed.get('error')), 'Missing runtime failure omitted its diagnostic')
            require(not (reports / 'run-missing/allure-report/index.html').exists(),
                    'Missing runtime manufactured a successful HTML report')
            for run_id, original in originals.items():
                require((reports / run_id / 'run.json').read_bytes() == original, 'Allure operation rewrote a run')
            print('PASS real-service official Allure results, offline authenticated iframe HTML/CSP, lifecycle failure, idempotence, restart and missing runtime (seeded evidence only)')
        finally:
            if started:
                smoke.cli('stop')


if __name__ == '__main__':
    try:
        main()
    except (module.SmokeFailure, OSError, ValueError, IndexError) as error:
        print(f'FAIL {error}', file=sys.stderr)
        sys.exit(1)
