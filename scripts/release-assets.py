#!/usr/bin/env python3
"""Validate and package the 2.0 Web service using only Python's standard library."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent
TARGETS = ('aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-unknown-linux-gnu', 'x86_64-pc-windows-msvc')
SEMVER = r'2\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?'

def check(root=ROOT, tag=None):
    version = tomllib.loads((root/'Cargo.toml').read_text())['workspace']['package']['version']
    if not re.fullmatch(SEMVER, version) or ('-' in version and any(p.isdigit() and len(p)>1 and p.startswith('0') for p in version.split('-',1)[1].split('.'))):
        raise ValueError('Expected a 2.x semantic version')
    ui = root/'crates/web_client/ui'
    package = json.loads((ui/'package.json').read_text())
    lock = json.loads((ui/'package-lock.json').read_text())
    versions = [package['version'], lock['version'], lock['packages']['']['version']]
    cargo = tomllib.loads((root/'Cargo.lock').read_text())
    versions += [p['version'] for p in cargo['package'] if p['name'] in ('qinAegis-web','qin_aegis_core')]
    versions += re.findall(r'^VERSION=(\S+)$', (root/'scripts/install.sh').read_text(), re.M)
    versions += re.findall(r"^    \[string\]\$Version = '([^']+)',", (root/'scripts/install.ps1').read_text(), re.M)
    if len(versions) != 7 or any(v != version for v in versions):
        raise ValueError('Cargo, UI, lockfiles and installer versions must agree')
    if tag is not None and tag != 'v'+version:
        raise ValueError('Release tag must exactly match workspace version')
    return version

def package(target):
    version = check()
    binary = ROOT/'target'/target/'release'/('qinaegis.exe' if 'windows' in target else 'qinaegis')
    output = subprocess.check_output([str(binary), 'version'], text=True)
    if version not in output.split():
        raise ValueError('Built executable version does not match source')
    commit = subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    manifest = {'schema_version':1,'product':'qinaegis','version':version,'target':target,
                'commit':commit,'entry':binary.name,'interface':'local-web','engine_included':False}
    build = ROOT/'build'; build.mkdir(exist_ok=True)
    manifest_path = build/f'qinaegis-{target}.json'
    manifest_path.write_text(json.dumps(manifest,indent=2)+'\n')
    paths = {binary.name:binary,'LICENSE':ROOT/'LICENSE','release.json':manifest_path}
    archive = build/f'qinaegis-{target}{".zip" if "windows" in target else ".tar.gz"}'
    if 'windows' in target:
        with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED) as out:
            for name,path in paths.items(): out.write(path,name)
    else:
        with tarfile.open(archive,'w:gz') as out:
            for name,path in paths.items(): out.add(path,arcname=name,recursive=False)
    verify_archive(archive, manifest)
    (build/f'qinaegis-{target}.sha256').write_text(checksum_text([archive, manifest_path]))
    print(archive)

def checksum_text(paths):
    return ''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in sorted(paths))

def verify_archive(path, expected):
    if str(path).endswith('.zip'):
        with zipfile.ZipFile(path) as archive:
            names=archive.namelist()
            if any(info.is_dir() or (info.external_attr >> 16) & 0o170000 == 0o120000 for info in archive.infolist()):
                raise ValueError('Archive contains non-regular entry')
            if not archive.read(expected['entry']): raise ValueError('Empty executable')
            manifest=json.loads(archive.read('release.json'))
    else:
        with tarfile.open(path) as archive:
            entries=archive.getmembers()
            if not all(e.isfile() for e in entries): raise ValueError('Archive contains non-regular entry')
            names=[e.name for e in entries]
            if archive.getmember(expected['entry']).size == 0: raise ValueError('Empty executable')
            manifest=json.load(archive.extractfile('release.json'))
    if sorted(names)!=sorted([expected['entry'],'LICENSE','release.json']) or manifest!=expected:
        raise ValueError('Invalid service archive or manifest')

def bundle(directory):
    version=check(tag=os.environ.get('RELEASE_TAG') or None)
    directory=Path(directory)
    commits=set()
    for target in TARGETS:
        manifest_path=directory/f'qinaegis-{target}.json'
        manifest=json.loads(manifest_path.read_text())
        if manifest['version']!=version or manifest['target']!=target:
            raise ValueError('Mixed release versions or targets')
        commits.add(manifest['commit'])
        archive=directory/f'qinaegis-{target}{".zip" if "windows" in target else ".tar.gz"}'
        verify_archive(archive,manifest)
        if (directory/f'qinaegis-{target}.sha256').read_text()!=checksum_text([archive,manifest_path]):
            raise ValueError('Target checksums do not match archive and manifest')
    if len(commits)!=1 or commits!={subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()}:
        raise ValueError('Release artifacts must come from the checked-out commit')
    (directory/'install.sh').write_bytes((ROOT/'scripts/install.sh').read_bytes())
    (directory/'install.ps1').write_bytes((ROOT/'scripts/install.ps1').read_bytes())
    files=sorted(p for p in directory.iterdir() if p.is_file() and p.name!='SHA256SUMS')
    (directory/'SHA256SUMS').write_text(checksum_text(files))

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command',choices=['check','package','bundle'])
    parser.add_argument('--tag')
    parser.add_argument('--target',choices=TARGETS)
    parser.add_argument('--directory')
    args=parser.parse_args()
    try:
        if args.command=='check': print(check(tag=args.tag))
        elif args.command=='package': package(args.target)
        else: bundle(args.directory)
    except (ValueError,KeyError,OSError,subprocess.SubprocessError) as error:
        parser.exit(1,f'Release validation failed: {error}\n')
