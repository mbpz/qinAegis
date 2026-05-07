// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use std::io::stdout;

use ratatui::{
    crossterm::{
        event::{self, Event, KeyCode, KeyEventKind},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    prelude::*,
    Terminal,
};

use crate::config::Config;
use crate::tui::dashboard;
use crate::tui::project_list;
use crate::tui::config_form;
use crate::tui::explore_view;
use crate::tui::generate_view;
use crate::tui::run_view;
use crate::tui::review_view;

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Dashboard,
    ProjectList,
    ConfigForm,
    ExploreView { project_name: String },
    GenerateView { project_name: String },
    RunView { project_name: String },
    ReviewView { project_name: String },
}

/// Lightweight case info for TUI display (avoids storing full TestCase).
#[derive(Debug, Clone)]
pub struct ReviewCaseEntry {
    pub id: String,
    pub name: String,
    pub priority: String,
    pub status: String,
}

#[derive(Debug)]
pub struct App {
    pub current_state: AppState,
    pub projects: Vec<String>,
    pub selected_project: Option<usize>,
    pub message: Option<String>,
    pub is_loading: bool,
    // Config state for ConfigForm
    pub config: Option<Config>,
    // Input state for ExploreView
    pub explore_url: String,
    pub explore_depth: u32,
    pub explore_input_mode: bool,
    pub explore_depth_input: bool,
    // ReviewView state
    pub review_cases: Vec<ReviewCaseEntry>,
    pub review_selected: Option<usize>,
}

impl App {
    pub fn new() -> Self {
        Self {
            current_state: AppState::Dashboard,
            projects: Vec::new(),
            selected_project: None,
            message: None,
            is_loading: false,
            config: Config::load().ok().flatten(),
            explore_url: String::new(),
            explore_depth: 3,
            explore_input_mode: false,
            explore_depth_input: false,
            review_cases: Vec::new(),
            review_selected: None,
        }
    }
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
    }
}

pub fn run() -> anyhow::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let _guard = TerminalGuard;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new();

    run_app(&mut terminal, &mut app)
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|frame| {
            let area = frame.size();
            match &app.current_state {
                AppState::Dashboard => dashboard::render(frame, app, area),
                AppState::ProjectList => project_list::render(frame, app, area),
                AppState::ConfigForm => config_form::render(frame, app, area),
                AppState::ExploreView { .. } => explore_view::render(frame, app, area),
                AppState::GenerateView { .. } => generate_view::render(frame, app, area),
                AppState::RunView { .. } => run_view::render(frame, app, area),
                AppState::ReviewView { .. } => review_view::render(frame, app, area),
            }
        })?;

        // Call on_enter when transitioning to ProjectList
        if let AppState::ProjectList = &app.current_state {
            project_list::on_enter(app);
        }

        // Load cases when entering ReviewView
        if let AppState::ReviewView { project_name } = &app.current_state.clone() {
            review_view::on_enter(app, project_name);
        }

        // Load config when entering ConfigForm
        if matches!(&app.current_state, AppState::ConfigForm) {
            app.config = Config::load().ok().flatten();
        }

        // Pre-fill explore URL from project config when entering ExploreView
        if let AppState::ExploreView { project_name } = &app.current_state.clone() {
            if app.explore_url.is_empty() && !project_name.is_empty() {
                if let Ok(project_cfg) = qin_aegis_core::storage::LocalStorage::load_project(&project_name) {
                    app.explore_url = project_cfg.url;
                }
            }
        }

        if !handle_events(app)? {
            break;
        }
    }
    Ok(())
}

