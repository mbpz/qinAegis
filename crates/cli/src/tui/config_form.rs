// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use ratatui::{Frame, prelude::Rect, widgets::{Block, Borders, Paragraph}, style::Stylize};
use crate::tui::app::App;
use crate::tui::components;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let [top, middle, bottom] = components::three_panel(area);
    components::title_bar(frame, top, "Settings");

    let content = match &app.config {
        Some(cfg) => {
            let api_key_display = if cfg.llm.api_key.is_empty() {
                "(not set)".to_string()
            } else {
                format!("{}****", &cfg.llm.api_key[..cfg.llm.api_key.len().min(4)])
            };

            Paragraph::new(format!(
                "LLM Configuration\n\n\
                 Provider:  {}\n\
                 Base URL:  {}\n\
                 API Key:   {}\n\
                 Model:     {}\n\n\
                 Sandbox Configuration\n\n\
                 CDP Port:    {}\n\n\
                 Exploration\n\n\
                 Max Depth:        {}\n\
                 Max Pages/Seed:   {}",
                cfg.llm.provider,
                cfg.llm.base_url,
                api_key_display,
                cfg.llm.model,
                cfg.sandbox.cdp_port,
                cfg.exploration.max_depth,
                cfg.exploration.max_pages_per_seed,
            )).block(Block::default().title("Settings").borders(Borders::ALL))
        }
        None => {
            Paragraph::new(
                "No configuration found.\n\n\
                 Run 'qinAegis init' to set up AI model credentials.\n\n\
                 [Esc] Back"
            ).block(Block::default().title("Settings").borders(Borders::ALL))
        }
    };

    frame.render_widget(content, middle);
    components::status_bar(frame, bottom, "[Esc] Back");
}
