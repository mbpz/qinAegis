//! Explicit single-source configuration migration. No source is merged automatically.
use super::app::{parse_config, AppConfig, MAX_CONFIG_BYTES};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretPolicy {
    KeepTarget,
    UseSource,
}

#[derive(Serialize)]
pub struct SourceInfo {
    pub id: String,
    pub label: String,
    pub status: &'static str,
}

pub struct MigrationSource {
    pub info: SourceInfo,
    path: PathBuf,
}
impl MigrationSource {
    pub fn prepare(&self) -> Result<MigrationPlan> {
        MigrationPlan::prepare(&self.path, &AppConfig::global_path())
    }
}

/// Only known product paths and an existing managed project may enter Web migration.
/// The caller receives labels/IDs, never an arbitrary-path input mechanism.
pub fn sources(project: Option<&str>) -> Result<Vec<MigrationSource>> {
    let target = AppConfig::global_path();
    let mut candidates = vec![("current", "Current configuration", target.clone())];
    if let Some(root) = dirs::config_dir() {
        candidates.push((
            "os_global",
            "OS global configuration",
            root.join("qinAegis/config.toml"),
        ));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push((
            "legacy_data",
            "Default user data configuration",
            home.join(".qinAegis/config.toml"),
        ));
    }
    if let Some(project) = project {
        ensure!(
            !project.is_empty()
                && project.len() <= 128
                && !project.starts_with('.')
                && project
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')),
            "invalid migration project"
        );
        ensure!(
            crate::LocalStorage::list_projects()?
                .iter()
                .any(|p| p == project),
            "migration project does not exist"
        );
        candidates.push((
            "project_override",
            "Selected project's legacy override",
            AppConfig::project_path(&crate::LocalStorage::project_dir(project)),
        ));
    }
    let mut result: Vec<MigrationSource> = Vec::new();
    for (id, label, path) in candidates {
        if result.iter().any(|existing| existing.path == path) {
            continue;
        }
        let status = match read_config(&path) {
            Ok(None) => "missing",
            Ok(Some(bytes)) if strict_config(&bytes).is_ok() => "ready",
            _ => "invalid",
        };
        result.push(MigrationSource {
            info: SourceInfo {
                id: id.into(),
                label: label.into(),
                status,
            },
            path,
        });
    }
    Ok(result)
}

fn safe_ancestors(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute(),
        "configuration migration requires a known absolute location"
    );
    for part in path.ancestors() {
        match std::fs::symlink_metadata(part) {
            Ok(meta) => ensure!(
                !meta.file_type().is_symlink(),
                "unsafe configuration migration location"
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => anyhow::bail!("configuration migration location unavailable"),
        }
    }
    Ok(())
}
fn read_config(path: &Path) -> Result<Option<Vec<u8>>> {
    safe_ancestors(path)?;
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => anyhow::bail!("configuration migration source unavailable"),
    };
    ensure!(
        meta.is_file() && meta.len() <= MAX_CONFIG_BYTES as u64,
        "invalid configuration migration file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(meta.nlink() == 1, "unsafe configuration migration file");
    }
    let file = std::fs::File::open(path)
        .map_err(|_| anyhow::anyhow!("configuration migration source unavailable"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let opened = file.metadata()?;
        ensure!(
            opened.dev() == meta.dev() && opened.ino() == meta.ino(),
            "configuration migration source changed"
        );
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow::anyhow!("configuration migration source unavailable"))?;
    ensure!(
        bytes.len() <= MAX_CONFIG_BYTES,
        "configuration migration file exceeds limit"
    );
    safe_ancestors(path)?;
    Ok(Some(bytes))
}
fn strict_config(bytes: &[u8]) -> Result<(AppConfig, u64)> {
    let (config, revision) = parse_config(bytes)
        .map_err(|_| anyhow::anyhow!("invalid or unsupported configuration migration source"))?;
    let value: toml::Value = toml::from_str(std::str::from_utf8(bytes).unwrap())
        .map_err(|_| anyhow::anyhow!("invalid configuration migration source"))?;
    // Reject unknown data instead of silently dropping an unsupported old format.
    for (section, allowed) in [
        (
            None,
            &["llm", "sandbox", "exploration", "qinaegis_revision"][..],
        ),
        (
            Some("llm"),
            &[
                "provider",
                "base_url",
                "api_key",
                "model",
                "secondary_provider",
                "secondary_base_url",
                "secondary_api_key",
                "secondary_model",
            ][..],
        ),
        (Some("sandbox"), &["cdp_port"][..]),
        (
            Some("exploration"),
            &["max_depth", "max_pages_per_seed"][..],
        ),
    ] {
        if let Some(table) = section
            .map(|s| value.get(s))
            .unwrap_or(Some(&value))
            .and_then(toml::Value::as_table)
        {
            ensure!(
                table.keys().all(|key| allowed.contains(&key.as_str())),
                "unsupported configuration fields; migration refused"
            );
        }
    }
    ensure!(
        config.sandbox.cdp_port > 0
            && (1..=10).contains(&config.exploration.max_depth)
            && (1..=1000).contains(&config.exploration.max_pages_per_seed),
        "invalid configuration limits"
    );
    for endpoint in [&config.llm.base_url, &config.llm.secondary_base_url] {
        if endpoint.is_empty() {
            continue;
        }
        let resolved = super::resolve_env_var(endpoint);
        let url = reqwest::Url::parse(&resolved)
            .map_err(|_| anyhow::anyhow!("invalid or unresolved migration endpoint"))?;
        ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none(),
            "invalid migration endpoint"
        );
    }
    Ok((config, revision))
}
fn summary(config: &AppConfig) -> Value {
    let mut safe = config.clone();
    safe.llm.api_key.clear();
    safe.llm.secondary_api_key.clear();
    for endpoint in [&mut safe.llm.base_url, &mut safe.llm.secondary_base_url] {
        if let Ok(mut url) = reqwest::Url::parse(endpoint) {
            url.set_query(None);
            url.set_fragment(None);
            *endpoint = url.to_string();
        }
    }
    let mut value = serde_json::to_value(safe).expect("config serializes");
    // A key accidentally echoed in a label must not escape through the preview.
    let resolved_primary = super::resolve_env_var(&config.llm.api_key);
    let resolved_secondary = super::resolve_env_var(&config.llm.secondary_api_key);
    scrub(
        &mut value,
        &[
            &config.llm.api_key,
            &config.llm.secondary_api_key,
            &resolved_primary,
            &resolved_secondary,
        ],
    );
    value["llm"]["api_key_configured"] = json!(!config.llm.api_key.is_empty());
    value["llm"]["secondary_api_key_configured"] = json!(!config.llm.secondary_api_key.is_empty());
    value
}

