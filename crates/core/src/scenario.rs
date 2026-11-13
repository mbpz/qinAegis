//! Declarative capability ordering, independent of any concrete executor.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioStep {
    pub id: String,
    pub capability: String,
    pub depends_on: Vec<String>,
    pub idempotent: bool,
    /// Total attempts, including the initial attempt.
    pub max_attempts: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub version: u32,
    pub steps: Vec<ScenarioStep>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScenarioError {
    #[error("scenario identity/version or step identity/capability is invalid")]
    InvalidIdentity,
    #[error("duplicate step: {0}")]
    DuplicateStep(String),
    #[error("missing dependency {dependency} for step {step}")]
    MissingDependency { step: String, dependency: String },
    #[error("duplicate dependency on step {0}")]
    DuplicateDependency(String),
    #[error(
        "retry must be bounded to 1..=5 attempts, and non-idempotent actions cannot retry: {0}"
    )]
    UnsafeRetry(String),
    #[error("scenario dependencies contain a cycle")]
    Cycle,
}

impl Scenario {
    /// Stable topological order. Declaration order breaks ties, allowing users
    /// to adjust timing among independent steps without violating dependencies.
    pub fn execution_order(&self) -> Result<Vec<&ScenarioStep>, ScenarioError> {
        if self.id.trim().is_empty() || self.version == 0 || self.steps.is_empty() {
            return Err(ScenarioError::InvalidIdentity);
        }
        let mut index = BTreeMap::new();
        for step in &self.steps {
            if step.id.trim().is_empty() || step.capability.trim().is_empty() {
                return Err(ScenarioError::InvalidIdentity);
            }
            if index.insert(step.id.as_str(), step).is_some() {
                return Err(ScenarioError::DuplicateStep(step.id.clone()));
            }
            if !(1..=5).contains(&step.max_attempts) || (!step.idempotent && step.max_attempts > 1)
            {
                return Err(ScenarioError::UnsafeRetry(step.id.clone()));
            }
        }
        for step in &self.steps {
            let mut unique = BTreeSet::new();
            for dependency in &step.depends_on {
                if !index.contains_key(dependency.as_str()) {
                    return Err(ScenarioError::MissingDependency {
                        step: step.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
                if !unique.insert(dependency) {
                    return Err(ScenarioError::DuplicateDependency(step.id.clone()));
                }
            }
        }
        let mut finished = BTreeSet::new();
        let mut ordered = Vec::with_capacity(self.steps.len());
        while ordered.len() < self.steps.len() {
            let next = self
                .steps
                .iter()
                .find(|step| {
                    !finished.contains(step.id.as_str())
                        && step
                            .depends_on
                            .iter()
                            .all(|d| finished.contains(d.as_str()))
                })
                .ok_or(ScenarioError::Cycle)?;
            finished.insert(next.id.as_str());
            ordered.push(next);
        }
        Ok(ordered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn step(id: &str, deps: &[&str]) -> ScenarioStep {
        ScenarioStep {
            id: id.into(),
            capability: "browser.observe".into(),
            depends_on: deps.iter().map(|s| (*s).into()).collect(),
            idempotent: true,
            max_attempts: 1,
        }
    }
    fn scenario(steps: Vec<ScenarioStep>) -> Scenario {
        Scenario {
            id: "receipt-check".into(),
            version: 1,
            steps,
        }
    }
    #[test]
    fn declaration_order_can_change_without_breaking_dependencies() {
        let flow = scenario(vec![
            step("verify", &["submit"]),
            step("setup", &[]),
            step("submit", &["setup"]),
        ]);
        let ids: Vec<_> = flow
            .execution_order()
            .unwrap()
            .iter()
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(ids, vec!["setup", "submit", "verify"]);
        let mut reordered = flow.clone();
        reordered.steps.reverse();
        assert_eq!(
            reordered
                .execution_order()
                .unwrap()
                .iter()
                .map(|s| &s.id)
                .collect::<Vec<_>>(),
            flow.execution_order()
                .unwrap()
                .iter()
                .map(|s| &s.id)
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn rejects_cycles_missing_and_duplicate_dependencies() {
        assert_eq!(
            scenario(vec![step("a", &["b"]), step("b", &["a"])]).execution_order(),
            Err(ScenarioError::Cycle)
        );
        assert!(matches!(
            scenario(vec![step("a", &["missing"])]).execution_order(),
            Err(ScenarioError::MissingDependency { .. })
        ));
        assert!(matches!(
            scenario(vec![step("a", &[]), step("a", &[])]).execution_order(),
            Err(ScenarioError::DuplicateStep(_))
        ));
        assert!(matches!(
            scenario(vec![step("a", &[]), step("b", &["a", "a"])]).execution_order(),
            Err(ScenarioError::DuplicateDependency(_))
        ));
    }
    #[test]
    fn non_idempotent_actions_cannot_retry() {
        let mut submit = step("submit", &[]);
        submit.idempotent = false;
        submit.max_attempts = 2;
        assert_eq!(
            scenario(vec![submit.clone()]).execution_order(),
            Err(ScenarioError::UnsafeRetry("submit".into()))
        );
        submit.max_attempts = 1;
        assert!(scenario(vec![submit]).execution_order().is_ok());
    }
    #[test]
    fn retries_are_bounded_even_for_idempotent_steps() {
        for attempts in [0, 6, 255] {
            let mut observe = step("observe", &[]);
            observe.max_attempts = attempts;
            assert!(matches!(
                scenario(vec![observe]).execution_order(),
                Err(ScenarioError::UnsafeRetry(_))
            ));
        }
    }
}
