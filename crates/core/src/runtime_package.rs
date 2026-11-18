//! Native verification of explicitly trusted runtime archives. Never executes package code.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
pub const MAX_ARCHIVE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_FILE: u64 = 256 * 1024 * 1024;
const MAX_TOTAL: u64 = 2 * 1024 * 1024 * 1024;
const MAX_STREAM: u64 = MAX_TOTAL + 128 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeManifest {
    pub schema_version: u32,
    pub engine: String,
    pub version: String,
    pub entry: String,
    pub node: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Member {
    File {
        size: u64,
        sha256: String,
        executable: bool,
    },
    Directory,
    Symlink {
        target: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VerifiedPackage {
    pub manifest: RuntimeManifest,
    pub version: String,
    pub archive_sha256: String,
    pub members: BTreeMap<String, Member>,
    pub provenance: serde_json::Value,
}
fn check(cancel: &AtomicBool) -> Result<()> {
    ensure!(
        !cancel.load(Ordering::Acquire),
        "runtime operation cancelled"
    );
    Ok(())
}
struct Bounded<'a, R> {
    inner: R,
    remaining: u64,
    cancel: &'a AtomicBool,
}
impl<R: Read> Read for Bounded<'_, R> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Acquire) {
            return Err(std::io::Error::other("runtime operation cancelled"));
        }
        let n = self.inner.read(b)?;
        if n as u64 > self.remaining {
            return Err(std::io::Error::other("runtime stream limit exceeded"));
        }
        self.remaining -= n as u64;
        Ok(n)
    }
}
fn hash(mut source: impl Read, cancel: &AtomicBool) -> Result<String> {
    let mut h = Sha256::new();
    let mut buf = [0; 65536];
    let mut total = 0u64;
    loop {
        check(cancel)?;
        let n = source.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        ensure!(
            total <= MAX_ARCHIVE_BYTES,
            "runtime digest input exceeds limit"
        );
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn safe(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name.is_ascii()
            && name.len() <= 4096
            && !name.chars().any(|c| c.is_control())
            && !name.contains(['\\', ':']),
        "invalid runtime path"
    );
    for p in name.split('/') {
        ensure!(
            !p.is_empty() && p != "." && p != ".." && !p.ends_with([' ', '.']),
            "runtime path escapes installation"
        );
        let base = p.split('.').next().unwrap().to_ascii_uppercase();
        ensure!(
            !["CON", "PRN", "AUX", "NUL"].contains(&base.as_str())
                && !(1..=9).any(|i| base == format!("COM{i}") || base == format!("LPT{i}")),
            "reserved runtime path"
        );
    }
    ensure!(
        !name.starts_with(".qinaegis-")
            && name != ".archive-sha256"
            && name != "browsers"
            && !name.starts_with("browsers/"),
        "reserved runtime metadata/cache path"
    );
    Ok(())
}
fn target(name: &str, to: &str) -> Result<String> {
    ensure!(
        !to.is_empty()
            && !to.starts_with('/')
            && !to.contains(['\\', ':'])
            && !to.chars().any(|c| c.is_control()),
        "invalid runtime symlink"
    );
    let mut parts: Vec<_> = name.split('/').collect();
    parts.pop();
    for p in to.split('/') {
        match p {
            "" | "." => (),
            ".." => {
                ensure!(
                    parts.pop().is_some(),
                    "runtime symlink escapes installation"
                )
            }
            _ => parts.push(p),
        }
    }
    let dest = parts.join("/");
    safe(&dest)?;
    Ok(dest)
}
fn plain_dirs(path: &Path) -> Result<()> {
    ensure!(path.is_absolute(), "runtime destination must be absolute");
    for ancestor in path.ancestors() {
        let m = fs::symlink_metadata(ancestor)?;
        ensure!(
            m.is_dir() && !m.file_type().is_symlink(),
            "unsafe runtime ancestor"
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            ensure!(m.file_attributes() & 0x400 == 0, "runtime reparse ancestor");
        }
    }
    Ok(())
}
fn reader<'a>(
    path: &Path,
    cancel: &'a AtomicBool,
) -> Result<Bounded<'a, flate2::read::MultiGzDecoder<File>>> {
    Ok(Bounded {
        inner: flate2::read::MultiGzDecoder::new(File::open(path)?),
        remaining: MAX_STREAM,
        cancel,
    })
}
fn preflight(path: &Path, cancel: &AtomicBool) -> Result<()> {
    let mut ar = tar::Archive::new(reader(path, cancel)?);
    let mut count = 0;
    for e in ar.entries()?.raw(true) {
        let mut e = e?;
        count += 1;
        ensure!(count <= 200_000, "runtime header count limit");
        let t = e.header().entry_type();
        let special = t.is_pax_global_extensions()
            || t.is_pax_local_extensions()
            || t.is_gnu_longname()
            || t.is_gnu_longlink();
        ensure!(
            e.size() <= if special { 65536 } else { MAX_FILE },
            "runtime header size limit"
        );
        std::io::copy(&mut e, &mut std::io::sink())?;
    }
    let mut r = ar.into_inner();
    std::io::copy(&mut r, &mut std::io::sink())?;
    Ok(())
}
fn validate_members(m: &BTreeMap<String, Member>) -> Result<()> {
    for (name, member) in m {
        safe(name)?;
        let mut parent = Path::new(name).parent();
        while let Some(p) = parent {
            if let Some(v) = m.get(p.to_str().context("invalid parent")?) {
                ensure!(
                    matches!(v, Member::Directory),
                    "non-directory runtime ancestor"
                )
            }
            parent = p.parent();
        }
        if let Member::Symlink { target: to } = member {
            let mut dest = target(name, to)?;
            let mut seen = BTreeSet::new();
            loop {
                ensure!(seen.insert(dest.clone()), "runtime symlink cycle");
                match m.get(&dest).context("runtime symlink target missing")? {
                    Member::Symlink { target: to } => dest = target(&dest, to)?,
                    _ => break,
                }
            }
        }
    }
    Ok(())
}
fn metadata(root: &Path, members: &BTreeMap<String, Member>) -> Result<RuntimeManifest> {
    let json = |name: &str| -> Result<serde_json::Value> {
        ensure!(
            matches!(members.get(name),Some(Member::File{size,..})if *size<=1024*1024),
            "runtime metadata missing or oversized"
        );
        Ok(serde_json::from_slice(&fs::read(root.join(name))?)?)
    };
    let manifest: RuntimeManifest = serde_json::from_value(json("runtime.json")?)?;
    ensure!(
        manifest.schema_version == 1
            && manifest.engine == "midscene"
            && !manifest.version.is_empty()
            && manifest.version.len() <= 64
            && manifest
                .version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')),
        "invalid runtime manifest"
    );
    for n in [&manifest.entry, &manifest.node] {
        safe(n)?;
        ensure!(
            matches!(members.get(n), Some(Member::File { .. })),
            "runtime entry/node must be packaged files"
        )
    }
    ensure!(
        matches!(
            Path::new(&manifest.entry)
                .extension()
                .and_then(|v| v.to_str()),
            Some("js" | "mjs" | "cjs")
        ),
        "compiled JavaScript required"
    );
    #[cfg(unix)]
    ensure!(
        matches!(
            members[&manifest.node],
            Member::File {
                executable: true,
                ..
            }
        ),
        "Node is not executable"
    );
    let p = json("provenance.json")?;
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        v => v,
    };
    let arch = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "arm64",
        ("windows", "x86_64") => "amd64",
        (_, v) => v,
    };
    let host = format!("{os}-{arch}");
    ensure!(
        p["schema_version"] == 1 && p["version"] == manifest.version && p["target"] == host,
        "runtime host/version mismatch"
    );
    let commit = p["source_commit"].as_str().unwrap_or("");
    ensure!(
        (40..=64).contains(&commit.len())
            && commit
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && p["node_version"].as_str().unwrap_or("").starts_with("v24."),
        "invalid provenance"
    );
    let mut hashes = p["compiled_sha256"]
        .as_object()
        .context("compiled hashes missing")?
        .clone();
    ensure!(hashes.contains_key(&manifest.entry), "entry digest missing");
    hashes.insert(manifest.node.clone(), p["node_sha256"].clone());
    hashes.insert("pnpm-lock.yaml".into(), p["lock_sha256"].clone());
    if let Some(support) = p.get("support_sha256") {
        for (n, h) in support.as_object().context("invalid support hashes")? {
            ensure!(!hashes.contains_key(n), "duplicate support digest");
            hashes.insert(n.clone(), h.clone());
        }
    }
    for (n, h) in hashes {
        safe(&n)?;
        let h = h.as_str().context("invalid digest")?;
        ensure!(
            digest(h) && matches!(members.get(&n),Some(Member::File{sha256,..})if sha256==h),
            "runtime content digest mismatch"
        );
    }
    Ok(manifest)
}
/// Destination must not exist. On failure only the newly created destination is removed.
pub fn verify_and_extract(
    archive: &Path,
    expected_sha256: &str,
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<VerifiedPackage> {
    check(cancel)?;
    let expected = expected_sha256.to_ascii_lowercase();
    ensure!(digest(&expected), "expected SHA256 required");
    let meta = fs::symlink_metadata(archive)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= MAX_ARCHIVE_BYTES,
        "invalid archive"
    );
    ensure!(
        hash(File::open(archive)?, cancel)? == expected,
        "runtime archive SHA256 mismatch"
    );
    preflight(archive, cancel)?;
    plain_dirs(destination.parent().context("destination parent missing")?)?;
    fs::create_dir(destination)?;
    let result = (|| {
        private(destination, true)?;
        let mut ar = tar::Archive::new(reader(archive, cancel)?);
        let mut members = BTreeMap::new();
        let mut folded = BTreeSet::new();
        let mut total = 0u64;
        for entry in ar.entries()? {
            check(cancel)?;
            let mut entry = entry?;
            let typ = entry.header().entry_type();
            let raw = entry.path_bytes();
            let raw_name = std::str::from_utf8(&raw)?;
            let name = if typ.is_dir() {
                raw_name.strip_suffix('/').unwrap_or(raw_name)
            } else {
                raw_name
            }
            .to_owned();
            safe(&name)?;
            ensure!(
                members.len() < 100_000 && folded.insert(name.to_lowercase()),
                "runtime duplicate or entry limit"
            );
            let mode = entry.header().mode()?;
            ensure!(mode & 0o7000 == 0, "privileged runtime mode");
            let size = entry.size();
            ensure!(size <= MAX_FILE, "runtime file size limit");
            total = total.checked_add(size).context("runtime size overflow")?;
            ensure!(total <= MAX_TOTAL, "runtime expanded size limit");
            let member = if typ.is_dir() {
                ensure!(size == 0, "directory contains data");
                Member::Directory
            } else if typ.is_file() {
                let path = destination.join(&name);
                make_parents(destination, &path)?;
                let mut output = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)?;
                let mut h = Sha256::new();
                let mut buf = [0; 65536];
                let mut written = 0;
                loop {
                    check(cancel)?;
                    let n = entry.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    written += n as u64;
                    output.write_all(&buf[..n])?;
                    h.update(&buf[..n]);
                }
                ensure!(written == size, "truncated runtime entry");
                private(&path, mode & 0o111 != 0)?;
                // Include executable-mode metadata in the durable file state.
                output.sync_all()?;
                Member::File {
                    size,
                    sha256: format!("{:x}", h.finalize()),
                    executable: mode & 0o111 != 0,
                }
            } else if typ.is_symlink() {
                ensure!(size == 0, "symlink contains data");
                let target = entry
                    .link_name()?
                    .context("missing symlink target")?
                    .to_str()
                    .context("non-UTF8 symlink")?
                    .to_owned();
                Member::Symlink { target }
            } else {
                anyhow::bail!("unsupported runtime entry type")
            };
            if matches!(member, Member::Directory) {
                let path = destination.join(&name);
                make_parents(destination, &path)?;
                if !path.exists() {
                    fs::create_dir(&path)?;
                }
                plain_dirs(&path)?;
                private(&path, true)?;
            }
            members.insert(name, member);
        }
        validate_members(&members)?;
        let manifest = metadata(destination, &members)?;
        for (name, member) in &members {
            if let Member::Symlink { target: to } = member {
                let path = destination.join(name);
                make_parents(destination, &path)?;
                #[cfg(unix)]
                std::os::unix::fs::symlink(to, &path)?;
                #[cfg(windows)]
                {
                    let resolved = target(name, to)?;
                    if matches!(members.get(&resolved), Some(Member::Directory)) {
                        std::os::windows::fs::symlink_dir(to, &path)?
                    } else {
                        std::os::windows::fs::symlink_file(to, &path)?
                    }
                }
            }
        }
        for (name, member) in &members {
            if matches!(member, Member::Symlink { .. }) {
                ensure!(
                    destination
                        .join(name)
                        .canonicalize()?
                        .starts_with(destination.canonicalize()?),
                    "runtime link escapes destination"
                );
            }
        }
        ensure!(
            hash(File::open(archive)?, cancel)? == expected,
            "archive changed during extraction"
        );
        sync_directories(destination, &members, cancel)?;
        check(cancel)?;
        Ok(VerifiedPackage {
            version: manifest.version.clone(),
            manifest,
            archive_sha256: expected,
            provenance: serde_json::from_slice(&fs::read(destination.join("provenance.json"))?)?,
            members,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result
}
/// Flush a previously verified installation before adopting its durable receipt.
/// Mutable browser caches are deliberately excluded from the package transaction.
pub fn sync_verified_installation(
    root: &Path,
    expected: &VerifiedPackage,
    cancel: &AtomicBool,
) -> Result<()> {
    verify_installed(root, expected, cancel)?;
    for (name, member) in &expected.members {
        check(cancel)?;
        if matches!(member, Member::File { .. }) {
            let path = root.join(name);
            plain_dirs(path.parent().context("missing package parent")?)?;
            let metadata = fs::symlink_metadata(&path)?;
            ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "package file changed before sync"
            );
            let mut options = OpenOptions::new();
            options.read(true);
            // Windows FlushFileBuffers requires GENERIC_WRITE, even when no
            // content is changed. Open the existing file without truncation.
            #[cfg(windows)]
            options.write(true);
            options.open(path)?.sync_all()?;
        }
    }
    sync_directories(root, &expected.members, cancel)?;
    check(cancel)
}

fn sync_directories(
    root: &Path,
    members: &BTreeMap<String, Member>,
    cancel: &AtomicBool,
) -> Result<()> {
    #[cfg(unix)]
    {
        // Infer implicit directories too: tar need not list parents explicitly.
        // Deriving paths from verified members avoids walking mutable caches or
        // following package symlinks while flushing directory entries.
        let mut directories = BTreeSet::from([root.to_path_buf()]);
        for (name, member) in members {
            check(cancel)?;
            let path = root.join(name);
            let mut dir = if matches!(member, Member::Directory) {
                Some(path.as_path())
            } else {
                path.parent()
            };
            while let Some(path) = dir {
                ensure!(
                    path.starts_with(root),
                    "package sync directory escapes root"
                );
                directories.insert(path.to_path_buf());
                ensure!(directories.len() <= 200_000, "package directory sync limit");
                if path == root {
                    break;
                }
                dir = path.parent();
            }
        }
        let mut directories: Vec<_> = directories.into_iter().collect();
        directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        for directory in directories {
            check(cancel)?;
            plain_dirs(&directory)?;
            File::open(directory)?.sync_all()?;
        }
        // Persist the staging root entry as well as all entries beneath it.
        check(cancel)?;
        let parent = root.parent().context("package sync parent missing")?;
        plain_dirs(parent)?;
        File::open(parent)?.sync_all()?;
    }
    #[cfg(not(unix))]
    let _ = (root, members);
    check(cancel)
}

fn make_parents(root: &Path, path: &Path) -> Result<()> {
    let parent = path.parent().context("missing parent")?;
    let relative = parent.strip_prefix(root)?;
    let mut p = root.to_path_buf();
    for c in relative.components() {
        p.push(c);
        if !p.exists() {
            fs::create_dir(&p)?;
            private(&p, true)?;
        }
        plain_dirs(&p)?;
    }
    Ok(())
}
fn private(path: &Path, executable: bool) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if executable { 0o700 } else { 0o600 }),
        )?;
    }
    #[cfg(not(unix))]
    let _ = (path, executable);
    Ok(())
}
/// `expected` must come from the service-owned receipt, not mutable package metadata.
pub fn verify_installed(
    root: &Path,
    expected: &VerifiedPackage,
    cancel: &AtomicBool,
) -> Result<VerifiedPackage> {
    plain_dirs(root)?;
    ensure!(
        expected.members.len() <= 100_000,
        "runtime receipt member limit"
    );
    validate_members(&expected.members)?;
    check_extra(root, root, &expected.members, cancel, &mut 0)?;
    for (name, member) in &expected.members {
        check(cancel)?;
        let path = root.join(name);
        plain_dirs(path.parent().context("missing parent")?)?;
        let m = fs::symlink_metadata(&path)?;
        match member {
            Member::Directory => ensure!(
                m.is_dir() && !m.file_type().is_symlink(),
                "runtime directory changed"
            ),
            Member::Symlink { target } => ensure!(
                m.file_type().is_symlink()
                    && fs::read_link(&path)?.to_str() == Some(target.as_str())
                    && path.canonicalize()?.starts_with(root.canonicalize()?),
                "runtime link changed"
            ),
            Member::File {
                size,
                sha256,
                executable,
            } => {
                ensure!(
                    m.is_file() && !m.file_type().is_symlink() && m.len() == *size,
                    "runtime file changed"
                );
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    ensure!(
                        (m.permissions().mode() & 0o111 != 0) == *executable,
                        "runtime permissions changed"
                    );
                }
                #[cfg(not(unix))]
                let _ = executable;
                ensure!(
                    hash(File::open(path)?, cancel)? == *sha256,
                    "runtime content changed"
                );
            }
        }
    }
    ensure!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(root.join("provenance.json"))?)?
            == expected.provenance
            && metadata(root, &expected.members)? == expected.manifest
            && expected.version == expected.manifest.version
            && digest(&expected.archive_sha256),
        "runtime receipt mismatch"
    );
    Ok(expected.clone())
}

