use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use zellij_agent_shared::AgentStatus;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TabLabel {
    pub(crate) position: usize,
    pub(crate) active: bool,
    pub(crate) badge: String,
    pub(crate) status: AgentStatus,
    pub(crate) name: String,
}

/// A piece of a rendered tab that can be styled independently of its
/// neighbours, so the status badge can carry its own color.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TabSegment {
    Space,
    Index,
    Badge,
    Name,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TabText {
    pub(crate) segments: Vec<(TabSegment, String)>,
}

impl TabText {
    pub(crate) fn segments(&self) -> &[(TabSegment, String)] {
        &self.segments
    }

    pub(crate) fn plain(&self) -> String {
        self.segments
            .iter()
            .map(|(_, piece)| piece.as_str())
            .collect()
    }

    pub(crate) fn width(&self) -> usize {
        self.segments.iter().map(|(_, piece)| piece.width()).sum()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FittedItem {
    Tab {
        text: TabText,
        status: AgentStatus,
        active: bool,
    },
    Indicator(String),
}

impl FittedItem {
    #[cfg(test)]
    pub(crate) fn width(&self) -> usize {
        match self {
            Self::Tab { text, .. } => text.width(),
            Self::Indicator(text) => text.width(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum VisibleItem {
    HiddenLeft(usize),
    Tab(TabLabel),
    HiddenRight(usize),
}

impl VisibleItem {
    pub(crate) fn plain_text(&self, show_index: bool, max_tab_width: usize) -> String {
        match self {
            Self::HiddenLeft(count) => format!("…+{count}"),
            Self::HiddenRight(count) => format!("+{count}…"),
            Self::Tab(tab) => tab_text(tab, show_index, max_tab_width).plain(),
        }
    }
}

pub(crate) fn visible_items(
    tabs: &[TabLabel],
    cols: usize,
    separator: &str,
    show_index: bool,
    max_tab_width: usize,
) -> Vec<VisibleItem> {
    if tabs.is_empty() || cols == 0 {
        return Vec::new();
    }

    let active_index = tabs.iter().position(|tab| tab.active).unwrap_or(0);
    let mut start = active_index;
    let mut end = active_index + 1;

    loop {
        let expand_left = start.checked_sub(1);
        let expand_right = (end < tabs.len()).then_some(end);
        let candidates = expansion_order(expand_left, expand_right, active_index, start, end);
        let mut expanded = false;

        for candidate in candidates.into_iter().flatten() {
            let (candidate_start, candidate_end) = match candidate {
                Expansion::Left(index) => (index, end),
                Expansion::Right(index) => (start, index + 1),
            };
            let items = items_for_range(tabs, candidate_start, candidate_end);
            if items_width(&items, separator, show_index, max_tab_width) <= cols {
                start = candidate_start;
                end = candidate_end;
                expanded = true;
                break;
            }
        }

        if !expanded {
            break;
        }
    }

    let mut items = items_for_range(tabs, start, end);
    if items_width(&items, separator, show_index, max_tab_width) <= cols {
        return items;
    }

    items.retain(|item| matches!(item, VisibleItem::Tab(_)));
    let active = items
        .into_iter()
        .find(|item| matches!(item, VisibleItem::Tab(tab) if tab.active))
        .or_else(|| tabs.get(active_index).cloned().map(VisibleItem::Tab));

    active.into_iter().collect()
}

pub(crate) fn fit_item_to_width(
    item: &VisibleItem,
    cols: usize,
    show_index: bool,
    max_tab_width: usize,
) -> FittedItem {
    match item {
        VisibleItem::Tab(tab) => FittedItem::Tab {
            text: tab_text(tab, show_index, cols.min(max_tab_width)),
            status: tab.status,
            active: tab.active,
        },
        VisibleItem::HiddenLeft(_) | VisibleItem::HiddenRight(_) => FittedItem::Indicator(
            truncate_to_width(&item.plain_text(show_index, max_tab_width), cols),
        ),
    }
}

fn items_for_range(tabs: &[TabLabel], start: usize, end: usize) -> Vec<VisibleItem> {
    let mut items = Vec::new();
    if start > 0 {
        items.push(VisibleItem::HiddenLeft(start));
    }
    items.extend(tabs[start..end].iter().cloned().map(VisibleItem::Tab));
    if end < tabs.len() {
        items.push(VisibleItem::HiddenRight(tabs.len() - end));
    }
    items
}

#[derive(Clone, Copy)]
enum Expansion {
    Left(usize),
    Right(usize),
}

fn expansion_order(
    left: Option<usize>,
    right: Option<usize>,
    active: usize,
    start: usize,
    end: usize,
) -> [Option<Expansion>; 2] {
    let shown_left = active.saturating_sub(start);
    let shown_right = end.saturating_sub(active + 1);
    if shown_left <= shown_right {
        [left.map(Expansion::Left), right.map(Expansion::Right)]
    } else {
        [right.map(Expansion::Right), left.map(Expansion::Left)]
    }
}

fn items_width(
    items: &[VisibleItem],
    separator: &str,
    show_index: bool,
    max_tab_width: usize,
) -> usize {
    let labels_width: usize = items
        .iter()
        .map(|item| item.plain_text(show_index, max_tab_width).width())
        .sum();
    labels_width.saturating_add(
        separator
            .width()
            .saturating_mul(items.len().saturating_sub(1)),
    )
}

fn tab_text(tab: &TabLabel, show_index: bool, max_tab_width: usize) -> TabText {
    let badge = clean_display_text(&tab.badge);
    let name = clean_display_text(&tab.name);
    let index = tab.position.saturating_add(1).to_string();
    let indexed_prefix_width = if show_index {
        index
            .width()
            .saturating_add(1)
            .saturating_add(badge.width())
    } else {
        badge.width()
    };

    let padded = assemble(Some(&index), &badge, Some(&name), show_index, true);
    if padded.width() <= max_tab_width {
        return padded;
    }

    let unpadded = assemble(Some(&index), &badge, Some(&name), show_index, false);
    if unpadded.width() <= max_tab_width {
        return unpadded;
    }

    if indexed_prefix_width <= max_tab_width {
        return prefix_with_name(
            show_index.then_some(index.as_str()),
            &badge,
            &name,
            max_tab_width,
        );
    }

    if badge.width() <= max_tab_width {
        return prefix_with_name(None, &badge, &name, max_tab_width);
    }

    TabText {
        segments: vec![(TabSegment::Badge, take_to_width(&badge, max_tab_width))],
    }
}

fn assemble(
    index: Option<&str>,
    badge: &str,
    name: Option<&str>,
    show_index: bool,
    padded: bool,
) -> TabText {
    let mut segments = Vec::new();
    if padded {
        segments.push((TabSegment::Space, " ".to_owned()));
    }
    if show_index {
        if let Some(index) = index {
            segments.push((TabSegment::Index, index.to_owned()));
            segments.push((TabSegment::Space, " ".to_owned()));
        }
    }
    segments.push((TabSegment::Badge, badge.to_owned()));
    if let Some(name) = name.filter(|name| !name.is_empty()) {
        segments.push((TabSegment::Space, " ".to_owned()));
        segments.push((TabSegment::Name, name.to_owned()));
    }
    if padded {
        segments.push((TabSegment::Space, " ".to_owned()));
    }
    TabText { segments }
}

fn prefix_with_name(index: Option<&str>, badge: &str, name: &str, max_width: usize) -> TabText {
    let prefix_width = index
        .map(|index| index.width().saturating_add(1))
        .unwrap_or(0)
        .saturating_add(badge.width());
    let remaining = max_width.saturating_sub(prefix_width);
    if name.is_empty() || remaining <= 1 {
        return assemble(index, badge, None, index.is_some(), false);
    }
    let name = truncate_to_width(name, remaining.saturating_sub(1));
    if name.is_empty() {
        assemble(index, badge, None, index.is_some(), false)
    } else {
        assemble(index, badge, Some(&name), index.is_some(), false)
    }
}

fn clean_display_text(value: &str) -> String {
    value
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

pub(crate) fn truncate_to_width(value: &str, max_width: usize) -> String {
    if value.width() <= max_width {
        return value.to_owned();
    }
    if max_width == 0 {
        return String::new();
    }

    let ellipsis = "…";
    let ellipsis_width = ellipsis.width();
    if max_width < ellipsis_width {
        return String::new();
    }

    let content_width = max_width.saturating_sub(ellipsis_width);
    let mut result = String::new();
    let mut used_width: usize = 0;
    for grapheme in value.graphemes(true) {
        let grapheme_width = grapheme.width();
        if used_width.saturating_add(grapheme_width) > content_width {
            break;
        }
        result.push_str(grapheme);
        used_width = used_width.saturating_add(grapheme_width);
    }
    result.push_str(ellipsis);
    result
}

fn take_to_width(value: &str, max_width: usize) -> String {
    let mut result = String::new();
    let mut used_width: usize = 0;
    for grapheme in value.graphemes(true) {
        let grapheme_width = grapheme.width();
        if used_width.saturating_add(grapheme_width) > max_width {
            break;
        }
        result.push_str(grapheme);
        used_width = used_width.saturating_add(grapheme_width);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(id: usize, active: bool, name: &str) -> TabLabel {
        TabLabel {
            position: id,
            active,
            badge: "●".to_owned(),
            status: AgentStatus::Running,
            name: name.to_owned(),
        }
    }

    #[test]
    fn truncation_preserves_grapheme_clusters_and_column_limit() {
        let truncated = truncate_to_width("api-👩🏽‍💻-日本語", 8);

        assert!(truncated.width() <= 8);
        assert!(truncated.ends_with('…'));
        assert!(!truncated.contains('�'));
    }

    #[test]
    fn responsive_layout_always_keeps_the_active_tab() {
        let tabs = [
            tab(0, false, "api"),
            tab(1, false, "worker"),
            tab(2, true, "active-service"),
            tab(3, false, "infra"),
        ];

        let items = visible_items(&tabs, 15, " ", true, 20);

        assert!(items
            .iter()
            .any(|item| matches!(item, VisibleItem::Tab(tab) if tab.active)));
        let width = items_width(&items, " ", true, 20);
        if items.len() > 1 {
            assert!(width <= 15);
        }
    }

    #[test]
    fn oversized_active_tab_can_be_fit_to_the_viewport() {
        let tabs = [tab(0, true, "a very long active tab name")];
        let items = visible_items(&tabs, 6, " ", true, 40);
        let rendered = fit_item_to_width(&items[0], 6, true, 40);

        assert!(rendered.width() <= 6);
    }

    #[test]
    fn one_column_viewport_preserves_the_status_badge() {
        let item = VisibleItem::Tab(tab(0, true, "active-service"));

        let fitted = fit_item_to_width(&item, 1, true, 40);

        assert_eq!(
            fitted,
            FittedItem::Tab {
                text: TabText {
                    segments: vec![(TabSegment::Badge, "●".to_owned())],
                },
                status: AgentStatus::Running,
                active: true,
            }
        );
    }

    #[test]
    fn fitted_tabs_expose_stylable_segments_in_reading_order() {
        let item = VisibleItem::Tab(tab(0, true, "api"));

        let fitted = fit_item_to_width(&item, 32, true, 40);

        assert_eq!(
            fitted,
            FittedItem::Tab {
                text: TabText {
                    segments: vec![
                        (TabSegment::Space, " ".to_owned()),
                        (TabSegment::Index, "1".to_owned()),
                        (TabSegment::Space, " ".to_owned()),
                        (TabSegment::Badge, "●".to_owned()),
                        (TabSegment::Space, " ".to_owned()),
                        (TabSegment::Name, "api".to_owned()),
                        (TabSegment::Space, " ".to_owned()),
                    ],
                },
                status: AgentStatus::Running,
                active: true,
            }
        );
    }

    #[test]
    fn segment_width_matches_the_plain_text_width() {
        for show_index in [true, false] {
            for max_tab_width in [0, 1, 3, 8, 32] {
                let text = tab_text(&tab(0, false, "api-👩🏽‍💻"), show_index, max_tab_width);
                assert_eq!(text.width(), text.plain().width());
            }
        }
    }

    #[test]
    fn zero_width_viewport_yields_no_items() {
        assert!(visible_items(&[tab(0, true, "api")], 0, " ", true, 20).is_empty());
    }

    #[test]
    fn control_characters_cannot_escape_the_rendered_label() {
        let label = tab(0, true, "api\u{1b}[31m\n");
        let item = VisibleItem::Tab(label);
        let rendered = item.plain_text(true, 32);

        assert!(!rendered.contains('\u{1b}'));
        assert!(!rendered.contains('\n'));
    }
}
