use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: ThemeConfig,
    pub badges: BadgeConfig,
    pub layout: LayoutConfig,
    pub behavior: BehaviorConfig,
    pub animation: AnimationConfig,
    pub debug: DebugConfig,
}

impl Config {
    pub fn from_map(values: &BTreeMap<String, String>) -> Result<Self, ConfigError> {
        let mut config = Self::default();
        for (key, value) in values {
            match key.as_str() {
                "theme.use_color" => config.theme.use_color = parse_bool(key, value)?,
                "theme_color" => {
                    if let Ok(parsed) = parse_bool(key, value) {
                        config.theme.use_color = parsed;
                    }
                }
                "theme.active_fg" => config.theme.active_fg = optional_string(value),
                "theme.inactive_fg" => config.theme.inactive_fg = optional_string(value),
                "theme.idle_fg" => config.theme.idle_fg = optional_string(value),
                "theme.running_fg" => config.theme.running_fg = optional_string(value),
                "theme.complete_fg" => config.theme.complete_fg = optional_string(value),
                "theme.error_fg" => config.theme.error_fg = optional_string(value),
                "badges.idle" => config.badges.idle.clone_from(value),
                "badges.running" => config.badges.running.clone_from(value),
                "badges.complete" => config.badges.complete.clone_from(value),
                "badges.error" => config.badges.error.clone_from(value),
                "badge_idle" => set_nonempty(&mut config.badges.idle, value),
                "badge_running" => set_nonempty(&mut config.badges.running, value),
                "badge_complete" => set_nonempty(&mut config.badges.complete, value),
                "badge_error" => set_nonempty(&mut config.badges.error, value),
                "layout.separator" | "layout_separator" => {
                    config.layout.separator.clone_from(value)
                }
                "layout.truncation_marker" => config.layout.truncation_marker.clone_from(value),
                "layout.min_name_width" => {
                    config.layout.min_name_width = parse_usize(key, value)?;
                }
                "layout.max_name_width" => {
                    config.layout.max_name_width = parse_usize(key, value)?;
                }
                "layout_max_tab_width" => {
                    if let Ok(parsed) = parse_usize(key, value) {
                        if (8..=256).contains(&parsed) {
                            config.layout.max_name_width = parsed;
                        }
                    }
                }
                "layout.show_index" => config.layout.show_index = parse_bool(key, value)?,
                "layout_show_index" => {
                    if let Ok(parsed) = parse_bool(key, value) {
                        config.layout.show_index = parsed;
                    }
                }
                "behavior.automatic_naming" => {
                    config.behavior.automatic_naming = parse_bool(key, value)?;
                }
                "behavior_auto_name" => {
                    if let Ok(parsed) = parse_bool(key, value) {
                        config.behavior.automatic_naming = parsed;
                    }
                }
                "behavior.disambiguate_duplicates" => {
                    config.behavior.disambiguate_duplicates = parse_bool(key, value)?;
                }
                "behavior.preserve_manual_names" => {
                    config.behavior.preserve_manual_names = parse_bool(key, value)?;
                }
                "animation.enabled" => config.animation.enabled = parse_bool(key, value)?,
                "animation.interval_ms" => {
                    config.animation.interval_ms = parse_u64(key, value)?;
                }
                "debug.enabled" => config.debug.enabled = parse_bool(key, value)?,
                "debug" => {
                    if let Ok(parsed) = parse_bool(key, value) {
                        config.debug.enabled = parsed;
                    }
                }
                _ => {}
            }
        }
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.layout.min_name_width > self.layout.max_name_width {
            return Err(ConfigError::InvalidNameWidths {
                minimum: self.layout.min_name_width,
                maximum: self.layout.max_name_width,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub use_color: bool,
    pub active_fg: Option<String>,
    pub inactive_fg: Option<String>,
    pub idle_fg: Option<String>,
    pub running_fg: Option<String>,
    pub complete_fg: Option<String>,
    pub error_fg: Option<String>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            use_color: true,
            active_fg: None,
            inactive_fg: None,
            idle_fg: None,
            running_fg: None,
            complete_fg: None,
            error_fg: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BadgeConfig {
    pub idle: String,
    pub running: String,
    pub complete: String,
    pub error: String,
}

impl Default for BadgeConfig {
    fn default() -> Self {
        Self {
            idle: "💤".to_owned(),
            running: "🚀".to_owned(),
            complete: "✅".to_owned(),
            error: "❌".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    pub separator: String,
    pub truncation_marker: String,
    pub min_name_width: usize,
    pub max_name_width: usize,
    pub show_index: bool,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            separator: "   ".to_owned(),
            truncation_marker: "…".to_owned(),
            min_name_width: 1,
            max_name_width: 32,
            show_index: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    pub automatic_naming: bool,
    pub disambiguate_duplicates: bool,
    pub preserve_manual_names: bool,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            automatic_naming: true,
            disambiguate_duplicates: true,
            preserve_manual_names: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnimationConfig {
    pub enabled: bool,
    pub interval_ms: u64,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_ms: 250,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DebugConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("configuration key `{key}` expects a boolean, got `{value}`")]
    InvalidBoolean { key: String, value: String },
    #[error("configuration key `{key}` expects a non-negative integer, got `{value}`")]
    InvalidInteger { key: String, value: String },
    #[error("minimum name width {minimum} exceeds maximum name width {maximum}")]
    InvalidNameWidths { minimum: usize, maximum: usize },
}

fn parse_bool(key: &str, value: &str) -> Result<bool, ConfigError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(ConfigError::InvalidBoolean {
            key: key.to_owned(),
            value: value.to_owned(),
        }),
    }
}

fn parse_usize(key: &str, value: &str) -> Result<usize, ConfigError> {
    value
        .trim()
        .parse::<usize>()
        .map_err(|_| ConfigError::InvalidInteger {
            key: key.to_owned(),
            value: value.to_owned(),
        })
}

fn parse_u64(key: &str, value: &str) -> Result<u64, ConfigError> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| ConfigError::InvalidInteger {
            key: key.to_owned(),
            value: value.to_owned(),
        })
}

fn optional_string(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

fn set_nonempty(target: &mut String, value: &str) {
    if !value.trim().is_empty() {
        target.clear();
        target.push_str(value);
    }
}
