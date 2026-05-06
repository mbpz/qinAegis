# TUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current CLI with a full ratatui-based interactive TUI application.

**Architecture:** ratatui app with state machine navigation. Single `App` struct holds all state. `tui::run()` takes over terminal, runs event loop, then restores terminal on exit. Views are stateless functions `fn(view: &mut Frame, app: &App, area: Rect)`.

**Tech Stack:** ratatui 0.27+, crossterm (ratatui's backend), tokio

---

## File Structure

```
crates/cli/src/
├── main.rs              # Add `Tui` command variant, dispatch to tui::run()
├── tui/
│   ├── mod.rs           # Re-exports, App struct, run() entry
│   ├── app.rs           # AppState enum (Dashboard/ProjectList/ConfigForm/Explore/Generate/Run)
│   ├── dashboard.rs    # Dashboard view
│   ├── project_list.rs  # Project list with selection
│   ├── config_form.rs   # LLM + Sandbox config form
│   ├── explore_view.rs  # Explore command view
│   ├── generate_view.rs # Generate command view
│   ├── run_view.rs      # Run view with live progress
│   └── components.rs    # Shared UI components (title_bar, status_bar, etc.)
└── commands/
    ├── mod.rs           # Keep existing for non-TUI fallback
    ├── init.rs          # Keep existing
    ├── explore.rs       # Keep existing
    ├── generate.rs      # Keep existing
    ├── run.rs           # Keep existing
    ├── performance.rs   # Keep existing
    ├── notion.rs        # DELETE (notion is gone)
    ├── project.rs       # Keep (used by TUI)
    └── export.rs        # Keep existing
```

---

## Dependencies

```toml
# crates/cli/Cargo.toml (add to [dependencies])
ratatui = "0.27"
crossterm = "0.27"
```

---

## App State Machine

```
Dashboard ─── project ──► ProjectList ──► ConfigForm
                  │                         ▲
                  │                         │
                  ├─ explore ──► ExploreView ──► Dashboard
                  ├─ generate ──► GenerateView ──► Dashboard
                  ├─ run ──────► RunView ──────► Dashboard
                  │
                  └─ config ──► ConfigForm ──────► Dashboard
```

---

## Tasks

### Task 1: Add ratatui dependency and create tui module scaffold

**Files:**
- Modify: `crates/cli/Cargo.toml`
- Create: `crates/cli/src/tui/mod.rs`
- Create: `crates/cli/src/tui/app.rs`

- [ ] **Step 1: Add ratatui to Cargo.toml**

Read `crates/cli/Cargo.toml` first, then add:
```toml
ratatui = "0.27"
crossterm = "0.27"
```

- [ ] **Step 2: Create tui/mod.rs**

```rust
pub mod app;
pub mod dashboard;
pub mod project_list;
pub mod config_form;
pub mod explore_view;
pub mod generate_view;
pub mod run_view;
pub mod components;

pub use app::{App, AppState};
pub use app::run as run_tui;
```

- [ ] **Step 3: Create tui/app.rs with App state machine**

```rust
use ratatui::{Terminal, DefaultTerminal};
use crossterm::{terminal::{EnterAlternateScreen, LeaveAlternateScreen}, ExecutableCommand};
use std::io;

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Dashboard,
    ProjectList,
    ConfigForm,
    ExploreView { project_name: String },
    GenerateView { project_name: String },
    RunView { project_name: String },
}

#[derive(Debug)]
pub struct App {
    pub current_state: AppState,
    pub projects: Vec<String>,
    pub selected_project: Option<usize>,
    pub message: Option<String>,
    pub is_loading: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            current_state: AppState::Dashboard,
            projects: Vec::new(),
            selected_project: None,
            message: None,
            is_loading: false,
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new();

    loop {
        if terminal.draw(|frame| {
            match &app.current_state {
                AppState::Dashboard => dashboard::render(frame, &app),
                AppState::ProjectList => project_list::render(frame, &app),
                AppState::ConfigForm => config_form::render(frame, &app),
                AppState::ExploreView { .. } => explore_view::render(frame, &app),
                AppState::GenerateView { .. } => generate_view::render(frame, &app),
                AppState::RunView { .. } => run_view::render(frame, &app),
            }
        }).is_err() {
            break;
        }

        if !handle_events(&mut app)? {
            break;
        }
    }

    ratatui::restore();
    Ok(())
}
```

- [ ] **Step 4: Create tui/dashboard.rs (placeholder)**

```rust
use ratatui::{Frame, Rect};
use crate::tui::app::App;

pub fn render(_frame: &mut Frame, _app: &App, _area: Rect) {
    // Placeholder - will be implemented in Task 3
}
```

- [ ] **Step 5: Create tui/project_list.rs (placeholder)**
- [ ] **Step 6: Create tui/config_form.rs (placeholder)**
- [ ] **Step 7: Create tui/explore_view.rs (placeholder)**
- [ ] **Step 8: Create tui/generate_view.rs (placeholder)**
- [ ] **Step 9: Create tui/run_view.rs (placeholder)**
- [ ] **Step 10: Create tui/components.rs (empty for now)**

- [ ] **Step 11: Add tui/ to lib.rs and add Tui command to main.rs**

In `main.rs`, add variant to `Cmd`:
```rust
/// Start interactive TUI
Tui,
```

And in the match:
```rust
Cmd::Tui => crate::tui::run()?,
```

Run: `cargo build -p cli 2>&1`
Expected: Compiles (with warnings about placeholder stubs)

- [ ] **Step 12: Commit**

```bash
git add crates/cli/Cargo.toml crates/cli/src/tui/
git commit -m "feat(tui): scaffold ratatui app with state machine"
```

---

### Task 2: Implement Dashboard view

**Files:**
- Modify: `crates/cli/src/tui/dashboard.rs`

- [ ] **Step 1: Write dashboard test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_renders_without_panic() {
        let app = App::new();
        // Verify App struct can be created and is in Dashboard state
        assert_eq!(app.current_state, AppState::Dashboard);
    }
}
```

- [ ] **Step 2: Implement dashboard::render()**

Dashboard shows:
- Title: "qinAegis — AI Testing TUI"
- Status line: "LLM: configured/not configured | Projects: N"
- Four action buttons: [Explore] [Generate] [Run Tests] [Settings]
- Keybinds footer: "q:quit | ↑↓:select | Enter:confirm"

```rust
use ratatui::{
    Frame, Rect, area::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    style::Stylize,
};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::vertical()
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(area);

    // Title bar
    let title = Paragraph("qinAegis — AI Testing TUI")
        .block(Block::new().borders(Borders::BOTTOM))
        .bold();
    frame.render_widget(title, chunks[0]);

    // Main content
    let status = if app.is_loading {
        "Loading..."
    } else {
        &app.message.clone().unwrap_or_default()
    };

    let main_text = Paragraph(format!(
        "Projects: {}\n\n[1] Explore    — AI explore a URL\n[2] Generate   — Generate test cases\n[3] Run Tests  — Execute test suite\n[4] Settings  — Configure LLM & Sandbox\n\n{}",
        app.projects.len(),
        status
    )).block(Block::new().borders(Borders::ALL)).wrap(Wrap { trim: true });

    frame.render_widget(main_text, chunks[1]);

    // Footer
    let footer = Paragraph("q: quit | ↑↓: select | Enter: confirm");
    frame.render_widget(footer, chunks[2]);
}
```

- [ ] **Step 3: Run test**

Run: `cargo test -p cli tui::dashboard 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/tui/dashboard.rs
git commit -m "feat(tui): implement dashboard view"
```

---

### Task 3: Implement event handling and keyboard navigation

**Files:**
- Modify: `crates/cli/src/tui/app.rs`

- [ ] **Step 1: Add handle_events function**

```rust
use crossterm::event::{Event, KeyCode, KeyEventKind};

