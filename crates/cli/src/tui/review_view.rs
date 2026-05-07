// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use qin_aegis_core::storage::Storage;
use ratatui::{
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, prelude::Rect,
};
use crate::tui::app::{App, ReviewCaseEntry};
use crate::tui::components;

/// Called when entering the ReviewView — loads cases from storage.
pub fn on_enter(app: &mut App, project_name: &str) {
    let rt = tokio::runtime::Handle::current();
    let project = project_name.to_string();
    let cases: Vec<ReviewCaseEntry> = rt.block_on(async {
        let storage = qin_aegis_core::storage::LocalStorageInstance::new();
        let all_cases = storage.load_cases(&project).await.unwrap_or_default();
        all_cases
            .into_iter()
            .filter(|c| c.status != qin_aegis_core::CaseStatus::Archived)
            .map(|c| ReviewCaseEntry {
                id: c.id,
                name: c.name,
                priority: c.priority,
                status: c.status.as_str().to_string(),
            })
            .collect()
    });
    app.review_cases = cases;
    app.review_selected = if app.review_cases.is_empty() { None } else { Some(0) };
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let [top, middle, bottom] = components::three_panel(area);
    components::title_bar(frame, top, "qinAegis — Case Review");

    if app.review_cases.is_empty() {
        let text = Paragraph::new("No cases to review.\n\nRun 'qinAegis generate' first, or go back with [q].")
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(text, middle);
    } else {
        let items: Vec<ListItem> = app
            .review_cases
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let marker = if Some(i) == app.review_selected { "▶" } else { " " };
                let status_icon = match c.status.as_str() {
                    "draft" => "📝",
                    "reviewed" => "🔍",
                    "approved" => "✅",
                    "flaky" => "⚠️",
                    _ => "  ",
                };
                ListItem::new(format!(
                    "{} {} [{}] {} ({})",
                    marker, status_icon, c.id, c.name, c.priority
                ))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Cases"))
            .highlight_style(ratatui::style::Style::default());
        frame.render_widget(list, middle);
    }

    components::status_bar(
        frame,
        bottom,
        "q: back | ↑↓: select | a: approve | r: reject | Esc: dashboard",
    );
}
