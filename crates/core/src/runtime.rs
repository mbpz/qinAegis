//! Resolve an installed Playwright/Midscene engine without depending on a source checkout.
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RuntimeCommand {
    pub directory: PathBuf,
    pub node: PathBuf,
    pub arguments: Vec<PathBuf>,
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    engine: String,
    version: String,
    entry: PathBuf,
    node: Option<PathBuf>,
}

pub struct RuntimeResolver;
impl RuntimeResolver {
    /// Explicit overrides fail closed; an invalid selected engine never silently falls back.
    pub fn resolve() -> Result<RuntimeCommand> {
        if let Some(root) = std::env::var_os("QINAEGIS_RUNTIME_DIR") {
            return Self::installed(Path::new(&root));
        }
        if let Some(root) = std::env::var_os("QINAEGIS_DEV_SANDBOX") {
            return Self::developer(Path::new(&root));
        }
        Self::installed(&crate::LocalStorage::base_path().join("runtimes/midscene/current"))
            .context("Midscene engine unavailable; install its runtime or set QINAEGIS_RUNTIME_DIR")
    }

    pub fn installed(root: &Path) -> Result<RuntimeCommand> {
        let root = absolute_root(root)?;
        let manifest_path = confined_file(&root, Path::new("runtime.json"))?;
        let manifest: Manifest = serde_json::from_slice(&std::fs::read(manifest_path)?)
            .context("invalid runtime.json")?;
        if manifest.schema_version != 1 || manifest.engine != "midscene" {
            bail!("unsupported engine manifest schema or engine");
        }
        if manifest.version.is_empty()
            || manifest.version.len() > 64
            || !manifest
                .version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        {
            bail!("invalid engine version");
        }
        if !matches!(
            manifest.entry.extension().and_then(|v| v.to_str()),
            Some("js" | "mjs" | "cjs")
        ) {
            bail!("installed runtime entry must be compiled JavaScript");
        }
        let entry = confined_file(&root, &manifest.entry)?;
        let node = match manifest.node {
            Some(path) => confined_file(&root, &path)?,
            None => find_node().context(
                "compatible Node runtime not found; install Node or bundle it in the engine",
            )?,
        };
        executable(&node)?;
        Ok(RuntimeCommand {
            directory: root,
            node,
            arguments: vec![entry],
            version: manifest.version,
        })
    }

    /// Developer mode is explicit and requires a fully installed sandbox.
    pub fn developer(root: &Path) -> Result<RuntimeCommand> {
        let root = absolute_root(root)?;
        let entry = confined_file(&root, Path::new("src/executor.ts"))?;
        let tsx = confined_file(&root, Path::new("node_modules/tsx/dist/cli.mjs"))?;
        let node = find_node().context("Node runtime not found for developer sandbox")?;
        Ok(RuntimeCommand {
            directory: root,
            node,
            arguments: vec![tsx, entry],
            version: "developer".into(),
        })
    }
}

fn absolute_root(root: &Path) -> Result<PathBuf> {
    if !root.is_absolute() {
        bail!("runtime directory must be absolute");
    }
    let root = root
        .canonicalize()
        .context("runtime directory does not exist")?;
    if !root.is_dir() {
        bail!("runtime root must be a directory");
    }
    Ok(root)
}

fn confined_file(root: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.as_os_str().is_empty()
        || !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        bail!("runtime paths must be confined relative paths");
    }
    let path = root
        .join(relative)
        .canonicalize()
        .context("required runtime file missing")?;
    if !path.starts_with(root) || !path.is_file() {
        bail!("runtime file escapes its installation directory");
    }
    Ok(path)
}

fn executable(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("Node executable missing");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if path.metadata()?.permissions().mode() & 0o111 == 0 {
            bail!("Node runtime is not executable");
        }
    }
    Ok(())
}

fn find_node() -> Option<PathBuf> {
    let executable_name = if cfg!(windows) { "node.exe" } else { "node" };
    let mut directories: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p)
                .filter(|p| p.is_absolute())
                .collect()
        })
        .unwrap_or_default();
    if !cfg!(windows) {
        directories.extend(
            [
                "/opt/homebrew/bin",
                "/usr/local/bin",
                "/usr/bin",
                "/opt/local/bin",
            ]
            .map(PathBuf::from),
        );
        if let Some(home) = dirs::home_dir() {
            directories.extend([home.join(".n/bin"), home.join(".local/bin")]);
        }
    }
    directories
        .into_iter()
        .map(|dir| dir.join(executable_name))
        .find(|p| executable(p).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("dist")).unwrap();
        std::fs::write(dir.path().join("dist/executor.js"), "// fixture").unwrap();
        std::fs::write(dir.path().join("node"), "fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                dir.path().join("node"),
                std::fs::Permissions::from_mode(0o700),
            )
            .unwrap();
        }
        manifest(dir.path(), "dist/executor.js");
        dir
    }
    fn manifest(root: &Path, entry: &str) {
        std::fs::write(root.join("runtime.json"),json!({"schema_version":1,"engine":"midscene","version":"2.0.0","entry":entry,"node":"node"}).to_string()).unwrap();
    }
    #[test]
    fn resolves_installed_absolute_entry_without_source_or_cwd() {
        let dir = fixture();
        let resolved = RuntimeResolver::installed(dir.path()).unwrap();
        assert!(resolved.node.is_absolute());
        assert!(resolved.arguments[0].is_absolute());
        assert_eq!(
            resolved.arguments[0],
            dir.path().canonicalize().unwrap().join("dist/executor.js")
        );
        assert_eq!(resolved.version, "2.0.0");
    }
    #[test]
    fn rejects_relative_root_and_source_only_installed_entry() {
        assert!(RuntimeResolver::installed(Path::new("sandbox")).is_err());
        let dir = fixture();
        manifest(dir.path(), "src/executor.ts");
        assert!(RuntimeResolver::installed(dir.path()).is_err());
    }
    #[test]
    fn rejects_traversal_and_absolute_entry() {
        let dir = fixture();
        for entry in [
            "../executor.js",
            "/tmp/executor.js",
            "dist/../dist/executor.js",
        ] {
            manifest(dir.path(), entry);
            assert!(RuntimeResolver::installed(dir.path()).is_err());
        }
    }
    #[test]
    fn requires_manifest_and_files() {
        let dir = fixture();
        std::fs::remove_file(dir.path().join("dist/executor.js")).unwrap();
        assert!(RuntimeResolver::installed(dir.path()).is_err());
        std::fs::remove_file(dir.path().join("runtime.json")).unwrap();
        assert!(RuntimeResolver::installed(dir.path()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlink_file_escape() {
        let dir = fixture();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("executor.js"), "").unwrap();
        std::fs::remove_file(dir.path().join("dist/executor.js")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("executor.js"),
            dir.path().join("dist/executor.js"),
        )
        .unwrap();
        assert!(RuntimeResolver::installed(dir.path()).is_err());
    }
    #[test]
    fn developer_requires_installed_tsx_and_explicit_absolute_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/executor.ts"), "").unwrap();
        assert!(RuntimeResolver::developer(dir.path()).is_err());
    }
}
