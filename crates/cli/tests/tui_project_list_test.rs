//! ProjectList render snapshot tests

use ratatui::{backend::TestBackend, buffer::Buffer, prelude::Rect, Frame, Terminal};
use qinAegis_lib::tui::app::App;
use qinAegis_lib::tui::project_list;

/// Renders a view function to a String by capturing the TestBackend buffer.
fn render_to_string<F>(app: &App, view: F) -> String
where
    F: Fn(&mut Frame, &App, Rect),
{
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| {
        let area = frame.size();
        view(frame, app, area);
    }).unwrap();
    buffer_to_string(terminal.backend().buffer())
}

/// Extracts buffer content as newline-separated String
fn buffer_to_string(buffer: &Buffer) -> String {
    let area = buffer.area();
    let mut result = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buffer.get(x, y);
            result.push_str(cell.symbol());
        }
        if y < area.height - 1 {
            result.push('\n');
        }
    }
    result
}

fn make_app_with_projects() -> App {
    let mut app = App::new();
    app.projects = vec!["proj-a".into(), "proj-b".into(), "proj-c".into()];
    app
}

#[test]
fn test_project_list_first_selected() {
    let mut app = make_app_with_projects();
    app.selected_project = Some(0);
    let output = render_to_string(&app, project_list::render);
    insta::assert_snapshot!("project_list_first_selected", output);
}

#[test]
fn test_project_list_second_selected() {
    let mut app = make_app_with_projects();
    app.selected_project = Some(1);
    let output = render_to_string(&app, project_list::render);
    insta::assert_snapshot!("project_list_second_selected", output);
}

#[test]
fn test_project_list_empty() {
    let app = App::new();
    let output = render_to_string(&app, project_list::render);
    insta::assert_snapshot!("project_list_empty", output);
}