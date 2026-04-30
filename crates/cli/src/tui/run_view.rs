use ratatui::{Frame, prelude::Rect, widgets::{Block, Borders, Paragraph}};
use crate::tui::app::App;
use crate::tui::components;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let [top, middle, bottom] = components::three_panel(area);
    components::title_bar(frame, top, "Run Tests");

    let content = Paragraph::new("Run Tests\n\n[Esc] Back")
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(content, middle);
    components::status_bar(frame, bottom, "[Esc] Back");
}
