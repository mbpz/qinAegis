use ratatui::{Frame, prelude::Rect, widgets::{Block, Borders, List, ListItem}};
use qin_aegis_core::storage::LocalStorage;
use crate::tui::app::App;

pub fn on_enter(app: &mut App) {
    app.is_loading = true;
    match LocalStorage::list_projects() {
        Ok(projects) => app.projects = projects,
        Err(e) => app.message = Some(format!("Error: {}", e)),
    }
    app.is_loading = false;
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app.projects.iter()
        .enumerate()
        .map(|(i, name)| {
            let prefix = if Some(i) == app.selected_project { "▶ " } else { "  " };
            ListItem::new(format!("{}{}", prefix, name))
        })
        .collect();

    let list = List::new(items)
        .block(Block::new().title("Select Project").borders(Borders::ALL));

    frame.render_widget(list, area);
}
