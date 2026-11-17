#!/usr/bin/env python3
"""Install a selected local runtime archive after checking its expected SHA256.

Stop QinAegis first: installation holds its instance lock and will refuse an
active service. Current-directory replacement has a short rename window; a
failed promotion restores the previous directory. No remote downloads occur.
"""
import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import stat
import tarfile
import tempfile

MAX_ENTRIES = 100000
MAX_FILE_BYTES = 256 * 1024 * 1024
MAX_TOTAL_BYTES = 2 * 1024 * 1024 * 1024
MAX_ARCHIVE_BYTES = 1024 * 1024 * 1024


def digest_file(file):
    file.seek(0)
    digest = hashlib.sha256()
    for block in iter(lambda: file.read(1024 * 1024), b''):
        digest.update(block)
    return digest.hexdigest()


def safe_path(name):
    if not isinstance(name, str) or not name or len(name) > 4096 or any(ord(c) < 32 for c in name) or '\\' in name or ':' in name:
        raise ValueError('invalid runtime path')
    parts = name.split('/')
    if any(part in ('', '.', '..') or part.endswith((' ', '.')) for part in parts):
        raise ValueError('runtime path escapes installation')
    if any(part.split('.')[0].upper() in {'CON', 'PRN', 'AUX', 'NUL', *(f'COM{i}' for i in range(1, 10)), *(f'LPT{i}' for i in range(1, 10))} for part in parts):
        raise ValueError('reserved runtime path')
    return PurePosixPath(name)


def link_destination(name, target):
    if not target or '\\' in target or ':' in target or target.startswith('/'):
        raise ValueError('runtime symlink must be relative')
    parts = list(PurePosixPath(name).parent.parts)
    for part in target.split('/'):
        if part in ('', '.'):
            continue
        if part == '..':
            if not parts:
                raise ValueError('runtime symlink escapes installation')
            parts.pop()
        else:
            parts.append(part)
    if not parts:
        raise ValueError('runtime symlink points to root')
    destination = '/'.join(parts)
    safe_path(destination)
    return destination


