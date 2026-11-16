#!/usr/bin/env python3
"""Offline release integrity and version preparation regression checks."""
import importlib.util
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

spec=importlib.util.spec_from_file_location('release_assets',Path(__file__).with_name('release-assets.py'))
release=importlib.util.module_from_spec(spec);spec.loader.exec_module(release)

class ReleaseTests(unittest.TestCase):
    def test_source_versions_and_tag(self):
        version=release.check()
        self.assertEqual(release.check(tag='v'+version),version)
        with self.assertRaises(ValueError): release.check(tag='v2.99.99')
        with self.assertRaises(ValueError): release.check(tag='v1.0.0')

    def test_manifest_and_archive_reject_extra_or_changed_entries(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            expected={'entry':'qinaegis','version':'2.0.0'}
            for suffix in ('.tar.gz','.zip'):
                archive=root/('package'+suffix)
                def write(extra=False,change=False):
                    entries={'qinaegis':b'binary','LICENSE':b'license','release.json':json.dumps(expected if not change else {}).encode()}
                    if extra:entries['../unexpected']=b'invalid'
                    if suffix=='.zip':
                        with zipfile.ZipFile(archive,'w') as out:
                            for name,data in entries.items():out.writestr(name,data)
                    else:
                        with tarfile.open(archive,'w:gz') as out:
                            for name,data in entries.items():
                                info=tarfile.TarInfo(name);info.size=len(data);out.addfile(info,io.BytesIO(data))
                write();release.verify_archive(archive,expected)
                write(extra=True)
                with self.assertRaises(ValueError):release.verify_archive(archive,expected)
                write(change=True)
                with self.assertRaises(ValueError):release.verify_archive(archive,expected)

    def test_bundle_checksums_cover_installer_and_reject_mixed_commits(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=release.ROOT,text=True).strip()
            for target in release.TARGETS:
                entry='qinaegis.exe' if 'windows' in target else 'qinaegis'
                manifest={'entry':entry,'version':release.check(),'target':target,'commit':commit}
                (root/f'qinaegis-{target}.json').write_text(json.dumps(manifest))
                contents={entry:b'binary','LICENSE':b'license','release.json':json.dumps(manifest).encode()}
                if 'windows' in target:
                    with zipfile.ZipFile(root/f'qinaegis-{target}.zip','w') as archive:
                        for name,data in contents.items():archive.writestr(name,data)
                else:
                    with tarfile.open(root/f'qinaegis-{target}.tar.gz','w:gz') as archive:
                        for name,data in contents.items():
                            info=tarfile.TarInfo(name);info.size=len(data);archive.addfile(info,io.BytesIO(data))
                archive_path=root/f'qinaegis-{target}{".zip" if "windows" in target else ".tar.gz"}'
                manifest_path=root/f'qinaegis-{target}.json'
                (root/f'qinaegis-{target}.sha256').write_text(release.checksum_text([archive_path,manifest_path]))
            with patch.dict(os.environ,{'RELEASE_TAG':''}):
                release.bundle(root)
            checks=(root/'SHA256SUMS').read_text()
            self.assertIn('  install.sh\n',checks)
            self.assertEqual(len(checks.splitlines()),13)
            for line in checks.splitlines():
                digest,name=line.split()
                self.assertEqual(digest,hashlib.sha256((root/name).read_bytes()).hexdigest())
            target_checksum=root/f'qinaegis-{release.TARGETS[0]}.sha256'
            valid_checksum=target_checksum.read_text()
            self.assertEqual(len(valid_checksum.splitlines()),2)
            target_checksum.write_text('invalid')
            with self.assertRaises(ValueError):release.bundle(root)
            target_checksum.write_text(valid_checksum)
            (root/f'qinaegis-{release.TARGETS[0]}.json').write_text('{}')
            with self.assertRaises((ValueError,KeyError)):release.bundle(root)

    def test_version_bump_updates_all_products_without_git_side_effects(self):
        bash=shutil.which('bash')
        if os.name=='nt':
            # Windows' bash.exe can be the WSL launcher. Use Git for Windows'
            # Bash explicitly, matching the shell used by the release workflow.
            git=shutil.which('git')
            self.assertIsNotNone(git,'Git for Windows is required for release script tests')
            bash=next((str(parent/'bin/bash.exe') for parent in Path(git).resolve().parents if (parent/'bin/bash.exe').is_file()),None)
        self.assertIsNotNone(bash,'Bash is required for release script tests')
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            for name in ['Cargo.toml','Cargo.lock','scripts/install.sh','scripts/bump-version.sh','scripts/release-assets.py','crates/web_client/ui/package.json','crates/web_client/ui/package-lock.json']:
                destination=root/name;destination.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(release.ROOT/name,destination)
            result=subprocess.run([bash,'scripts/bump-version.sh','2.1.0-rc.1'],cwd=root,capture_output=True,text=True)
            self.assertEqual(result.returncode,0,f'{result.args}\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}')
            self.assertEqual(release.check(root,tag='v2.1.0-rc.1'),'2.1.0-rc.1')
            self.assertFalse((root/'.git').exists())
            result=subprocess.run([bash,'scripts/bump-version.sh','1.9.0'],cwd=root,capture_output=True,text=True)
            self.assertNotEqual(result.returncode,0,f'Invalid version was accepted:\n{result.stdout}\n{result.stderr}')
            self.assertIn('Usage:',result.stderr)
            self.assertEqual(release.check(root),'2.1.0-rc.1')

if __name__=='__main__':unittest.main()
