use ratatui::{Frame, prelude::Rect, layout::Layout, layout::Constraint, widgets::{Block, Borders, Paragraph}, style::Stylize};

/// Renders a title bar at the top of the area
pub fn title_bar(frame: &mut Frame, area: Rect, title: &str) {
    let block = Block::default().borders(Borders::BOTTOM);
    let para = Paragraph::new(title).bold().block(block);
    frame.render_widget(para, area);
}

/// Renders a status bar at the bottom of the area
pub fn status_bar(frame: &mut Frame, area: Rect, text: &str) {
    let block = Block::default().borders(Borders::TOP);
    let para = Paragraph::new(text).block(block);
    frame.render_widget(para, area);
}

/// Splits area into three vertical chunks: top, middle, bottom
pub fn three_panel(area: Rect) -> [Rect; 3] {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ]).split(area);
    [chunks[0], chunks[1], chunks[2]]
}