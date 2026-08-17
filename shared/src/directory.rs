use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{PaneState, State, TabId, TabState};

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DirectoryPath(String);

impl DirectoryPath {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn from_path_lossy(path: &Path) -> Self {
        Self(path.to_string_lossy().into_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }

    #[must_use]
    pub fn basename(&self) -> Option<String> {
        basename(&self.0)
    }

    fn components(&self) -> Vec<&str> {
        path_components(&self.0)
    }
}

impl From<String> for DirectoryPath {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for DirectoryPath {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameSource {
    Manual,
    FocusedPane,
    LastFocusedPane,
    FirstTerminalPane,
    TabDirectory,
    ExistingName,
    Fallback,
}

impl NameSource {
    #[must_use]
    pub const fn is_directory(self) -> bool {
        matches!(
            self,
            Self::FocusedPane
                | Self::LastFocusedPane
                | Self::FirstTerminalPane
                | Self::TabDirectory
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedTabName {
    tab_id: TabId,
    name: String,
    source: NameSource,
    directory: Option<DirectoryPath>,
}

impl ResolvedTabName {
    #[must_use]
    pub const fn tab_id(&self) -> TabId {
        self.tab_id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn source(&self) -> NameSource {
        self.source
    }

    #[must_use]
    pub fn directory(&self) -> Option<&DirectoryPath> {
        self.directory.as_ref()
    }
}

#[must_use]
pub fn basename(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let trimmed = path.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        return path.chars().next().map(|separator| separator.to_string());
    }
    trimmed
        .rsplit(['/', '\\'])
        .find(|component| !component.is_empty())
        .map(ToOwned::to_owned)
}

#[must_use]
pub fn resolve_tab_names(state: &State) -> Vec<ResolvedTabName> {
    let mut candidates: Vec<NameCandidate> = state.tabs_in_order().map(candidate_for_tab).collect();
    disambiguate(&mut candidates);
    candidates
        .into_iter()
        .map(|candidate| ResolvedTabName {
            tab_id: candidate.tab_id,
            name: candidate.name,
            source: candidate.source,
            directory: candidate.directory,
        })
        .collect()
}

#[derive(Debug)]
struct NameCandidate {
    tab_id: TabId,
    name: String,
    source: NameSource,
    directory: Option<DirectoryPath>,
    components: Vec<String>,
    depth: usize,
}

fn candidate_for_tab(tab: &TabState) -> NameCandidate {
    if let Some(name) = tab.manual_name() {
        return fixed_candidate(tab.id(), name, NameSource::Manual);
    }

    if let Some(pane) = focused_pane_with_directory(tab) {
        return directory_candidate(tab.id(), pane.directory(), NameSource::FocusedPane);
    }

    if let Some(pane) = last_focused_pane_with_directory(tab) {
        return directory_candidate(tab.id(), pane.directory(), NameSource::LastFocusedPane);
    }

    if let Some(pane) = first_terminal_pane_with_directory(tab) {
        return directory_candidate(tab.id(), pane.directory(), NameSource::FirstTerminalPane);
    }

    if tab.directory().is_some() {
        return directory_candidate(tab.id(), tab.directory(), NameSource::TabDirectory);
    }

    if let Some(name) = tab.existing_name() {
        return fixed_candidate(tab.id(), name, NameSource::ExistingName);
    }

    fixed_candidate(tab.id(), &format!("tab-{}", tab.id()), NameSource::Fallback)
}

fn focused_pane_with_directory(tab: &TabState) -> Option<&PaneState> {
    tab.focused_pane_id()
        .and_then(|pane_id| tab.pane(pane_id))
        .filter(|pane| pane.directory().is_some())
}

fn last_focused_pane_with_directory(tab: &TabState) -> Option<&PaneState> {
    tab.panes()
        .iter()
        .filter(|pane| {
            Some(pane.id()) != tab.focused_pane_id()
                && pane.last_focused_order() > 0
                && pane.directory().is_some()
        })
        .max_by_key(|pane| pane.last_focused_order())
}

fn first_terminal_pane_with_directory(tab: &TabState) -> Option<&PaneState> {
    tab.panes()
        .iter()
        .filter(|pane| pane.is_terminal() && pane.directory().is_some())
        .min_by_key(|pane| pane.created_order())
}

fn fixed_candidate(tab_id: TabId, name: &str, source: NameSource) -> NameCandidate {
    NameCandidate {
        tab_id,
        name: name.to_owned(),
        source,
        directory: None,
        components: Vec::new(),
        depth: 0,
    }
}

fn directory_candidate(
    tab_id: TabId,
    directory: Option<&DirectoryPath>,
    source: NameSource,
) -> NameCandidate {
    let owned_directory = directory.cloned();
    let components: Vec<String> = directory
        .map(DirectoryPath::components)
        .unwrap_or_default()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect();
    let name = directory
        .and_then(DirectoryPath::basename)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("tab-{tab_id}"));
    NameCandidate {
        tab_id,
        name,
        source,
        directory: owned_directory,
        components,
        depth: 1,
    }
}

fn disambiguate(candidates: &mut [NameCandidate]) {
    loop {
        let counts = name_counts(candidates);
        let mut advanced = false;
        for candidate in candidates.iter_mut() {
            let is_duplicate = counts.get(&candidate.name).copied().unwrap_or(0) > 1;
            if is_duplicate
                && candidate.source.is_directory()
                && candidate.depth < candidate.components.len()
            {
                candidate.depth += 1;
                candidate.name = suffix(&candidate.components, candidate.depth);
                advanced = true;
            }
        }
        if !advanced {
            break;
        }
    }

    let counts = name_counts(candidates);
    for candidate in candidates.iter_mut() {
        if candidate.source.is_directory() && counts.get(&candidate.name).copied().unwrap_or(0) > 1
        {
            let base = candidate
                .components
                .last()
                .cloned()
                .unwrap_or_else(|| format!("tab-{}", candidate.tab_id));
            candidate.name = format!("{base} #{tab_id}", tab_id = candidate.tab_id);
        }
    }
}

fn name_counts(candidates: &[NameCandidate]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for candidate in candidates {
        let count = counts.entry(candidate.name.clone()).or_insert(0_usize);
        *count = count.saturating_add(1);
    }
    counts
}

fn path_components(path: &str) -> Vec<&str> {
    path.split(['/', '\\'])
        .filter(|component| !component.is_empty())
        .collect()
}

fn suffix(components: &[String], depth: usize) -> String {
    let start = components.len().saturating_sub(depth);
    components[start..].join("/")
}
