use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{AgentStatus, Config, State, TabId};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderModel {
    pub tabs: Vec<RenderTab>,
}

impl RenderModel {
    #[must_use]
    pub fn from_state(state: &State, active_tab: Option<TabId>) -> Self {
        let names = crate::resolve_tab_names(state);
        let tabs = names
            .into_iter()
            .enumerate()
            .map(|(index, resolved)| RenderTab {
                tab_id: resolved.tab_id(),
                position: index.saturating_add(1),
                name: resolved.name().to_owned(),
                status: state.aggregate_status_for_tab(resolved.tab_id()),
                is_active: active_tab == Some(resolved.tab_id()),
            })
            .collect();
        Self { tabs }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderTab {
    pub tab_id: TabId,
    pub position: usize,
    pub name: String,
    pub status: AgentStatus,
    pub is_active: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderOutput {
    pub text: String,
    pub visible_tabs: usize,
    pub truncated: bool,
}

#[must_use]
pub fn truncate_to_width(input: &str, max_width: usize, marker: &str) -> String {
    if UnicodeWidthStr::width(input) <= max_width {
        return input.to_owned();
    }
    if max_width == 0 {
        return String::new();
    }

    let fitted_marker = fit_without_marker(marker, max_width);
    let marker_width = UnicodeWidthStr::width(fitted_marker.as_str());
    let content_width = max_width.saturating_sub(marker_width);
    let mut result = fit_without_marker(input, content_width);
    result.push_str(&fitted_marker);
    result
}

#[must_use]
pub fn sanitize_terminal_text(input: &str) -> String {
    input
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

#[must_use]
pub fn render_tab_bar(
    model: &RenderModel,
    config: &Config,
    available_width: usize,
) -> RenderOutput {
    if model.tabs.is_empty() || available_width == 0 {
        return RenderOutput::default();
    }

    let separator = sanitize_terminal_text(&config.layout.separator);
    let marker = sanitize_terminal_text(&config.layout.truncation_marker);
    let separator_width = UnicodeWidthStr::width(separator.as_str());
    let separators_width = separator_width.saturating_mul(model.tabs.len().saturating_sub(1));
    let fixed_width = model
        .tabs
        .iter()
        .map(|tab| entry_fixed_width(tab, config))
        .fold(separators_width, usize::saturating_add);
    let name_budget = available_width.saturating_sub(fixed_width);
    let shared_name_width = if model.tabs.is_empty() {
        0
    } else {
        name_budget / model.tabs.len()
    }
    .min(config.layout.max_name_width)
    .max(config.layout.min_name_width.min(name_budget));

    let entries: Vec<String> = model
        .tabs
        .iter()
        .map(|tab| render_entry(tab, config, shared_name_width, &marker))
        .collect();
    let complete = entries.join(&separator);
    let complete_width = UnicodeWidthStr::width(complete.as_str());
    let name_was_truncated = model.tabs.iter().any(|tab| {
        UnicodeWidthStr::width(sanitize_terminal_text(&tab.name).as_str()) > shared_name_width
    });
    let text = truncate_to_width(&complete, available_width, &marker);
    RenderOutput {
        text,
        visible_tabs: model.tabs.len(),
        truncated: name_was_truncated || complete_width > available_width,
    }
}

fn entry_fixed_width(tab: &RenderTab, config: &Config) -> usize {
    let index_width = if config.layout.show_index {
        UnicodeWidthStr::width(tab.position.to_string().as_str()).saturating_add(1)
    } else {
        0
    };
    let badge = sanitize_terminal_text(badge(tab.status, config));
    index_width
        .saturating_add(UnicodeWidthStr::width(badge.as_str()))
        .saturating_add(1)
}

fn render_entry(tab: &RenderTab, config: &Config, name_width: usize, marker: &str) -> String {
    let sanitized_name = sanitize_terminal_text(&tab.name);
    let name = truncate_to_width(&sanitized_name, name_width, marker);
    let badge = sanitize_terminal_text(badge(tab.status, config));
    let index = if config.layout.show_index {
        format!("{} ", tab.position)
    } else {
        String::new()
    };
    format!("{index}{badge} {name}")
}

fn badge(status: AgentStatus, config: &Config) -> &str {
    match status {
        AgentStatus::Idle => &config.badges.idle,
        AgentStatus::Running => &config.badges.running,
        AgentStatus::Complete => &config.badges.complete,
        AgentStatus::Error => &config.badges.error,
    }
}

fn fit_without_marker(input: &str, max_width: usize) -> String {
    let mut result = String::new();
    let mut width = 0_usize;
    for grapheme in UnicodeSegmentation::graphemes(input, true) {
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if width.saturating_add(grapheme_width) > max_width {
            break;
        }
        result.push_str(grapheme);
        width = width.saturating_add(grapheme_width);
    }
    result
}
