// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// Storage data types
// ============================================================================

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

#[derive(Debug, Clone)]
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

    async fn delete_case(&self, name: &str, case_id: &str) -> Result<(), StorageError>;

    // Transaction support
    async fn begin_transaction(&self) -> Result<Box<dyn StorageTransaction>, StorageError>;
}
