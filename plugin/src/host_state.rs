use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use zellij_tile::prelude::{PaneId, PaneInfo, PaneManifest, TabInfo};

use crate::directory::{DirectoryCandidate, PaneDirectory, PaneKey};

#[derive(Clone, Debug, Default)]
pub(crate) struct HostState {
    tabs: Vec<TabInfo>,
    manifest_by_position: HashMap<usize, Vec<PaneInfo>>,
    panes_by_tab: HashMap<usize, Vec<PaneInfo>>,
    pane_cwds: HashMap<PaneKey, PathBuf>,
    last_focused: HashMap<usize, PaneKey>,
    tab_cwds: HashMap<usize, PathBuf>,
}

impl HostState {
    pub(crate) fn tabs(&self) -> &[TabInfo] {
        &self.tabs
    }

    pub(crate) fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }

    pub(crate) fn set_tabs(&mut self, mut tabs: Vec<TabInfo>) -> bool {
        tabs.sort_by_key(|tab| tab.position);
        let tabs_changed = tabs != self.tabs;

        let live_tab_ids: HashSet<usize> = tabs.iter().map(|tab| tab.tab_id).collect();
        self.last_focused
            .retain(|tab_id, _| live_tab_ids.contains(tab_id));
        self.tab_cwds
            .retain(|tab_id, _| live_tab_ids.contains(tab_id));
        self.panes_by_tab
            .retain(|tab_id, _| live_tab_ids.contains(tab_id));
        self.tabs = tabs;
        let panes_by_tab = self.map_manifest_to_stable_tabs(&self.manifest_by_position);
        let panes_changed = panes_by_tab != self.panes_by_tab;
        self.panes_by_tab = panes_by_tab;
        self.refresh_focus_history();
        tabs_changed || panes_changed
    }

    pub(crate) fn set_panes<F>(&mut self, panes: PaneManifest, mut cwd_for: F) -> bool
    where
        F: FnMut(PaneId) -> Result<PathBuf, String>,
    {
        self.manifest_by_position = panes.panes;
        let panes_by_tab = self.map_manifest_to_stable_tabs(&self.manifest_by_position);
        let manifest_changed = panes_by_tab != self.panes_by_tab;
        self.panes_by_tab = panes_by_tab;
        let cwd_changed = self.seed_missing_cwds(&mut cwd_for);
        self.refresh_focus_history();
        manifest_changed || cwd_changed
    }

    pub(crate) fn seed_cwds<F>(&mut self, mut cwd_for: F) -> bool
    where
        F: FnMut(PaneId) -> Result<PathBuf, String>,
    {
        let changed = self.seed_missing_cwds(&mut cwd_for);
        self.refresh_focus_history();
        changed
    }

    pub(crate) fn cwd_changed(&mut self, pane_id: PaneId, cwd: PathBuf) -> bool {
        let key = PaneKey::from(pane_id);
        let changed = self.pane_cwds.get(&key) != Some(&cwd);
        self.pane_cwds.insert(key, cwd.clone());
        if let Some(tab_id) = self.tab_id_for_pane(key) {
            self.tab_cwds.insert(tab_id, cwd);
        }
        changed
    }

    pub(crate) fn pane_closed(&mut self, pane_id: PaneId) -> bool {
        let key = PaneKey::from(pane_id);
        let removed_cwd = self.pane_cwds.remove(&key).is_some();
        let mut manifest_removed = false;
        for panes in self.manifest_by_position.values_mut() {
            let previous_len = panes.len();
            panes.retain(|pane| PaneKey::from(pane) != key);
            manifest_removed |= previous_len != panes.len();
        }
        let previous_pane_count: usize = self.panes_by_tab.values().map(Vec::len).sum();
        for panes in self.panes_by_tab.values_mut() {
            panes.retain(|pane| PaneKey::from(pane) != key);
        }
        let pane_removed =
            previous_pane_count != self.panes_by_tab.values().map(Vec::len).sum::<usize>();
        let previous_len = self.last_focused.len();
        self.last_focused.retain(|_, pane| *pane != key);
        removed_cwd || manifest_removed || pane_removed || previous_len != self.last_focused.len()
    }

    pub(crate) fn directory_candidates(&self) -> Vec<DirectoryCandidate> {
        self.tabs
            .iter()
            .map(|tab| {
                let panes = self
                    .panes_by_tab
                    .get(&tab.tab_id)
                    .map(|panes| panes.iter().map(|pane| self.directory_pane(pane)).collect())
                    .unwrap_or_default();
                DirectoryCandidate {
                    tab_id: tab.tab_id,
                    existing_name: tab.name.clone(),
                    floating_panes_visible: tab.are_floating_panes_visible,
                    panes,
                    last_focused: self.last_focused.get(&tab.tab_id).copied(),
                    tab_cwd: self.tab_cwds.get(&tab.tab_id).cloned(),
                }
            })
            .collect()
    }

    fn seed_missing_cwds<F>(&mut self, cwd_for: &mut F) -> bool
    where
        F: FnMut(PaneId) -> Result<PathBuf, String>,
    {
        let live_panes: HashSet<PaneKey> = self
            .panes_by_tab
            .values()
            .flatten()
            .map(PaneKey::from)
            .collect();
        self.pane_cwds.retain(|key, _| live_panes.contains(key));

        let missing_terminals: Vec<PaneKey> = live_panes
            .into_iter()
            .filter(|key| !key.is_plugin && !self.pane_cwds.contains_key(key))
            .collect();
        let mut changed = false;
        for key in missing_terminals {
            if let Ok(cwd) = cwd_for(key.into()) {
                self.pane_cwds.insert(key, cwd);
                changed = true;
            }
        }
        changed
    }

    fn refresh_focus_history(&mut self) {
        let focused: Vec<(usize, PaneKey, Option<PathBuf>)> = self
            .tabs
            .iter()
            .filter_map(|tab| {
                let panes = self.panes_by_tab.get(&tab.tab_id)?;
                let pane = focused_terminal(panes, tab.are_floating_panes_visible)?;
                let key = PaneKey::from(pane);
                Some((tab.tab_id, key, self.pane_cwds.get(&key).cloned()))
            })
            .collect();
        for (tab_id, pane, cwd) in focused {
            self.last_focused.insert(tab_id, pane);
            if let Some(cwd) = cwd {
                self.tab_cwds.insert(tab_id, cwd);
            }
        }
    }

    fn directory_pane(&self, pane: &PaneInfo) -> PaneDirectory {
        let key = PaneKey::from(pane);
        PaneDirectory {
            key,
            cwd: self.pane_cwds.get(&key).cloned(),
            focused: pane.is_focused,
            floating: pane.is_floating,
            suppressed: pane.is_suppressed,
            x: pane.pane_x,
            y: pane.pane_y,
        }
    }

    pub(crate) fn tab_id_for_pane(&self, key: PaneKey) -> Option<usize> {
        self.panes_by_tab
            .iter()
            .find(|(_, panes)| panes.iter().any(|pane| PaneKey::from(pane) == key))
            .map(|(tab_id, _)| *tab_id)
    }

    pub(crate) fn terminal_pane_assignments(&self) -> Vec<(PaneKey, usize)> {
        self.panes_by_tab
            .iter()
            .flat_map(|(tab_id, panes)| {
                panes.iter().filter_map(|pane| {
                    let key = PaneKey::from(pane);
                    (!key.is_plugin).then_some((key, *tab_id))
                })
            })
            .collect()
    }

    fn map_manifest_to_stable_tabs(
        &self,
        manifest: &HashMap<usize, Vec<PaneInfo>>,
    ) -> HashMap<usize, Vec<PaneInfo>> {
        let mut positioned_panes: Vec<(usize, Vec<PaneInfo>)> = manifest
            .iter()
            .map(|(position, panes)| (*position, panes.clone()))
            .collect();
        positioned_panes.sort_by_key(|(position, _)| *position);

        let mut mapped = HashMap::new();
        let mut assigned_tabs = HashSet::new();
        let mut assigned_positions = HashSet::new();
        for (position, panes) in &positioned_panes {
            let pane_keys: HashSet<PaneKey> = panes
                .iter()
                .map(PaneKey::from)
                .filter(|pane| !pane.is_plugin)
                .collect();
            let positioned_tab = self
                .tabs
                .iter()
                .find(|tab| tab.position == *position)
                .map(|tab| tab.tab_id);
            let previous_match = self
                .panes_by_tab
                .iter()
                .filter(|(tab_id, _)| !assigned_tabs.contains(*tab_id))
                .map(|(tab_id, previous_panes)| {
                    let overlap = previous_panes
                        .iter()
                        .map(PaneKey::from)
                        .filter(|pane| !pane.is_plugin && pane_keys.contains(pane))
                        .count();
                    (*tab_id, overlap, positioned_tab == Some(*tab_id))
                })
                .filter(|(_, overlap, _)| *overlap > 0)
                .max_by_key(|(tab_id, overlap, at_position)| {
                    (*overlap, *at_position, std::cmp::Reverse(*tab_id))
                })
                .map(|(tab_id, _, _)| tab_id);
            if let Some(tab_id) = previous_match {
                assigned_tabs.insert(tab_id);
                assigned_positions.insert(*position);
                mapped.insert(tab_id, panes.clone());
            }
        }

        for (position, panes) in positioned_panes {
            if assigned_positions.contains(&position) {
                continue;
            }
            let positioned_tab = self
                .tabs
                .iter()
                .find(|tab| tab.position == position)
                .map(|tab| tab.tab_id)
                .filter(|tab_id| !assigned_tabs.contains(tab_id));
            if let Some(tab_id) = positioned_tab {
                assigned_tabs.insert(tab_id);
                mapped.insert(tab_id, panes);
            }
        }
        mapped
    }
}

