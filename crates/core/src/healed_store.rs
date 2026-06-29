// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! Healed-YAML persistence.
//!
//! Writes healed YAML scripts under `runs/{run_id}/healed/{case_id}.yaml` so
//! they're recoverable per run. Never touches the original approved case
//! files in `cases/` — preserves the "never overwrite approved assets"
//! invariant from the original healer module.

use std::path::PathBuf;

pub struct HealedStore {
    root: PathBuf,
}

impl HealedStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Writes a healed YAML to runs/{run_id}/healed/{case_id}.yaml.
    /// Never touches the original case path.
    pub fn write(&self, run_id: &str, case_id: &str, yaml: &str) -> std::io::Result<PathBuf> {
        let dir = self.root.join("runs").join(run_id).join("healed");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.yaml", case_id));
        // ponytail: temp + rename for atomic write
        let tmp = dir.join(format!(".{}.yaml.tmp", case_id));
        std::fs::write(&tmp, yaml)?;
        std::fs::rename(&tmp, &path)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_creates_run_and_case_dirs() {
        let dir = tempdir().unwrap();
        let store = HealedStore::new(dir.path().to_path_buf());
        let path = store.write("run-x", "case-1", "yaml: yes").unwrap();
        assert!(path.exists());
        let body = std::fs::read_to_string(&path).unwrap();
        assert_eq!(body, "yaml: yes");
    }

    #[test]
    fn original_case_yaml_is_untouched() {
        let dir = tempdir().unwrap();
        // Pre-existing approved case path.
        let case_path = dir.path().join("cases/case-1.yaml");
        std::fs::create_dir_all(case_path.parent().unwrap()).unwrap();
        std::fs::write(&case_path, "approved: true").unwrap();

        let store = HealedStore::new(dir.path().to_path_buf());
        store.write("run-x", "case-1", "yaml: yes").unwrap();

        let body = std::fs::read_to_string(&case_path).unwrap();
        assert_eq!(body, "approved: true", "approved case must not be overwritten");
    }
}