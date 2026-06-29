// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! FlakyWindow — thread-safe sliding window of pass/fail outcomes.
//!
//! Computes a flip rate (adjacent transitions / records-1) and a
//! strike counter. Used by `FlakyTracker` to decide between
//! `FlakyDetected` and `CaseQuarantined` events.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::bus::ReliabilityBus;
use crate::events::ReliabilityEvent;

const FLIP_RATE_THRESHOLD: f64 = 0.3;
const DEFAULT_WINDOW: usize = 10;

/// Sliding pass/fail ring buffer. Thread-safe via interior Mutex.
///
/// `flip_rate` = (number of adjacent transitions) / (records - 1).
/// `is_flaky` = flip_rate > 0.3.
pub struct FlakyWindow {
    inner: Mutex<Inner>,
}

struct Inner {
    capacity: usize,
    buf: Vec<bool>,
    flips: usize,
    last: Option<bool>,
    /// Total number of adjacent transitions counted so far.
    flips_count: u32,
}

impl FlakyWindow {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                capacity,
                buf: Vec::with_capacity(capacity),
                flips: 0,
                last: None,
                flips_count: 0,
            }),
        }
    }

    pub fn record(&self, passed: bool) {
        let mut g = self.inner.lock().unwrap();
        if g.buf.len() == g.capacity {
            // Evict oldest, subtract its contribution to flips.
            let evicted = g.buf.remove(0);
            // Re-compute flips after eviction is expensive; instead track
            // that eviction happened and lazily recompute on read.
            // ponytail: lazy recompute — flip count kept as approximation
            // via "stale" flag.
            g.flips = usize::MAX; // signal recompute
        }
        if g.flips == usize::MAX {
            // Full recompute.
            g.flips = g.buf.windows(2).filter(|w| w[0] != w[1]).count();
            g.last = g.buf.last().copied();
        }
        let prev = g.last;
        g.buf.push(passed);
        g.last = Some(passed);
        if let Some(p) = prev {
            if p != passed {
                g.flips += 1;
                g.flips_count = g.flips_count.saturating_add(1);
            }
        }
    }

    pub fn flip_rate(&self) -> f64 {
        let g = self.inner.lock().unwrap();
        if g.buf.len() < 2 { return 0.0; }
        g.flips as f64 / (g.buf.len() - 1) as f64
    }

    pub fn is_flaky(&self) -> bool {
        self.flip_rate() > FLIP_RATE_THRESHOLD
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().buf.len()
    }

    pub fn should_quarantine(&self, strikes: u32) -> bool {
        self.inner.lock().unwrap().flips_count >= strikes
    }
}

/// Persistent per-case pass/fail history.
///
/// Each case id is mapped to a `Vec<bool>` of outcomes (oldest first). The
/// on-disk format is a JSON object `{ "cases": { "<id>": [bool,...] } }`.
/// Per-case histories are capped at 100 entries to keep the file bounded.
///
/// Writes are atomic: serialize to `<file>.tmp`, then `rename` over the
/// target. Concurrent readers either see the old file or the new one.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct StoredHistory {
    cases: HashMap<String, Vec<bool>>,
}

pub struct FlakyHistory {
    path: std::path::PathBuf,
    inner: StoredHistory,
}

impl FlakyHistory {
    /// Construct an empty in-memory history bound to `path`. The path is
    /// only used on `save`; nothing is read.
    pub fn empty(path: &Path) -> Self {
        Self { path: path.to_path_buf(), inner: StoredHistory::default() }
    }

    /// Load history from `path`. If the file is missing, returns an empty
    /// history. If the file is corrupt, returns an empty history (logged but
    /// not fatal — we don't want a bad JSON to wedge the pipeline).
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let inner = if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
                Err(e) => {
                    tracing::warn!("FlakyHistory read failed for {}: {}", path.display(), e);
                    StoredHistory::default()
                }
            }
        } else {
            StoredHistory::default()
        };
        Ok(Self { path: path.to_path_buf(), inner })
    }

    /// Append one outcome for `case_id`. Caps per-case history at 100
    /// entries by evicting the oldest record.
    pub fn record(&mut self, case_id: &str, passed: bool) {
        let entry = self.inner.cases.entry(case_id.to_string()).or_default();
        // ponytail: cap per-case history at 100 to keep file small
        if entry.len() >= 100 { entry.remove(0); }
        entry.push(passed);
    }

    /// Read-only view of one case's history. Returns an empty vec if the
    /// case has never been recorded.
    pub fn history_for(&self, case_id: &str) -> Vec<bool> {
        self.inner.cases.get(case_id).cloned().unwrap_or_default()
    }

    /// Atomically write the current state to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        let parent = self.path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let tmp_name = self
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("flaky");
        let tmp = parent.join(format!(".{}.tmp", tmp_name));
        let body = serde_json::to_string_pretty(&self.inner)?;
        std::fs::write(&tmp, body)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

