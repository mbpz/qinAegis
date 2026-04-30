use ratatui::{backend::TestBackend, buffer::Buffer, prelude::Rect, Frame, Terminal};
use qinAegis_lib::tui::app::App;
use qinAegis_lib::tui::dashboard;

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

#[test]
fn test_dashboard_empty_state() {
    let app = App::new();
    let output = render_to_string(&app, dashboard::render);
    insta::assert_snapshot!("dashboard_empty", output);
}

#[test]
fn test_dashboard_with_projects() {
    let mut app = App::new();
    app.projects.push("my-project".into());
    app.projects.push("another-project".into());
    let output = render_to_string(&app, dashboard::render);
    insta::assert_snapshot!("dashboard_with_projects", output);
}

#[test]
fn test_dashboard_loading_state() {
    let mut app = App::new();
    app.is_loading = true;
    let output = render_to_string(&app, dashboard::render);
    insta::assert_snapshot!("dashboard_loading", output);
}

#[test]
fn test_dashboard_with_message() {
    let mut app = App::new();
    app.message = Some("Explore complete!".into());
    let output = render_to_string(&app, dashboard::render);
    insta::assert_snapshot!("dashboard_with_message", output);
}