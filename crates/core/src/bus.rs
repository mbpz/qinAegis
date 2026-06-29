// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! Thin facade over `tokio::sync::broadcast` for `ReliabilityEvent`.
//!
//! All workers in the reliability loop share a single `ReliabilityBus`.
//! The wrapper exposes the subset of broadcast semantics we rely on:
//! multiple subscribers, bounded ring buffer, and a `Lagged` error
//! surfaced to consumers (no silent drops).

use crate::events::ReliabilityEvent;
use tokio::sync::broadcast;

pub use tokio::sync::broadcast::error::RecvError;

#[derive(Clone)]
pub struct ReliabilityBus {
    tx: broadcast::Sender<ReliabilityEvent>,
}

impl ReliabilityBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ReliabilityEvent> {
        self.tx.subscribe()
    }

    pub fn publish(&self, ev: ReliabilityEvent) {
        // `send` only returns `Err` when there are no receivers; that's fine.
        let _ = self.tx.send(ev);
    }

    /// Number of active receivers attached to this bus.
    ///
    /// Useful in tests to deterministically wait for workers to subscribe
    /// before publishing events, avoiding fragile fixed-sleep rendezvous.
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::ReliabilityEvent;

    #[tokio::test]
    async fn subscriber_receives_published_event() {
        let bus = ReliabilityBus::new(16);
        let mut rx = bus.subscribe();
        bus.publish(ReliabilityEvent::RunFinished { run_id: "r1".into() });
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, ReliabilityEvent::RunFinished { .. }));
    }

    #[tokio::test]
    async fn lagged_subscriber_drains_then_sees_latest() {
        let bus = ReliabilityBus::new(2);
        let mut rx = bus.subscribe();
        for i in 0..5 {
            bus.publish(ReliabilityEvent::RunFinished { run_id: format!("r{}", i) });
        }
        // First recv should report Lagged.
        let res = rx.recv().await;
        assert!(matches!(res, Err(super::RecvError::Lagged(_))));
        // Drain remaining buffered events; the last one must be the
        // most recent publish ("r4"). The channel keeps the last
        // `capacity` messages, so we get at most two more.
        let mut last = None;
        while let Ok(ev) = rx.try_recv() {
            if let ReliabilityEvent::RunFinished { run_id } = ev {
                last = Some(run_id);
            } else {
                panic!("wrong variant");
            }
        }
        assert_eq!(last.as_deref(), Some("r4"));
    }
}
