//! Private receipts and recoverable native runtime activation.
use crate::{
    asset_file::AssetFile,
    runtime_package::{self, VerifiedPackage},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
const LIMIT: usize = 64 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    token: String,
    kind: String,
    expires_at: u64,
    package: VerifiedPackage,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    revision: u64,
    current: Option<VerifiedPackage>,
    previous: Option<VerifiedPackage>,
    pending: Option<Pending>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    before: State,
    after: State,
    pending: Pending,
}
#[derive(Clone, Debug, Serialize)]
pub struct Preview {
    pub token: String,
    pub kind: String,
    pub version: String,
    pub archive_sha256: String,
    pub source_commit: Value,
    pub expires_at: u64,
    pub revision: u64,
    pub discard_previous_required: bool,
}
pub struct Installer {
    root: PathBuf,
}
struct Lock(fs::File);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn check(c: &AtomicBool) -> Result<()> {
    ensure!(!c.load(Ordering::Acquire), "runtime operation cancelled");
    Ok(())
}
fn token(t: &str) -> Result<()> {
    ensure!(
        t.len() == 64 && t.bytes().all(|c| c.is_ascii_hexdigit()),
        "invalid runtime preview token"
    );
    Ok(())
}
fn exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}
fn plain_dir(p: &Path) -> Result<()> {
    let m = fs::symlink_metadata(p)?;
    ensure!(
        m.is_dir() && !m.file_type().is_symlink(),
        "unsafe runtime directory"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            m.file_attributes() & 0x400 == 0,
            "runtime directory is a reparse point"
        );
    }
    Ok(())
}
impl Installer {
    pub fn new(base: &Path) -> Result<Self> {
        ensure!(
            base.is_absolute(),
            "runtime data directory must be absolute"
        );
        let root = base.join("runtimes/midscene");
        for p in root.ancestors().collect::<Vec<_>>().into_iter().rev() {
            if exists(p) {
                plain_dir(p)?;
            } else {
                fs::create_dir(p)?;
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self { root })
    }
    fn lock(&self) -> Result<Lock> {
        let p = self.root.join("install.lock");
        if exists(&p) {
            let m = fs::symlink_metadata(&p)?;
            ensure!(
                m.is_file() && !m.file_type().is_symlink(),
                "unsafe runtime lock"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                ensure!(m.nlink() == 1, "runtime lock is linked");
            }
        }
        let mut o = fs::OpenOptions::new();
        o.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        let f = o.open(p)?;
        f.try_lock().context("runtime installer is busy")?;
        Ok(Lock(f))
    }
    fn read<T: serde::de::DeserializeOwned + Default>(&self, name: &str) -> Result<T> {
        let a = AssetFile::new(self.root.join(name), LIMIT);
        let g = a.lock()?;
        Ok(match g.read()? {
            Some(b) => serde_json::from_slice(&b)?,
            None => T::default(),
        })
    }
    fn write<T: Serialize>(&self, name: &str, v: &T) -> Result<()> {
        AssetFile::new(self.root.join(name), LIMIT)
            .lock()?
            .write(&serde_json::to_vec(v)?)
    }
    fn rename(&self, from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
        fs::rename(from, to)?;
        #[cfg(unix)]
        fs::File::open(&self.root)?.sync_all()?;
        Ok(())
    }
    fn stage(&self, t: &str) -> PathBuf {
        self.root.join(format!(".candidate-{t}"))
    }
    fn retired(&self, t: &str) -> PathBuf {
        self.root.join(format!(".retired-{t}"))
    }
    fn verify(&self, name: &str, p: &VerifiedPackage, c: &AtomicBool) -> Result<()> {
        runtime_package::verify_installed(&self.root.join(name), p, c).map(|_| ())
    }
    fn preview(s: &State, p: &Pending) -> Preview {
        Preview {
            token: p.token.clone(),
            kind: p.kind.clone(),
            version: p.package.version.clone(),
            archive_sha256: p.package.archive_sha256.clone(),
            source_commit: p.package.provenance["source_commit"].clone(),
            expires_at: p.expires_at,
            revision: s.revision,
            discard_previous_required: p.kind == "install" && s.previous.is_some(),
        }
    }
    pub fn inspect(&self) -> Result<Value> {
        let _l = self.lock()?;
        let s: State = self.read("native-state.json")?;
        let j: Option<Journal> = self.read("native-journal.json")?;
        let summary = |p: &VerifiedPackage| json!({"version":p.version,"archive_sha256":p.archive_sha256,"source_commit":p.provenance["source_commit"],"target":p.provenance["target"],"integrity":"not_checked"});
        Ok(
            json!({"revision":s.revision,"current":s.current.as_ref().map(summary),"previous":s.previous.as_ref().map(summary),"preview":s.pending.as_ref().map(|p|Self::preview(&s,p)),"recovery_required":j.is_some(),"unmanaged_current":s.current.is_none()&&exists(&self.root.join("current")),"unmanaged_previous":s.previous.is_none()&&exists(&self.root.join("previous"))}),
        )
    }
    pub fn ensure_ready(&self) -> Result<()> {
        let _lock = self.lock()?;
        self.ready().map(|_| ())
    }
    fn ready(&self) -> Result<State> {
        let j: Option<Journal> = self.read("native-journal.json")?;
        ensure!(j.is_none(), "runtime recovery required");
        self.read("native-state.json")
    }
    pub fn prepare(&self, archive: &Path, sha: &str, t: &str, c: &AtomicBool) -> Result<Preview> {
        token(t)?;
        let _l = self.lock()?;
        let mut s = self.ready()?;
        ensure!(s.pending.is_none(), "discard the existing preview first");
        check(c)?;
        ensure!(
            fs::read_dir(&self.root)?
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with(".candidate-"))
                .count()
                < 2,
            "candidate residue limit reached; manual inspection required"
        );
        let stage = self.stage(t);
        let package = runtime_package::verify_and_extract(archive, sha, &stage, c)?;
        let result = (|| -> Result<Preview> {
            // Legacy installations are adopted only after exhaustive comparison with trusted archive content.
            if s.current.is_none() && exists(&self.root.join("current")) {
                runtime_package::sync_verified_installation(&self.root.join("current"), &package, c)
                    .context("unmanaged runtime differs or cannot be synced; import its original archive first")?;
                s.current = Some(package.clone());
            }
            ensure!(
                s.previous.is_some() || !exists(&self.root.join("previous")),
                "unmanaged previous runtime requires manual preservation"
            );
            s.revision = s
                .revision
                .checked_add(1)
                .context("runtime revision exhausted")?;
            let pending = Pending {
                token: t.into(),
                kind: "install".into(),
                expires_at: now()? + 600,
                package: package.clone(),
            };
            s.pending = Some(pending.clone());
            self.write("native-state.json", &s)?;
            Ok(Self::preview(&s, &pending))
        })();
        if let Err(error) = result {
            let latest: State = self.read("native-state.json")?;
            if latest.pending.as_ref().is_some_and(|p| p.token == t) {
                return Err(anyhow::anyhow!(
                    "{error}; preview persisted, inspect before retrying"
                ));
            }
            let cleanup =
                runtime_package::verify_installed(&stage, &package, &AtomicBool::new(false))
                    .and_then(|_| {
                        fs::remove_dir_all(&stage)?;
                        Ok(())
                    });
            return match cleanup {
                Ok(()) => Err(error),
                Err(cleanup) => Err(anyhow::anyhow!("{error}; candidate retained: {cleanup}")),
            };
        }
        result
    }
    pub fn prepare_rollback(&self, t: &str, c: &AtomicBool) -> Result<Preview> {
        token(t)?;
        let _l = self.lock()?;
        let mut s = self.ready()?;
        ensure!(s.pending.is_none(), "discard the existing preview first");
        let package = s.previous.clone().context("no verified previous runtime")?;
        self.verify("previous", &package, c)?;
        s.revision = s
            .revision
            .checked_add(1)
            .context("runtime revision exhausted")?;
        let p = Pending {
            token: t.into(),
            kind: "rollback".into(),
            expires_at: now()? + 600,
            package,
        };
        s.pending = Some(p.clone());
        self.write("native-state.json", &s)?;
        Ok(Self::preview(&s, &p))
    }
    pub fn discard_preview(&self, t: &str, c: &AtomicBool) -> Result<()> {
        token(t)?;
        let _l = self.lock()?;
        let mut s = self.ready()?;
        let p = s.pending.as_ref().context("no runtime preview")?;
        ensure!(p.token == t, "runtime preview changed");
        if p.kind == "install" && exists(&self.stage(t)) {
            runtime_package::verify_installed(&self.stage(t), &p.package, c)?;
            fs::remove_dir_all(self.stage(t))?;
        }
        s.pending = None;
        s.revision = s
            .revision
            .checked_add(1)
            .context("runtime revision exhausted")?;
        self.write("native-state.json", &s)
    }
    pub fn activate<F>(
        &self,
        t: &str,
        discard_previous: bool,
        c: &AtomicBool,
        mut health: F,
    ) -> Result<Value>
    where
        F: FnMut(&Path, &AtomicBool) -> Result<()>,
    {
        token(t)?;
        let _l = self.lock()?;
        let before = self.ready()?;
        let p = before.pending.clone().context("no runtime preview")?;
        ensure!(
            p.token == t && now()? <= p.expires_at,
            "runtime preview changed or expired"
        );
        ensure!(
            p.kind == "rollback" || before.previous.is_none() || discard_previous,
            "explicit approval to discard older previous runtime required"
        );
        check(c)?;
        if let Some(old) = &before.current {
            self.verify("current", old, c)?;
        } else {
            ensure!(
                !exists(&self.root.join("current")),
                "unmanaged current runtime"
            );
        }
        if let Some(old) = &before.previous {
            self.verify("previous", old, c)?;
        } else {
            ensure!(
                !exists(&self.root.join("previous")),
                "unmanaged previous runtime"
            );
        }
        let stage = self.stage(t);
        if p.kind == "install" {
            runtime_package::verify_installed(&stage, &p.package, c)?;
            health(&stage, c)?;
        } else {
            health(&self.root.join("previous"), c)?;
        }
        if before
            .current
            .as_ref()
            .is_some_and(|v| v.archive_sha256 == p.package.archive_sha256)
            && p.kind == "install"
        {
            fs::remove_dir_all(stage)?;
            let mut done = before;
            done.pending = None;
            done.revision = done
                .revision
                .checked_add(1)
                .context("runtime revision exhausted")?;
            self.write("native-state.json", &done)?;
            return Ok(json!({"installed":true,"idempotent":true,"version":p.package.version}));
        }
        let mut after = before.clone();
        after.current = Some(p.package.clone());
        after.previous = before.current.clone();
        after.pending = None;
        after.revision = after
            .revision
            .checked_add(1)
            .context("runtime revision exhausted")?;
        let j = Journal {
            before,
            after,
            pending: p.clone(),
        };
        self.write("native-journal.json", &Some(&j))?;
        let operation = (|| -> Result<()> {
            if p.kind == "rollback" {
                self.rename(self.root.join("previous"), &stage)?;
            } else if j.before.previous.is_some() {
                self.rename(self.root.join("previous"), self.retired(t))?;
            }
            check(c)?;
            if j.before.current.is_some() {
                self.rename(self.root.join("current"), self.root.join("previous"))?;
            }
            check(c)?;
            self.rename(&stage, self.root.join("current"))?;
            health(&self.root.join("current"), c)?;
            check(c)?;
            self.write("native-state.json", &j.after)?;
            Ok(())
        })();
        if let Err(e) = operation {
            let recovery = self.recover_locked(&AtomicBool::new(false));
            return match recovery {
                Ok(()) => Err(e),
                Err(r) => Err(anyhow::anyhow!("{e}; runtime recovery failed: {r}")),
            };
        }
        self.recover_locked(&AtomicBool::new(false))?;
        Ok(
            json!({"installed":true,"version":p.package.version,"archive_sha256":p.package.archive_sha256,"operation":p.kind}),
        )
    }
    pub fn recover(&self, c: &AtomicBool) -> Result<()> {
        let _l = self.lock()?;
        self.recover_locked(c)
    }
    fn recover_locked(&self, c: &AtomicBool) -> Result<()> {
        let Some(j) = self.read::<Option<Journal>>("native-journal.json")? else {
            return Ok(());
        };
        token(&j.pending.token)?;
        let s: State = self.read("native-state.json")?;
        let stage = self.stage(&j.pending.token);
        let current = self.root.join("current");
        let previous = self.root.join("previous");
        let retired = self.retired(&j.pending.token);
        if s.revision == j.after.revision {
            ensure!(
                serde_json::to_value(&s)? == serde_json::to_value(&j.after)?,
                "committed runtime state changed"
            );
            self.verify("current", &j.pending.package, c)?;
            if exists(&retired) {
                let old = j
                    .before
                    .previous
                    .as_ref()
                    .context("unknown retired runtime")?;
                runtime_package::verify_installed(&retired, old, c)?;
                fs::remove_dir_all(&retired)?;
            }
        } else {
            ensure!(
                serde_json::to_value(&s)? == serde_json::to_value(&j.before)?,
                "runtime recovery state changed"
            );
            // Atomic rename presence disambiguates each interrupted boundary. Verify every moved tree.
            if !exists(&stage)
                && exists(&current)
                && !j
                    .before
                    .current
                    .as_ref()
                    .is_some_and(|old| runtime_package::verify_installed(&current, old, c).is_ok())
            {
                runtime_package::verify_installed(&current, &j.pending.package, c)?;
                self.rename(&current, &stage)?;
            }
            if !exists(&current) {
                if let Some(old) = &j.before.current {
                    runtime_package::verify_installed(&previous, old, c)?;
                    self.rename(&previous, &current)?;
                }
            }
            if j.pending.kind == "rollback" {
                if exists(&stage) {
                    ensure!(!exists(&previous), "rollback destination occupied");
                    runtime_package::verify_installed(&stage, &j.pending.package, c)?;
                    self.rename(&stage, &previous)?;
                }
            } else if exists(&retired) {
                ensure!(!exists(&previous), "previous runtime destination occupied");
                let old = j
                    .before
                    .previous
                    .as_ref()
                    .context("unknown retired runtime")?;
                runtime_package::verify_installed(&retired, old, c)?;
                self.rename(&retired, &previous)?;
            }
            if let Some(old) = &j.before.current {
                self.verify("current", old, c)?;
            }
            if let Some(old) = &j.before.previous {
                self.verify("previous", old, c)?;
            }
        }
        self.write("native-journal.json", &Option::<Journal>::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn hash(b: &[u8]) -> String {
        format!("{:x}", Sha256::digest(b))
    }
    fn package(dir: &Path, version: &str) -> (PathBuf, String) {
        let archive = dir.join(format!("{version}.tar.gz"));
        let encoder = flate2::write::GzEncoder::new(
            fs::File::create(&archive).unwrap(),
            flate2::Compression::fast(),
        );
        let mut tar = tar::Builder::new(encoder);
        let host = format!(
            "{}-{}",
            if cfg!(target_os = "macos") {
                "darwin"
            } else {
                std::env::consts::OS
            },
            if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
                "arm64"
            } else if cfg!(all(windows, target_arch = "x86_64")) {
                "amd64"
            } else {
                std::env::consts::ARCH
            }
        );
        let manifest = json!({"schema_version":1,"engine":"midscene","version":version,"entry":"entry.js","node":"node"});
        let provenance = json!({"schema_version":1,"version":version,"target":host,"source_commit":"a".repeat(40),"node_version":"v24.1","compiled_sha256":{"entry.js":hash(b"js")},"node_sha256":hash(b"node"),"lock_sha256":hash(b"lock")});
        for (name, data) in [
            ("runtime.json", manifest.to_string()),
            ("provenance.json", provenance.to_string()),
            ("entry.js", "js".into()),
            ("node", "node".into()),
            ("pnpm-lock.yaml", "lock".into()),
        ] {
            let mut h = tar::Header::new_gnu();
            h.set_path(name).unwrap();
            h.set_size(data.len() as u64);
            h.set_mode(0o700);
            h.set_cksum();
            tar.append(&h, data.as_bytes()).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
        let digest = hash(&fs::read(&archive).unwrap());
        (archive, digest)
    }
    fn install(i: &Installer, base: &Path, v: &str, t: &str) {
        let (a, h) = package(base, v);
        i.prepare(&a, &h, t, &AtomicBool::new(false)).unwrap();
        i.activate(t, true, &AtomicBool::new(false), |_, _| Ok(()))
            .unwrap();
    }
    #[test]
    fn activate_rollback_and_failed_health_preserve_previous() {
        let d = tempfile::tempdir().unwrap();
        let i = Installer::new(&d.path().canonicalize().unwrap()).unwrap();
        let c = AtomicBool::new(false);
        install(&i, d.path(), "one", &"1".repeat(64));
        install(&i, d.path(), "two", &"2".repeat(64));
        let (a, h) = package(d.path(), "three");
        let t = "3".repeat(64);
        i.prepare(&a, &h, &t, &c).unwrap();
        assert!(i.activate(&t, false, &c, |_, _| Ok(())).is_err());
        let mut calls = 0;
        assert!(
            i.activate(&t, true, &c, |_, _| {
                calls += 1;
                ensure!(calls != 2, "health failed");
                Ok(())
            })
            .is_err()
        );
        assert_eq!(i.inspect().unwrap()["current"]["version"], "two");
        assert_eq!(i.inspect().unwrap()["previous"]["version"], "one");
        i.discard_preview(&t, &c).unwrap();
        let t = "4".repeat(64);
        i.prepare_rollback(&t, &c).unwrap();
        i.activate(&t, false, &c, |_, _| Ok(())).unwrap();
        assert_eq!(i.inspect().unwrap()["current"]["version"], "one");
        assert_eq!(i.inspect().unwrap()["previous"]["version"], "two");
    }
    #[test]
    fn recovery_at_each_install_and_rollback_rename_boundary() {
        for rollback in [false, true] {
            for boundary in 0..=4 {
                let d = tempfile::tempdir().unwrap();
                let i = Installer::new(&d.path().canonicalize().unwrap()).unwrap();
                let c = AtomicBool::new(false);
                install(&i, d.path(), "one", &"1".repeat(64));
                install(&i, d.path(), "two", &"2".repeat(64));
                let t = "3".repeat(64);
                if rollback {
                    i.prepare_rollback(&t, &c).unwrap();
                } else {
                    let (a, h) = package(d.path(), "three");
                    i.prepare(&a, &h, &t, &c).unwrap();
                }
                let before: State = i.ready().unwrap();
                let p = before.pending.clone().unwrap();
                let mut after = before.clone();
                after.current = Some(p.package.clone());
                after.previous = before.current.clone();
                after.pending = None;
                after.revision += 1;
                let j = Journal {
                    before,
                    after,
                    pending: p,
                };
                i.write("native-journal.json", &Some(&j)).unwrap();
                if boundary >= 1 {
                    if rollback {
                        i.rename(i.root.join("previous"), i.stage(&t)).unwrap();
                    } else {
                        i.rename(i.root.join("previous"), i.retired(&t)).unwrap();
                    }
                }
                if boundary >= 2 {
                    i.rename(i.root.join("current"), i.root.join("previous"))
                        .unwrap();
                }
                if boundary >= 3 {
                    i.rename(i.stage(&t), i.root.join("current")).unwrap();
                }
                if boundary == 4 {
                    i.write("native-state.json", &j.after).unwrap();
                }
                i.recover(&c).unwrap();
                assert_eq!(
                    i.inspect().unwrap()["current"]["version"],
                    if boundary == 4 {
                        if rollback { "one" } else { "three" }
                    } else {
                        "two"
                    },
                    "rollback={rollback} boundary={boundary}"
                );
                assert_eq!(
                    i.inspect().unwrap()["previous"]["version"],
                    if boundary == 4 { "two" } else { "one" }
                );
                i.recover(&c).unwrap();
            }
        }
    }
    #[test]
    fn legacy_adoption_requires_matching_archive_and_preserves_unknown_assets() {
        let d = tempfile::tempdir().unwrap();
        let i = Installer::new(&d.path().canonicalize().unwrap()).unwrap();
        let c = AtomicBool::new(false);
        let (a, h) = package(d.path(), "one");
        runtime_package::verify_and_extract(&a, &h, &i.root.join("current"), &c).unwrap();
        let (b, bh) = package(d.path(), "two");
        let t = "1".repeat(64);
        assert!(i.prepare(&b, &bh, &t, &c).is_err());
        assert!(!exists(&i.stage(&t)));
        assert_eq!(fs::read(i.root.join("current/entry.js")).unwrap(), b"js");
        i.prepare(&a, &h, &t, &c).unwrap();
        let r = i.activate(&t, false, &c, |_, _| Ok(())).unwrap();
        assert_eq!(r["idempotent"], true);
        fs::create_dir(i.root.join("previous")).unwrap();
        fs::write(i.root.join("previous/unowned"), "evidence").unwrap();
        assert!(i.prepare(&b, &bh, &"2".repeat(64), &c).is_err());
        assert_eq!(
            fs::read(i.root.join("previous/unowned")).unwrap(),
            b"evidence"
        );
    }
    #[test]
    fn stale_token_tampering_and_cancel_fail_closed() {
        let d = tempfile::tempdir().unwrap();
        let i = Installer::new(&d.path().canonicalize().unwrap()).unwrap();
        install(&i, d.path(), "one", &"1".repeat(64));
        let (a, h) = package(d.path(), "two");
        let c = AtomicBool::new(false);
        let t = "2".repeat(64);
        i.prepare(&a, &h, &t, &c).unwrap();
        assert!(
            i.activate(&"3".repeat(64), true, &c, |_, _| panic!())
                .is_err()
        );
        fs::write(i.stage(&t).join("entry.js"), b"bad").unwrap();
        assert!(i.activate(&t, true, &c, |_, _| panic!()).is_err());
        assert!(
            i.activate(&t, true, &AtomicBool::new(true), |_, _| panic!())
                .is_err()
        );
        assert_eq!(i.inspect().unwrap()["current"]["version"], "one");
    }
}
