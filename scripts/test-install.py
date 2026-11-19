#!/usr/bin/env python3
"""Offline installer regressions using only temporary fixtures and the stdlib.

Run on macOS/Linux: python3 scripts/test-install.py
No downloads, actual services, browser actions, or user data are used.
"""
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

INSTALLER = Path(__file__).resolve().with_name("install.sh")
STOPPED = b'#!/bin/sh\nif [ "$1" = status ]; then exit 1; fi\nexit 0\n'
RUNNING = b'#!/bin/sh\nif [ "$1" = status ]; then exit 0; fi\nexit 0\n'


@unittest.skipIf(os.name == "nt", "POSIX shell installer is not provided on Windows")
class InstallTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="qinaegis-install-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.prefix = self.root / "bin with spaces"
        self.home = self.root / "isolated home"
        self.home.mkdir()
        self.env = os.environ.copy()
        self.env.update({"HOME": str(self.home), "TMPDIR": str(self.root)})

    def archive(self, *, symlink=False):
        path = self.root / ("symlink.tar.gz" if symlink else "service archive.tar.gz")
        with tarfile.open(path, "w:gz") as archive:
            member = tarfile.TarInfo("qinaegis")
            if symlink:
                member.type = tarfile.SYMTYPE
                member.linkname = "/etc/passwd"
                archive.addfile(member)
            else:
                member.mode = 0o755
                member.size = len(STOPPED)
                archive.addfile(member, io.BytesIO(STOPPED))
        return path, hashlib.sha256(path.read_bytes()).hexdigest()

    def installed(self, content):
        self.prefix.mkdir()
        destination = self.prefix / "qinaegis"
        destination.write_bytes(content)
        destination.chmod(0o755)
        return destination

    def install(self, archive, checksum):
        result = subprocess.run(
            ["sh", str(INSTALLER), "--archive", str(archive), "--checksum", checksum,
             "--prefix", str(self.prefix), "--no-start"],
            cwd=self.root, env=self.env, capture_output=True, text=True, timeout=20,
        )
        self.assertEqual(list(self.prefix.glob(".qinaegis-install.*")), [],
                         "installer left a staging executable")
        return result

    def test_install_into_path_with_spaces(self):
        archive, digest = self.archive()
        result = self.install(archive, digest)
        self.assertEqual(result.returncode, 0, result.stderr)
        binary = self.prefix / "qinaegis"
        self.assertEqual(binary.read_bytes(), STOPPED)
        self.assertTrue(os.access(binary, os.X_OK))
        self.assertEqual(list(self.home.iterdir()), [], "installation touched HOME despite --prefix")

    def test_bad_checksum_preserves_existing_executable(self):
        binary = self.installed(STOPPED + b"# previous installation\n")
        original = binary.read_bytes()
        archive, _ = self.archive()
        result = self.install(archive, "0" * 64)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Checksum mismatch", result.stderr)
        self.assertEqual(binary.read_bytes(), original)

    def test_archive_symlink_is_rejected(self):
        binary = self.installed(STOPPED + b"# keep this file\n")
        original = binary.read_bytes()
        archive, digest = self.archive(symlink=True)
        result = self.install(archive, digest)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not a regular file", result.stderr)
        self.assertFalse(binary.is_symlink())
        self.assertEqual(binary.read_bytes(), original)

    def test_running_service_is_not_overwritten(self):
        binary = self.installed(RUNNING)
        archive, digest = self.archive()
        result = self.install(archive, digest)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Existing service is running", result.stderr)
        self.assertEqual(binary.read_bytes(), RUNNING)


if __name__ == "__main__":
    unittest.main(verbosity=2)
