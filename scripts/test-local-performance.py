#!/usr/bin/env python3
"""Measure an isolated local service on macOS/Linux, excluding browser and engine.

Use an optimized binary. Five fresh workspaces measure process startup to the
launcher's successful health check (OS disk caches are not flushed). The final
workspace contains 10 projects and 100 draft cases. Record RSS after 60 idle
seconds, 300 ordinary RPCs and 10 fixed maintenance submissions. This is a local
baseline, not a stress test or proof for another platform.
"""
import argparse
import importlib.util
import json
import hashlib
import math
import platform
from pathlib import Path
import statistics
import subprocess
import tempfile
import time

loader = importlib.util.spec_from_file_location('local_web_smoke', Path(__file__).with_name('test-local-web.py'))
smoke_module = importlib.util.module_from_spec(loader)
loader.loader.exec_module(smoke_module)


def p95(values):
    return sorted(values)[math.ceil(len(values) * .95) - 1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if platform.system() not in ('Darwin', 'Linux'):
        parser.error('RSS measurement currently supports macOS/Linux only')
    binary = args.binary.resolve(strict=True)
    startup = []
    with tempfile.TemporaryDirectory(prefix='qinaegis-performance-') as temporary:
        smoke = None
        try:
            for index in range(5):
                root = Path(temporary) / str(index)
                root.mkdir()
                smoke = smoke_module.Smoke(binary, root)
                begin = time.perf_counter()
                # Avoid the smoke helper's extra Python pipe-regression wrapper.
                subprocess.run([str(binary), 'start', '--no-open', '--port', str(smoke.port)],
                               env=smoke.env, cwd=smoke.home, stdin=subprocess.DEVNULL,
                               capture_output=True, check=True, timeout=20)
                startup.append((time.perf_counter() - begin) * 1000)
                smoke.instance = smoke.read_instance()
                if index < 4:
                    smoke.cleanup()
            smoke.authenticate()
            for project in range(10):
                name = f'benchmark-{project}'
                smoke.rpc('createProject', {'name': name, 'url': 'https://example.invalid', 'tech_stack': []})
                cases = smoke.data / 'projects' / name / 'cases/draft'
                cases.mkdir(parents=True, exist_ok=True)
                for case in range(10):
                    identity = f'case-{case}'
                    (cases / f'{identity}.json').write_text(json.dumps({
                        'id': identity, 'name': f'Benchmark fixture {case}', 'requirement_id': 'benchmark',
                        'test_type': 'smoke', 'priority': 'P1', 'yaml_script': '- aiAssert: benchmark fixture',
                        'expected_result': 'benchmark fixture', 'created_at': '2026-11-17T00:00:00Z',
                        'status': 'draft'}))
            print('Measuring 60 seconds of service idle RSS (no browser or engine)', flush=True)
            idle_start = time.monotonic()
            rss = []
            while time.monotonic() - idle_start < 60:
                rss.append(int(subprocess.check_output(['ps', '-o', 'rss=', '-p', str(smoke.instance['pid'])], text=True).strip()) / 1024)
                time.sleep(min(5, max(0, 60 - (time.monotonic() - idle_start))))
            rss.append(int(subprocess.check_output(['ps', '-o', 'rss=', '-p', str(smoke.instance['pid'])], text=True).strip()) / 1024)
            idle_seconds = time.monotonic() - idle_start
            requests = []
            for index in range(300):
                method = ('getState', 'getProjects', 'getReviewCases')[index % 3]
                begin = time.perf_counter()
                smoke.rpc(method, {'project': 'benchmark-0'} if method == 'getReviewCases' else {})
                requests.append((time.perf_counter() - begin) * 1000)
            submissions = []
            for _ in range(10):
                begin = time.perf_counter()
                job = smoke.rpc('diagnoseRuntime')['jobId']
                submissions.append((time.perf_counter() - begin) * 1000)
                deadline = time.monotonic() + 10
                while True:
                    status = smoke.rpc('getJobStatus', {'jobId': job})['status']
                    if status == 'completed':
                        break
                    if status in ('failed', 'stopped', 'interrupted') or time.monotonic() > deadline:
                        raise RuntimeError('Fixed maintenance task did not complete')
                    time.sleep(.025)
            result = {
                'platform': platform.platform(), 'machine': platform.machine(),
                'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                'dataset': {'projects': 10, 'draft_cases': 100, 'rpc_requests': 300, 'task_submissions': 10},
                'startup_ms': startup, 'startup_max_ms': max(startup),
                'idle_seconds': idle_seconds, 'idle_rss_mib': rss,
                'idle_rss_max_mib': max(rss), 'idle_rss_median_mib': statistics.median(rss),
                'ordinary_rpc_p95_ms': p95(requests), 'task_submission_max_ms': max(submissions),
                'limits': {'startup_ms': 3000, 'idle_rss_mib': 100, 'rpc_p95_ms': 200, 'task_submission_ms': 2000},
                'scope': 'local optimized service; fresh state, warm OS caches; browser, engine, model and exports excluded',
            }
            result['passed'] = max(startup) <= 3000 and max(rss) <= 100 and p95(requests) <= 200 and max(submissions) <= 2000
            args.output.write_text(json.dumps(result, indent=2) + '\n')
            print(json.dumps(result, indent=2), flush=True)
            if not result['passed']:
                raise SystemExit(1)
        finally:
            if smoke:
                smoke.cleanup()


if __name__ == '__main__':
    main()