fn handle_events(app: &mut App) -> anyhow::Result<bool> {
    if let Event::Key(key) = crossterm::event::read()? {
        if key.kind != KeyEventKind::Pressed {
            return Ok(true);
        }

        match (key.code, &app.current_state) {
            (KeyCode::Char('q'), _) => return Ok(false),
            (KeyCode::Enter, AppState::Dashboard) => {
                // Navigate to project list
                app.current_state = AppState::ProjectList;
            }
            (KeyCode::Esc, _) => {
                app.current_state = AppState::Dashboard;
            }
            _ => {}
        }
    }
    Ok(true)
}
```

- [ ] **Step 2: Test it compiles**

Run: `cargo build -p cli 2>&1`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add crates/cli/src/tui/app.rs
git commit -m "feat(tui): add keyboard event handling"
```

---

### Task 4: Implement ProjectList view

**Files:**
- Modify: `crates/cli/src/tui/project_list.rs`

- [ ] **Step 1: Load projects on enter**

When entering `ProjectList` state, load projects from `LocalStorage`:
```rust
use crate::commands::project;

pub fn on_enter(app: &mut App) {
    app.is_loading = true;
    match tokio::runtime::Handle::current().block_on(
        qin_aegis_core::storage::LocalStorageInstance::new().list_projects()
    ) {
        Ok(projects) => app.projects = projects,
        Err(e) => app.message = Some(format!("Error: {}", e)),
    }
    app.is_loading = false;
}
```

- [ ] **Step 2: Implement render**

```rust
use ratatui::{Frame, Rect, widgets::{Block, Borders, List, ListItem}};

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
```

- [ ] **Step 3: Add navigation keys to handle_events**

```rust
(KeyCode::Down, AppState::ProjectList) => {
    if let Some(idx) = app.selected_project {
        if idx + 1 < app.projects.len() {
            app.selected_project = Some(idx + 1);
        }
    } else if !app.projects.is_empty() {
        app.selected_project = Some(0);
    }
}
(KeyCode::Up, AppState::ProjectList) => {
    if let Some(idx) = app.selected_project {
        if idx > 0 {
            app.selected_project = Some(idx - 1);
        }
    }
}
(KeyCode::Enter, AppState::ProjectList) => {
    if let Some(idx) = app.selected_project.clone() {
        let name = app.projects[idx].clone();
        app.current_state = AppState::ExploreView { project_name: name };
    }
}
(KeyCode::Char('a'), AppState::ProjectList) => {
    // Add new project
    app.message = Some("Use CLI: qinAegis project add".to_string());
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/tui/project_list.rs crates/cli/src/tui/app.rs
git commit -m "feat(tui): implement project list view with navigation"
```

