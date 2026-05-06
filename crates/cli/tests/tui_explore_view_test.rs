// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! ExploreView render snapshot tests

use ratatui::{backend::TestBackend, buffer::Buffer, prelude::Rect, Frame, Terminal};
use qinAegis_lib::tui::app::{App, AppState};
use qinAegis_lib::tui::explore_view;

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

fn make_explore_app(project_name: &str) -> App {
    let mut app = App::new();
    app.current_state = AppState::ExploreView {
        project_name: project_name.into(),
    };
    app.explore_url = String::new();
    app.explore_depth = 3;
    app.explore_input_mode = false;
    app
}

#[test]
fn test_explore_view_idle_empty_url() {
    let app = make_explore_app("test-project");
    let output = render_to_string(&app, explore_view::render);
    insta::assert_snapshot!("explore_idle_empty_url", output);
}

#[test]
fn test_explore_view_with_url() {
    let mut app = make_explore_app("my-project");
    app.explore_url = "https://example.com".into();
    let output = render_to_string(&app, explore_view::render);
    insta::assert_snapshot!("explore_with_url", output);
}

#[test]
fn test_explore_view_input_mode() {
    let mut app = make_explore_app("test-project");
    app.explore_url = "https://example.com".into();
    app.explore_input_mode = true;
    let output = render_to_string(&app, explore_view::render);
    insta::assert_snapshot!("explore_input_mode", output);
}

#[test]
fn test_explore_view_url_empty_input_mode() {
    let mut app = make_explore_app("test-project");
    app.explore_url = "http".into();
    app.explore_input_mode = true;
    let output = render_to_string(&app, explore_view::render);
    insta::assert_snapshot!("explore_url_empty_input_mode", output);
}

#[test]
fn test_explore_view_depth_adjusted() {
    let mut app = make_explore_app("test-project");
    app.explore_url = "https://example.com".into();
    app.explore_depth = 7;
    let output = render_to_string(&app, explore_view::render);
    insta::assert_snapshot!("explore_depth_adjusted", output);
}
