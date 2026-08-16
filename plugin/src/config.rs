use std::collections::BTreeMap;

const DEFAULT_MAX_TAB_WIDTH: usize = 32;
const MIN_MAX_TAB_WIDTH: usize = 8;
const MAX_MAX_TAB_WIDTH: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PluginConfig {
    pub(crate) badge_idle: String,
    pub(crate) badge_running: String,
    pub(crate) badge_complete: String,
    pub(crate) badge_error: String,
    pub(crate) separator: String,
    pub(crate) max_tab_width: usize,
    pub(crate) show_index: bool,
    pub(crate) auto_name: bool,
    pub(crate) color: bool,
    pub(crate) debug: bool,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            badge_idle: "💤".to_owned(),
            badge_running: "🚀".to_owned(),
            badge_complete: "✅".to_owned(),
            badge_error: "❌".to_owned(),
            separator: "   ".to_owned(),
            max_tab_width: DEFAULT_MAX_TAB_WIDTH,
            show_index: true,
            auto_name: true,
            color: true,
            debug: false,
        }
    }
}

impl PluginConfig {
    pub(crate) fn from_map(values: &BTreeMap<String, String>) -> Self {
        let defaults = Self::default();
        Self {
            badge_idle: nonempty(values, &["badge_idle", "badges.idle"], &defaults.badge_idle),
            badge_running: nonempty(
                values,
                &["badge_running", "badges.running"],
                &defaults.badge_running,
            ),
            badge_complete: nonempty(
                values,
                &["badge_complete", "badges.complete"],
                &defaults.badge_complete,
            ),
            badge_error: nonempty(
                values,
                &["badge_error", "badges.error"],
                &defaults.badge_error,
            ),
            separator: first(values, &["layout_separator", "layout.separator"])
                .map(|value| clean_text(value, 64))
                .unwrap_or(defaults.separator),
            max_tab_width: bounded_usize(
                values,
                &["layout_max_tab_width", "layout.max_name_width"],
                defaults.max_tab_width,
                MIN_MAX_TAB_WIDTH,
                MAX_MAX_TAB_WIDTH,
            ),
            show_index: boolean(
                values,
                &["layout_show_index", "layout.show_index"],
                defaults.show_index,
            ),
            auto_name: boolean(
                values,
                &["behavior_auto_name", "behavior.automatic_naming"],
                defaults.auto_name,
            ),
            color: boolean(values, &["theme_color", "theme.use_color"], defaults.color),
            debug: boolean(values, &["debug", "debug.enabled"], defaults.debug),
        }
    }
}

fn first<'a>(values: &'a BTreeMap<String, String>, keys: &[&str]) -> Option<&'a String> {
    keys.iter().find_map(|key| values.get(*key))
}

fn nonempty(values: &BTreeMap<String, String>, keys: &[&str], fallback: &str) -> String {
    first(values, keys)
        .map(|value| clean_text(value, 16))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn clean_text(value: &str, max_chars: usize) -> String {
    value
        .chars()
        .take(max_chars)
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn bounded_usize(
    values: &BTreeMap<String, String>,
    keys: &[&str],
    fallback: usize,
    minimum: usize,
    maximum: usize,
) -> usize {
    first(values, keys)
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| (*value >= minimum) && (*value <= maximum))
        .unwrap_or(fallback)
}

fn boolean(values: &BTreeMap<String, String>, keys: &[&str], fallback: bool) -> bool {
    first(values, keys)
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_values_fall_back_without_discarding_valid_values() {
        let values = BTreeMap::from([
            ("badge_running".to_owned(), "R".to_owned()),
            ("badge_error".to_owned(), String::new()),
            ("layout_max_tab_width".to_owned(), "many".to_owned()),
            ("layout_show_index".to_owned(), "off".to_owned()),
            ("behavior_auto_name".to_owned(), "perhaps".to_owned()),
        ]);

        let config = PluginConfig::from_map(&values);

        assert_eq!(config.badge_running, "R");
        assert_eq!(config.badge_error, "❌");
        assert_eq!(config.max_tab_width, DEFAULT_MAX_TAB_WIDTH);
        assert!(!config.show_index);
        assert!(config.auto_name);
    }

    #[test]
    fn an_explicit_empty_separator_is_supported() {
        let values = BTreeMap::from([("layout_separator".to_owned(), String::new())]);

        assert!(PluginConfig::from_map(&values).separator.is_empty());
    }
}
