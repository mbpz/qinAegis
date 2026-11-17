#!/usr/bin/env python3
"""Runtime installation integrity, archive confinement and rollback regressions."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import platform
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('runtime_install', Path(__file__).with_name('install-runtime.py'))
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


def fixture(path, extra=(), version='2.0.0'):
    content = {'dist/executor.js': b'// compiled fixture', 'bin/node': b'node fixture', 'pnpm-lock.yaml': b'lock fixture'}
    manifest = {'schema_version': 1, 'engine': 'midscene', 'version': version, 'entry': 'dist/executor.js', 'node': 'bin/node'}
    provenance = {'schema_version': 1, 'version': version, 'target': f'{platform.system().lower()}-{platform.machine().lower()}',
                  'source_commit': '1' * 40, 'node_version': 'v24.0.0',
                  'compiled_sha256': {'dist/executor.js': hashlib.sha256(content['dist/executor.js']).hexdigest()},
                  'node_sha256': hashlib.sha256(content['bin/node']).hexdigest(), 'lock_sha256': hashlib.sha256(content['pnpm-lock.yaml']).hexdigest()}
    content['runtime.json'] = json.dumps(manifest).encode()
    content['provenance.json'] = json.dumps(provenance).encode()
    with tarfile.open(path, 'w:gz') as archive:
        for name, value in content.items():
            item = tarfile.TarInfo(name)
            item.size = len(value)
            item.mode = 0o755 if name == 'bin/node' else 0o644
            archive.addfile(item, io.BytesIO(value))
        for name, kind, target in extra:
            item = tarfile.TarInfo(name)
            item.type, item.linkname = kind, target
            archive.addfile(item)
    return hashlib.sha256(path.read_bytes()).hexdigest()


class RuntimeInstallTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.data = self.root / 'private data'
        self.archive = self.root / 'runtime.tar.gz'
        self.digest = fixture(self.archive)

    def tearDown(self):
        self.temporary.cleanup()

    def install(self, digest=None):
        return installer.install(self.archive, digest or self.digest, self.data, self.data / 'state')

    def test_install_repeat_and_backup_on_upgrade(self):
        current = self.install()
        self.assertEqual((current / 'dist/executor.js').read_bytes(), b'// compiled fixture')
        self.assertEqual(self.install(), current)
        self.digest = fixture(self.archive, version='2.0.1')
        self.install()
        self.assertTrue((current.parent / 'previous/runtime.json').is_file())
        self.assertEqual(json.loads((current / 'runtime.json').read_text())['version'], '2.0.1')
        if os.name != 'nt':
            self.assertEqual((current / 'bin/node').stat().st_mode & 0o777, 0o700)
            self.assertEqual((current / 'runtime.json').stat().st_mode & 0o777, 0o600)

    def test_identical_archive_repairs_missing_or_tampered_installation(self):
        current = self.install()
        (current / 'bin/node').unlink()
        self.install()
        self.assertEqual((current / 'bin/node').read_bytes(), b'node fixture')
        self.assertTrue((current.parent / 'previous').is_dir())
        (current / 'dist/executor.js').write_bytes(b'damaged')
        with self.assertRaisesRegex(ValueError, 'previous runtime exists'):
            self.install()
        self.assertEqual((current / 'dist/executor.js').read_bytes(), b'damaged')
        self.assertTrue((current.parent / 'previous').is_dir())

    @unittest.skipIf(os.name == 'nt', 'POSIX owner execute permissions')
    def test_identical_archive_repairs_owner_execute_permission(self):
        current = self.install()
        (current / 'bin/node').chmod(0o601)
        self.install()
        self.assertEqual((current / 'bin/node').stat().st_mode & 0o777, 0o700)

    def test_bad_checksum_preserves_existing_and_creates_no_new_data(self):
        with self.assertRaises(ValueError):
            self.install('0' * 64)
        self.assertFalse(self.data.exists())
        current = self.install()
        with self.assertRaises(ValueError):
            self.install('0' * 64)
        self.assertTrue((current / 'runtime.json').exists())

    def test_unsafe_archive_paths_and_types_rejected_before_writes(self):
        cases = [('../escape', tarfile.REGTYPE, ''), ('/absolute', tarfile.REGTYPE, ''),
                 ('dist/executor.js', tarfile.REGTYPE, ''), ('pipe', tarfile.FIFOTYPE, ''),
                 ('hardlink', tarfile.LNKTYPE, 'dist/executor.js'),
                 ('escape', tarfile.SYMTYPE, '../escape'), ('absolute-link', tarfile.SYMTYPE, '/tmp/escape'),
                 ('dist', tarfile.SYMTYPE, 'bin'), ('CON', tarfile.REGTYPE, '')]
        for entry in cases:
            with self.subTest(entry=entry):
                self.digest = fixture(self.archive, [entry])
                with self.assertRaises(ValueError):
                    self.install()
                self.assertFalse(self.data.exists())

    @unittest.skipIf(os.name == 'nt', 'requires symlink privilege')
    def test_internal_pnpm_symlink_preserved(self):
        self.digest = fixture(self.archive, [('entry.js', tarfile.SYMTYPE, 'dist/executor.js')])
        current = self.install()
        self.assertTrue((current / 'entry.js').is_symlink())
        self.assertEqual((current / 'entry.js').read_bytes(), b'// compiled fixture')

    def test_failed_promotion_rolls_back(self):
        current = self.install()
        self.digest = fixture(self.archive, version='2.0.1')
        original = os.replace
        def replacement(source, destination):
            if Path(source).name == 'runtime' and Path(destination).name == 'current':
                raise OSError('simulated promotion failure')
            return original(source, destination)
        with patch.object(installer.os, 'replace', side_effect=replacement):
            with self.assertRaises(OSError):
                self.install()
        self.assertEqual(json.loads((current / 'runtime.json').read_text())['version'], '2.0.0')

    def test_manifest_content_digest_tampering_rejected(self):
        with tarfile.open(self.archive) as archive:
            entries = [(item, archive.extractfile(item).read()) for item in archive.getmembers()]
        with tarfile.open(self.archive, 'w:gz') as archive:
            for item, content in entries:
                if item.name == 'bin/node':
                    content = b'tampered executable'
                    item.size = len(content)
                archive.addfile(item, io.BytesIO(content))
        self.digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        with self.assertRaisesRegex(ValueError, 'content digest'):
            self.install()
        self.assertFalse(self.data.exists())

    def test_refuses_active_service_lock(self):
        installer.private_dir(self.data / 'state')
        with installer.locked(self.data / 'state/instance.lock'):
            with self.assertRaisesRegex(ValueError, 'active'):
                self.install()
        self.assertFalse((self.data / 'runtimes/midscene/current').exists())

    def test_host_mismatch_and_content_tampering_rejected(self):
        with patch.object(installer.platform, 'machine', return_value='not-this-host'):
            with self.assertRaisesRegex(ValueError, 'mismatch'):
                self.install()
        with patch.object(installer, 'MAX_TOTAL_BYTES', 1):
            with self.assertRaisesRegex(ValueError, 'size limit'):
                self.install()


if __name__ == '__main__':
    unittest.main()