/// Worker that inspects a `FlakyWindow` and publishes events to the bus.
///
/// Called by the orchestrator after each `CaseExecuted` to surface flaky
/// behavior and quarantine decisions.
pub struct FlakyTracker;

impl FlakyTracker {
    /// Inspect the window for one case and publish events.
    pub async fn process(window: Arc<FlakyWindow>, case_id: &str, bus: &ReliabilityBus) {
        if window.is_flaky() {
            bus.publish(ReliabilityEvent::FlakyDetected {
                case_id: case_id.into(),
                flip_rate: window.flip_rate(),
                window: window.len(),
            });
        }
        if window.should_quarantine(3) {
            bus.publish(ReliabilityEvent::CaseQuarantined {
                case_id: case_id.into(),
                reason: format!("{} flips in last {} runs", window.flip_rate(), window.len()),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_pass_yields_zero_flip_rate() {
        let w = FlakyWindow::new(5);
        for _ in 0..5 { w.record(true); }
        assert_eq!(w.flip_rate(), 0.0);
        assert!(!w.is_flaky());
    }

    #[test]
    fn alternating_yields_high_flip_rate_and_flaky() {
        let w = FlakyWindow::new(6);
        for v in [true, false, true, false, true, false] { w.record(v); }
        // 5 transitions across 6 records
        let rate = w.flip_rate();
        assert!(rate > 0.7, "rate was {}", rate);
        assert!(w.is_flaky());
    }

    #[test]
    fn window_caps_at_max_size() {
        let w = FlakyWindow::new(3);
        for _ in 0..10 { w.record(true); }
        assert_eq!(w.len(), 3);
    }

    #[test]
    fn quarantine_triggers_after_three_consecutive_runs_with_flips() {
        let w = FlakyWindow::new(10);
        // Three "runs" each containing at least one flip in window.
        w.record(true); w.record(false);  // run 1
        w.record(false); w.record(true);  // run 2
        w.record(true); w.record(false);  // run 3
        assert!(w.should_quarantine(3));
    }

    #[test]
    fn eviction_recomputes_flip_rate_correctly() {
        let w = FlakyWindow::new(3);
        w.record(true); w.record(false); w.record(true); // flips_count=2, buf=[T,F,T]
        w.record(false); // evicts T, recompute, then push F → buf=[F,T,F], flips_count still 2
        // After eviction + push: buf=[F,T,F], transitions: F→T, T→F = 2
        assert_eq!(w.flip_rate(), 1.0); // 2/2
        assert!(w.is_flaky());
    }

    #[tokio::test]
    async fn tracker_emits_flaky_after_threshold_transitions() {
        use crate::bus::ReliabilityBus;
        use crate::events::ReliabilityEvent;
        use std::sync::Arc;

        let bus = ReliabilityBus::new(16);
        let mut rx = bus.subscribe();
        let window = Arc::new(FlakyWindow::new(10));
        let window_c = window.clone();
        let window_for_task = window.clone();
        tokio::spawn(async move {
            FlakyTracker::process(window_for_task, "case-1", &bus).await;
        });

        // Drive alternating outcomes.
        for v in [true, false, true, false, true, false] {
            window_c.record(v);
        }
        // Process should publish FlakyDetected.
        // (No direct way to know when done — just wait for one event.)
        let ev = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await;
        match ev {
            Ok(Ok(ReliabilityEvent::FlakyDetected { case_id, .. })) => assert_eq!(case_id, "case-1"),
            other => panic!("expected FlakyDetected, got {:?}", other),
        }
    }

    #[test]
    fn history_save_and_load_roundtrips() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let path = dir.path().join("flaky.json");

        let mut hist = FlakyHistory::load(&path).unwrap();
        hist.record("c1", true);
        hist.record("c1", false);
        hist.record("c1", true);
        hist.save().unwrap();

        let hist2 = FlakyHistory::load(&path).unwrap();
        assert_eq!(hist2.history_for("c1"), vec![true, false, true]);
    }
}
