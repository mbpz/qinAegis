pub use self::trait_def::{AutomationError, AutomationCommand, AutomationResponse};
pub use self::trait_def::{BrowserAutomation, ExploreResult, PageInfo, FormInfo, TestResult};
pub use self::midscene::{BfsExplorer, MidsceneAutomation};

pub mod trait_def;
pub mod midscene;
