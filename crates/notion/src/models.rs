pub mod project;
pub mod requirement;
pub mod test_case;
pub mod test_result;

pub use project::{Project, ProjectStatus};
pub use requirement::{Requirement, RequirementStatus};
pub use test_case::{TestCase, TestType, TestCaseStatus, CreatedBy};
pub use test_result::{TestResult, TestResultStatus, Environment};

// Re-export PropertySchema and DatabaseSpec for backward compatibility
use serde::{Deserialize, Serialize};

// Shared types accessible across all models
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    P0,
    P1,
    P2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertySchema {
    pub name: String,
    pub property_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSpec {
    pub name: String,
    pub properties: Vec<PropertySchema>,
}
