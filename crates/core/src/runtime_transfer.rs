//! One bounded, resumable native runtime transfer. Client paths are never accepted.
use crate::asset_file::AssetFile;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const CHUNK_BYTES: usize = 512 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Upload {
    pub upload_id: String,
    pub size: u64,
    pub expected_sha256: String,
    pub expires_at: u64,
    pub claimed: bool,
    pub received: u64,
}
pub struct TransferStore {
    root: PathBuf,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn plain_file(path: &Path) -> Result<Option<std::fs::Metadata>> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            ensure!(
                meta.is_file() && !meta.file_type().is_symlink(),
                "unsafe runtime transfer file"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                ensure!(meta.nlink() == 1, "runtime transfer has multiple links");
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                ensure!(
                    meta.file_attributes() & 0x400 == 0,
                    "runtime transfer is a reparse point"
                );
            }
            Ok(Some(meta))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
impl TransferStore {
    pub fn new(base: &Path) -> Result<Self> {
        ensure!(base.is_absolute(), "runtime data path must be absolute");
        let root = base.join("runtimes/midscene/transfers");
        for ancestor in root.ancestors().collect::<Vec<_>>().into_iter().rev() {
            match std::fs::symlink_metadata(ancestor) {
                Ok(m) => {
                    ensure!(
                        m.is_dir() && !m.file_type().is_symlink(),
                        "unsafe runtime transfer directory"
                    );
                    #[cfg(windows)]
                    {
                        use std::os::windows::fs::MetadataExt;
                        ensure!(
                            m.file_attributes() & 0x400 == 0,
                            "runtime transfer directory is a reparse point"
                        );
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    std::fs::create_dir(ancestor)?;
                }
                Err(e) => return Err(e.into()),
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self { root })
    }
    fn asset(&self) -> AssetFile {
        AssetFile::new(self.root.join("upload.json"), 4096)
    }
    fn path(&self) -> PathBuf {
        self.root.join("upload.bin")
    }
    fn decode(&self, bytes: Option<Vec<u8>>) -> Result<Option<Upload>> {
        let Some(bytes) = bytes else { return Ok(None) };
        let state: Option<Upload> =
            serde_json::from_slice(&bytes).context("invalid runtime transfer state")?;
        if let Some(s) = &state {
            ensure!(
                s.upload_id.len() == 64
                    && s.upload_id.bytes().all(|b| b.is_ascii_hexdigit())
                    && s.size > 0
                    && s.size <= crate::runtime_package::MAX_ARCHIVE_BYTES
                    && s.expected_sha256.len() == 64
                    && s.expected_sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid runtime transfer state"
            );
        }
        Ok(state)
    }
    pub fn inspect(&self) -> Result<Option<Upload>> {
        let asset = self.asset();
        let guard = asset.lock()?;
        let mut state = self.decode(guard.read()?)?;
        if let Some(s) = &mut state {
            s.received = plain_file(&self.path())?.map(|m| m.len()).unwrap_or(0);
            ensure!(
                s.received <= s.size,
                "runtime upload exceeds its declared size"
            );
        }
        Ok(state)
    }
    pub fn begin(&self, id: &str, size: u64, sha: &str) -> Result<Upload> {
        ensure!(
            id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid upload identity"
        );
        ensure!(
            size > 0 && size <= crate::runtime_package::MAX_ARCHIVE_BYTES,
            "runtime archive must be between 1 byte and 1 GiB"
        );
        ensure!(
            sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
            "trusted expected SHA-256 required"
        );
        let asset = self.asset();
        let guard = asset.lock()?;
        ensure!(
            self.decode(guard.read()?)?.is_none() && plain_file(&self.path())?.is_none(),
            "An upload already exists; explicitly discard it before starting another"
        );
        let state = Upload {
            upload_id: id.into(),
            size,
            expected_sha256: sha.to_ascii_lowercase(),
            expires_at: now() + 1800,
            claimed: false,
            received: 0,
        };
        // Journal first: a crash before file creation still leaves an explicit abort handle.
        guard.write(&serde_json::to_vec(&Some(&state))?)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(self.path())?.sync_all()?;
        Ok(state)
    }
    pub fn append(&self, id: &str, offset: u64, bytes: &[u8]) -> Result<u64> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= CHUNK_BYTES,
            "runtime upload chunk exceeds 512 KiB"
        );
        let asset = self.asset();
        let guard = asset.lock()?;
        let state = self
            .decode(guard.read()?)?
            .context("runtime upload unavailable")?;
        ensure!(
            state.upload_id == id && !state.claimed && now() <= state.expires_at,
            "runtime upload expired or owned by verification"
        );
        let before = plain_file(&self.path())?.context("runtime upload file missing")?;
        ensure!(
            before.len() == offset
                && offset
                    .checked_add(bytes.len() as u64)
                    .is_some_and(|n| n <= state.size),
            "runtime upload offset or size mismatch"
        );
        let mut file = OpenOptions::new().append(true).open(self.path())?;
        let after = plain_file(&self.path())?.context("runtime upload disappeared")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let opened = file.metadata()?;
            ensure!(
                before.dev() == after.dev()
                    && before.ino() == after.ino()
                    && opened.dev() == after.dev()
                    && opened.ino() == after.ino(),
                "runtime upload replaced while opening"
            );
        }
        ensure!(
            file.metadata()?.len() == offset,
            "runtime upload changed while opening"
        );
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(offset + bytes.len() as u64)
    }
    pub fn claim(&self, id: &str) -> Result<(PathBuf, String)> {
        let asset = self.asset();
        let guard = asset.lock()?;
        let mut state = self
            .decode(guard.read()?)?
            .context("runtime upload unavailable")?;
        ensure!(
            state.upload_id == id && !state.claimed && now() <= state.expires_at,
            "runtime upload expired or already verifying"
        );
        ensure!(
            plain_file(&self.path())?.is_some_and(|m| m.len() == state.size),
            "runtime upload is incomplete"
        );
        state.claimed = true;
        guard.write(&serde_json::to_vec(&Some(&state))?)?;
        Ok((self.path(), state.expected_sha256))
    }
    pub fn release(&self, id: &str) -> Result<()> {
        let asset = self.asset();
        let guard = asset.lock()?;
        let mut state = self
            .decode(guard.read()?)?
            .context("runtime upload unavailable")?;
        ensure!(state.upload_id == id, "runtime upload changed");
        state.claimed = false;
        guard.write(&serde_json::to_vec(&Some(state))?)
    }
    pub fn abort(&self, id: &str) -> Result<()> {
        let asset = self.asset();
        let guard = asset.lock()?;
        let state = self
            .decode(guard.read()?)?
            .context("runtime upload unavailable")?;
        ensure!(
            state.upload_id == id && !state.claimed,
            "Stop verification before discarding the runtime upload"
        );
        if plain_file(&self.path())?.is_some() {
            std::fs::remove_file(self.path())?;
        }
        guard.write(b"null")?;
        #[cfg(unix)]
        File::open(&self.root)?.sync_all()?;
        Ok(())
    }
    /// Instance lock is already held. A restart interrupts verification, never replays it.
    pub fn recover(&self) -> Result<()> {
        let asset = self.asset();
        let guard = asset.lock()?;
        if let Some(mut state) = self.decode(guard.read()?)? {
            state.claimed = false;
            guard.write(&serde_json::to_vec(&Some(state))?)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upload_is_bounded_ordered_and_survives_restart_without_replaying_verification() {
        let temp = tempfile::tempdir().unwrap();
        let s = TransferStore::new(&temp.path().canonicalize().unwrap()).unwrap();
        let id = "a".repeat(64);
        assert!(s.begin(&id, 0, &id).is_err());
        s.begin(&id, 6, &id).unwrap();
        assert!(s.begin(&id, 6, &id).is_err());
        assert!(s.append(&id, 1, b"abc").is_err());
        s.append(&id, 0, b"abc").unwrap();
        assert!(s.claim(&id).is_err());
        s.append(&id, 3, b"def").unwrap();
        s.claim(&id).unwrap();
        assert!(s.abort(&id).is_err());
        assert!(s.append(&id, 6, b"x").is_err());
        s.recover().unwrap();
        assert_eq!(s.inspect().unwrap().unwrap().received, 6);
        s.abort(&id).unwrap();
        assert!(s.inspect().unwrap().is_none());
    }
    #[cfg(unix)]
    #[test]
    fn linked_upload_is_never_appended_or_removed() {
        let temp = tempfile::tempdir().unwrap();
        let s = TransferStore::new(&temp.path().canonicalize().unwrap()).unwrap();
        let id = "b".repeat(64);
        s.begin(&id, 3, &id).unwrap();
        let victim = temp.path().join("victim");
        std::fs::write(&victim, b"safe").unwrap();
        std::fs::remove_file(s.path()).unwrap();
        std::os::unix::fs::symlink(&victim, s.path()).unwrap();
        assert!(s.append(&id, 0, b"abc").is_err());
        assert!(s.abort(&id).is_err());
        assert_eq!(std::fs::read(victim).unwrap(), b"safe");
    }
}
