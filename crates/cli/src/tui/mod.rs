pub mod app;
pub mod dashboard;
pub mod project_list;
pub mod config_form;
pub mod explore_view;
pub mod generate_view;
pub mod run_view;
pub mod components;

pub use app::{App, AppState};
pub use app::run as run_tui;
