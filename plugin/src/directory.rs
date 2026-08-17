use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PaneKey {
    pub(crate) id: u32,
    pub(crate) is_plugin: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PaneDirectory {
    pub(crate) key: PaneKey,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) focused: bool,
    pub(crate) floating: bool,
    pub(crate) suppressed: bool,
    pub(crate) x: usize,
    pub(crate) y: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryCandidate {
    pub(crate) tab_id: usize,
    pub(crate) existing_name: String,
    pub(crate) floating_panes_visible: bool,
    pub(crate) panes: Vec<PaneDirectory>,
    pub(crate) last_focused: Option<PaneKey>,
    pub(crate) tab_cwd: Option<PathBuf>,
}

pub(crate) fn automatic_names(candidates: &[DirectoryCandidate]) -> HashMap<usize, String> {
    let paths: Vec<Option<PathBuf>> = candidates.iter().map(preferred_path).collect();
    candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let name = paths[index]
                .as_deref()
                .map(|path| disambiguated_path_label(path, &paths, index, candidate.tab_id))
                .filter(|name| !name.is_empty())
                .or_else(|| {
                    (!candidate.existing_name.is_empty()).then(|| candidate.existing_name.clone())
                })
                .unwrap_or_else(|| format!("tab-{}", candidate.tab_id));
            (candidate.tab_id, name)
        })
        .collect()
}

pub(crate) fn is_automatic_tab_name(name: &str) -> bool {
    let trimmed = name.trim();
    trimmed.is_empty() || is_default_tab_number(trimmed)
}

/// Zellij's default names take the form `Tab #N`. Closed and re-created tabs can
/// leave numbering that no longer matches the display position, so the exact
/// position is not part of the match; any `Tab #N` is treated as automatic.
fn is_default_tab_number(name: &str) -> bool {
    match name.strip_prefix("Tab #") {
        Some(number) if !number.is_empty() => {
            number.chars().all(|character| character.is_ascii_digit())
        }
        _ => false,
    }
}

fn preferred_path(candidate: &DirectoryCandidate) -> Option<PathBuf> {
    selected_pane(candidate)
        .and_then(|pane| pane.cwd.clone())
        .or_else(|| {
            candidate.last_focused.and_then(|last_focused| {
                candidate
                    .panes
                    .iter()
                    .find(|pane| pane.key == last_focused)
                    .and_then(|pane| pane.cwd.clone())
            })
        })
        .or_else(|| first_terminal(candidate).and_then(|pane| pane.cwd.clone()))
        .or_else(|| candidate.tab_cwd.clone())
}

pub(crate) fn selected_pane(candidate: &DirectoryCandidate) -> Option<&PaneDirectory> {
    let preferred_layer = candidate.panes.iter().find(|pane| {
        !pane.key.is_plugin
            && pane.focused
            && !pane.suppressed
            && (pane.floating == candidate.floating_panes_visible)
    });
    preferred_layer.or_else(|| {
        candidate
            .panes
            .iter()
            .find(|pane| !pane.key.is_plugin && pane.focused && !pane.suppressed)
    })
}

fn first_terminal(candidate: &DirectoryCandidate) -> Option<&PaneDirectory> {
    candidate
        .panes
        .iter()
        .filter(|pane| !pane.key.is_plugin && !pane.suppressed)
        .min_by_key(|pane| (pane.y, pane.x, pane.key.id))
}

fn disambiguated_path_label(
    path: &Path,
    all_paths: &[Option<PathBuf>],
    index: usize,
    tab_id: usize,
) -> String {
    let components = display_components(path);
    if components.is_empty() {
        return path.to_string_lossy().into_owned();
    }

    let basename = components.last().cloned().unwrap_or_default();
    let duplicates: Vec<(usize, Vec<String>)> = all_paths
        .iter()
        .enumerate()
        .filter_map(|(other_index, other_path)| {
            let other_components = display_components(other_path.as_deref()?);
            (other_components.last() == Some(&basename)).then_some((other_index, other_components))
        })
        .collect();

    if duplicates.len() <= 1 {
        return basename;
    }

    for depth in 2..=components.len() {
        let candidate_suffix = suffix(&components, depth);
        let unique = duplicates.iter().all(|(other_index, other_components)| {
            *other_index == index || suffix(other_components, depth) != candidate_suffix
        });
        if unique {
            return candidate_suffix;
        }
    }

    format!("{basename} · tab-{tab_id}")
}

fn display_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().into_owned()),
            Component::RootDir | Component::CurDir | Component::ParentDir => None,
        })
        .collect()
}

fn suffix(components: &[String], depth: usize) -> String {
    let start = components.len().saturating_sub(depth);
    components[start..].join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: u32, cwd: &str, focused: bool) -> PaneDirectory {
        PaneDirectory {
            key: PaneKey {
                id,
                is_plugin: false,
            },
            cwd: Some(PathBuf::from(cwd)),
            focused,
            floating: false,
            suppressed: false,
            x: usize::try_from(id).unwrap_or_default(),
            y: 0,
        }
    }

    fn candidate(tab_id: usize, position: usize, panes: Vec<PaneDirectory>) -> DirectoryCandidate {
        DirectoryCandidate {
            tab_id,
            existing_name: format!("Tab #{}", position + 1),
            floating_panes_visible: false,
            panes,
            last_focused: None,
            tab_cwd: None,
        }
    }

    #[test]
    fn focused_pane_wins_over_first_terminal() {
        let candidates = [candidate(
            9,
            0,
            vec![
                pane(1, "/work/first", false),
                pane(2, "/work/focused", true),
            ],
        )];

        assert_eq!(
            automatic_names(&candidates).get(&9),
            Some(&"focused".to_owned())
        );
    }

    #[test]
    fn duplicate_basenames_use_the_shortest_unique_parent() {
        let candidates = [
            candidate(1, 0, vec![pane(1, "/work/api", true)]),
            candidate(2, 1, vec![pane(2, "/srv/api", true)]),
        ];
        let names = automatic_names(&candidates);

        assert_eq!(names.get(&1), Some(&"work/api".to_owned()));
        assert_eq!(names.get(&2), Some(&"srv/api".to_owned()));
    }

    #[test]
    fn identical_paths_receive_a_stable_tab_suffix() {
        let candidates = [
            candidate(7, 0, vec![pane(1, "/work/api", true)]),
            candidate(8, 1, vec![pane(2, "/work/api", true)]),
        ];
        let names = automatic_names(&candidates);

        assert_eq!(names.get(&7), Some(&"api · tab-7".to_owned()));
        assert_eq!(names.get(&8), Some(&"api · tab-8".to_owned()));
    }

    #[test]
    fn only_empty_and_default_style_names_are_automatic() {
        assert!(is_automatic_tab_name(""));
        assert!(is_automatic_tab_name("Tab #4"));
        assert!(is_automatic_tab_name("Tab #9"));
        assert!(is_automatic_tab_name("  Tab #2  "));
        assert!(!is_automatic_tab_name("api"));
        assert!(!is_automatic_tab_name("Tab #"));
        assert!(!is_automatic_tab_name("tab-7"));
        assert!(!is_automatic_tab_name("Tab #2b"));
    }
}
