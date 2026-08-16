use serde::{Deserialize, Serialize};

use crate::{AgentId, AgentStatus, DirectoryPath, Generation, PaneId, Sequence, TabId};

pub const EVENT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    #[serde(default = "current_schema_version")]
    pub version: u16,
    #[serde(flatten)]
    pub kind: EventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum EventKind {
    StatusChanged {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tab_id: Option<TabId>,
        pane_id: PaneId,
        #[serde(default)]
        agent_id: AgentId,
        status: AgentStatus,
        generation: Generation,
        sequence: Sequence,
    },
    DirectoryChanged {
        tab_id: TabId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pane_id: Option<PaneId>,
        directory: DirectoryPath,
    },
    TabCreated {
        tab_id: TabId,
        position: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        directory: Option<DirectoryPath>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        existing_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        manual_name: Option<String>,
    },
    TabClosed {
        tab_id: TabId,
    },
    TabMoved {
        tab_id: TabId,
        position: usize,
    },
    TabRenamed {
        tab_id: TabId,
        name: String,
    },
    AutomaticNamingRestored {
        tab_id: TabId,
    },
    PaneFocused {
        tab_id: TabId,
        pane_id: PaneId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        directory: Option<DirectoryPath>,
        #[serde(default = "default_true")]
        is_terminal: bool,
    },
    PaneExited {
        tab_id: TabId,
        pane_id: PaneId,
    },
}

impl Event {
    #[must_use]
    pub const fn new(kind: EventKind) -> Self {
        Self {
            version: EVENT_SCHEMA_VERSION,
            kind,
        }
    }

    #[must_use]
    pub fn status_changed(
        pane_id: PaneId,
        status: AgentStatus,
        generation: Generation,
        sequence: Sequence,
    ) -> Self {
        Self::status_changed_for(
            None,
            pane_id,
            AgentId::default(),
            status,
            generation,
            sequence,
        )
    }

    #[must_use]
    pub const fn status_changed_for(
        tab_id: Option<TabId>,
        pane_id: PaneId,
        agent_id: AgentId,
        status: AgentStatus,
        generation: Generation,
        sequence: Sequence,
    ) -> Self {
        Self::new(EventKind::StatusChanged {
            tab_id,
            pane_id,
            agent_id,
            status,
            generation,
            sequence,
        })
    }

    #[must_use]
    pub const fn tab_closed(tab_id: TabId) -> Self {
        Self::new(EventKind::TabClosed { tab_id })
    }
}

const fn current_schema_version() -> u16 {
    EVENT_SCHEMA_VERSION
}

const fn default_true() -> bool {
    true
}
