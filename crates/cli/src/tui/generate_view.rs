// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use ratatui::{Frame, prelude::Rect, widgets::{Block, Borders, Paragraph}};
use crate::tui::app::App;
use crate::tui::components;

pub fn render(frame: &mut Frame, _app: &App, area: Rect) {
    let [top, middle, bottom] = components::three_panel(area);
    components::title_bar(frame, top, "Generate Test Cases");

    let content = Paragraph::new("Generate test cases\n\n[Esc] Back")
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(content, middle);
    components::status_bar(frame, bottom, "[Esc] Back");
}
