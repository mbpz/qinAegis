#!/usr/bin/env python3
"""Offline runtime archive boundary and deterministic metadata tests."""
import argparse
import json
import subprocess
import importlib.util
import os
from pathlib import Path
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('runtime_builder', Path(__file__).with_name('build-runtime.py'))
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)
install_spec = importlib.util.spec_from_file_location('runtime_install', Path(__file__).with_name('install-runtime.py'))
installer = importlib.util.module_from_spec(install_spec)
install_spec.loader.exec_module(installer)


class RuntimePackageTests(unittest.TestCase):
    def test_stable_archive_without_host_paths_or_ownership(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / 'runtime'
            (root / 'dist').mkdir(parents=True)
            entry = root / 'dist/executor.js'
            entry.write_text('// fixture\n')
            first, second = Path(temporary) / 'one.tar.gz', Path(temporary) / 'two.tar.gz'
            builder.archive_tree(root, first)
            os.utime(entry, (1700000000, 1700000000))
            builder.archive_tree(root, second)
            self.assertEqual(builder.sha256(first), builder.sha256(second))
            with tarfile.open(first) as archive:
                self.assertEqual(archive.getnames(), ['dist', 'dist/executor.js'])
                for item in archive.getmembers():
                    self.assertEqual((item.uid, item.gid, item.mtime), (0, 0, 0))
                    self.assertFalse(item.name.startswith('/'))

    @unittest.skipIf(os.name == 'nt', 'symlink privilege varies on Windows')
    def test_rejects_absolute_and_escaping_symlinks(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / 'runtime'
            root.mkdir()
            outside = Path(temporary) / 'secret'
            outside.write_text('do not ship')
            link = root / 'dependency'
            for target in (outside, Path('../secret')):
                link.symlink_to(target)
                with self.assertRaises(ValueError):
                    builder.validate_tree(root)
                link.unlink()

    @unittest.skipIf(os.name == 'nt', 'symlink privilege varies on Windows')
    def test_preserves_internal_pnpm_links(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / 'runtime'
            dependency = root / 'node_modules/.pnpm/example/node_modules/example'
            dependency.mkdir(parents=True)
            (dependency / 'index.js').write_text('export default 1')
            (root / 'node_modules/example').symlink_to('.pnpm/example/node_modules/example')
            archive_path = Path(temporary) / 'runtime.tar.gz'
            builder.archive_tree(root, archive_path)
            with tarfile.open(archive_path) as archive:
                info = archive.getmember('node_modules/example')
                self.assertTrue(info.issym())
                self.assertEqual(info.linkname, '.pnpm/example/node_modules/example')

    @unittest.skipUnless(hasattr(os, 'mkfifo'), 'POSIX file type')
    def test_rejects_special_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            os.mkfifo(root / 'pipe')
            with self.assertRaises(ValueError):
                builder.validate_tree(root)


def smoke_archive(archive_path):
    """Import through the verified installer, then exclude global Node/pnpm."""
    with tempfile.TemporaryDirectory(prefix='qinaegis-runtime-proof-') as temporary:
        # Resolve macOS /var aliases before the installer's ancestor checks.
        temporary_root = Path(temporary).resolve()
        data = temporary_root / 'data'
        state = temporary_root / 'state'
        expected_sha256 = builder.sha256(archive_path)
        root = installer.install(archive_path, expected_sha256, data, state)
        assert root == data / 'runtimes/midscene/current'
        assert (root / '.archive-sha256').read_text().strip() == expected_sha256
        node = root / 'bin' / ('node.exe' if os.name == 'nt' else 'node')
        system_path = str(Path(os.environ.get('SystemRoot', 'C:/Windows')) / 'System32') if os.name == 'nt' else '/usr/bin:/bin'
        environment = dict(os.environ, PATH=system_path)
        assert not (root / 'src').exists()
        assert not (root / 'node_modules/tsx').exists()
        result = subprocess.run([str(node), str(root / 'dist/executor.js')], cwd=root.parent,
                                input='{"id":"relocated","method":"shutdown","args":null}\n',
                                capture_output=True, text=True, env=environment, timeout=60, check=True)
        assert json.loads(result.stdout) == {'id': 'relocated', 'ok': True, 'data': None}
        results = root / 'results'
        results.mkdir()
        (results / 'fixture-result.json').write_text(json.dumps({
            'uuid': 'runtime-package-fixture', 'name': 'Explicit packaging fixture',
            'status': 'passed', 'stage': 'finished', 'steps': [], 'attachments': [], 'parameters': []}))
        script = 'const {generateAllure}=await import(' + json.dumps((root / 'dist/allure_report.js').as_uri()) + '); console.log(JSON.stringify(await generateAllure(' + json.dumps(str(results)) + ',' + json.dumps(str(root / 'report')) + ')));'
        result = subprocess.run([str(node), '--input-type=module', '-e', script], cwd=root.parent,
                                capture_output=True, text=True, env=environment, timeout=120, check=True)
        assert json.loads(result.stdout)['version'] == '3.33.0'
        assert (root / 'report/index.html').stat().st_size > 0
        provenance = json.loads((root / 'provenance.json').read_text())
        assert builder.sha256(node) == provenance['node_sha256']
        for relative, digest in provenance['compiled_sha256'].items():
            assert builder.sha256(root / relative) == digest
        print('Verified archive import + relocated compiled executor + official Allure passed with bundled Node and system-only PATH.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=Path, help='also verify a real runtime archive after relocation')
    args = parser.parse_args()
    result = unittest.TextTestRunner().run(unittest.defaultTestLoader.loadTestsFromTestCase(RuntimePackageTests))
    if not result.wasSuccessful():
        raise SystemExit(1)
    if args.archive:
        smoke_archive(args.archive.resolve())
