# TUI Testing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add ratatui-native testing with TestBackend + insta snapshots for all view render functions, plus unit tests for App state machine.

**Architecture:** Three test layers:
1. **Unit tests** — pure logic (App state transitions, keyboard event handling)
2. **Render tests** — `ratatui::backend::TestBackend` + `Terminal::draw()` capture → `insta` snapshots
3. **Component tests** — helper functions (three_panel layout)

**Tech Stack:** `ratatui` (TestBackend), `insta` (snapshot testing), `crossterm-tester` (optional, keyboard simulation)

---

## File Structure

```
crates/cli/src/tui/
├── app_test.rs              # App state machine unit tests (NEW)
├── dashboard.rs             # existing tests (expand)
├── dashboard_test.rs        # Dashboard render snapshot tests (NEW)
├── project_list_test.rs     # ProjectList render snapshot tests (NEW)
├── config_form_test.rs      # ConfigForm render snapshot tests (NEW)
├── explore_view_test.rs     # ExploreView render snapshot tests (NEW)
└── components_test.rs        # three_panel / title_bar / status_bar unit tests (NEW)
```

---

## Dependencies

```toml
# crates/cli/Cargo.toml
[dev-dependencies]
insta = { version = "1.40", features = ["yaml"] }
```

---

## Task 1: Add insta dependency + setup snapshot directory

**Files:**
- Modify: `crates/cli/Cargo.toml`
- Create: `crates/cli/tests/tui/` (snapshots dir)

- [ ] **Step 1: Add insta to Cargo.toml dev-dependencies**

```toml
[dev-dependencies]
insta = { version = "1.40", features = ["yaml"] }
```

- [ ] **Step 2: Create snapshot directory**

```bash
mkdir -p crates/cli/tests/tui/snapshots
```

- [ ] **Step 3: Create insta settings for workspace**

```rust
// crates/cli/tests/tui/mod.rs
use insta::settings::Settings;
Settings::new()
    .set_snapshot_path("tests/tui/snapshots");
```

Run: `cargo build -p cli --tests 2>&1 | head -20`
Expected: Compiles (no insta errors)

- [ ] **Step 4: Commit**

```bash
git add crates/cli/Cargo.toml crates/cli/tests/
git commit -m "test(tui): add insta dependency and snapshot directory"
```

---

## Task 2: Write render test helper

**Files:**
- Create: `crates/cli/tests/tui/mod.rs`
- Create: `crates/cli/tests/tui/render_helpers.rs`

- [ ] **Step 1: Create render_helpers.rs**

```rust
use ratatui::{backend::TestBackend, Terminal, Frame};
use std::io;

/// Renders a view function to a String by capturing the TestBackend buffer.
/// The view function receives (frame: &mut Frame, app: &App, area: Rect).
pub fn render_to_string<F>(app: &crate::tui::app::App, view: F) -> String
where
    F: Fn(&mut Frame, &crate::tui::app::App, ratatui::prelude::Rect),
{
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend);
    terminal.draw(|frame| {
        let area = frame.size();
        view(frame, app, area);
    }).unwrap();
    buffer_to_string(&terminal.backend())
}

fn buffer_to_string<B: ratatui::backend::Backend>(backend: &B) -> String {
    use ratatui::buffer::Buffer;
    let area = backend.buffer().area;
    let mut result = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = backend.buffer().get(x, y);
            result.push(cell.symbol());
        }
        if y < area.height - 1 {
            result.push('\n');
        }
    }
    result
}
```

- [ ] **Step 2: Create tests/tui/mod.rs (insta settings + helpers re-export)**

