//! Compact model-attempt facts reference the separately frozen route snapshot.
use anyhow::{ensure, Context, Result};
use qin_aegis_core::models::{ModelCallAttempt, ModelCallOutcome};
use serde::Serialize;
use serde_json::Value;

const MAX_CALLS: u64 = 201; // One generator and at most 200 per-case critics.
const MAX_RESULT_BYTES: usize = 32 * 1024;

#[derive(Serialize)]
struct Evidence<'a> {
    schema_version: u8,
    source: &'static str,
    application_outcome: &'a str,
    attempt_fields: [&'static str; 4],
    /// Candidate indices refer to snapshot.model.candidates, not current config.
    attempts: Vec<(u64, u8, u8, ModelCallOutcome)>,
    call_sequence: &'static str,
}

pub(crate) fn encode(
    candidates: &[String],
    attempts: &[ModelCallAttempt],
    outcome: &str,
) -> Result<Value> {
    ensure!(
        matches!(
            outcome,
            "in_progress"
                | "completed"
                | "completed_with_unverified_reviews"
                | "failed"
                | "cancelled"
        ),
        "invalid generation evidence outcome"
    );
    ensure!(
        !candidates.is_empty() && candidates.len() <= 4,
        "invalid generation evidence candidates"
    );
    ensure!(
        attempts.len() <= MAX_CALLS as usize * candidates.len(),
        "generation evidence exceeds attempt limit"
    );
    let mut compact = Vec::with_capacity(attempts.len());
    let mut previous = (0, 0);
    for attempt in attempts {
        ensure!(
            (1..=MAX_CALLS).contains(&attempt.call),
            "invalid generation evidence call"
        );
        let index = candidates
            .iter()
            .position(|id| id == &attempt.profile_id)
            .context("generation evidence differs from frozen candidates")?;
        ensure!(
            usize::from(attempt.attempt) == index + 1,
            "invalid generation evidence candidate order"
        );
        ensure!(
            (attempt.call, attempt.attempt) > previous,
            "invalid generation evidence attempt ordering"
        );
        ensure!(
            attempt.call <= previous.0 + 1
                && (attempt.call != previous.0 || attempt.attempt == previous.1 + 1),
            "missing generation evidence attempt"
        );
        if attempt.call != previous.0 {
            ensure!(
                attempt.attempt == 1,
                "generation evidence must start with the approved primary"
            );
        }
        compact.push((
            attempt.call,
            attempt.attempt,
            index as u8,
            attempt.outcome.clone(),
        ));
        previous = (attempt.call, attempt.attempt);
    }
    let value = serde_json::to_value(Evidence {
        schema_version: 1,
        source: "registered_route",
        application_outcome: outcome,
        attempt_fields: ["call", "attempt", "candidate_index", "outcome"],
        attempts: compact,
        call_sequence: "generator_then_one_critic_per_case",
    })?;
    ensure!(
        serde_json::to_vec(&value)?.len() <= MAX_RESULT_BYTES,
        "generation evidence exceeds task result limit"
    );
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_generation_trace_survives_existing_task_result_limit_and_restart() {
        let candidates: Vec<_> = (0..4)
            .map(|n| format!("profile-{n}-{}", "x".repeat(100)))
            .collect();
        let mut attempts = Vec::new();
        for call in 1..=MAX_CALLS {
            for (index, id) in candidates.iter().enumerate() {
                attempts.push(ModelCallAttempt {
                    call,
                    attempt: (index + 1) as u8,
                    profile_id: id.clone(),
                    outcome: if index == 3 {
                        ModelCallOutcome::Succeeded
                    } else {
                        ModelCallOutcome::ServiceUnavailable
                    },
                });
            }
        }
        let result = encode(&candidates, &attempts, "completed").unwrap();
        assert!(serde_json::to_vec(&result).unwrap().len() < MAX_RESULT_BYTES);
        let root = std::env::temp_dir().join(format!(
            "qinaegis-generation-evidence-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let store = qin_aegis_core::jobs::JobStore::new(root.join("jobs.json"));
        store
            .checkpoint(qin_aegis_core::jobs::JobRecord {
                id: "generation".into(),
                job_type: "generate".into(),
                status: "completed".into(),
                output: String::new(),
                output_start: 0,
                result: Some(result.clone()),
                error: None,
                snapshot: None,
                created_at: 1,
                updated_at: 2,
            })
            .unwrap();
        assert_eq!(store.load_and_recover().unwrap()[0].result, Some(result));
        let mut invalid = attempts.clone();
        invalid[2].profile_id = "foreign".into();
        assert!(encode(&candidates, &invalid, "failed").is_err());
        invalid = attempts.clone();
        invalid.remove(1);
        assert!(encode(&candidates, &invalid, "failed").is_err());
        attempts.push(ModelCallAttempt {
            call: 202,
            attempt: 1,
            profile_id: candidates[0].clone(),
            outcome: ModelCallOutcome::Started,
        });
        assert!(encode(&candidates, &attempts, "in_progress").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
