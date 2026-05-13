// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

pub mod explorer;
pub mod generator;
pub mod critic;
pub mod llm;
pub mod protocol;
pub mod executor;
pub mod reporter;
pub mod performance;
pub mod stress;
pub mod storage;
pub mod automation;
pub mod service;
pub mod prompts;
pub mod sandbox;
pub mod config;
pub mod gate;
pub mod knowledge;
pub mod healer;

pub use explorer::Explorer;
pub use generator::TestCaseGenerator;
pub use critic::{Critic, CriticReview};
pub use llm::{ArcLlmClient, LlmClient, MiniMaxClient, Message};
pub use protocol::{JsonRpcRequest, JsonRpcResponse, MidsceneProcess, LlmConfig, SandboxConfig};
pub use executor::{TestExecutor, TestCaseRef, TestResult};
pub use reporter::Reporter;
pub use performance::{LighthouseMetrics, LighthouseResult, PerformanceComparison};
pub use stress::{LocustStats, LocustResult, StressTestConfig};
pub use service::TestCaseService;
// Gate service
pub use gate::{GateService, GateResult, GateThresholds, GateStatus, E2EGateResult, PerfGateResult, StressGateResult};
// Automation — new BrowserAutomation trait and implementations
pub use automation::{
    AutomationError, AutomationCommand, AutomationResponse, BrowserAutomation,
    ExploreResult, PageInfo, FormInfo,
    MidsceneAutomation, BfsExplorer, AuthConfig,
};
// Storage
pub use storage::{CaseStatus, LocalStorage, LocalStorageInstance};
pub use config::app::{AppConfig, resolve_env_var};

// Sandbox adapters
pub use sandbox::PlaywrightBrowserAdapter;
#[cfg(test)]
mod basic_test {
    #[test]
    fn test_simple() {
        assert_eq!(2 + 2, 4);
    }
}
