use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AgentId, AgentStatus, DirectoryPath, Event, EventKind, Generation, PaneId, Sequence, TabId,
    EVENT_SCHEMA_VERSION,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneState {
    id: PaneId,
    directory: Option<DirectoryPath>,
    is_terminal: bool,
    created_order: u64,
    last_focused_order: u64,
}

impl PaneState {
    #[must_use]
    pub const fn id(&self) -> PaneId {
        self.id
    }

    #[must_use]
    pub fn directory(&self) -> Option<&DirectoryPath> {
        self.directory.as_ref()
    }

    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        self.is_terminal
    }

    #[must_use]
    pub const fn created_order(&self) -> u64 {
        self.created_order
    }

    #[must_use]
    pub const fn last_focused_order(&self) -> u64 {
        self.last_focused_order
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabState {
    id: TabId,
    directory: Option<DirectoryPath>,
    existing_name: Option<String>,
    manual_name: Option<String>,
    focused_pane: Option<PaneId>,
    panes: Vec<PaneState>,
}

impl TabState {
    #[must_use]
    pub const fn id(&self) -> TabId {
        self.id
    }

    #[must_use]
    pub fn directory(&self) -> Option<&DirectoryPath> {
        self.directory.as_ref()
    }

    #[must_use]
    pub fn existing_name(&self) -> Option<&str> {
        self.existing_name.as_deref()
    }

    #[must_use]
    pub fn manual_name(&self) -> Option<&str> {
        self.manual_name.as_deref()
    }

    #[must_use]
    pub const fn focused_pane_id(&self) -> Option<PaneId> {
        self.focused_pane
    }

    #[must_use]
    pub fn panes(&self) -> &[PaneState] {
        &self.panes
    }

    #[must_use]
    pub fn pane(&self, pane_id: PaneId) -> Option<&PaneState> {
        self.panes.iter().find(|pane| pane.id == pane_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRecord {
    tab_id: TabId,
    pane_id: PaneId,
    agent_id: AgentId,
    generation: Generation,
    sequence: Sequence,
    status: AgentStatus,
}

impl AgentRecord {
    #[must_use]
    pub const fn tab_id(&self) -> TabId {
        self.tab_id
    }

    #[must_use]
    pub const fn pane_id(&self) -> PaneId {
        self.pane_id
    }

    #[must_use]
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    #[must_use]
    pub const fn sequence(&self) -> Sequence {
        self.sequence
    }

    #[must_use]
    pub const fn status(&self) -> AgentStatus {
        self.status
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    tabs: BTreeMap<TabId, TabState>,
    tab_order: Vec<TabId>,
    agents: Vec<AgentRecord>,
    next_pane_order: u64,
    next_focus_order: u64,
}

impl State {
    #[must_use]
    pub fn tab(&self, tab_id: TabId) -> Option<&TabState> {
        self.tabs.get(&tab_id)
    }

    #[must_use]
    pub fn pane_tab(&self, pane_id: PaneId) -> Option<TabId> {
        self.tab_order.iter().find_map(|tab_id| {
            self.tabs
                .get(tab_id)
                .and_then(|tab| tab.pane(pane_id).map(|_| *tab_id))
        })
    }

    #[must_use]
    pub fn tab_order(&self) -> &[TabId] {
        &self.tab_order
    }

    pub fn tabs_in_order(&self) -> impl Iterator<Item = &TabState> {
        self.tab_order
            .iter()
            .filter_map(|tab_id| self.tabs.get(tab_id))
    }

    #[must_use]
    pub fn agents(&self) -> &[AgentRecord] {
        &self.agents
    }

    #[must_use]
    pub fn aggregate_status_for_tab(&self, tab_id: TabId) -> AgentStatus {
        AgentStatus::aggregate(
            self.agents
                .iter()
                .filter(|agent| agent.tab_id == tab_id)
                .map(|agent| agent.status),
        )
    }

    pub fn apply(&mut self, event: Event) -> Result<ApplyOutcome, StateError> {
        self.check_version(&event)?;
        Ok(self.apply_kind(event.kind))
    }

    pub fn reduce(&self, event: &Event) -> Result<Reduction, StateError> {
        let mut state = self.clone();
        let outcome = state.apply(event.clone())?;
        Ok(Reduction { state, outcome })
    }

    fn check_version(&self, event: &Event) -> Result<(), StateError> {
        if event.version == EVENT_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(StateError::UnsupportedEventVersion {
                received: event.version,
                supported: EVENT_SCHEMA_VERSION,
            })
        }
    }

    fn apply_kind(&mut self, kind: EventKind) -> ApplyOutcome {
        match kind {
            EventKind::StatusChanged {
                tab_id,
                pane_id,
                agent_id,
                status,
                generation,
                sequence,
            } => self.apply_status(tab_id, pane_id, agent_id, status, generation, sequence),
            EventKind::DirectoryChanged {
                tab_id,
                pane_id,
                directory,
            } => self.apply_directory(tab_id, pane_id, directory),
            EventKind::TabCreated {
                tab_id,
                position,
                directory,
                existing_name,
                manual_name,
            } => self.create_tab(tab_id, position, directory, existing_name, manual_name),
            EventKind::TabClosed { tab_id } => self.close_tab(tab_id),
            EventKind::TabMoved { tab_id, position } => self.move_tab(tab_id, position),
            EventKind::TabRenamed { tab_id, name } => self.rename_tab(tab_id, name),
            EventKind::AutomaticNamingRestored { tab_id } => self.restore_automatic_name(tab_id),
            EventKind::PaneFocused {
                tab_id,
                pane_id,
                directory,
                is_terminal,
            } => self.focus_pane(tab_id, pane_id, directory, is_terminal),
            EventKind::PaneExited { tab_id, pane_id } => self.exit_pane(tab_id, pane_id),
        }
    }

    fn apply_status(
        &mut self,
        explicit_tab_id: Option<TabId>,
        pane_id: PaneId,
        agent_id: AgentId,
        status: AgentStatus,
        generation: Generation,
        sequence: Sequence,
    ) -> ApplyOutcome {
        let tab_id = explicit_tab_id.or_else(|| self.pane_tab(pane_id));
        let Some(tab_id) = tab_id.filter(|candidate| self.tabs.contains_key(candidate)) else {
            return ApplyOutcome::IgnoredUnknownTarget;
        };

        if let Some(record) = self
            .agents
            .iter_mut()
            .find(|record| record.pane_id == pane_id && record.agent_id == agent_id)
        {
            if generation < record.generation
                || (generation == record.generation && sequence <= record.sequence)
            {
                return ApplyOutcome::IgnoredStale;
            }
            record.tab_id = tab_id;
            record.generation = generation;
            record.sequence = sequence;
            record.status = status;
            return ApplyOutcome::Applied;
        }

        self.agents.push(AgentRecord {
            tab_id,
            pane_id,
            agent_id,
            generation,
            sequence,
            status,
        });
        ApplyOutcome::Applied
    }

    fn apply_directory(
        &mut self,
        tab_id: TabId,
        pane_id: Option<PaneId>,
        directory: DirectoryPath,
    ) -> ApplyOutcome {
        if let Some(pane_id) = pane_id {
            let Some(tab) = self.tabs.get(&tab_id) else {
                return ApplyOutcome::IgnoredUnknownTarget;
            };
            let has_other_location = self.tabs.iter().any(|(candidate_id, candidate)| {
                *candidate_id != tab_id && candidate.pane(pane_id).is_some()
            });
            if !has_other_location
                && tab
                    .pane(pane_id)
                    .is_some_and(|pane| pane.directory.as_ref() == Some(&directory))
            {
                return ApplyOutcome::NoChange;
            }
            let target_has_pane = tab.pane(pane_id).is_some();
            let moved_pane = self.take_pane_from_other_tabs(tab_id, pane_id);
            let pane_order = if !target_has_pane && moved_pane.is_none() {
                Some(self.take_pane_order())
            } else {
                None
            };
            let Some(tab) = self.tabs.get_mut(&tab_id) else {
                return ApplyOutcome::IgnoredUnknownTarget;
            };
            if let Some(pane) = tab.panes.iter_mut().find(|pane| pane.id == pane_id) {
                pane.directory = Some(directory);
            } else {
                let mut pane = moved_pane.unwrap_or(PaneState {
                    id: pane_id,
                    directory: None,
                    is_terminal: true,
                    created_order: pane_order.unwrap_or_default(),
                    last_focused_order: 0,
                });
                pane.directory = Some(directory);
                tab.panes.push(pane);
            }
            self.retarget_agents_for_pane(pane_id, tab_id);
        } else if let Some(tab) = self.tabs.get_mut(&tab_id) {
            if tab.directory.as_ref() == Some(&directory) {
                return ApplyOutcome::NoChange;
            }
            tab.directory = Some(directory);
        } else {
            return ApplyOutcome::IgnoredUnknownTarget;
        }
        ApplyOutcome::Applied
    }

    fn create_tab(
        &mut self,
        tab_id: TabId,
        position: usize,
        directory: Option<DirectoryPath>,
        existing_name: Option<String>,
        manual_name: Option<String>,
    ) -> ApplyOutcome {
        if self.tabs.contains_key(&tab_id) {
            return ApplyOutcome::IgnoredDuplicate;
        }
        self.tabs.insert(
            tab_id,
            TabState {
                id: tab_id,
                directory,
                existing_name,
                manual_name,
                focused_pane: None,
                panes: Vec::new(),
            },
        );
        let insertion_index = position.min(self.tab_order.len());
        self.tab_order.insert(insertion_index, tab_id);
        ApplyOutcome::Applied
    }

    fn close_tab(&mut self, tab_id: TabId) -> ApplyOutcome {
        if self.tabs.remove(&tab_id).is_none() {
            return ApplyOutcome::IgnoredUnknownTarget;
        }
        self.tab_order.retain(|candidate| *candidate != tab_id);
        self.agents.retain(|agent| agent.tab_id != tab_id);
        ApplyOutcome::Applied
    }

    fn move_tab(&mut self, tab_id: TabId, position: usize) -> ApplyOutcome {
        let Some(current_position) = self
            .tab_order
            .iter()
            .position(|candidate| *candidate == tab_id)
        else {
            return ApplyOutcome::IgnoredUnknownTarget;
        };
        let mut reordered = self.tab_order.clone();
        reordered.remove(current_position);
        let insertion_index = position.min(reordered.len());
        reordered.insert(insertion_index, tab_id);
        if reordered == self.tab_order {
            return ApplyOutcome::NoChange;
        }
        self.tab_order = reordered;
        ApplyOutcome::Applied
    }

    fn rename_tab(&mut self, tab_id: TabId, name: String) -> ApplyOutcome {
        let Some(tab) = self.tabs.get_mut(&tab_id) else {
            return ApplyOutcome::IgnoredUnknownTarget;
        };
        if tab.manual_name.as_ref() == Some(&name) {
            return ApplyOutcome::NoChange;
        }
        tab.manual_name = Some(name);
        ApplyOutcome::Applied
    }

    fn restore_automatic_name(&mut self, tab_id: TabId) -> ApplyOutcome {
        let Some(tab) = self.tabs.get_mut(&tab_id) else {
            return ApplyOutcome::IgnoredUnknownTarget;
        };
        if tab.manual_name.take().is_some() {
            ApplyOutcome::Applied
        } else {
            ApplyOutcome::NoChange
        }
    }

    fn focus_pane(
        &mut self,
        tab_id: TabId,
        pane_id: PaneId,
        directory: Option<DirectoryPath>,
        is_terminal: bool,
    ) -> ApplyOutcome {
        let Some(tab) = self.tabs.get(&tab_id) else {
            return ApplyOutcome::IgnoredUnknownTarget;
        };
        let has_other_location = self.tabs.iter().any(|(candidate_id, candidate)| {
            *candidate_id != tab_id && candidate.pane(pane_id).is_some()
        });
        let pane_exists = tab.pane(pane_id).is_some();
        let changed = has_other_location
            || tab.focused_pane != Some(pane_id)
            || tab.pane(pane_id).is_none_or(|pane| {
                pane.is_terminal != is_terminal
                    || directory
                        .as_ref()
                        .is_some_and(|value| pane.directory.as_ref() != Some(value))
            });
        if !changed {
            return ApplyOutcome::NoChange;
        }
        let moved_pane = self.take_pane_from_other_tabs(tab_id, pane_id);
        let pane_order = if pane_exists || moved_pane.is_some() {
            None
        } else {
            Some(self.take_pane_order())
        };
        let focus_order = self.take_focus_order();
        let Some(tab) = self.tabs.get_mut(&tab_id) else {
            return ApplyOutcome::IgnoredUnknownTarget;
        };
        if let Some(pane) = tab.panes.iter_mut().find(|pane| pane.id == pane_id) {
            if directory.is_some() {
                pane.directory = directory;
            }
            pane.is_terminal = is_terminal;
            pane.last_focused_order = focus_order;
        } else {
            let mut pane = moved_pane.unwrap_or(PaneState {
                id: pane_id,
                directory: None,
                is_terminal,
                created_order: pane_order.unwrap_or_default(),
                last_focused_order: 0,
            });
            if directory.is_some() {
                pane.directory = directory;
            }
            pane.is_terminal = is_terminal;
            pane.last_focused_order = focus_order;
            tab.panes.push(pane);
        }
        tab.focused_pane = Some(pane_id);
        self.retarget_agents_for_pane(pane_id, tab_id);
        ApplyOutcome::Applied
    }

    fn exit_pane(&mut self, _tab_id: TabId, pane_id: PaneId) -> ApplyOutcome {
        let pane_removed = self.tabs.values_mut().fold(false, |removed_any, tab| {
            let previous_len = tab.panes.len();
            tab.panes.retain(|pane| pane.id != pane_id);
            let removed = previous_len != tab.panes.len();
            if removed && tab.focused_pane == Some(pane_id) {
                tab.focused_pane = None;
            }
            removed_any || removed
        });
        let previous_agent_len = self.agents.len();
        self.agents.retain(|agent| agent.pane_id != pane_id);
        let agent_removed = previous_agent_len != self.agents.len();
        if pane_removed || agent_removed {
            ApplyOutcome::Applied
        } else {
            ApplyOutcome::IgnoredUnknownTarget
        }
    }

    fn take_pane_order(&mut self) -> u64 {
        self.next_pane_order = self.next_pane_order.saturating_add(1);
        self.next_pane_order
    }

    fn take_focus_order(&mut self) -> u64 {
        self.next_focus_order = self.next_focus_order.saturating_add(1);
        self.next_focus_order
    }

    fn take_pane_from_other_tabs(
        &mut self,
        target_tab_id: TabId,
        pane_id: PaneId,
    ) -> Option<PaneState> {
        let mut moved_pane = None;
        for (candidate_id, tab) in &mut self.tabs {
            if *candidate_id == target_tab_id {
                continue;
            }
            if let Some(position) = tab.panes.iter().position(|pane| pane.id == pane_id) {
                let pane = tab.panes.remove(position);
                if moved_pane.is_none() {
                    moved_pane = Some(pane);
                }
                if tab.focused_pane == Some(pane_id) {
                    tab.focused_pane = None;
                }
            }
        }
        moved_pane
    }

    fn retarget_agents_for_pane(&mut self, pane_id: PaneId, tab_id: TabId) {
        for agent in self
            .agents
            .iter_mut()
            .filter(|agent| agent.pane_id == pane_id)
        {
            agent.tab_id = tab_id;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApplyOutcome {
    Applied,
    NoChange,
    IgnoredStale,
    IgnoredUnknownTarget,
    IgnoredDuplicate,
}

impl ApplyOutcome {
    #[must_use]
    pub const fn changed(self) -> bool {
        matches!(self, Self::Applied)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reduction {
    state: State,
    outcome: ApplyOutcome,
}

impl Reduction {
    #[must_use]
    pub fn state(&self) -> &State {
        &self.state
    }

    #[must_use]
    pub const fn outcome(&self) -> ApplyOutcome {
        self.outcome
    }

    #[must_use]
    pub fn into_state(self) -> State {
        self.state
    }

    #[must_use]
    pub fn into_parts(self) -> (State, ApplyOutcome) {
        (self.state, self.outcome)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum StateError {
    #[error("unsupported event schema version {received}; supported version is {supported}")]
    UnsupportedEventVersion { received: u16, supported: u16 },
}
