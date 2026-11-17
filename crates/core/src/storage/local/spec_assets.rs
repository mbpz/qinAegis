//! Fixed project specifications with private atomic writes and exact revisions.
use crate::asset_file::AssetFile;
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
pub const MAX_SPEC: usize = 128 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSpec {
    pub project: String,
    pub exists: bool,
    pub markdown: String,
    pub revision: Option<String>,
}
impl ProjectSpec {
    pub fn validate_snapshot(&self, project: &str) -> Result<()> {
        ensure!(
            self.project == project && self.exists,
            "specification project mismatch or missing"
        );
        validate(&self.markdown)?;
        ensure!(
            self.revision.as_deref() == Some(STANDARD.encode(self.markdown.as_bytes()).as_str()),
            "specification revision mismatch"
        );
        Ok(())
    }
}
fn validate(markdown: &str) -> Result<()> {
    ensure!(markdown.len() <= MAX_SPEC, "specification exceeds 128 KiB");
    ensure!(
        !markdown.trim().is_empty() && !markdown.contains('\0'),
        "specification must be nonempty and contain no NUL"
    );
    Ok(())
}
pub(super) struct SpecAssets {
    root: PathBuf,
    project: String,
}
impl SpecAssets {
    pub(super) fn new(root: PathBuf, project: &str) -> Result<Self> {
        ensure!(
            !project.is_empty()
                && project.len() <= 128
                && !project.starts_with('.')
                && project
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_')),
            "invalid project identity"
        );
        ensure!(root.is_absolute(), "project root must be absolute");
        for path in root.ancestors().take(3) {
            directory(path)?;
        }
        let root = root.canonicalize()?;
        for path in root.ancestors() {
            directory(path)?;
        }
        let config = AssetFile::new(root.join("config.yaml"), MAX_SPEC);
        let bytes = config.lock()?.read()?.context("project does not exist")?;
        let config: crate::storage::ProjectConfig = serde_yaml::from_slice(&bytes)?;
        ensure!(
            config.name == project,
            "project configuration identity mismatch"
        );
        Ok(Self {
            root,
            project: project.into(),
        })
    }
    fn asset(&self) -> AssetFile {
        AssetFile::new(self.root.join("spec.md"), MAX_SPEC)
    }
    fn result(&self, bytes: Option<Vec<u8>>) -> Result<ProjectSpec> {
        let exists = bytes.is_some();
        let revision = bytes.as_ref().map(|b| STANDARD.encode(b));
        let markdown =
            String::from_utf8(bytes.unwrap_or_default()).context("specification is not UTF-8")?;
        ensure!(!markdown.contains('\0'), "specification contains NUL");
        Ok(ProjectSpec {
            project: self.project.clone(),
            exists,
            markdown,
            revision,
        })
    }
    pub(super) fn get(&self) -> Result<ProjectSpec> {
        self.result(self.asset().lock()?.read()?)
    }
    pub(super) fn save(&self, expected: Option<&str>, markdown: &str) -> Result<ProjectSpec> {
        validate(markdown)?;
        let asset = self.asset();
        let guard = asset.lock()?;
        let before = guard.read()?;
        ensure!(
            before.as_ref().map(|b| STANDARD.encode(b)).as_deref() == expected,
            "specification revision conflict; reload before saving"
        );
        if before.as_deref() != Some(markdown.as_bytes()) {
            guard.write(markdown.as_bytes())?;
        }
        self.result(Some(markdown.as_bytes().to_vec()))
    }
    pub(super) fn save_current(&self, markdown: &str) -> Result<()> {
        validate(markdown)?;
        self.asset().lock()?.write(markdown.as_bytes())
    }
}
fn directory(path: &Path) -> Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    ensure!(
        m.is_dir() && !m.file_type().is_symlink(),
        "unsafe specification ancestor"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.yaml"),
            "name: alpha\nurl: https://alpha.invalid\ntech_stack: []\ncreated_at: now\n",
        )
        .unwrap();
        dir
    }
    #[test]
    fn cas_missing_empty_limits_and_failure_preserve_original() {
        let dir = fixture();
        let assets = SpecAssets::new(dir.path().to_path_buf(), "alpha").unwrap();
        assert!(!assets.get().unwrap().exists);
        let first = assets.save(None, "# First").unwrap();
        assert!(assets.save(None, "# Lost").is_err());
        assert!(assets.save(first.revision.as_deref(), " ").is_err());
        assert!(assets
            .save(first.revision.as_deref(), "bad\0content")
            .is_err());
        assert!(assets
            .save(first.revision.as_deref(), &"x".repeat(MAX_SPEC + 1))
            .is_err());
        assert_eq!(assets.get().unwrap().markdown, "# First");
        assert_eq!(
            assets
                .save(first.revision.as_deref(), "# First")
                .unwrap()
                .revision,
            first.revision
        );
        let second = assets.save(first.revision.as_deref(), "# Second").unwrap();
        assert!(assets.save(first.revision.as_deref(), "# Lost").is_err());
        second.validate_snapshot("alpha").unwrap();
        assert!(second.validate_snapshot("beta").is_err());
        std::fs::write(dir.path().join("spec.md"), b"").unwrap();
        let empty = assets.get().unwrap();
        assert!(empty.exists);
        assert_eq!(empty.revision, Some(String::new()));
        assets.save(empty.revision.as_deref(), "corrected").unwrap();
        std::fs::write(dir.path().join("spec.md"), [255]).unwrap();
        assert!(assets.get().is_err());
        assert!(SpecAssets::new(dir.path().join("missing"), "missing").is_err());
        assert!(SpecAssets::new(dir.path().to_path_buf(), "beta").is_err());
        assert!(SpecAssets::new(dir.path().to_path_buf(), "../alpha").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn links_and_unsafe_ancestors_are_rejected() {
        let dir = fixture();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("value"), b"preserve").unwrap();
        std::os::unix::fs::symlink(outside.path().join("value"), dir.path().join("spec.md"))
            .unwrap();
        let assets = SpecAssets::new(dir.path().to_path_buf(), "alpha").unwrap();
        assert!(assets.get().is_err());
        assert!(assets.save_current("replace").is_err());
        std::fs::remove_file(dir.path().join("spec.md")).unwrap();
        std::fs::hard_link(outside.path().join("value"), dir.path().join("spec.md")).unwrap();
        assert!(assets.get().is_err());
        assert!(assets.save_current("replace").is_err());
        assert_eq!(
            std::fs::read(outside.path().join("value")).unwrap(),
            b"preserve"
        );
        let alias = outside.path().join("alias");
        std::os::unix::fs::symlink(dir.path(), &alias).unwrap();
        assert!(SpecAssets::new(alias, "alpha").is_err());
    }
}
