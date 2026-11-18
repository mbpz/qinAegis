#!/usr/bin/env python3
"""Windows user-local installer acceptance using the actual packaged executable.

No network or models. Run: python scripts/test-windows-install.py BINARY
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import socket
import tempfile
import zipfile

SCRIPT = Path(__file__).with_name('install.ps1').resolve()


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    args = parser.parse_args()
    if os.name != 'nt':
        parser.error('Requires actual Windows; do not substitute static checks.')
    binary_bytes = args.binary.resolve(strict=True).read_bytes()
    version = subprocess.check_output([str(args.binary.resolve()), 'version'], text=True).split()[1]
    with tempfile.TemporaryDirectory(prefix='qinaegis-windows-installer-') as temp:
        root = Path(temp)
        prefix = root / 'installation with spaces'
        data = root / 'preserved data'
        state = root / 'service state'
        data.mkdir()
        sentinel = data / 'preserve-me.txt'
        sentinel.write_text('user-owned data')
        env = dict(os.environ, QINAEGIS_DATA_DIR=str(data), QINAEGIS_STATE_DIR=str(state))
        installed = prefix / 'qinaegis.exe'
        previous = prefix / 'qinaegis.previous.exe'

        def archive(name, payload=binary_bytes, bad=None):
            path = root / (name + '.zip')
            manifest = dict(schema_version=1, product='qinaegis', version=version,
                target='x86_64-pc-windows-msvc', entry='qinaegis.exe',
                interface='local-web', engine_included=False, commit='1' * 40)
            if bad == 'metadata':
                manifest['target'] = 'aarch64-apple-darwin'
            with zipfile.ZipFile(path, 'w', zipfile.ZIP_DEFLATED) as z:
                info = zipfile.ZipInfo('qinaegis.exe')
                if bad == 'link':
                    info.external_attr = 0o120777 << 16
                z.writestr(info, payload)
                z.writestr('LICENSE', 'Explicit installer acceptance fixture')
                z.writestr('release.json', json.dumps(manifest))
                if bad == 'traversal':
                    z.writestr('../outside', 'must not extract')
            return path, hashlib.sha256(path.read_bytes()).hexdigest()

        def invoke(*options, success=True):
            result = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive',
                '-ExecutionPolicy', 'Bypass', '-File', str(SCRIPT), '-Prefix', str(prefix),
                '-Version', version, '-NoStart', *map(str, options)], env=env,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=40)
            require((result.returncode == 0) == success,
                    f'Installer unexpected status {result.returncode}: {result.stderr}')
            require(sentinel.read_text() == 'user-owned data', 'Installer modified user data')
            return result

        def install(pair, **kwargs):
            invoke('-Archive', pair[0], '-Checksum', pair[1], **kwargs)

        original = archive('original')
        install(original)
        require(installed.read_bytes() == binary_bytes, 'Wrong binary installed')
        subprocess.run([str(installed), 'version'], check=True, env=env, timeout=10)
        install((original[0], '0' * 64), success=False)
        oversized = root / 'oversized sparse.zip'
        with oversized.open('wb') as stream:
            stream.truncate(129 * 1024 * 1024)
        rejected = invoke('-Archive', oversized, '-Checksum', '0' * 64, success=False)
        require('Archive exceeds limit' in rejected.stderr, 'Oversized archive was hashed before bounds check')
        require(installed.read_bytes() == binary_bytes, 'Oversized archive changed the installation')
        for failure in ('metadata', 'link', 'traversal'):
            install(archive(failure, bad=failure), success=False)
            require(installed.read_bytes() == binary_bytes, 'Invalid ZIP changed installed binary')
        unknown = prefix / 'unknown.txt'
        unknown.write_text('preserve')
        install(original, success=False)
        require(unknown.read_text() == 'preserve', 'Unknown file changed')
        unknown.unlink()
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        try:
            subprocess.run([str(installed), 'start', '--no-open', '--port', str(port)], env=env, check=True, timeout=20)
            install(original, success=False)
        finally:
            subprocess.run([str(installed), 'stop'], env=env, check=True, timeout=20)
        # PE overlay changes the exact bytes while preserving a runnable program.
        upgraded_bytes = binary_bytes + b'\nQINAEGIS_INSTALLER_EXPLICIT_FIXTURE\n'
        install(archive('upgrade', upgraded_bytes))
        require(installed.read_bytes() == upgraded_bytes and previous.read_bytes() == binary_bytes,
                'Atomic upgrade did not preserve previous binary')
        install(original, success=False)
        invoke('-Rollback')
        require(installed.read_bytes() == binary_bytes and previous.read_bytes() == upgraded_bytes,
                'Rollback did not preserve both verified versions')
        invoke('-Rollback')
        require(installed.read_bytes() == upgraded_bytes, 'Second rollback failed')
        invoke('-Uninstall')
        require(not installed.exists() and not previous.exists(), 'Uninstall left verified executables')
        print('PASS Windows SHA/ZIP boundaries, spaces, running guard, upgrade/rollback and data-preserving uninstall')


if __name__ == '__main__':
    main()
