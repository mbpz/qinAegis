// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

// Quarantine: tracks case IDs marked flaky.
// On pipeline start, cases in the quarantine set are skipped entirely.
// Atomic write via temp+rename prevents half-written JSON on crash.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Stored {
    cases: HashSet<String>,
}

pub struct Quarantine {
    path: PathBuf,
    inner: Stored,
}

impl Quarantine {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let inner = if path.exists() {
            let s = std::fs::read_to_string(path)?;
            serde_json::from_str(&s).unwrap_or_default()
        } else {
            Stored::default()
        };
        Ok(Self {
            path: path.to_path_buf(),
            inner,
        })
    }

    pub fn contains(&self, case_id: &str) -> bool {
        self.inner.cases.contains(case_id)
    }

    pub fn add(&mut self, case_id: &str, _reason: &str) {
        self.inner.cases.insert(case_id.to_string());
    }

    pub fn save(&self) -> anyhow::Result<()> {
        // ponytail: write to temp + rename to avoid half-written JSON
        let parent = self
            .path
            .parent()
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)?;
        let tmp = parent.join(format!(
            ".{}.tmp",
            self.path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("quarantine")
        ));
        let body = serde_json::to_string_pretty(&self.inner)?;
        std::fs::write(&tmp, body)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn add_then_contains_returns_true() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("q.json");
        let mut q = Quarantine::load(&path).unwrap();
        q.add("case-1", "3 flips in last 10 runs");
        q.save().unwrap();
        let q2 = Quarantine::load(&path).unwrap();
        assert!(q2.contains("case-1"));
    }

    #[test]
    fn missing_file_yields_empty_set() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.json");
        let q = Quarantine::load(&path).unwrap();
        assert!(!q.contains("anything"));
    }
}