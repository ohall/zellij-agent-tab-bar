use zellij_agent_shared::AgentStatus;
use zellij_tile::prelude::{ModeInfo, Palette, PaletteColor, StyleDeclaration};

use crate::render::{TabSegment, TabText};

const ATTR_NORMAL: &str = "\u{1b}[22m";
const ATTR_BOLD: &str = "\u{1b}[1m";
const ATTR_DIM: &str = "\u{1b}[2m";
const RESET: &str = "\u{1b}[0m";

pub(crate) fn style_tab(
    text: &TabText,
    status: AgentStatus,
    active: bool,
    mode: &ModeInfo,
    color: bool,
) -> String {
    if !color {
        let plain = text.plain();
        return if active {
            format!("\u{1b}[7;1m{plain}{RESET}")
        } else {
            plain
        };
    }

    let ribbon = ribbon_style(active, mode);
    let badge_foreground = badge_foreground(status, ribbon, mode);
    let mut styled = String::new();
    for (segment, piece) in text.segments() {
        let (foreground, attribute) = match segment {
            TabSegment::Space => (ribbon.base, ATTR_NORMAL),
            TabSegment::Index => (ribbon.base, ATTR_DIM),
            TabSegment::Badge => {
                let attribute = if status == AgentStatus::Idle {
                    ATTR_DIM
                } else {
                    ATTR_BOLD
                };
                (badge_foreground, attribute)
            }
            TabSegment::Name => (ribbon.base, if active { ATTR_BOLD } else { ATTR_NORMAL }),
        };
        styled.push_str(&foreground_code(foreground));
        styled.push_str(&background_code(ribbon.background));
        styled.push_str(attribute);
        styled.push_str(piece);
    }
    styled.push_str(RESET);
    styled
}

pub(crate) fn style_separator(separator: &str, mode: &ModeInfo, color: bool) -> String {
    if !color || separator.is_empty() {
        return separator.to_owned();
    }
    format!(
        "{}{separator}",
        background_code(mode.style.colors.text_unselected.background)
    )
}

pub(crate) fn style_indicator(text: &str, mode: &ModeInfo, color: bool) -> String {
    if !color {
        return text.to_owned();
    }
    let ribbon = mode.style.colors.ribbon_unselected;
    format!(
        "{}{}{ATTR_DIM}{text}{RESET}",
        foreground_code(ribbon.base),
        background_code(ribbon.background)
    )
}

pub(crate) fn finish_line(output: &str, mode: &ModeInfo, color: bool) -> String {
    if !color {
        return format!("{output}\u{1b}[0K");
    }
    let background = background_code(mode.style.colors.text_unselected.background);
    format!("{output}{RESET}{background}\u{1b}[0K{RESET}")
}

fn ribbon_style(active: bool, mode: &ModeInfo) -> StyleDeclaration {
    if active {
        mode.style.colors.ribbon_selected
    } else {
        mode.style.colors.ribbon_unselected
    }
}

/// Status colors come from the active Zellij theme palette so badges stay
/// consistent with the user's theme. A badge that would blend into its tab
/// background falls back to the tab's base foreground.
fn badge_foreground(
    status: AgentStatus,
    ribbon: StyleDeclaration,
    mode: &ModeInfo,
) -> PaletteColor {
    let palette = Palette::from(mode.style.colors);
    let candidate = match status {
        AgentStatus::Idle => ribbon.base,
        AgentStatus::Running => palette.yellow,
        AgentStatus::Complete => palette.green,
        AgentStatus::Error => palette.red,
    };
    if candidate == ribbon.background {
        ribbon.base
    } else {
        candidate
    }
}

fn foreground_code(color: PaletteColor) -> String {
    match color {
        PaletteColor::Rgb((red, green, blue)) => format!("\u{1b}[38;2;{red};{green};{blue}m"),
        PaletteColor::EightBit(value) => format!("\u{1b}[38;5;{value}m"),
    }
}

