use ratatui::{Frame, prelude::Rect, widgets::{Block, Borders, Paragraph}};
use crate::tui::app::{App, AppState};
use crate::tui::components;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let [top, middle, bottom] = components::three_panel(area);

    let project_name = match &app.current_state {
        AppState::ExploreView { project_name } => project_name.clone(),
        _ => String::new(),
    };

    components::title_bar(frame, top, &format!("Explore — {}", project_name));

    let input_display = if app.explore_input_mode {
        format!("{}_", app.explore_url)
    } else {
        format!("{}", if app.explore_url.is_empty() { "<enter URL>" } else { &app.explore_url })
    };

    let help_text = if app.explore_input_mode {
        "[Type URL] [Enter] Confirm  [Esc] Cancel"
    } else {
        "[i] Enter URL  [Enter] Start  [Esc] Back"
    };

    let content = Paragraph::new(format!(
        "URL: {}\nDepth: {}\n\n{}",
        input_display, app.explore_depth, help_text
    )).block(Block::default().borders(Borders::ALL));

    frame.render_widget(content, middle);
    components::status_bar(frame, bottom, "[Esc] Back");
}
