// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! FlakyWindow — thread-safe sliding window of pass/fail outcomes.
//!
//! Computes a flip rate (adjacent transitions / records-1) and a
//! strike counter. Used by `FlakyTracker` to decide between
//! `FlakyDetected` and `CaseQuarantined` events.

use std::sync::Mutex;

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
}
