use ratatui::{Frame, prelude::Rect, widgets::{Block, Borders, Paragraph}};
use crate::tui::app::App;
use crate::tui::components;

pub fn render(frame: &mut Frame, _app: &App, area: Rect) {
    let [top, middle, bottom] = components::three_panel(area);
    components::title_bar(frame, top, "Settings");

    let content = Paragraph::new(
        "LLM Configuration\n\n\
         Provider:  minimax\n\
         Base URL:  https://api.minimax.chat/v1\n\
         Model:     MiniMax-VL-01\n\n\
         Sandbox Configuration\n\n\
         Steel Port:  3333\n\
         CDP Port:    9222"
    ).block(Block::default().borders(Borders::ALL));

    frame.render_widget(content, middle);
    components::status_bar(frame, bottom, "[Enter] Save  [Esc] Cancel");
}