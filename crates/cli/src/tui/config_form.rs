use ratatui::{Frame, prelude::Rect, layout::Layout, layout::Constraint, widgets::{Block, Borders, Paragraph}};
use crate::tui::app::App;

pub fn render(frame: &mut Frame, _app: &App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ]).split(area);

    let content = Paragraph::new(
        "LLM Configuration\n\n\
         Provider:  minimax\n\
         Base URL:  https://api.minimax.chat/v1\n\
         Model:     MiniMax-VL-01\n\n\
         Sandbox Configuration\n\n\
         Steel Port:  3333\n\
         CDP Port:    9222\n\n\
         [Enter] Save  [Esc] Cancel"
    ).block(Block::default().title("Settings").borders(Borders::ALL));

    frame.render_widget(content, chunks[1]);
}