fn scrub(value: &mut Value, secrets: &[&str]) {
    match value {
        Value::String(text) => {
            for key in secrets {
                if !key.is_empty() {
                    *text = text.replace(key, "[REDACTED]");
                }
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                scrub(value, secrets);
            }
        }
        Value::Array(values) => {
            for value in values {
                scrub(value, secrets);
            }
        }
        _ => {}
    }
}

pub struct MigrationPlan {
    source: PathBuf,
    target: PathBuf,
    source_bytes: Vec<u8>,
    target_bytes: Option<Vec<u8>>,
    source_config: AppConfig,
    target_config: AppConfig,
    source_revision: u64,
    target_revision: u64,
}
#[derive(Serialize)]
pub struct MigrationResult {
    pub revision: u64,
    pub unchanged: bool,
}
impl MigrationPlan {
    /// Path-based API for trusted callers; Web selects from `sources` only.
    pub fn prepare(source: &Path, target: &Path) -> Result<Self> {
        let source_bytes =
            read_config(source)?.context("configuration migration source missing")?;
        let (source_config, source_revision) = strict_config(&source_bytes)?;
        let target_bytes = read_config(target)?;
        let (target_config, target_revision) = target_bytes
            .as_deref()
            .map(strict_config)
            .transpose()?
            .unwrap_or((AppConfig::default(), 0));
        Ok(Self {
            source: source.into(),
            target: target.into(),
            source_bytes,
            target_bytes,
            source_config,
            target_config,
            source_revision,
            target_revision,
        })
    }
    fn candidate(&self, policy: SecretPolicy) -> AppConfig {
        let mut config = self.source_config.clone();
        if matches!(policy, SecretPolicy::KeepTarget) {
            config.llm.api_key = self.target_config.llm.api_key.clone();
            config.llm.secondary_api_key = self.target_config.llm.secondary_api_key.clone();
        }
        config
    }
    pub fn preview(&self) -> Value {
        let choice = |policy| {
            let config = self.candidate(policy);
            let candidate = serde_json::to_value(&config).expect("config serializes");
            let target = serde_json::to_value(&self.target_config).expect("config serializes");
            let mut changes = Vec::new();
            for section in ["llm", "sandbox", "exploration"] {
                for (field, value) in candidate[section].as_object().unwrap() {
                    if target[section].get(field) != Some(value) {
                        changes.push(format!("{section}.{field}"));
                    }
                }
            }
            json!({"summary":summary(&config),"changed_fields":changes})
        };
        let mut preview = json!({"source_revision":self.source_revision,"target_revision":self.target_revision,
            "target_exists":self.target_bytes.is_some(),"source_summary":summary(&self.source_config),
            "target_summary":summary(&self.target_config),
            "choices":{"keep_target":choice(SecretPolicy::KeepTarget),"use_source":choice(SecretPolicy::UseSource)}});
        // Scrub both origins in both choices, even when one choice keeps the
        // other origin's credentials while retaining labels from the source.
        let keys = [
            &self.source_config.llm.api_key,
            &self.source_config.llm.secondary_api_key,
            &self.target_config.llm.api_key,
            &self.target_config.llm.secondary_api_key,
        ];
        let resolved = keys
            .iter()
            .map(|key| super::resolve_env_var(key))
            .collect::<Vec<_>>();
        let secrets = keys
            .iter()
            .map(|key| key.as_str())
            .chain(resolved.iter().map(String::as_str))
            .collect::<Vec<_>>();
        scrub(&mut preview, &secrets);
        preview
    }
    pub fn apply(&self, policy: SecretPolicy) -> Result<MigrationResult> {
        safe_ancestors(&self.target)?;
        // Same-path legacy format upgrades are checked by the target CAS instead.
        if self.source != self.target {
            ensure!(
                read_config(&self.source)?.as_ref() == Some(&self.source_bytes),
                "migration source changed; preview again"
            );
        }
        let (revision, unchanged) = self
            .candidate(policy)
            .save_migration_checked(&self.target, self.target_revision, &self.target_bytes)
            .map_err(|_| {
                anyhow::anyhow!(
                    "configuration migration failed or conflicted; reload and preview again"
                )
            })?;
        Ok(MigrationResult {
            revision,
            unchanged,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn directory() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }
    fn legacy(path: &Path, key: &str) {
        let mut config = AppConfig::default();
        config.llm.api_key = key.into();
        std::fs::write(path, toml::to_string(&config).unwrap()).unwrap();
    }
    #[test]
    fn explicit_secret_choice_and_idempotent_backup_preserve_source() {
        let dir = directory();
        let root = dir.path().canonicalize().unwrap();
        let source = root.join("old.toml");
        let target = root.join("config.toml");
        legacy(&source, "source-private");
        legacy(&target, "target-private");
        let original = std::fs::read(&source).unwrap();
        let plan = MigrationPlan::prepare(&source, &target).unwrap();
        let preview = plan.preview().to_string();
        assert!(!preview.contains("source-private") && !preview.contains("target-private"));
        assert!(preview.contains("api_key_configured"));
        let applied = plan.apply(SecretPolicy::KeepTarget).unwrap();
        assert_eq!(applied.revision, 1);
        assert_eq!(
            AppConfig::load_from(&target).unwrap().llm.api_key,
            "target-private"
        );
        assert_eq!(
            AppConfig::load_from(&AppConfig::backup_path(&target))
                .unwrap()
                .llm
                .api_key,
            "target-private"
        );
        assert!(plan.apply(SecretPolicy::KeepTarget).unwrap().unchanged);
        assert_eq!(std::fs::read(&source).unwrap(), original);
        let plan = MigrationPlan::prepare(&source, &target).unwrap();
        assert_eq!(plan.apply(SecretPolicy::UseSource).unwrap().revision, 2);
        assert_eq!(
            AppConfig::load_from(&target).unwrap().llm.api_key,
            "source-private"
        );
    }
    #[test]
    fn exact_source_and_legacy_target_cas_preserve_concurrent_changes() {
        let dir = directory();
        let root = dir.path().canonicalize().unwrap();
        let source = root.join("old.toml");
        let target = root.join("config.toml");
        legacy(&source, "private");
        legacy(&target, "current");
        let plan = MigrationPlan::prepare(&source, &target).unwrap();
        legacy(&source, "changed");
        assert!(plan.apply(SecretPolicy::UseSource).is_err());
        assert_eq!(
            AppConfig::load_from(&target).unwrap().llm.api_key,
            "current"
        );
        let plan = MigrationPlan::prepare(&source, &target).unwrap();
        legacy(&target, "changed-target-without-revision");
        assert!(plan.apply(SecretPolicy::UseSource).is_err());
        assert_eq!(
            AppConfig::load_from(&target).unwrap().llm.api_key,
            "changed-target-without-revision"
        );
    }
    #[test]
    fn same_location_upgrade_is_explicit_and_repeated_apply_is_noop() {
        let dir = directory();
        let root = dir.path().canonicalize().unwrap();
        let path = root.join("config.toml");
        legacy(&path, "private");
        let before = std::fs::read(&path).unwrap();
        let plan = MigrationPlan::prepare(&path, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(plan.apply(SecretPolicy::UseSource).unwrap().revision, 1);
        assert!(plan.apply(SecretPolicy::UseSource).unwrap().unchanged);
    }
    #[test]
    fn blank_source_keys_only_clear_target_after_explicit_source_choice() {
        let dir = directory();
        let root = dir.path().canonicalize().unwrap();
        let source = root.join("old.toml");
        let target = root.join("config.toml");
        legacy(&source, "");
        let mut current = AppConfig::default();
        current.llm.api_key = "target-primary".into();
        current.llm.secondary_api_key = "target-secondary".into();
        current.save_to_checked(&target, 0).unwrap();
        let plan = MigrationPlan::prepare(&source, &target).unwrap();
        assert!(plan.apply(SecretPolicy::KeepTarget).unwrap().unchanged);
        let loaded = AppConfig::load_from(&target).unwrap();
        assert_eq!(loaded.llm.api_key, "target-primary");
        assert_eq!(loaded.llm.secondary_api_key, "target-secondary");
        assert_eq!(plan.apply(SecretPolicy::UseSource).unwrap().revision, 2);
        let loaded = AppConfig::load_from(&target).unwrap();
        assert!(loaded.llm.api_key.is_empty() && loaded.llm.secondary_api_key.is_empty());
    }
    #[test]
    fn preview_redacts_both_origins_even_when_keys_are_echoed_in_other_labels() {
        let dir = directory();
        let root = dir.path().canonicalize().unwrap();
        let source = root.join("old.toml");
        let target = root.join("config.toml");
        let mut old = AppConfig::default();
        old.llm.api_key = "source-private".into();
        old.llm.model = "target-private source-private".into();
        std::fs::write(&source, toml::to_string(&old).unwrap()).unwrap();
        let mut current = AppConfig::default();
        current.llm.api_key = "target-private".into();
        current.llm.model = "source-private".into();
        current.save_to_checked(&target, 0).unwrap();
        let preview = MigrationPlan::prepare(&source, &target)
            .unwrap()
            .preview()
            .to_string();
        assert!(!preview.contains("source-private") && !preview.contains("target-private"));
        assert!(preview.contains("[REDACTED]"));
    }
    #[test]
    fn locked_target_and_corrupt_target_preserve_all_original_bytes() {
        let dir = directory();
        let root = dir.path().canonicalize().unwrap();
        let source = root.join("old.toml");
        let target = root.join("config.toml");
        legacy(&source, "source-private");
        legacy(&target, "target-private");
        let before = std::fs::read(&target).unwrap();
        let plan = MigrationPlan::prepare(&source, &target).unwrap();
        let file = crate::asset_file::AssetFile::new(target.clone(), MAX_CONFIG_BYTES);
        let lock = file.lock().unwrap();
        let error = plan
            .apply(SecretPolicy::UseSource)
            .err()
            .unwrap()
            .to_string();
        assert!(!error.contains("source-private") && !error.contains("target-private"));
        assert_eq!(std::fs::read(&target).unwrap(), before);
        assert!(!AppConfig::backup_path(&target).exists());
        drop(lock);
        std::fs::write(&target, b"malformed private target").unwrap();
        assert!(MigrationPlan::prepare(&source, &target).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"malformed private target");
        assert!(sources(Some("../outside")).is_err());
    }
    #[test]
    fn corrupt_unknown_oversized_or_bad_backup_never_overwrites() {
        let dir = directory();
        let root = dir.path().canonicalize().unwrap();
        let source = root.join("old.toml");
        let target = root.join("config.toml");
        legacy(&source, "private");
        legacy(&target, "current");
        let before = std::fs::read(&target).unwrap();
        std::fs::write(&source, "private malformed secret").unwrap();
        assert!(MigrationPlan::prepare(&source, &target).is_err());
        legacy(&source, "private");
        let mut bytes = std::fs::read(&source).unwrap();
        bytes.extend_from_slice(b"\nunsupported = 'lost'\n");
        std::fs::write(&source, bytes).unwrap();
        assert!(MigrationPlan::prepare(&source, &target).is_err());
        std::fs::write(&source, vec![b'x'; MAX_CONFIG_BYTES + 1]).unwrap();
        assert!(MigrationPlan::prepare(&source, &target).is_err());
        legacy(&source, "private");
        std::fs::write(AppConfig::backup_path(&target), "corrupt").unwrap();
        let plan = MigrationPlan::prepare(&source, &target).unwrap();
        assert!(plan.apply(SecretPolicy::UseSource).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), before);
    }
    #[cfg(unix)]
    #[test]
    fn symlink_refusal_and_private_replacement() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = directory();
        let root = dir.path().canonicalize().unwrap();
        let source = root.join("old.toml");
        let target = root.join("config.toml");
        legacy(&source, "private");
        let link = root.join("link.toml");
        symlink(&source, &link).unwrap();
        assert!(MigrationPlan::prepare(&link, &target).is_err());
        let alias = root.join("alias");
        symlink(&root, &alias).unwrap();
        assert!(MigrationPlan::prepare(&alias.join("old.toml"), &target).is_err());
        let plan = MigrationPlan::prepare(&source, &target).unwrap();
        plan.apply(SecretPolicy::UseSource).unwrap();
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
