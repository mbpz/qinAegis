#!/usr/bin/env python3
"""Build a host-native Midscene runtime, independently of the service archive.

Requires the locked sandbox development dependencies and pnpm 11.24.0 on the
build machine. Consumers use bundled Node and compiled JS, never these tools.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile

REPO = Path(__file__).resolve().parents[1]


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def validate_tree(root):
    """Preserve pnpm's internal links, reject escaping links and special files."""
    root = root.resolve()
    for entry in root.rglob('*'):
        if entry.is_symlink():
            if Path(os.readlink(entry)).is_absolute():
                raise ValueError('absolute runtime symlink')
            if not entry.resolve(strict=True).is_relative_to(root):
                raise ValueError('runtime symlink escapes package')
        elif not (entry.is_file() or entry.is_dir()):
            raise ValueError('unsupported runtime entry')


def archive_tree(root, output):
    validate_tree(root)
    with output.open('wb') as raw, gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0) as zipped:
        with tarfile.open(fileobj=zipped, mode='w', dereference=False) as archive:
            for entry in sorted(root.rglob('*')):
                # Store regular files independently; installers need no hard-link support.
                archive.inodes.clear()
                info = archive.gettarinfo(str(entry), entry.relative_to(root).as_posix())
                info.uid = info.gid = info.mtime = 0
                info.uname = info.gname = ''
                info.mode = 0o755 if entry.is_dir() or info.mode & 0o111 else 0o644
                if info.isfile():
                    with entry.open('rb') as source:
                        archive.addfile(info, source)
                else:
                    archive.addfile(info)


def run(args, cwd, **kwargs):
    return subprocess.run([str(v) for v in args], cwd=cwd, check=True, **kwargs)


def build(output_dir, offline=False):
    sandbox = REPO / 'sandbox'
    node = Path(shutil.which('node') or '').resolve()
    pnpm = shutil.which('pnpm')
    if not node.is_file() or not pnpm:
        raise ValueError('build requires Node and pnpm')
    node_version = subprocess.check_output([str(node), '--version'], text=True).strip()
    if int(node_version.removeprefix('v').split('.')[0]) != 24:
        raise ValueError('runtime builds require Node 24')
    pnpm_command = [pnpm]
    if os.name == 'nt' and Path(pnpm).suffix.lower() in ('.cmd', '.bat'):
        # Invoke the underlying JS through Node, avoiding cmd.exe quoting rules.
        candidates = [Path(pnpm).parent / 'node_modules/pnpm/bin/pnpm.cjs',
                      Path(pnpm).parent / 'node_modules/corepack/dist/pnpm.js']
        entry = next((path for path in candidates if path.is_file()), None)
        if entry is None:
            raise ValueError('cannot locate pnpm JavaScript entry; use standalone pnpm.exe')
        pnpm_command = [str(node), str(entry)]
    manager_version = subprocess.check_output(pnpm_command + ['--version'], text=True).strip()
    if manager_version != '11.24.0':
        raise ValueError('runtime builds require pnpm 11.24.0')
    version = json.loads((REPO / 'crates/web_client/ui/package.json').read_text())['version']
    source_commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()
    source_dirty = bool(subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=REPO, text=True).strip())
    target = f'{platform.system().lower()}-{platform.machine().lower()}'
    output_dir.mkdir(parents=True, exist_ok=True)
    output = output_dir / f'qinaegis-runtime-{target}.tar.gz'
    # Build in a sibling staging directory so publication is an atomic rename.
    with tempfile.TemporaryDirectory(prefix='.runtime-build-', dir=output_dir) as temporary:
        stage = Path(temporary) / 'runtime'
        stage.mkdir(mode=0o700)
        for name in ('package.json', 'pnpm-lock.yaml', 'pnpm-workspace.yaml'):
            shutil.copy2(sandbox / name, stage / name)
        run(pnpm_command + ['exec', 'tsc', '--outDir', stage / 'dist'], sandbox)
        for test in (stage / 'dist').rglob('*.test.js'):
            test.unlink()
        for name in ('midscene_test.js',):
            (stage / 'dist' / name).unlink(missing_ok=True)
        env = os.environ.copy()
        env['PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD'] = '1'
        env['npm_config_registry'] = 'https://registry.npmjs.org/'
        command = pnpm_command + ['install', '--prod', '--frozen-lockfile', '--ignore-scripts']
        if offline:
            command.append('--offline')
        run(command, stage, env=env)
        # Host install paths and pnpm store locations are not consumer metadata.
        for name in ('.modules.yaml', '.pnpm/lock.yaml', '.pnpm-workspace-state-v1.json'):
            (stage / 'node_modules' / name).unlink(missing_ok=True)
        for wrappers in list((stage / 'node_modules').rglob('.bin')):
            if wrappers.is_symlink():
                wrappers.unlink()
            elif wrappers.is_dir():
                shutil.rmtree(wrappers)
        (stage / 'bin').mkdir()
        node_name = 'node.exe' if os.name == 'nt' else 'node'
        bundled = stage / 'bin' / node_name
        shutil.copy2(node, bundled)
        bundled.chmod(0o755)
        shutil.copy2(REPO / 'LICENSE', stage / 'LICENSE')
        manifest = {'schema_version': 1, 'engine': 'midscene', 'version': version,
                    'entry': 'dist/executor.js', 'node': f'bin/{node_name}'}
        (stage / 'runtime.json').write_text(json.dumps(manifest, indent=2) + '\n')
        package_probe = "const fs=require('fs');const paths=['playwright','@midscene/web','allure']; console.log(JSON.stringify(Object.fromEntries(paths.map(n=>[n,JSON.parse(fs.readFileSync('node_modules/'+n+'/package.json')).version]))))"
        packages = json.loads(subprocess.check_output([str(bundled), '-e', package_probe], cwd=stage, text=True))
        provenance = {'schema_version': 1, 'version': version, 'source_commit': source_commit,
                      'source_dirty': source_dirty, 'target': target, 'node_version': node_version, 'node_sha256': sha256(bundled),
                      'lock_sha256': sha256(stage / 'pnpm-lock.yaml'), 'packages': packages,
                      'compiled_sha256': {p.relative_to(stage).as_posix(): sha256(p) for p in sorted((stage / 'dist').glob('*.js'))},
                      'browser_included': False, 'install_scripts_executed': False}
        (stage / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
        validate_tree(stage)
        if (stage / 'node_modules/tsx').exists() or (stage / 'src').exists():
            raise ValueError('development files unexpectedly included')
        result = run([bundled, stage / 'dist/executor.js'], stage,
                     input='{"id":"package-smoke","method":"shutdown","args":null}\n',
                     text=True, capture_output=True, timeout=60)
        if not any(json.loads(line).get('ok') for line in result.stdout.splitlines() if line.startswith('{')):
            raise ValueError('compiled executor smoke failed')
        candidate = Path(temporary) / 'runtime.tar.gz'
        archive_tree(stage, candidate)
        os.replace(candidate, output)
    (output.with_suffix(output.suffix + '.sha256')).write_text(f'{sha256(output)}  {output.name}\n')
    (output.with_suffix(output.suffix + '.json')).write_text(json.dumps(provenance, indent=2) + '\n')
    print(json.dumps({'archive': str(output), 'bytes': output.stat().st_size, **provenance}, indent=2))
    return output


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir', type=Path, default=REPO / 'build/runtime')
    parser.add_argument('--offline', action='store_true', help='require an already populated pnpm store')
    args = parser.parse_args()
    build(args.output_dir.resolve(), args.offline)
