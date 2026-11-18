#!/usr/bin/env python3
"""Real bundled Playwright CLI failure/cancel/retry through authenticated HTTP.

The loopback download transport is an explicit fault fixture, not a browser
archive. Retry success uses the genuine matching browser cache supplied by CI.
No model, business site, global Node or elevated installation is used.
"""
import argparse
import hashlib
import http.server
import importlib.util
from pathlib import Path
import shutil
import tempfile
import threading
import time


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


web = load('browser_local_web', 'test-local-web.py')
environment = load('browser_environment', 'test-environment.py')
installer = load('browser_runtime_install', 'install-runtime.py')
require = web.require


class Transport(http.server.BaseHTTPRequestHandler):
    mode = 'fail'
    disconnected = threading.Event()

    def log_message(self, *_):
        pass  # Deliberately never print the private fixture URL.

    def do_GET(self):
        if self.mode == 'fail':
            self.send_error(503)
            return
        self.send_response(200)
        size = 64 * 1024 * 1024
        self.send_header('Content-Length', str(size))
        self.end_headers()
        try:
            for _ in range(size // (128 * 1024)):
                self.wfile.write(b'x' * (128 * 1024))
                self.wfile.flush()
                time.sleep(0.025)
        except (BrokenPipeError, ConnectionResetError, OSError):
            self.disconnected.set()


def wait_progress(smoke, job):
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        facts = smoke.rpc('getJobStatus', {'jobId': job['jobId']})
        require(facts['status'] not in ('failed', 'stopped', 'completed'), 'fault download ended before cancellation')
        if 'Current browser component download:' in facts['output']:
            return
        time.sleep(0.05)
    raise web.SmokeFailure('real CLI emitted no download progress within the acceptance deadline')


def exercise(binary, archive, cache):
    with tempfile.TemporaryDirectory(prefix='qinaegis-browser-acceptance-') as directory:
        root = Path(directory).resolve()
        # Import through the product's verified compatibility installer; fixture
        # transport settings apply only to this isolated service process.
        runtime = installer.install(archive, hashlib.sha256(archive.read_bytes()).hexdigest(), root / 'runtime-data', root / 'runtime-state')
        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Transport)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        smoke = web.Smoke(binary, root)
        private = 'private-download-fixture-token'
        smoke.secrets.append(private)
        smoke.env.update({
            'QINAEGIS_RUNTIME_DIR': str(runtime),
            'PLAYWRIGHT_DOWNLOAD_HOST': f'http://127.0.0.1:{server.server_port}/{private}',
            'PLAYWRIGHT_DOWNLOAD_CONNECTION_TIMEOUT': '3000',
        })
        try:
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance()
            smoke.authenticate()
            smoke.rpc('installRuntimeBrowser', {}, success=False)
            failed = environment.terminal(smoke, smoke.rpc('installRuntimeBrowser', {'approved': True}), expected='failed')
            require('Installing Chromium' in failed['output'] and 'Browser installation ended' in failed['output'], 'failed real CLI lost its safe phases')
            require('https://' not in failed['output'] and private not in failed['output'], 'raw installer output leaked')
            print('PASS real bundled CLI download failure and safe progress through HTTP', flush=True)

            Transport.mode = 'slow'
            job = smoke.rpc('installRuntimeBrowser', {'approved': True})
            wait_progress(smoke, job)
            started = time.monotonic()
            smoke.rpc('stopJob', {'jobId': job['jobId']})
            stopped = environment.terminal(smoke, job, timeout=6, expected='stopped')
            elapsed = time.monotonic() - started
            require(elapsed <= 5, 'browser install cancellation exceeded five seconds')
            require(Transport.disconnected.wait(3), 'cancelled CLI retained its download connection')
            require('Current browser component download:' in stopped['output'], 'cancel lost progress facts')
            print(f'PASS real CLI cancellation and owned cleanup in {elapsed:.3f}s', flush=True)

            # Seed only the matching genuine cache after cleanup. This proves
            # explicit retry + actual launch, not a successful fixture download.
            shutil.copytree(cache, runtime / 'browsers', dirs_exist_ok=True)
            done = environment.terminal(smoke, smoke.rpc('installRuntimeBrowser', {'approved': True}), timeout=60)
            require(done['result']['ready'] and done['result']['browser']['launched'], 'approved retry did not launch genuine Chromium')
            require('Chromium launch verification passed.' in done['output'], 'retry skipped the actual launch phase')
            smoke.cli('stop')
            smoke.instance = None
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance()
            smoke.authenticate()
            require(smoke.rpc('getJobStatus', {'jobId': job['jobId']})['status'] == 'stopped', 'restart lost cancelled job')
            require(all(task['status'] not in ('running', 'queued', 'cancelling') for task in smoke.rpc('getJobs')['jobs']), 'restart replayed installation')
            print('PASS explicit cache retry, genuine Chromium launch and restart without replay', flush=True)
        finally:
            smoke.cleanup()
            server.shutdown()
            server.server_close()
            thread.join(timeout=3)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--browser-cache', type=Path, required=True)
    args = parser.parse_args()
    require(args.browser_cache.is_dir(), 'matching genuine browser cache is required')
    exercise(args.binary.resolve(strict=True), args.archive.resolve(strict=True), args.browser_cache.resolve(strict=True))
