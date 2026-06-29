// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! Event types for the closed-loop reliability pipeline.
//!
//! These events flow across the event bus and are produced/consumed by the
//! orchestrator and its workers (generator, critic, executor, healer, flaky
//! tracker, gate).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ReliabilityEvent {
    CaseGenerated    { case_id: String, yaml: String, critic_score: Option<u8> },
    CriticRejected   { case_id: String, reason: String, attempt: u8 },

    CaseExecuted     { case_id: String, run_id: String, passed: bool,
                       error: Option<String>, failed_step: Option<String>,
                       yaml: String },

    HealRequested    { case_id: String, original_yaml: String,
                       error: String, failed_step: String, attempt: u8 },
    HealProduced     { case_id: String, healed_yaml: String, attempt: u8 },
    HealExhausted    { case_id: String, attempts: u8, last_error: String },

    FlakyDetected    { case_id: String, flip_rate: f64, window: usize },
    CaseQuarantined  { case_id: String, reason: String },

    RunFinished      { run_id: String },
    GateDecided      { run_id: String, passed: bool, reasons: Vec<String> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_executed_roundtrip_preserves_failed_step() {
        let ev = ReliabilityEvent::CaseExecuted {
            case_id: "c1".into(),
            run_id: "r1".into(),
            passed: false,
            error: Some("boom".into()),
            failed_step: Some("aiAct: click X".into()),
            yaml: "target:\n  url: https://x\ntasks: []".into(),
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: ReliabilityEvent = serde_json::from_str(&json).unwrap();
        match back {
            ReliabilityEvent::CaseExecuted { failed_step, .. } => {
                assert_eq!(failed_step.as_deref(), Some("aiAct: click X"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn heal_exhausted_carries_attempt_count() {
        let ev = ReliabilityEvent::HealExhausted {
            case_id: "c2".into(),
            attempts: 2,
            last_error: "still broken".into(),
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("\"attempts\":2"));
    }
}
