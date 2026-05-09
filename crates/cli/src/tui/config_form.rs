// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use ratatui::{
    Frame, prelude::Rect,
    widgets::{Block, Borders, Paragraph},
};
use crate::tui::app::App;
use crate::tui::components;

#[derive(Debug, Clone, Default)]
pub struct ConfigFormState {
    pub editing_field: Option<Field>,
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub input_buffer: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Provider,
    BaseUrl,
    ApiKey,
    Model,
}

impl Default for Field {
    fn default() -> Self {
        Field::Provider
    }
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let [top, middle, bottom] = components::three_panel(area);
    components::title_bar(frame, top, "LLM Configuration");

    let form_state = app.config_form_state();

    let content = if app.config.is_none() || form_state.editing_field.is_some() {
        // Edit mode or first time setup
        let cursor_marker = if form_state.editing_field.is_some() { "█" } else { "" };
        let current_field = form_state.editing_field.clone().unwrap_or(Field::Provider);

        let provider_line = format!(
            "  [{}] Provider:  {} {}",
            if current_field == Field::Provider { "●" } else { "○" },
            form_state.provider,
            if current_field == Field::Provider { cursor_marker } else { "" }
        );
        let base_url_line = format!(
            "  [{}] Base URL:  {} {}",
            if current_field == Field::BaseUrl { "●" } else { "○" },
            form_state.base_url,
            if current_field == Field::BaseUrl { cursor_marker } else { "" }
        );
        let api_key_line = format!(
            "  [{}] API Key:   {} {}",
            if current_field == Field::ApiKey { "●" } else { "○" },
            if form_state.api_key.is_empty() {
                "(not set)".to_string()
            } else {
                "****".to_string()
            },
            if current_field == Field::ApiKey { cursor_marker } else { "" }
        );
        let model_line = format!(
            "  [{}] Model:     {} {}",
            if current_field == Field::Model { "●" } else { "○" },
            form_state.model,
            if current_field == Field::Model { cursor_marker } else { "" }
        );

        let input_hint = if form_state.editing_field.is_some() {
            format!("\n\nInput: {}{}", form_state.input_buffer, cursor_marker)
        } else {
            String::new()
        };

        let message = form_state.message.clone().unwrap_or_default();

        Paragraph::new(format!(
            "Configure AI Model\n\n{}\n{}\n{}\n{}\n{}\n\n{}\n[↑↓] Select  [Enter] Edit  [s] Save  [Esc] Cancel",
            provider_line, base_url_line, api_key_line, model_line, input_hint, message
        )).block(Block::default().title("LLM Settings").borders(Borders::ALL))
    } else {
        // Display mode (config exists)
        let cfg = app.config.as_ref().unwrap();
        let api_key_display = if cfg.llm.api_key.is_empty() {
            "(not set)".to_string()
        } else {
            format!("{}****", &cfg.llm.api_key[..cfg.llm.api_key.len().min(4)])
        };

        Paragraph::new(format!(
            "Current Configuration\n\n\
             Provider:  {}\n\
             Base URL:  {}\n\
             API Key:   {}\n\
             Model:     {}\n\n\
             [↑↓] Select  [Enter] Edit  [s] Save  [Esc] Back",
            cfg.llm.provider,
            cfg.llm.base_url,
            api_key_display,
            cfg.llm.model
        )).block(Block::default().title("LLM Settings").borders(Borders::ALL))
    };

    frame.render_widget(content, middle);
    components::status_bar(frame, bottom, "[↑↓] Navigate  [Enter] Select/Edit  [s] Save  [Esc] Cancel");
}