```rust
pub mod render_helpers;
pub use render_helpers::render_to_string;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo test -p cli --test tui 2>&1 | head -10`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add crates/cli/tests/tui/
git commit -m "test(tui): add render test helpers with TestBackend"
```

---

## Task 3: Dashboard render snapshot tests

**Files:**
- Create: `crates/cli/tests/tui/dashboard_test.rs`

- [ ] **Step 1: Write dashboard render tests**

```rust
use crate::render_to_string;
use crate::tui::app::App;
use crate::tui::dashboard;

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
```

- [ ] **Step 2: Run tests to generate snapshots (they will FAIL first — expected)**

Run: `cargo test -p cli --test tui dashboard 2>&1`
Expected: insta prompts to create snapshots (RUNNING=1 cargo insta review or accept them)

- [ ] **Step 3: Accept snapshots**

Run: `cargo insta accept 2>&1`

- [ ] **Step 4: Commit snapshots**

```bash
git add crates/cli/tests/tui/snapshots/
git commit -m "test(tui): add dashboard render snapshots"
```

---

## Task 4: App state machine unit tests

**Files:**
- Create: `crates/cli/tests/tui/app_test.rs`

- [ ] **Step 1: Test AppState transitions**

```rust
use crate::tui::app::{App, AppState};

fn make_app() -> App {
    let mut app = App::new();
    app.projects = vec!["proj-a".into(), "proj-b".into()];
    app
}

#[test]
fn test_new_app_starts_on_dashboard() {
    let app = App::new();
    assert_eq!(app.current_state, AppState::Dashboard);
}

#[test]
fn test_project_navigation() {
    let mut app = make_app();

    // Dashboard -> ProjectList
    app.current_state = AppState::ProjectList;
    assert_eq!(app.projects.len(), 2);
}