fn background_code(color: PaletteColor) -> String {
    match color {
        PaletteColor::Rgb((red, green, blue)) => format!("\u{1b}[48;2;{red};{green};{blue}m"),
        PaletteColor::EightBit(value) => format!("\u{1b}[48;5;{value}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn badge_only(badge: &str) -> TabText {
        TabText {
            segments: vec![(TabSegment::Badge, badge.to_owned())],
        }
    }

    #[test]
    fn badges_use_theme_palette_status_colors() {
        let mut mode = ModeInfo::default();
        mode.style.colors.ribbon_unselected.background = PaletteColor::EightBit(0);
        let palette = Palette::from(mode.style.colors);

        for (status, expected) in [
            (AgentStatus::Running, palette.yellow),
            (AgentStatus::Complete, palette.green),
            (AgentStatus::Error, palette.red),
        ] {
            let styled = style_tab(&badge_only("●"), status, false, &mode, true);

            assert!(
                styled.contains(&foreground_code(expected)),
                "status {status:?} should color its badge"
            );
            assert!(styled.contains("\u{1b}[1m"), "active states stay bold");
            assert!(styled.ends_with(RESET));
        }
    }

    #[test]
    fn idle_badges_are_dimmed_rather_than_colored() {
        let mut mode = ModeInfo::default();
        mode.style.colors.ribbon_unselected.background = PaletteColor::EightBit(0);
        let base = mode.style.colors.ribbon_unselected.base;

        let styled = style_tab(&badge_only("○"), AgentStatus::Idle, false, &mode, true);

        assert!(styled.contains(&foreground_code(base)));
        assert!(styled.contains(ATTR_DIM));
    }

    #[test]
    fn a_badge_matching_the_tab_background_falls_back_to_the_base_color() {
        let mut mode = ModeInfo::default();
        let green = Palette::from(mode.style.colors).green;
        mode.style.colors.ribbon_selected.background = green;
        let base = mode.style.colors.ribbon_selected.base;

        let styled = style_tab(&badge_only("✓"), AgentStatus::Complete, true, &mode, true);

        assert!(styled.contains(&foreground_code(base)));
        assert!(!styled.contains(&foreground_code(green)));
    }

    #[test]
    fn the_tab_index_is_dimmed_relative_to_the_name() {
        let mode = ModeInfo::default();
        let text = TabText {
            segments: vec![
                (TabSegment::Index, "1".to_owned()),
                (TabSegment::Space, " ".to_owned()),
                (TabSegment::Name, "api".to_owned()),
            ],
        };

        let styled = style_tab(&text, AgentStatus::Idle, false, &mode, true);

        assert!(styled.contains(&format!("{ATTR_DIM}1")));
        assert!(styled.contains(&format!("{ATTR_NORMAL}api")));
    }

    #[test]
    fn the_separator_uses_the_bar_background_so_tabs_read_as_chips() {
        let mut mode = ModeInfo::default();
        mode.style.colors.text_unselected.background = PaletteColor::EightBit(42);

        assert_eq!(style_separator("   ", &mode, true), "\u{1b}[48;5;42m   ");
        assert_eq!(style_separator("   ", &mode, false), "   ");
    }

    #[test]
    fn hidden_tab_indicators_are_dimmed() {
        let mode = ModeInfo::default();

        let styled = style_indicator("+2…", &mode, true);

        assert!(styled.contains(ATTR_DIM));
        assert!(styled.contains("+2…"));
        assert!(styled.ends_with(RESET));
    }

    #[test]
    fn colorless_output_keeps_a_non_color_active_cue() {
        let mode = ModeInfo::default();
        let text = TabText {
            segments: vec![
                (TabSegment::Badge, "●".to_owned()),
                (TabSegment::Space, " ".to_owned()),
                (TabSegment::Name, "api".to_owned()),
            ],
        };

        let active = style_tab(&text, AgentStatus::Running, true, &mode, false);
        let inactive = style_tab(&text, AgentStatus::Running, false, &mode, false);

        assert!(active.contains("● api"));
        assert_eq!(inactive, "● api");
        assert_ne!(active, inactive);
    }
}
