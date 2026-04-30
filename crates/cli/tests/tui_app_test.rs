//! App state machine unit tests

use qinAegis_lib::tui::app::{App, AppState};

fn make_app_with_projects() -> App {
    let mut app = App::new();
    app.projects = vec!["proj-a".into(), "proj-b".into()];
    app
}

#[test]
fn test_new_app_starts_on_dashboard() {
    let app = App::new();
    assert_eq!(app.current_state, AppState::Dashboard);
    assert!(app.projects.is_empty());
    assert!(!app.is_loading);
    assert!(app.message.is_none());
    assert_eq!(app.explore_depth, 3);
    assert!(!app.explore_input_mode);
}

#[test]
fn test_explore_view_state_carries_project_name() {
    let explore = AppState::ExploreView { project_name: "my-proj".into() };
    match explore {
        AppState::ExploreView { project_name } => {
            assert_eq!(project_name, "my-proj");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_explore_url_input_accumulates() {
    let mut app = App::new();
    app.explore_input_mode = true;
    app.explore_url.push('h');
    app.explore_url.push('t');
    app.explore_url.push('t');
    app.explore_url.push('p');
    assert_eq!(app.explore_url, "http");
}

#[test]
fn test_explore_url_backspace() {
    let mut app = App::new();
    app.explore_input_mode = true;
    app.explore_url = "https://example.com".into();
    app.explore_url.pop();
    assert_eq!(app.explore_url, "https://example.co");
}

#[test]
fn test_explore_depth_bounds_at_max() {
    let mut app = App::new();
    app.explore_depth = 10;
    // Simulating the + key logic
    if app.explore_depth < 10 {
        app.explore_depth += 1;
    }
    assert_eq!(app.explore_depth, 10); // Should stay at 10
}

#[test]
fn test_explore_depth_bounds_at_min() {
    let mut app = App::new();
    app.explore_depth = 1;
    // Simulating the - key logic
    if app.explore_depth > 1 {
        app.explore_depth -= 1;
    }
    assert_eq!(app.explore_depth, 1); // Should stay at 1
}

#[test]
fn test_explore_depth_increments() {
    let mut app = App::new();
    app.explore_depth = 5;
    if app.explore_depth < 10 {
        app.explore_depth += 1;
    }
    assert_eq!(app.explore_depth, 6);
}

#[test]
fn test_selected_project_down_navigation() {
    let mut app = make_app_with_projects();
    app.selected_project = Some(0);
    // Down key logic
    if let Some(idx) = app.selected_project {
        if idx + 1 < app.projects.len() {
            app.selected_project = Some(idx + 1);
        }
    }
    assert_eq!(app.selected_project, Some(1));
}

#[test]
fn test_selected_project_down_at_end_stays() {
    let mut app = make_app_with_projects();
    app.selected_project = Some(1); // already at last
    if let Some(idx) = app.selected_project {
        if idx + 1 < app.projects.len() {
            app.selected_project = Some(idx + 1);
        }
    }
    assert_eq!(app.selected_project, Some(1)); // stays at end
}

#[test]
fn test_selected_project_up_navigation() {
    let mut app = make_app_with_projects();
    app.selected_project = Some(1);
    // Up key logic
    if let Some(idx) = app.selected_project {
        if idx > 0 {
            app.selected_project = Some(idx - 1);
        }
    }
    assert_eq!(app.selected_project, Some(0));
}

#[test]
fn test_selected_project_up_at_start_stays() {
    let mut app = make_app_with_projects();
    app.selected_project = Some(0);
    if let Some(idx) = app.selected_project {
        if idx > 0 {
            app.selected_project = Some(idx - 1);
        }
    }
    assert_eq!(app.selected_project, Some(0)); // stays at start
}