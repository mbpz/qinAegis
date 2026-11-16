//! Private, bounded JSON asset files, serialized across processes.
use anyhow::{ensure, Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub(crate) struct AssetFile {
    path: PathBuf,
    max_bytes: usize,
}

pub(crate) struct AssetGuard<'a> {
    asset: &'a AssetFile,
    _lock: File,
}

fn regular(path: &Path) -> Result<Option<std::fs::Metadata>> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            ensure!(
                meta.is_file() && !meta.file_type().is_symlink(),
                "unsafe asset file: {}",
                path.display()
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                ensure!(meta.nlink() == 1, "asset file has multiple links");
            }
            Ok(Some(meta))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn private_open(path: &Path, create: bool, fresh: bool) -> Result<File> {
    let before = regular(path)?;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(create)
        .create(create)
        .create_new(fresh);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    let after = regular(path)?.context("asset file disappeared")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let opened = file.metadata()?;
        ensure!(
            opened.dev() == after.dev() && opened.ino() == after.ino(),
            "asset file changed while opening"
        );
        if let Some(before) = before {
            ensure!(
                before.dev() == after.dev() && before.ino() == after.ino(),
                "asset file replaced while opening"
            );
        }
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = (before, after);
    Ok(file)
}

impl AssetFile {
    pub(crate) fn new(path: PathBuf, max_bytes: usize) -> Self {
        Self { path, max_bytes }
    }

    pub(crate) fn lock(&self) -> Result<AssetGuard<'_>> {
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .context("asset path needs a parent directory")?;
        if let Ok(meta) = std::fs::symlink_metadata(parent) {
            ensure!(
                meta.is_dir() && !meta.file_type().is_symlink(),
                "unsafe asset directory"
            );
        }
        std::fs::create_dir_all(parent)?;
        let meta = std::fs::symlink_metadata(parent)?;
        ensure!(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "unsafe asset directory"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
        regular(&self.path)?;
        let lock_path = self.path.with_extension("lock");
        let lock = private_open(&lock_path, true, false)?;
        lock.try_lock().context("asset library is busy")?;
        Ok(AssetGuard {
            asset: self,
            _lock: lock,
        })
    }
}

impl AssetGuard<'_> {
    pub(crate) fn read(&self) -> Result<Option<Vec<u8>>> {
        let Some(meta) = regular(&self.asset.path)? else {
            return Ok(None);
        };
        ensure!(
            meta.len() <= self.asset.max_bytes as u64,
            "asset file exceeds size limit"
        );
        let file = private_open(&self.asset.path, false, false)?;
        let mut bytes = Vec::new();
        file.take(self.asset.max_bytes as u64 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= self.asset.max_bytes,
            "asset file exceeds size limit"
        );
        Ok(Some(bytes))
    }

    pub(crate) fn write(&self, bytes: &[u8]) -> Result<()> {
        ensure!(
            bytes.len() <= self.asset.max_bytes,
            "asset file exceeds size limit"
        );
        regular(&self.asset.path)?;
        let name = self
            .asset
            .path
            .file_name()
            .context("asset path needs a filename")?
            .to_string_lossy();
        let tmp = self.asset.path.with_file_name(format!(
            ".{name}.{}.{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = private_open(&tmp, true, true)?;
        let result = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            regular(&self.asset.path)?;
            std::fs::rename(&tmp, &self.asset.path)?;
            #[cfg(unix)]
            File::open(self.asset.path.parent().unwrap())?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }
}
