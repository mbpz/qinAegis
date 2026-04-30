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

use crate::tui::dashboard;
use crate::tui::project_list;
use crate::tui::config_form;
use crate::tui::explore_view;
use crate::tui::generate_view;
use crate::tui::run_view;

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Dashboard,
    ProjectList,
    ConfigForm,
    ExploreView { project_name: String },
    GenerateView { project_name: String },
    RunView { project_name: String },
}

#[derive(Debug)]
pub struct App {
    pub current_state: AppState,
    pub projects: Vec<String>,
    pub selected_project: Option<usize>,
    pub message: Option<String>,
    pub is_loading: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            current_state: AppState::Dashboard,
            projects: Vec::new(),
            selected_project: None,
            message: None,
            is_loading: false,
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new();

    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    res
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
            }
        })?;

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
                KeyCode::Char('q') | KeyCode::Esc => {
                    return Ok(false);
                }
                KeyCode::Enter => {
                    if let AppState::Dashboard = &app.current_state {
                        app.current_state = AppState::ProjectList;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(true)
}
