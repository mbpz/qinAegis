use ratatui::{Frame, prelude::Rect, layout::{Layout, Constraint}, widgets::{Block, Borders, Paragraph}};
use crate::tui::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(area);
    let content = Paragraph::new("Run Tests\n\n[Esc] Back")
        .block(Block::default().title("Run").borders(Borders::ALL));
    frame.render_widget(content, chunks[0]);
}
