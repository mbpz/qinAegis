//! Engine-neutral, versioned business contracts and project-specific bindings.
//!
//! Publish one template, bind it independently for each project, and retain its
//! exact version in each run. Corrections produce reviewable proposals and new
//! publications; neither correction nor binding mutates the original asset.
//! Reviewer identity must come from an authenticated application service; these
//! domain records validate structure, not the identity of a remote caller.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    String,
    Number,
    Boolean,
    Object,
}

impl ValueType {
    fn accepts(self, value: &Value) -> bool {
        match self {
            Self::String => value.is_string(),
            Self::Number => value.is_number(),
            Self::Boolean => value.is_boolean(),
            Self::Object => value.is_object(),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContractError {
    #[error("required contract field is empty: {0}")]
    EmptyField(String),
    #[error("contract version must be positive")]
    InvalidVersion,
    #[error("binding parameters must exactly match declared inputs")]
    ParameterMismatch,
    #[error("parameter has incorrect type: {0}")]
    ParameterType(String),
    #[error("only an explicit human review may publish a correction")]
    HumanReviewRequired,
    #[error("correction must preserve contract identity and advance its version")]
    InvalidCorrection,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContractDraft {
    pub id: String,
    pub version: u32,
    pub intent: String,
    pub inputs: BTreeMap<String, ValueType>,
    pub outputs: BTreeMap<String, ValueType>,
    pub preconditions: Vec<String>,
    pub assertions: Vec<String>,
    pub required_capabilities: Vec<String>,
}

impl ContractDraft {
    pub fn validate(&self) -> Result<(), ContractError> {
        required("id", &self.id)?;
        required("intent", &self.intent)?;
        if self.version == 0 {
            return Err(ContractError::InvalidVersion);
        }
        if self.assertions.is_empty() {
            return Err(ContractError::EmptyField("assertions".into()));
        }
        for key in self.inputs.keys().chain(self.outputs.keys()) {
            required("port name", key)?;
        }
        for value in self
            .preconditions
            .iter()
            .chain(&self.assertions)
            .chain(&self.required_capabilities)
        {
            required("condition or capability", value)?;
        }
        Ok(())
    }

    pub fn publish(self, reviewer: &str) -> Result<PublishedContract, ContractError> {
        self.validate()?;
        required("reviewer", reviewer)?;
        Ok(PublishedContract {
            definition: self,
            reviewer: reviewer.into(),
            correction: None,
        })
    }
}

fn required(field: &str, value: &str) -> Result<(), ContractError> {
    if value.trim().is_empty() {
        Err(ContractError::EmptyField(field.into()))
    } else {
        Ok(())
    }
}

/// Publication is immutable through the public API. Deserialize via validation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PublishedRecord")]
pub struct PublishedContract {
    definition: ContractDraft,
    reviewer: String,
    correction: Option<CorrectionRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CorrectionRecord {
    original_version: u32,
    reason: String,
    evidence: Vec<String>,
}

impl CorrectionRecord {
    pub fn original_version(&self) -> u32 {
        self.original_version
    }
    pub fn reason(&self) -> &str {
        &self.reason
    }
    pub fn evidence(&self) -> &[String] {
        &self.evidence
    }
}

#[derive(Deserialize)]
struct PublishedRecord {
    definition: ContractDraft,
    reviewer: String,
    #[serde(default)]
    correction: Option<CorrectionRecord>,
}

impl TryFrom<PublishedRecord> for PublishedContract {
    type Error = ContractError;
    fn try_from(record: PublishedRecord) -> Result<Self, Self::Error> {
        let mut publication = record.definition.publish(&record.reviewer)?;
        if let Some(correction) = record.correction {
            if correction.original_version == 0
                || correction.original_version >= publication.definition.version
            {
                return Err(ContractError::InvalidCorrection);
            }
            required("reason", &correction.reason)?;
            if correction.evidence.is_empty() {
                return Err(ContractError::EmptyField("evidence".into()));
            }
            for item in &correction.evidence {
                required("evidence", item)?;
            }
            publication.correction = Some(correction);
        }
        Ok(publication)
    }
}

impl PublishedContract {
    pub fn definition(&self) -> &ContractDraft {
        &self.definition
    }
    pub fn reviewer(&self) -> &str {
        &self.reviewer
    }

    pub fn correction(&self) -> Option<&CorrectionRecord> {
        self.correction.as_ref()
    }

    pub fn bind(
        &self,
        project: &str,
        parameters: BTreeMap<String, Value>,
    ) -> Result<ProjectBinding, ContractError> {
        required("project", project)?;
        if parameters.keys().ne(self.definition.inputs.keys()) {
            return Err(ContractError::ParameterMismatch);
        }
        for (key, kind) in &self.definition.inputs {
            if !kind.accepts(&parameters[key]) {
                return Err(ContractError::ParameterType(key.clone()));
            }
        }
        Ok(ProjectBinding {
            project: project.into(),
            contract_id: self.definition.id.clone(),
            contract_version: self.definition.version,
            parameters,
        })
    }

    pub fn propose_correction(
        &self,
        replacement: ContractDraft,
        reason: &str,
        evidence: Vec<String>,
    ) -> Result<CorrectionProposal, ContractError> {
        replacement.validate()?;
        required("reason", reason)?;
        if evidence.is_empty() {
            return Err(ContractError::EmptyField("evidence".into()));
        }
        for item in &evidence {
            required("evidence", item)?;
        }
        if replacement.id != self.definition.id || replacement.version <= self.definition.version {
            return Err(ContractError::InvalidCorrection);
        }
        Ok(CorrectionProposal {
            original: self.clone(),
            replacement,
            reason: reason.into(),
            evidence,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectBinding {
    project: String,
    contract_id: String,
    contract_version: u32,
    parameters: BTreeMap<String, Value>,
}

impl ProjectBinding {
    pub fn project(&self) -> &str {
        &self.project
    }
    pub fn contract_id(&self) -> &str {
        &self.contract_id
    }
    pub fn contract_version(&self) -> u32 {
        self.contract_version
    }
    pub fn parameters(&self) -> &BTreeMap<String, Value> {
        &self.parameters
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewerKind {
    Human,
    Model,
}

/// A proposal is never itself an approved contract. Evidence contains references,
/// not arbitrary runtime output or credentials.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CorrectionProposal {
    original: PublishedContract,
    replacement: ContractDraft,
    reason: String,
    evidence: Vec<String>,
}

impl CorrectionProposal {
    pub fn original(&self) -> &PublishedContract {
        &self.original
    }
    pub fn replacement(&self) -> &ContractDraft {
        &self.replacement
    }
    pub fn reason(&self) -> &str {
        &self.reason
    }
    pub fn evidence(&self) -> &[String] {
        &self.evidence
    }
    pub fn approve(
        &self,
        reviewer: &str,
        kind: ReviewerKind,
    ) -> Result<PublishedContract, ContractError> {
        if kind != ReviewerKind::Human {
            return Err(ContractError::HumanReviewRequired);
        }
        let mut publication = self.replacement.clone().publish(reviewer)?;
        publication.correction = Some(CorrectionRecord {
            original_version: self.original.definition.version,
            reason: self.reason.clone(),
            evidence: self.evidence.clone(),
        });
        Ok(publication)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn draft() -> ContractDraft {
        ContractDraft {
            id: "submit-form".into(),
            version: 1,
            intent: "Submitting valid input produces a receipt".into(),
            inputs: BTreeMap::from([("target".into(), ValueType::String)]),
            outputs: BTreeMap::new(),
            preconditions: vec!["authorized session".into()],
            assertions: vec!["receipt matches input".into()],
            required_capabilities: vec!["browser.interact".into()],
        }
    }
    #[test]
    fn projects_reuse_one_unchanged_publication() {
        let contract = draft().publish("owner").unwrap();
        let before = serde_json::to_value(&contract).unwrap();
        let a = contract
            .bind(
                "project-a",
                BTreeMap::from([("target".into(), Value::from("https://a.test"))]),
            )
            .unwrap();
        let b = contract
            .bind(
                "project-b",
                BTreeMap::from([("target".into(), Value::from("https://b.test"))]),
            )
            .unwrap();
        assert_ne!(a.project(), b.project());
        assert_ne!(a.parameters(), b.parameters());
        assert_eq!(a.contract_id(), b.contract_id());
        assert_eq!(a.contract_version(), b.contract_version());
        assert_eq!(before, serde_json::to_value(&contract).unwrap());
    }
    #[test]
    fn correction_needs_review_and_preserves_original() {
        let original = draft().publish("owner").unwrap();
        let mut next = original.definition().clone();
        next.version = 2;
        next.preconditions.push("form loaded".into());
        let proposal = original
            .propose_correction(next, "Clarify readiness", vec!["run-1/observation".into()])
            .unwrap();
        assert_eq!(
            proposal.approve("model", ReviewerKind::Model),
            Err(ContractError::HumanReviewRequired)
        );
        let corrected = proposal.approve("owner", ReviewerKind::Human).unwrap();
        assert_eq!(corrected.definition().version, 2);
        assert_eq!(original.definition().version, 1);
        let record = corrected.correction().unwrap();
        assert_eq!(record.original_version(), 1);
        assert_eq!(record.reason(), "Clarify readiness");
        assert_eq!(record.evidence(), &["run-1/observation"]);
        let roundtrip: PublishedContract =
            serde_json::from_value(serde_json::to_value(&corrected).unwrap()).unwrap();
        assert_eq!(corrected, roundtrip);
        assert_eq!(proposal.original(), &original);
        assert_eq!(
            corrected.definition().assertions,
            original.definition().assertions
        );
    }
    #[test]
    fn rejects_invalid_binding_and_unversioned_correction() {
        let original = draft().publish("owner").unwrap();
        assert_eq!(
            original.bind("p", BTreeMap::new()),
            Err(ContractError::ParameterMismatch)
        );
        assert_eq!(
            original.bind("p", BTreeMap::from([("target".into(), Value::from(3))])),
            Err(ContractError::ParameterType("target".into()))
        );
        assert_eq!(
            original.propose_correction(draft(), "reason", vec!["evidence".into()]),
            Err(ContractError::InvalidCorrection)
        );
    }
    #[test]
    fn serialized_publication_is_validated_when_loaded() {
        let valid = serde_json::to_value(draft().publish("owner").unwrap()).unwrap();
        assert!(serde_json::from_value::<PublishedContract>(valid.clone()).is_ok());
        let mut invalid = valid;
        invalid["definition"]["assertions"] = serde_json::json!([]);
        assert!(serde_json::from_value::<PublishedContract>(invalid).is_err());
    }
}
