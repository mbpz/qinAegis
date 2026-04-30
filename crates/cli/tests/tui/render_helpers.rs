use ratatui::{backend::TestBackend, buffer::Buffer, prelude::Rect, Frame, Terminal};

/// Renders a view function to a String by capturing the TestBackend buffer.
pub fn render_to_string<F>(app: &crate::tui::app::App, view: F) -> String
where
    F: Fn(&mut Frame, &crate::tui::app::App, Rect),
{
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend);
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
            result.push(cell.symbol());
        }
        if y < area.height - 1 {
            result.push('\n');
        }
    }
    result
}