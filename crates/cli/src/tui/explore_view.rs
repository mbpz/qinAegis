use ratatui::{Frame, prelude::Rect, layout::{Layout, Constraint}, widgets::{Block, Borders, Paragraph}};
use crate::tui::app::{App, AppState};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ]).split(area);

    let (project_name, url, depth) = match &app.current_state {
        AppState::ExploreView { project_name } => {
            (project_name.clone(), "<enter URL>".to_string(), "3".to_string())
        }
        _ => (String::new(), String::new(), String::new()),
    };

    let content = Paragraph::new(format!(
        "Explore: {}\n\n\
         URL: {}\n\
         Depth: {}\n\n\
         [Enter] Start Explore  [Esc] Back",
        project_name, url, depth
    )).block(Block::default().title("Explore").borders(Borders::ALL));

    frame.render_widget(content, chunks[1]);
}