fn check_extra(
    root: &Path,
    dir: &Path,
    members: &BTreeMap<String, Member>,
    cancel: &AtomicBool,
    count: &mut usize,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        check(cancel)?;
        *count += 1;
        ensure!(*count <= 200_000, "installed member count limit");
        let path = entry?.path();
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .context("invalid installed name")?
            .replace('\\', "/");
        let m = fs::symlink_metadata(&path)?;
        if relative == "browsers" {
            plain_dirs(&path)?;
            continue;
        }
        if relative == ".archive-sha256" {
            ensure!(
                m.is_file() && !m.file_type().is_symlink() && m.len() <= 128,
                "invalid legacy marker"
            );
            continue;
        }
        ensure!(
            members.contains_key(&relative)
                || (m.is_dir()
                    && !m.file_type().is_symlink()
                    && members
                        .keys()
                        .any(|n| n.starts_with(&format!("{relative}/")))),
            "unrecorded installed member"
        );
        if m.is_dir() && !m.file_type().is_symlink() {
            plain_dirs(&path)?;
            check_extra(root, &path, members, cancel, count)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bytes_hash(b: &[u8]) -> String {
        format!("{:x}", Sha256::digest(b))
    }
    fn fixture(extra: Vec<(&str, Vec<u8>, u8)>) -> (tempfile::TempDir, std::path::PathBuf, String) {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("package.tar.gz");
        let out = flate2::write::GzEncoder::new(
            File::create(&archive).unwrap(),
            flate2::Compression::fast(),
        );
        let mut tar = tar::Builder::new(out);
        let node = b"bundled-node".to_vec();
        let entry = b"compiled-js".to_vec();
        let lock = b"locked".to_vec();
        let host = format!(
            "{}-{}",
            match std::env::consts::OS {
                "macos" => "darwin",
                v => v,
            },
            match (std::env::consts::OS, std::env::consts::ARCH) {
                ("macos", "aarch64") => "arm64",
                ("windows", "x86_64") => "amd64",
                (_, v) => v,
            }
        );
        let manifest = serde_json::json!({"schema_version":1,"engine":"midscene","version":"2.0.0","entry":"dist/executor.js","node":"node"});
        let provenance = serde_json::json!({"schema_version":1,"version":"2.0.0","target":host,"source_commit":"a".repeat(40),"node_version":"v24.1.0","compiled_sha256":{"dist/executor.js":bytes_hash(&entry)},"node_sha256":bytes_hash(&node),"lock_sha256":bytes_hash(&lock)});
        let mut entries = vec![
            ("node", node, 0),
            ("dist/executor.js", entry, 0),
            ("pnpm-lock.yaml", lock, 0),
            ("runtime.json", manifest.to_string().into_bytes(), 0),
            ("provenance.json", provenance.to_string().into_bytes(), 0),
            ("node_modules/package/index.js", b"support".to_vec(), 0),
        ];
        entries.extend(extra);
        for (name, data, kind) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_path(name).unwrap();
            h.set_mode(0o755);
            h.set_size(if kind == b'2' { 0 } else { data.len() as u64 });
            if kind == b'2' {
                h.set_link_name(std::str::from_utf8(&data).unwrap())
                    .unwrap();
            }
            h.set_entry_type(tar::EntryType::new(if kind == 0 { b'0' } else { kind }));
            h.set_cksum();
            tar.append(
                &h,
                if kind == b'2' {
                    &[][..]
                } else {
                    data.as_slice()
                },
            )
            .unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
        let digest = bytes_hash(&fs::read(&archive).unwrap());
        (dir, archive, digest)
    }
    fn stage(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().canonicalize().unwrap().join("stage")
    }
    #[test]
    #[ignore = "requires explicitly selected standalone archive and trusted digest"]
    fn selected_standalone_archive() {
        let archive = std::env::var("QINAEGIS_TEST_ARCHIVE").unwrap();
        let expected = std::env::var("QINAEGIS_TEST_ARCHIVE_SHA256").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let stage = dir.path().canonicalize().unwrap().join("runtime");
        let receipt = verify_and_extract(
            Path::new(&archive),
            &expected,
            &stage,
            &AtomicBool::new(false),
        )
        .unwrap();
        verify_installed(&stage, &receipt, &AtomicBool::new(false)).unwrap();
        assert!(receipt.members.len() > 100);
    }
    #[test]
    fn installs_and_revalidates_every_dependency_then_detects_tamper() {
        let (d, a, h) = fixture(vec![]);
        let s = stage(&d);
        let p = verify_and_extract(&a, &h, &s, &AtomicBool::new(false)).unwrap();
        verify_installed(&s, &p, &AtomicBool::new(false)).unwrap();
        fs::write(s.join("node_modules/package/index.js"), b"changed").unwrap();
        assert!(verify_installed(&s, &p, &AtomicBool::new(false)).is_err());
    }
    #[test]
    fn rejects_bad_checksum_and_preexisting_destination() {
        let (d, a, h) = fixture(vec![]);
        let s = stage(&d);
        assert!(verify_and_extract(&a, &"0".repeat(64), &s, &AtomicBool::new(false)).is_err());
        assert!(!s.exists());
        fs::create_dir(&s).unwrap();
        fs::write(s.join("keep"), b"safe").unwrap();
        assert!(verify_and_extract(&a, &h, &s, &AtomicBool::new(false)).is_err());
        assert_eq!(fs::read(s.join("keep")).unwrap(), b"safe");
    }
    #[test]
    fn rejects_duplicates_hardlinks_and_reserved_paths() {
        for extra in [
            vec![("NODE", vec![], 0)],
            vec![("bad", vec![], b'1')],
            vec![(".archive-sha256", vec![], 0)],
            vec![("browsers/a", vec![], 0)],
            vec![("node_modules/CON.js", vec![], 0)],
        ] {
            let (d, a, h) = fixture(extra);
            let s = stage(&d);
            assert!(verify_and_extract(&a, &h, &s, &AtomicBool::new(false)).is_err());
            assert!(!s.exists());
        }
    }
    #[test]
    fn cancelled_import_leaves_destination_absent() {
        let (d, a, h) = fixture(vec![]);
        let s = stage(&d);
        assert!(verify_and_extract(&a, &h, &s, &AtomicBool::new(true)).is_err());
        assert!(!s.exists());
    }
    #[test]
    fn rejects_unrecorded_files_but_accepts_browser_cache() {
        let (d, a, h) = fixture(vec![]);
        let s = stage(&d);
        let p = verify_and_extract(&a, &h, &s, &AtomicBool::new(false)).unwrap();
        fs::create_dir(s.join("browsers")).unwrap();
        fs::write(s.join("browsers/cache"), b"mutable").unwrap();
        verify_installed(&s, &p, &AtomicBool::new(false)).unwrap();
        fs::write(s.join("surprise.js"), b"extra").unwrap();
        assert!(verify_installed(&s, &p, &AtomicBool::new(false)).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn links_are_created_last_and_escape_is_rejected() {
        let (d, a, h) = fixture(vec![
            ("alias-a", b"alias-b".to_vec(), b'2'),
            ("alias-b", b"node".to_vec(), b'2'),
        ]);
        let s = stage(&d);
        let p = verify_and_extract(&a, &h, &s, &AtomicBool::new(false)).unwrap();
        verify_installed(&s, &p, &AtomicBool::new(false)).unwrap();
        let (d, a, h) = fixture(vec![("escape", b"../outside".to_vec(), b'2')]);
        let s = stage(&d);
        assert!(verify_and_extract(&a, &h, &s, &AtomicBool::new(false)).is_err());
        assert!(!s.exists());
    }
    #[test]
    fn oversized_pax_is_rejected_before_parser_allocation() {
        let (d, a, h) = fixture(vec![("pax-header", vec![b'x'; 65537], b'x')]);
        let s = stage(&d);
        assert!(verify_and_extract(&a, &h, &s, &AtomicBool::new(false))
            .unwrap_err()
            .to_string()
            .contains("header size"));
        assert!(!s.exists());
    }
    #[test]
    fn installation_sync_preserves_contents_excludes_cache_and_checks_cancellation() {
        let (dir, archive, digest) = fixture(vec![]);
        let stage = stage(&dir);
        let cancel = AtomicBool::new(false);
        let receipt = verify_and_extract(&archive, &digest, &stage, &cancel).unwrap();
        fs::create_dir(stage.join("browsers")).unwrap();
        fs::write(
            stage.join("browsers/unmanaged-cache"),
            b"mutable browser cache",
        )
        .unwrap();
        let before = fs::read(stage.join("node")).unwrap();
        sync_verified_installation(&stage, &receipt, &cancel).unwrap();
        assert_eq!(fs::read(stage.join("node")).unwrap(), before);
        verify_installed(&stage, &receipt, &cancel).unwrap();
        cancel.store(true, Ordering::Release);
        assert!(sync_verified_installation(&stage, &receipt, &cancel).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn installation_sync_rejects_replaced_ancestor_without_following_cache_links() {
        let (dir, archive, digest) = fixture(vec![]);
        let stage = stage(&dir);
        let cancel = AtomicBool::new(false);
        let receipt = verify_and_extract(&archive, &digest, &stage, &cancel).unwrap();
        fs::create_dir(stage.join("browsers")).unwrap();
        std::os::unix::fs::symlink(
            "/not-a-real-browser-cache-target",
            stage.join("browsers/link"),
        )
        .unwrap();
        sync_verified_installation(&stage, &receipt, &cancel).unwrap();
        let old = stage.join("old-modules");
        fs::rename(stage.join("node_modules"), &old).unwrap();
        std::os::unix::fs::symlink(&old, stage.join("node_modules")).unwrap();
        assert!(sync_directories(&stage, &receipt.members, &cancel).is_err());
    }
    #[test]
    fn paths_and_links_reject_escape_cycles() {
        for p in [
            "../a",
            "/a",
            "a//b",
            "a/./b",
            "a\\b",
            "a:b",
            "a/COM1.js",
            "a/trailing.",
        ] {
            assert!(safe(p).is_err(), "{p}");
        }
        assert!(target("link", "../../outside").is_err());
        let m = BTreeMap::from([
            ("a".into(), Member::Symlink { target: "b".into() }),
            ("b".into(), Member::Symlink { target: "a".into() }),
        ]);
        assert!(validate_members(&m).is_err());
    }
}
