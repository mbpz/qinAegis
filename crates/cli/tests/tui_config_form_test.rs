//! ConfigForm render snapshot tests

use ratatui::{backend::TestBackend, buffer::Buffer, prelude::Rect, Frame, Terminal};
use qinAegis_lib::tui::app::App;
use qinAegis_lib::tui::config_form;
use qinAegis_lib::config::{Config, LlmConfig, SandboxConfig, ExplorationConfig};

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
fn test_config_form_with_real_config() {
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
    let output = render_to_string(&app, config_form::render);
    insta::assert_snapshot!("config_form_real", output);
}

#[test]
fn test_config_form_no_config() {
    let app = App::new(); // config is None
    let output = render_to_string(&app, config_form::render);
    insta::assert_snapshot!("config_form_none", output);
}