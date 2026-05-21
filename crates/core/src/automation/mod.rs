// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

pub use self::trait_def::{AutomationError, AutomationCommand, AutomationResponse};
pub use self::trait_def::{BrowserAutomation, ExploreResult, PageInfo, FormInfo, TestResult};
pub use self::midscene::{BfsExplorer, MidsceneAutomation, AuthConfig};
pub use self::native::NativeAutomation;

pub mod trait_def;
pub mod midscene;
pub mod native;
