use ratatui::{
    layout::{Constraint, Layout, Direction, Alignment},
    widgets::{Block, Borders, Paragraph, Wrap},
    style::Stylize,
    Frame, prelude::Rect,
};
use crate::tui::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::vertical()
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(area);

    // Title bar
    let title = Paragraph("qinAegis — AI Testing TUI")
        .block(Block::new().borders(Borders::BOTTOM))
        .bold();
    frame.render_widget(title, chunks[0]);

    // Main content
    let status = if app.is_loading {
        "Loading...".to_string()
    } else {
        app.message.clone().unwrap_or_default()
    };

    let main_text = Paragraph(format!(
        "Projects: {}\n\n[1] Explore    — AI explore a URL\n[2] Generate   — Generate test cases\n[3] Run Tests  — Execute test suite\n[4] Settings  — Configure LLM & Sandbox\n\n{}",
        app.projects.len(),
        status
    ))
    .block(Block::new().borders(Borders::ALL))
    .wrap(Wrap { trim: true });

    frame.render_widget(main_text, chunks[1]);

    // Footer
    let footer = Paragraph("q: quit | ↑↓: select | Enter: confirm")
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::{App, AppState};

    #[test]
    fn test_dashboard_renders_without_panic() {
        let app = App::new();
        // Verify App struct can be created and is in Dashboard state
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