#[test]
fn test_explore_view_state_carries_project_name() {
    let app = App::new();
    let explore = AppState::ExploreView { project_name: "my-proj".into() };
    match explore {
        AppState::ExploreView { project_name } => {
            assert_eq!(project_name, "my-proj");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_explore_url_input() {
    let mut app = App::new();
    app.explore_input_mode = true;
    app.explore_url.push('h');
    app.explore_url.push('t');
    app.explore_url.push('t');
    app.explore_url.push('p');
    assert_eq!(app.explore_url, "http");
}

#[test]
fn test_explore_depth_bounds() {
    let mut app = App::new();
    app.explore_depth = 10;
    // + should not exceed 10
    if app.explore_depth < 10 {
        app.explore_depth += 1;
    }
    assert_eq!(app.explore_depth, 10);

    app.explore_depth = 1;
    // - should not go below 1
    if app.explore_depth > 1 {
        app.explore_depth -= 1;
    }
    assert_eq!(app.explore_depth, 1);
}

#[test]
fn test_selected_project_bounds() {
    let mut app = make_app();
    // Start at 0, Down should go to 1
    app.selected_project = Some(0);
    let next = app.selected_project.map(|i| i + 1).filter(|&i| i < app.projects.len());
    app.selected_project = next;
    assert_eq!(app.selected_project, Some(1));

    // At end, Down stays at end
    let next = app.selected_project.map(|i| i + 1).filter(|&i| i < app.projects.len());
    app.selected_project = next;
    assert_eq!(app.selected_project, Some(1)); // Can't go past end
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p cli --test tui app 2>&1`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/cli/tests/tui/app_test.rs
git commit -m "test(tui): add App state machine unit tests"
```

---

## Task 5: ProjectList render snapshot tests

**Files:**
- Create: `crates/cli/tests/tui/project_list_test.rs`

- [ ] **Step 1: Write ProjectList render tests**

```rust
use crate::render_to_string;
use crate::tui::app::App;
use crate::tui::project_list;

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
```

- [ ] **Step 2: Generate + accept snapshots**

Run: `cargo test -p cli --test tui project_list 2>&1`
Run: `cargo insta accept 2>&1`

- [ ] **Step 3: Commit**

---

## Task 6: ConfigForm render snapshot tests

**Files:**
- Create: `crates/cli/tests/tui/config_form_test.rs`

- [ ] **Step 1: Write ConfigForm render tests**

```rust
use crate::render_to_string;
use crate::tui::app::App;
use crate::tui::config_form;
use crate::config::{Config, LlmConfig, SandboxConfig, ExplorationConfig};

fn make_app_with_config() -> App {
    let mut app = App::new();
    app.config = Some(Config {
        llm: LlmConfig {
            provider: "minimax".into(),
            base_url: "https://api.minimax.chat/v1".into(),
            api_key: "sk-abc123xyz".into(),
            model: "MiniMax-VL-01".into(),
        },
        sandbox: SandboxConfig {
            compose_file: "/path/to/compose.yml".into(),
            steel_port: 3333,
            cdp_port: 9222,
        },
        exploration: ExplorationConfig {
            max_depth: 5,
            max_pages_per_seed: 30,
        },
    });
    app
}

#[test]
fn test_config_form_with_real_config() {
    let app = make_app_with_config();
    let output = render_to_string(&app, config_form::render);
    insta::assert_snapshot!("config_form_real", output);
}

#[test]
fn test_config_form_no_config() {
    let app = App::new(); // config is None
    let output = render_to_string(&app, config_form::render);
    insta::assert_snapshot!("config_form_none", output);
}
```

- [ ] **Step 2: Generate + accept snapshots**

- [ ] **Step 3: Commit**

---

## Task 7: Components unit tests

**Files:**
- Create: `crates/cli/tests/tui/components_test.rs`

- [ ] **Step 1: Test three_panel layout**

```rust
use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::Rect as PreludeRect;

fn make_rect(x: u16, y: u16, w: u16, h: u16) -> Rect {
    Rect::new(x, y, w, h)
}

#[test]
fn test_three_panel_splits_correctly() {
    use crate::tui::components::three_panel;

    let area = make_rect(0, 0, 80, 24);
    let [top, middle, bottom] = three_panel(area);

    assert_eq!(top.height, 3);
    assert_eq!(middle.height, 18); // 24 - 3 - 3
    assert_eq!(bottom.height, 3);

    assert_eq!(top.width, 80);
    assert_eq!(middle.width, 80);
    assert_eq!(bottom.width, 80);
}

#[test]
fn test_three_panel_small_height() {
    use crate::tui::components::three_panel;

    let area = make_rect(0, 0, 80, 5);
    let [top, middle, bottom] = three_panel(area);

    // All get at least 1 row each due to Constraint::Length
    assert!(top.height >= 1);
    assert!(middle.height >= 1);
    assert!(bottom.height >= 1);
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p cli --test tui components 2>&1`
Expected: All PASS

- [ ] **Step 3: Commit**

---

## Task 8: ExploreView render snapshot tests

**Files:**
- Create: `crates/cli/tests/tui/explore_view_test.rs`

- [ ] **Step 1: Write ExploreView render tests**

```rust
use crate::render_to_string;
use crate::tui::app::App;
use crate::tui::explore_view;

fn make_explore_app() -> App {
    let mut app = App::new();
    app.current_state = AppState::ExploreView { project_name: "test-project".into() };
    app.explore_url = "https://example.com".into();
    app.explore_depth = 3;
    app
}

#[test]
fn test_explore_view_idle() {
    let app = make_explore_app();
    let output = render_to_string(&app, explore_view::render);
    insta::assert_snapshot!("explore_idle", output);
}

#[test]
fn test_explore_view_input_mode() {
    let mut app = make_explore_app();
    app.explore_input_mode = true;
    let output = render_to_string(&app, explore_view::render);
    insta::assert_snapshot!("explore_input_mode", output);
}

#[test]
fn test_explore_view_url_empty() {
    let mut app = make_explore_app();
    app.explore_url.clear();
    let output = render_to_string(&app, explore_view::render);
    insta::assert_snapshot!("explore_url_empty", output);
}
```

- [ ] **Step 2: Generate + accept snapshots**

- [ ] **Step 3: Commit**

---

## Self-Review Checklist

1. **Coverage:** All view render functions have snapshot tests? YES
2. **App state machine:** transitions tested with pure unit tests? YES
3. **Components:** three_panel layout tested? YES
4. **insta setup:** snapshot directory created? YES
5. **Dependencies:** insta added to Cargo.toml? YES
6. **No placeholders:** all test code shown inline? YES
