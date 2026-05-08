// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// Storage data types
// ============================================================================

/// Test case lifecycle status.
///
/// ```text
/// Draft ──review──▶ Reviewed ──approve──▶ Approved ──run──▶ Passed / Failed
/// Failed ──triage──▶ Approved / Draft / Flaky
/// Flaky ──stabilize──▶ Approved
/// Approved ──archive──▶ Archived
/// Flaky ──archive──▶ Archived
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaseStatus {
    Draft,
    Reviewed,
    Approved,
    Flaky,
    Archived,
}

impl CaseStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CaseStatus::Draft => "draft",
            CaseStatus::Reviewed => "reviewed",
            CaseStatus::Approved => "approved",
            CaseStatus::Flaky => "flaky",
            CaseStatus::Archived => "archived",
        }
    }

    pub fn dir_name(&self) -> &'static str {
        self.as_str()
    }

    /// Valid transitions from this status.
    pub fn can_transition_to(&self, target: CaseStatus) -> bool {
        matches!(
            (self, target),
            (CaseStatus::Draft, CaseStatus::Reviewed)
                | (CaseStatus::Reviewed, CaseStatus::Approved)
                | (CaseStatus::Reviewed, CaseStatus::Draft)
                | (CaseStatus::Approved, CaseStatus::Archived)
                | (CaseStatus::Approved, CaseStatus::Flaky)
                | (CaseStatus::Flaky, CaseStatus::Approved)
                | (CaseStatus::Flaky, CaseStatus::Archived)
        )
    }
}

impl std::fmt::Display for CaseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub tech_stack: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub name: String,
    pub requirement_id: String,
    pub test_type: String,
    pub yaml_script: String,
    pub priority: String,
    pub created_at: String,
    #[serde(default = "default_status")]
    pub status: CaseStatus,
}

fn default_status() -> CaseStatus {
    CaseStatus::Draft
}

// ============================================================================
// Storage error
// ============================================================================

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("project not found: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("transaction failed: {0}")]
    Transaction(String),
    #[error("credential error: {0}")]
    Credential(String),
    #[error("internal: {0}")]
    Internal(String),
}

// ============================================================================
// Storage credentials
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageCredentials {
    Local,
    S3 {
        bucket: String,
        region: String,
        access_key_id: String,
        secret_access_key: String,
    },
    Gcs {
        bucket: String,
        project_id: String,
        service_account_json: String,
    },
}

impl Default for StorageCredentials {
    fn default() -> Self {
        Self::Local
    }
}

// ============================================================================
// Storage transaction guard
// ============================================================================

pub trait StorageTransaction: Send {
    fn commit(&mut self) -> Result<(), StorageError>;
    fn rollback(&mut self) -> Result<(), StorageError>;
}

// ============================================================================
// Storage trait
// ============================================================================

#[async_trait]
pub trait Storage: Send + Sync {
    // Project operations
    async fn init_project(
        &self,
        name: &str,
        url: &str,
        tech_stack: Vec<String>,
    ) -> Result<ProjectConfig, StorageError>;

    async fn list_projects(&self) -> Result<Vec<String>, StorageError>;

    async fn load_project(&self, name: &str) -> Result<ProjectConfig, StorageError>;

    async fn delete_project(&self, name: &str) -> Result<(), StorageError>;

    // Spec operations
    async fn save_spec(&self, name: &str, markdown: &str) -> Result<(), StorageError>;

    async fn load_spec(&self, name: &str) -> Result<String, StorageError>;

    // Case operations
    async fn save_case(&self, name: &str, case: &TestCase) -> Result<(), StorageError>;

    async fn load_cases(&self, name: &str) -> Result<Vec<TestCase>, StorageError>;

    /// Load cases filtered by status.
    async fn load_cases_by_status(&self, name: &str, status: CaseStatus) -> Result<Vec<TestCase>, StorageError>;

    async fn delete_case(&self, name: &str, case_id: &str) -> Result<(), StorageError>;

    /// Move a case from one status to another (atomic file move).
    async fn move_case(&self, name: &str, case_id: &str, from: CaseStatus, to: CaseStatus) -> Result<(), StorageError>;

    // Transaction support
    async fn begin_transaction(&self) -> Result<Box<dyn StorageTransaction>, StorageError>;
}
