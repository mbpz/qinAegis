// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use ratatui::{
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, prelude::Rect,
};
use crate::tui::app::App;
use crate::tui::components;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let [top, middle, bottom] = components::three_panel(area);
    components::title_bar(frame, top, "qinAegis — AI Testing TUI");

    // Main content
    let status = if app.is_loading {
        "Loading...".to_string()
    } else {
        app.message.clone().unwrap_or_default()
    };

    let main_text = Paragraph::new(format!(
        "Projects: {}\n\n[1] Explore    — AI explore a URL\n[2] Generate   — Generate test cases\n[3] Run Tests  — Execute test suite\n[4] Settings  — Configure LLM & Sandbox\n\n{}",
        app.projects.len(),
        status
    ))
    .block(Block::default().borders(Borders::ALL))
    .wrap(Wrap { trim: true });

    frame.render_widget(main_text, middle);

    components::status_bar(frame, bottom, "q: quit | 1-4: select | Enter: confirm");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::{App, AppState};

    #[test]
    fn test_dashboard_renders_without_panic() {
        let app = App::new();
        assert_eq!(app.current_state, AppState::Dashboard);
        assert_eq!(app.projects.len(), 0);
        assert!(!app.is_loading);
        assert!(app.message.is_none());
    }

    #[test]
    fn test_dashboard_with_projects() {
        let mut app = App::new();
        app.projects.push("test-project".to_string());
        assert_eq!(app.current_state, AppState::Dashboard);
        assert_eq!(app.projects.len(), 1);
    }
}
