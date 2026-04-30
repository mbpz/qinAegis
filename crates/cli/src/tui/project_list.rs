use ratatui::{Frame, prelude::Rect, widgets::{Block, Borders, List, ListItem}};
use qin_aegis_core::storage::LocalStorage;
use crate::tui::app::App;
use crate::tui::components;

pub fn on_enter(app: &mut App) {
    app.is_loading = true;
    match LocalStorage::list_projects() {
        Ok(projects) => app.projects = projects,
        Err(e) => app.message = Some(format!("Error: {}", e)),
    }
    app.is_loading = false;
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let [top, middle, bottom] = components::three_panel(area);
    components::title_bar(frame, top, "Select Project");

    let items: Vec<ListItem> = app.projects.iter()
        .enumerate()
        .map(|(i, name)| {
            let prefix = if Some(i) == app.selected_project { "▶ " } else { "  " };
            ListItem::new(format!("{}{}", prefix, name))
        })
        .collect();

    let list = List::new(items)
        .block(Block::new().borders(Borders::ALL));

    frame.render_widget(list, middle);

    let hint = if app.projects.is_empty() {
        "q: quit | a: add project"
    } else {
        "q: quit | ↑↓: select | Enter: confirm | a: add"
    };
    components::status_bar(frame, bottom, hint);
}