fn handle_events(app: &mut App) -> anyhow::Result<bool> {
    if let Event::Key(key) = event::read()? {
        if key.kind == KeyEventKind::Press {
            match key.code {
                KeyCode::Char('q') => {
                    match &app.current_state {
                        AppState::Dashboard | AppState::ProjectList => {
                            return Ok(false);
                        }
                        AppState::ConfigForm | AppState::ExploreView { .. } | AppState::GenerateView { .. } | AppState::RunView { .. } | AppState::ReviewView { .. } => {
                            app.current_state = AppState::Dashboard;
                        }
                    }
                }
                KeyCode::Esc => {
                    match &app.current_state {
                        AppState::ExploreView { .. } => {
                            if app.explore_input_mode {
                                app.explore_input_mode = false;
                            } else {
                                app.current_state = AppState::Dashboard;
                            }
                        }
                        AppState::ConfigForm | AppState::GenerateView { .. } | AppState::RunView { .. } | AppState::ReviewView { .. } => {
                            app.current_state = AppState::Dashboard;
                        }
                        _ => {}
                    }
                }
                KeyCode::Enter => {
                    if let AppState::ConfigForm = &app.current_state {
                        app.current_state = AppState::Dashboard;
                        app.message = Some("Settings saved".to_string());
                    } else if let AppState::Dashboard = &app.current_state {
                        app.current_state = AppState::ProjectList;
                    } else if let AppState::ProjectList = &app.current_state {
                        if let Some(idx) = app.selected_project.clone() {
                            let name = app.projects[idx].clone();
                            app.explore_input_mode = false;
                            app.explore_url.clear();
                            app.current_state = AppState::ExploreView { project_name: name };
                        }
                    } else if let AppState::ExploreView { .. } = &app.current_state {
                        if app.explore_input_mode {
                            app.explore_input_mode = false;
                            // Start explore
                            let url = app.explore_url.clone();
                            let depth = app.explore_depth;
                            let project_name = match &app.current_state {
                                AppState::ExploreView { project_name } => project_name.clone(),
                                _ => String::new(),
                            };
                            app.is_loading = true;
                            app.message = Some("Exploring...".to_string());
                            // Run async command in background
                            let handle = tokio::runtime::Handle::current();
                            std::thread::spawn(move || {
                                let result = handle.block_on(
                                    crate::commands::explore::run_explore(&project_name, Some(url), depth)
                                );
                                if let Err(e) = result {
                                    eprintln!("Explore error: {}", e);
                                }
                            });
                            app.current_state = AppState::Dashboard;
                        } else {
                            app.explore_input_mode = true;
                        }
                    }
                }
                KeyCode::Down => {
                    match &app.current_state {
                        AppState::ProjectList => {
                            if let Some(idx) = app.selected_project {
                                if idx + 1 < app.projects.len() {
                                    app.selected_project = Some(idx + 1);
                                }
                            } else if !app.projects.is_empty() {
                                app.selected_project = Some(0);
                            }
                        }
                        AppState::ReviewView { .. } => {
                            if let Some(idx) = app.review_selected {
                                if idx + 1 < app.review_cases.len() {
                                    app.review_selected = Some(idx + 1);
                                }
                            } else if !app.review_cases.is_empty() {
                                app.review_selected = Some(0);
                            }
                        }
                        _ => {}
                    }
                }
                KeyCode::Up => {
                    match &app.current_state {
                        AppState::ProjectList => {
                            if let Some(idx) = app.selected_project {
                                if idx > 0 {
                                    app.selected_project = Some(idx - 1);
                                }
                            }
                        }
                        AppState::ReviewView { .. } => {
                            if let Some(idx) = app.review_selected {
                                if idx > 0 {
                                    app.review_selected = Some(idx - 1);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                KeyCode::Char('a') => {
                    match &app.current_state {
                        AppState::ProjectList => {
                            app.message = Some("Use CLI: qinAegis project add".to_string());
                        }
                        AppState::ReviewView { project_name } => {
                            let project = project_name.clone();
                            if let Some(idx) = app.review_selected {
                                if let Some(case) = app.review_cases.get(idx) {
                                    let case_id = case.id.clone();
                                    app.message = Some(format!("Approving {}...", case_id));
                                    let handle = tokio::runtime::Handle::current();
                                    std::thread::spawn(move || {
                                        let result = handle.block_on(
                                            crate::commands::review::run_review(
                                                &project,
                                                Some(crate::commands::review::ReviewAction::Approve { case_id }),
                                            )
                                        );
                                        if let Err(e) = result {
                                            eprintln!("Approve error: {}", e);
                                        }
                                    });
                                    app.review_cases.remove(idx);
                                    app.review_selected = None;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                KeyCode::Char('r') => {
                    if let AppState::ReviewView { project_name } = &app.current_state {
                        let project = project_name.clone();
                        if let Some(idx) = app.review_selected {
                            if let Some(case) = app.review_cases.get(idx) {
                                let case_id = case.id.clone();
                                app.message = Some(format!("Rejecting {}...", case_id));
                                let handle = tokio::runtime::Handle::current();
                                std::thread::spawn(move || {
                                    let result = handle.block_on(
                                        crate::commands::review::run_review(
                                            &project,
                                            Some(crate::commands::review::ReviewAction::Reject { case_id }),
                                        )
                                    );
                                    if let Err(e) = result {
                                        eprintln!("Reject error: {}", e);
                                    }
                                });
                                app.review_cases.remove(idx);
                                app.review_selected = None;
                            }
                        }
                    }
                }
                KeyCode::Char('1') => {
                    if let AppState::Dashboard = &app.current_state {
                        app.current_state = AppState::ProjectList;
                    }
                }
                KeyCode::Char('2') => {
                    if let AppState::Dashboard = &app.current_state {
                        app.current_state = AppState::GenerateView { project_name: String::new() };
                    }
                }
                KeyCode::Char('3') => {
                    if let AppState::Dashboard = &app.current_state {
                        app.current_state = AppState::ConfigForm;
                    }
                }
                KeyCode::Char('4') => {
                    if let AppState::Dashboard = &app.current_state {
                        app.current_state = AppState::ReviewView { project_name: String::new() };
                    }
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    if let AppState::ExploreView { .. } = &app.current_state {
                        if app.explore_depth < 10 {
                            app.explore_depth += 1;
                        }
                    }
                }
                KeyCode::Char('-') => {
                    if let AppState::ExploreView { .. } = &app.current_state {
                        if app.explore_depth > 1 {
                            app.explore_depth -= 1;
                        }
                    }
                }
                KeyCode::Char(c) => {
                    if let AppState::ExploreView { .. } = &app.current_state {
                        if app.explore_input_mode {
                            app.explore_url.push(c);
                        } else if c == 'i' {
                            app.explore_input_mode = true;
                        }
                    }
                }
                KeyCode::Backspace => {
                    if let AppState::ExploreView { .. } = &app.current_state {
                        if app.explore_input_mode {
                            app.explore_url.pop();
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(true)
}