fn focused_terminal(panes: &[PaneInfo], floating_visible: bool) -> Option<&PaneInfo> {
    panes
        .iter()
        .find(|pane| {
            !pane.is_plugin
                && pane.is_focused
                && !pane.is_suppressed
                && pane.is_floating == floating_visible
        })
        .or_else(|| {
            panes
                .iter()
                .find(|pane| !pane.is_plugin && pane.is_focused && !pane.is_suppressed)
        })
}

impl From<PaneId> for PaneKey {
    fn from(value: PaneId) -> Self {
        match value {
            PaneId::Terminal(id) => Self {
                id,
                is_plugin: false,
            },
            PaneId::Plugin(id) => Self {
                id,
                is_plugin: true,
            },
        }
    }
}

impl From<&PaneInfo> for PaneKey {
    fn from(value: &PaneInfo) -> Self {
        Self {
            id: value.id,
            is_plugin: value.is_plugin,
        }
    }
}

impl From<PaneKey> for PaneId {
    fn from(value: PaneKey) -> Self {
        if value.is_plugin {
            Self::Plugin(value.id)
        } else {
            Self::Terminal(value.id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(tab_id: usize, position: usize) -> TabInfo {
        TabInfo {
            tab_id,
            position,
            name: format!("Tab #{}", position + 1),
            active: position == 0,
            ..TabInfo::default()
        }
    }

    fn pane(id: u32, focused: bool) -> PaneInfo {
        PaneInfo {
            id,
            is_plugin: false,
            is_focused: focused,
            ..PaneInfo::default()
        }
    }

    #[test]
    fn tab_reorder_keeps_focus_history_attached_to_stable_id() {
        let mut state = HostState::default();
        state.set_tabs(vec![tab(10, 0), tab(20, 1)]);
        state.set_panes(
            PaneManifest {
                panes: HashMap::from([(0, vec![pane(1, true)]), (1, vec![pane(2, true)])]),
            },
            |pane_id| match pane_id {
                PaneId::Terminal(1) => Ok(PathBuf::from("/work/one")),
                PaneId::Terminal(2) => Ok(PathBuf::from("/work/two")),
                PaneId::Plugin(_) | PaneId::Terminal(_) => Err("unknown pane".to_owned()),
            },
        );

        state.set_tabs(vec![tab(20, 0), tab(10, 1)]);
        let candidates = state.directory_candidates();

        assert_eq!(candidates[0].tab_id, 20);
        assert_eq!(candidates[0].last_focused.map(|pane| pane.id), Some(2));
        assert_eq!(candidates[1].tab_id, 10);
        assert_eq!(candidates[1].last_focused.map(|pane| pane.id), Some(1));
    }

    #[test]
    fn closing_unknown_pane_is_a_noop() {
        assert!(!HostState::default().pane_closed(PaneId::Terminal(999)));
    }

    #[test]
    fn pane_snapshot_before_tab_snapshot_is_reconciled() {
        let mut state = HostState::default();
        state.set_panes(
            PaneManifest {
                panes: HashMap::from([(0, vec![pane(7, true)])]),
            },
            |_| Ok(PathBuf::from("/work/early")),
        );

        state.set_tabs(vec![tab(10, 0)]);
        state.seed_cwds(|_| Ok(PathBuf::from("/work/early")));

        let candidates = state.directory_candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].panes.len(), 1);
        assert_eq!(
            candidates[0].panes[0].cwd.as_deref(),
            Some(std::path::Path::new("/work/early"))
        );
    }

    #[test]
    fn moved_pane_is_attached_to_its_new_stable_tab() {
        let mut state = HostState::default();
        state.set_tabs(vec![tab(10, 0), tab(20, 1)]);
        state.set_panes(
            PaneManifest {
                panes: HashMap::from([(0, vec![pane(1, true)]), (1, vec![pane(2, true)])]),
            },
            |_| Err("cwd unavailable".to_owned()),
        );

        state.set_panes(
            PaneManifest {
                panes: HashMap::from([(0, Vec::new()), (1, vec![pane(1, false), pane(2, true)])]),
            },
            |_| Err("cwd unavailable".to_owned()),
        );

        let terminal = PaneKey {
            id: 1,
            is_plugin: false,
        };
        assert_eq!(state.tab_id_for_pane(terminal), Some(20));
    }

    #[test]
    fn stale_manifest_does_not_shift_closed_tab_panes_onto_a_live_tab() {
        let mut state = HostState::default();
        state.set_tabs(vec![tab(10, 0), tab(20, 1), tab(30, 2)]);
        state.set_panes(
            PaneManifest {
                panes: HashMap::from([
                    (0, vec![pane(1, true)]),
                    (1, vec![pane(2, false)]),
                    (2, vec![pane(3, false)]),
                ]),
            },
            |_| Err("cwd unavailable".to_owned()),
        );

        state.set_tabs(vec![tab(10, 0), tab(30, 1)]);

        let live_pane = PaneKey {
            id: 3,
            is_plugin: false,
        };
        let closed_pane = PaneKey {
            id: 2,
            is_plugin: false,
        };
        assert_eq!(state.tab_id_for_pane(live_pane), Some(30));
        assert_eq!(state.tab_id_for_pane(closed_pane), None);
    }
}
