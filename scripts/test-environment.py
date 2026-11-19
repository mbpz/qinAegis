#!/usr/bin/env python3
"""Real-service configuration CAS/recovery and fixed runtime diagnostic acceptance.

No external model or business site. --runtime selects a standalone runtime;
--install-browser explicitly permits the matching Chromium download.
"""
import argparse
import importlib.util
from pathlib import Path
import tempfile
import time

loader = importlib.util.spec_from_file_location('local_web_smoke', Path(__file__).with_name('test-local-web.py'))
module = importlib.util.module_from_spec(loader)
loader.loader.exec_module(module)
require = module.require


def terminal(smoke, job, timeout=30, expected='completed'):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        status = smoke.rpc('getJobStatus', {'jobId': job['jobId']})
        if status['status'] in ('completed', 'failed', 'stopped', 'interrupted'):
            require(status['status'] == expected, 'environment task ended unexpectedly: ' + str(status.get('error')))
            return status
        time.sleep(0.1)
    raise module.SmokeFailure('environment task exceeded acceptance deadline')


def finish(smoke, job, timeout=30):
    return terminal(smoke, job, timeout)['result']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('--runtime', type=Path)
    parser.add_argument('--install-browser', action='store_true')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='qinaegis-environment-') as directory:
        smoke = module.Smoke(args.binary.resolve(), directory)
        if args.runtime:
            smoke.env['QINAEGIS_RUNTIME_DIR'] = str(args.runtime.resolve(strict=True))
        try:
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance()
            smoke.authenticate()
            state = smoke.rpc('getState')
            require(state['config_revision'] == 0 and not state['config_backup_available'], 'new config metadata incorrect')
            key = 'local-config-acceptance-secret'
            smoke.secrets.append(key)
            smoke.rpc('setConfig', {'config': {'llm': {'api_key': key, 'model': 'original'}}, 'expected_revision': 0})
            first = (smoke.data / 'config.toml').read_bytes()
            smoke.rpc('setConfig', {'config': {'llm': {'model': 'stale'}}, 'expected_revision': 0}, success=False)
            require((smoke.data / 'config.toml').read_bytes() == first, 'stale tab changed config')
            smoke.rpc('setConfig', {'config': {'llm': {'model': 'second', 'api_key': ''}}, 'expected_revision': 1})
            second = smoke.rpc('getState')
            require(second['config_revision'] == 2 and second['config_backup_available'] and second['config']['llm']['api_key_configured'], 'backup or empty-key preservation missing')
            smoke.rpc('restoreConfigBackup', {'expected_revision': 2}, success=False)
            smoke.rpc('restoreConfigBackup', {'expected_revision': 1, 'approved': True}, success=False)
            smoke.rpc('restoreConfigBackup', {'expected_revision': 2, 'approved': True})
            restored = smoke.rpc('getState')
            require(restored['config_revision'] == 3 and restored['config']['llm']['model'] == 'original', 'explicit config restore incorrect')
            require((smoke.data / 'config.toml.backup').is_file(), 'private backup missing')
            print('PASS config CAS, backup, explicit restore, key preservation and response redaction', flush=True)
            smoke.rpc('installRuntimeBrowser', {}, success=False)
            facts = finish(smoke, smoke.rpc('diagnoseRuntime'))
            require(facts['runtime_available'] is bool(args.runtime), 'runtime availability mismatched selected installation')
            if args.runtime:
                require(facts['mode'] == 'installed' and facts['node_supported'], 'standalone Node check failed')
                require(all(facts['packages'].get(name) for name in ('playwright', '@midscene/web', 'allure')), 'production package version missing')
                if args.install_browser:
                    installed = terminal(smoke, smoke.rpc('installRuntimeBrowser', {'approved': True}), 640)
                    facts = installed['result']
                    require(facts['ready'] and facts['browser']['launched'], 'installed Chromium did not actually launch')
                    require((args.runtime / 'browsers').is_dir(), 'browser cache escaped selected runtime')
                    require('Installer exited.' in installed['output'] and 'Chromium launch verification passed.' in installed['output'], 'browser installation omitted actual verification phases')
                print('PASS standalone runtime versions and actual browser check: ' + str(facts['browser']['launched']), flush=True)
            else:
                require(not facts['ready'] and facts.get('repair'), 'missing runtime incorrectly healthy')
                print('PASS missing runtime is diagnosable without blocking asset management', flush=True)
                failed_job = smoke.rpc('installRuntimeBrowser', {'approved': True})
                failed = terminal(smoke, failed_job, expected='failed')
                require('Checking the selected runtime' in failed['output'], 'failed install lost its preparation phase')
                smoke.rpc('installRuntimeBrowser', {}, success=False)
                # A terminal response includes completed worker cleanup. Restart
                # must restore the failed facts, and never replay a download.
                smoke.cli('stop')
                smoke.instance = None
                smoke.cli('start', '--no-open', '--port', str(smoke.port))
                smoke.instance = smoke.read_instance()
                smoke.authenticate()
                restored_job = smoke.rpc('getJobStatus', {'jobId': failed_job['jobId']})
                require(restored_job['status'] == 'failed' and restored_job['output'] == failed['output'], 'restart lost or replayed the failed browser installation')
                require(all(job['status'] not in ('running', 'queued') for job in smoke.rpc('getJobs')['jobs']), 'restart replayed a browser operation')
                print('PASS explicit install failure, saved progress, approval renewal and restart without replay', flush=True)
        finally:
            # A visible result can precede the worker guard's final durable write.
            # Wait for real cleanup before asking the owned service to stop.
            for _ in range(60):
                try:
                    smoke.cli('stop')
                    smoke.instance = None
                    break
                except module.SmokeFailure:
                    time.sleep(0.1)
            smoke.cleanup()


if __name__ == '__main__':
    main()