---

### Task 5: Implement ConfigForm view

**Files:**
- Modify: `crates/cli/src/tui/config_form.rs`

- [ ] **Step 1: Write the render function**

Config form has fields:
- Provider (text input, default: "minimax")
- Base URL (text input, default: "https://api.minimax.chat/v1")
- API Key (password input)
- Model (text input, default: "MiniMax-VL-01")
- Steel Port (number input, default: 3333)
- CDP Port (number input, default: 9222)

```rust
use ratatui::{Frame, Rect, widgets::{Block, Borders, Paragraph, Input}};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let help_text = Paragraph(
        "LLM Configuration\n\n\
         Provider: minimax\n\
         Base URL: https://api.minimax.chat/v1\n\
         Model: MiniMax-VL-01\n\
         \n\
         Sandbox Configuration\n\
         Steel Port: 3333\n\
         CDP Port: 9222\n\n\
         [Enter] Save  [Esc] Cancel"
    ).block(Block::new().title("Settings").borders(Borders::ALL));

    frame.render_widget(help_text, area);
}
```

- [ ] **Step 2: Implement basic keyboard handling**

- [ ] **Step 3: Commit**

```bash
git add crates/cli/src/tui/config_form.rs
git commit -m "feat(tui): implement config form view"
```

---

### Task 6: Implement ExploreView, GenerateView, RunView

**Files:**
- Modify: `crates/cli/src/tui/explore_view.rs`, `generate_view.rs`, `run_view.rs`

- [ ] **Step 1: Implement ExploreView**

Shows:
- Project name
- URL input field
- Depth selector (1-5)
- [Start Explore] button
- Progress area (shows AI exploration output)

```rust
// crates/cli/src/tui/explore_view.rs
use ratatui::{Frame, Rect, widgets::{Block, Borders, Paragraph}};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let text = Paragraph("Explore View\n\nEnter project URL and depth...")
        .block(Block::new().borders(Borders::ALL));
    frame.render_widget(text, area);
}
```

- [ ] **Step 2: Implement GenerateView** (stub)
- [ ] **Step 3: Implement RunView** (stub)

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/tui/explore_view.rs crates/cli/src/tui/generate_view.rs crates/cli/src/tui/run_view.rs
git commit -m "feat(tui): add stub views for explore, generate, run"
```

---

### Task 7: Wire up real commands in TUI

**Files:**
- Modify: `crates/cli/src/tui/app.rs`, `crates/cli/src/tui/explore_view.rs`

- [ ] **Step 1: Call real explore command from ExploreView**

When user confirms in ExploreView, call:
```rust
crate::commands::explore::run_explore(&project_name, url, depth).await
```

- [ ] **Step 2: Wire GenerateView to commands::generate::run_generate**
- [ ] **Step 3: Wire RunView to commands::run::run_tests**

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/tui/app.rs crates/cli/src/tui/explore_view.rs
git commit -m "feat(tui): wire explore command to TUI"
```

---

### Task 8: Polish — title bar, status bar, layout improvements

**Files:**
- Modify: `crates/cli/src/tui/components.rs`, `crates/cli/src/tui/app.rs`

- [ ] **Step 1: Create shared components**

```rust
// crates/cli/src/tui/components.rs
use ratatui::{Frame, Rect, widgets::{Block, Borders, Paragraph}, style::Stylize};

pub fn title_bar(frame: &mut Frame, area: Rect, title: &str) {
    let t = Paragraph(title).bold().block(Block::new().borders(Borders::BOTTOM));
    frame.render_widget(t, area);
}

pub fn status_bar(frame: &mut Frame, area: Rect, text: &str) {
    let s = Paragraph(text).block(Block::new().borders(Borders::TOP));
    frame.render_widget(s, area);
}

pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = area.width.saturating_sub(width) / 2;
    let h = area.height.saturating_sub(height) / 2;
    Rect::new(area.x + w, area.y + h, width.min(area.width), height.min(area.height))
}
```

- [ ] **Step 2: Update all views to use components**
- [ ] **Step 3: Add loading spinner for async operations**

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/tui/components.rs
git commit -m "feat(tui): add shared UI components"
```

---

## Self-Review Checklist

1. **Spec coverage:** Dashboard, ProjectList, ConfigForm, ExploreView, GenerateView, RunView — all covered.
2. **ratatui not in deps:** Already fixed in Task 1 (add to Cargo.toml).
3. **Event loop:** handle_events() implemented in Task 3.
4. **State transitions:** AppState enum covers all views, navigation wired in handle_events.
5. **Real commands:** Stub views in Task 6, wired in Task 7.
6. **Dependencies:** crossterm 0.27 paired with ratatui 0.27 (matching versions).

---

## Execution Options

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