def validate_archive(archive):
    members, folded, total = {}, set(), 0
    for item in archive:
        if len(members) >= MAX_ENTRIES:
            raise ValueError('runtime entry limit exceeded')
        safe_path(item.name)
        if item.name.casefold() in folded:
            raise ValueError('duplicate runtime entry')
        folded.add(item.name.casefold())
        if not (item.isdir() or item.isfile() or item.issym()) or item.islnk():
            raise ValueError('unsupported runtime entry type')
        if item.mode & 0o7000:
            raise ValueError('privileged runtime mode')
        if item.size < 0 or item.size > MAX_FILE_BYTES:
            raise ValueError('runtime file size limit exceeded')
        total += item.size
        if total > MAX_TOTAL_BYTES:
            raise ValueError('runtime expanded size limit exceeded')
        members[item.name] = item
    for name, item in members.items():
        for parent in PurePosixPath(name).parents:
            ancestor = members.get(str(parent))
            if ancestor is not None and not ancestor.isdir():
                raise ValueError('runtime entry has non-directory ancestor')
        if item.issym():
            destination = link_destination(name, item.linkname)
            if destination not in members:
                raise ValueError('runtime symlink target missing')
    def read_json(name):
        item = members.get(name)
        if item is None or not item.isfile() or item.size > 1024 * 1024:
            raise ValueError('runtime metadata missing or invalid')
        return json.loads(archive.extractfile(item).read())
    manifest = read_json('runtime.json')
    if not isinstance(manifest, dict) or set(manifest) != {'schema_version', 'engine', 'version', 'entry', 'node'} or manifest['schema_version'] != 1 or manifest['engine'] != 'midscene':
        raise ValueError('unsupported runtime manifest')
    if not isinstance(manifest['version'], str) or not re.fullmatch(r'[A-Za-z0-9_.-]{1,64}', manifest['version']):
        raise ValueError('invalid runtime version')
    for name in (manifest['entry'], manifest['node']):
        safe_path(name)
        if name not in members or not members[name].isfile():
            raise ValueError('runtime entry/node must be regular packaged files')
    if PurePosixPath(manifest['entry']).suffix not in ('.js', '.mjs', '.cjs'):
        raise ValueError('runtime requires compiled JavaScript')
    provenance = read_json('provenance.json')
    if not isinstance(provenance, dict):
        raise ValueError('invalid runtime provenance')
    if os.name != 'nt' and not members[manifest['node']].mode & 0o111:
        raise ValueError('runtime Node is not executable')
    expected_target = f'{platform.system().lower()}-{platform.machine().lower()}'
    if provenance.get('schema_version') != 1 or provenance.get('version') != manifest['version'] or provenance.get('target') != expected_target:
        raise ValueError('runtime host/version mismatch')
    if not re.fullmatch(r'[0-9a-f]{40,64}', str(provenance.get('source_commit', ''))) or not str(provenance.get('node_version', '')).startswith('v24.'):
        raise ValueError('invalid runtime provenance')
    compiled = provenance.get('compiled_sha256')
    if not isinstance(compiled, dict) or manifest['entry'] not in compiled:
        raise ValueError('runtime compiled digests missing')
    hashes = {**compiled, manifest['node']: provenance.get('node_sha256'), 'pnpm-lock.yaml': provenance.get('lock_sha256')}
    for name, expected in hashes.items():
        safe_path(name)
        item = members.get(name)
        if item is None or not item.isfile() or not re.fullmatch(r'[0-9a-f]{64}', str(expected)):
            raise ValueError('invalid runtime digest record')
        actual = hashlib.sha256(archive.extractfile(item).read()).hexdigest()
        if actual != expected:
            raise ValueError('runtime content digest mismatch')
    return members, manifest


def private_dir(path):
    if not path.is_absolute():
        raise ValueError('installation paths must be absolute')
    for ancestor in reversed((path, *path.parents)):
        if ancestor.exists() or ancestor.is_symlink():
            if ancestor.is_symlink() or not ancestor.is_dir():
                raise ValueError('installation directory must not contain symlinks')
        else:
            ancestor.mkdir(mode=0o700)
    path.chmod(0o700)


