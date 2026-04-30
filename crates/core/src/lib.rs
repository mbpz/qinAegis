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
// Automation — new BrowserAutomation trait and implementations
pub use automation::{
    AutomationError, AutomationCommand, AutomationResponse, BrowserAutomation,
    ExploreResult, PageInfo, FormInfo,
    MidsceneAutomation, BfsExplorer,
};