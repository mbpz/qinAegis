// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! Retry helpers with exponential backoff for transient LLM/network failures.
//!
//! Two variants are provided:
//! - [`retry_with_backoff`] — for `Result<T, E>` operations (distinguishes
//!   "LLM call failed" from "LLM said no fix").
//! - [`retry_with_backoff_option`] — for `Option<T>` operations, where the
//!   caller treats "None" as failure and wants to retry.
//!
//! Both honor a per-attempt sleep computed from a fresh `backoff_range()` so
//! the range can vary (e.g. jittered) across attempts.

use std::future::Future;
use std::ops::Range;
use std::time::Duration;

/// Retry an async `Result`-returning operation with exponential backoff.
///
/// `max_attempts` includes the first try. If `max_attempts == 0`, the
/// operation is called once (the "stdlib retries" convention). The
/// `backoff_range` closure is invoked before each sleep and must return the
/// `[min_ms, max_ms)` window to use; the actual sleep is the midpoint of the
/// range, scaled by `2^attempt`, clamped to the range.
///
/// Returns the first successful result, or the last error after
/// `max_attempts`.
pub async fn retry_with_backoff<F, Fut, T, E>(
    max_attempts: u32,
    backoff_range: F,
    mut op: impl FnMut() -> Fut,
) -> Result<T, E>
where
    F: Fn() -> Range<u64>,
    Fut: Future<Output = Result<T, E>>,
{
    let attempts = max_attempts.max(1);
    let mut last_err: Option<E> = None;
    for attempt in 0..attempts {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = Some(e);
                if attempt + 1 < attempts {
                    let range = backoff_range();
                    let delay_ms = compute_backoff_ms(&range, attempt);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }
    Err(last_err.expect("at least one attempt"))
}

/// Retry an async `Option`-returning operation with exponential backoff.
///
/// Semantics match [`retry_with_backoff`], but the operation returns
/// `Option<T>`: `Some(_)` is success, `None` is failure.
///
/// Returns the first `Some`, or `None` after `max_attempts`.
pub async fn retry_with_backoff_option<B, Op, Fut, T>(
    max_attempts: u32,
    backoff_range: B,
    mut op: Op,
) -> Option<T>
where
    B: Fn() -> Range<u64>,
    Op: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    let attempts = max_attempts.max(1);
    for attempt in 0..attempts {
        if let Some(v) = op().await {
            return Some(v);
        }
        if attempt + 1 < attempts {
            let range = backoff_range();
            let delay_ms = compute_backoff_ms(&range, attempt);
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
    }
    None
}

/// Compute the sleep duration in ms for a given attempt: midpoint of
/// `range` doubled per attempt, clamped to the range. This keeps sleeps
/// inside the configured window even at high attempt counts.
fn compute_backoff_ms(range: &Range<u64>, attempt: u32) -> u64 {
    let span = range.end.saturating_sub(range.start);
    let mid = range.start + span / 2;
    let pow = 2u64.saturating_pow(attempt.min(20));
    let delay = mid.saturating_mul(pow) / 2;
    delay.clamp(range.start, range.end.saturating_sub(1).max(range.start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    // ---- retry_with_backoff (Result variant) ----

    #[tokio::test]
    async fn succeeds_on_first_try() {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let r: Result<i32, &str> = retry_with_backoff(3, || 1..10, || {
            c.fetch_add(1, Ordering::SeqCst);
            async { Ok::<i32, &str>(42) }
        })
        .await;
        assert_eq!(r, Ok(42));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retries_on_recoverable_error() {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let r: Result<i32, &str> = retry_with_backoff(3, || 1..10, || {
            let n = c.fetch_add(1, Ordering::SeqCst);
            async move {
                if n < 2 {
                    Err("transient")
                } else {
                    Ok(99)
                }
            }
        })
        .await;
        assert_eq!(r, Ok(99));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn gives_up_after_max_attempts() {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let r: Result<i32, &str> = retry_with_backoff(3, || 1..10, || {
            c.fetch_add(1, Ordering::SeqCst);
            async { Err("permanent") }
        })
        .await;
        assert_eq!(r, Err("permanent"));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn zero_attempts_falls_back_to_one_try() {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let r: Result<i32, &str> = retry_with_backoff(0, || 1..10, || {
            c.fetch_add(1, Ordering::SeqCst);
            async { Ok(1) }
        })
        .await;
        // 0 max_attempts is normalized to 1 attempt (matches stdlib retries).
        assert_eq!(r, Ok(1));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    // ---- retry_with_backoff_option (Option variant) ----

    #[tokio::test]
    async fn option_succeeds_on_first_try() {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let r: Option<i32> = retry_with_backoff_option(3, || 1..10, || {
            c.fetch_add(1, Ordering::SeqCst);
            async { Some(7) }
        })
        .await;
        assert_eq!(r, Some(7));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn option_retries_then_succeeds() {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let r: Option<String> = retry_with_backoff_option(3, || 1..10, || {
            let n = c.fetch_add(1, Ordering::SeqCst);
            async move {
                if n < 2 {
                    None
                } else {
                    Some("fixed".to_string())
                }
            }
        })
        .await;
        assert_eq!(r, Some("fixed".to_string()));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn option_gives_up_after_max_attempts() {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let r: Option<i32> = retry_with_backoff_option(3, || 1..10, || {
            c.fetch_add(1, Ordering::SeqCst);
            async { None }
        })
        .await;
        assert_eq!(r, None);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }
}