@contextmanager
def locked(path):
    if path.is_symlink():
        raise ValueError('unsafe runtime lock')
    flags = os.O_RDWR | os.O_CREAT | getattr(os, 'O_NOFOLLOW', 0)
    fd = os.open(path, flags, 0o600)
    try:
        if not stat.S_ISREG(os.fstat(fd).st_mode) or os.fstat(fd).st_nlink != 1:
            raise ValueError('unsafe runtime lock')
        try:
            if os.name == 'nt':
                import msvcrt
                msvcrt.locking(fd, msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            raise ValueError('service or runtime installer active; stop QinAegis before installing') from error
        yield
    finally:
        os.close(fd)


def promote(stage, current, previous):
    if previous.exists() or previous.is_symlink():
        raise ValueError('previous runtime exists; retain or remove it explicitly before another upgrade')
    existed = current.exists()
    if current.is_symlink() or (existed and not current.is_dir()):
        raise ValueError('unsafe current runtime')
    if existed:
        os.replace(current, previous)
    try:
        os.replace(stage, current)
    except BaseException:
        if existed:
            os.replace(previous, current)
        raise


def matches_stage(current, stage, members):
    """A checksum marker alone cannot certify an installation is still intact."""
    if current.is_symlink() or not current.is_dir():
        return False
    try:
        for name, item in members.items():
            actual, expected = current / name, stage / name
            for parent in actual.parents:
                if parent == current:
                    break
                if parent.is_symlink():
                    return False
            if item.issym():
                if not actual.is_symlink() or os.readlink(actual) != os.readlink(expected):
                    return False
            elif item.isdir():
                if actual.is_symlink() or not actual.is_dir():
                    return False
            else:
                if actual.is_symlink() or not actual.is_file() or actual.stat().st_size != item.size:
                    return False
                with actual.open('rb') as left, expected.open('rb') as right:
                    if digest_file(left) != digest_file(right):
                        return False
                if os.name != 'nt' and stat.S_IMODE(actual.stat().st_mode) != stat.S_IMODE(expected.stat().st_mode):
                    return False
        return True
    except OSError:
        return False


def install(archive_path, expected_sha256, data_dir, state_dir=None):
    archive_path, data_dir = Path(archive_path), Path(data_dir)
    if archive_path.is_symlink() or not archive_path.is_file() or archive_path.stat().st_size > MAX_ARCHIVE_BYTES:
        raise ValueError('invalid runtime archive')
    if not re.fullmatch(r'[0-9a-fA-F]{64}', expected_sha256):
        raise ValueError('expected SHA256 is required')
    # Validate all metadata and content digests before creating installation files.
    with archive_path.open('rb') as file:
        if digest_file(file) != expected_sha256.lower():
            raise ValueError('runtime archive SHA256 mismatch')
        file.seek(0)
        with tarfile.open(fileobj=file, mode='r:gz') as archive:
            members, manifest = validate_archive(archive)
            state = Path(state_dir or os.environ.get('QINAEGIS_STATE_DIR') or data_dir / 'state')
            private_dir(data_dir)
            private_dir(state)
            runtime_parent = data_dir / 'runtimes/midscene'
            private_dir(runtime_parent)
            with locked(state / 'instance.lock'), locked(runtime_parent / 'install.lock'):
                with tempfile.TemporaryDirectory(prefix='.install-', dir=runtime_parent) as temporary:
                    stage = Path(temporary) / 'runtime'
                    stage.mkdir(mode=0o700)
                    for name, item in members.items():
                        if item.issym():
                            continue
                        destination = stage / name
                        if item.isdir():
                            destination.mkdir(mode=0o700, parents=True, exist_ok=True)
                        else:
                            destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                            with destination.open('xb') as output, archive.extractfile(item) as source:
                                shutil.copyfileobj(source, output)
                            destination.chmod(0o700 if item.mode & 0o111 else 0o600)
                    for name, item in members.items():
                        if item.issym():
                            (stage / name).parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                            (stage / name).symlink_to(item.linkname, target_is_directory=members[link_destination(name, item.linkname)].isdir())
                    for name, item in members.items():
                        if item.issym() and not (stage / name).resolve(strict=True).is_relative_to(stage):
                            raise ValueError('runtime symlink chain escapes staging')
                    if digest_file(file) != expected_sha256.lower():
                        raise ValueError('archive changed during installation')
                    current = runtime_parent / 'current'
                    # Identical archives are an idempotent no-op, even if a backup exists.
                    marker = current / '.archive-sha256'
                    if not current.is_symlink() and marker.is_file() and not marker.is_symlink() and marker.read_text().strip() == expected_sha256.lower() and matches_stage(current, stage, members):
                        return current
                    (stage / '.archive-sha256').write_text(expected_sha256.lower() + '\n')
                    (stage / '.archive-sha256').chmod(0o600)
                    promote(stage, current, runtime_parent / 'previous')
                    return current


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--data-dir', type=Path, default=Path(os.environ.get('QINAEGIS_DATA_DIR', str(Path.home() / '.qinAegis'))))
    parser.add_argument('--state-dir', type=Path, help='override service state path; otherwise respects QINAEGIS_STATE_DIR')
    args = parser.parse_args()
    try:
        print(install(args.archive, args.sha256, args.data_dir, args.state_dir))
    except (OSError, ValueError, tarfile.TarError, RuntimeError):
        parser.exit(1, 'Runtime installation failed; verify archive/checksum, host, paths and stopped service. Existing runtime retained.\n')
