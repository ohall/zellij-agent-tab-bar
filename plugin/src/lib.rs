mod config;
mod directory;
mod event_log;
mod host_state;
mod render;
mod style;

use std::collections::{BTreeMap, VecDeque};

use config::PluginConfig;
use directory::{automatic_names, is_automatic_tab_name, selected_pane, PaneKey};
use event_log::{EventLog, LogLevel};
use host_state::HostState;
use render::{fit_item_to_width, visible_items, FittedItem, TabLabel};
use style::{finish_line, style_indicator, style_separator, style_tab};
use zellij_agent_shared::{
    resolve_tab_names, AgentStatus, ApplyOutcome, DirectoryPath, Event as AgentEvent,
    EventKind as AgentEventKind, NameSource, PaneId as AgentPaneId, State as AgentState,
    TabId as AgentTabId,
};
use zellij_tile::prelude::*;

const EVENT_PIPE_NAME: &str = "zja.events";
const PENDING_EVENT_CAPACITY: usize = 128;
const MAX_PIPE_PAYLOAD_BYTES: usize = 64 * 1024;

#[derive(Debug, Default)]
pub struct AgentTabBar {
    config: PluginConfig,
    host: HostState,
    agents: AgentState,
    mode: ModeInfo,
    event_log: EventLog,
    pending_events: VecDeque<AgentEvent>,
}

impl ZellijPlugin for AgentTabBar {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.config = PluginConfig::from_map(&configuration);
        set_selectable(false);
        request_permission(&[PermissionType::ReadApplicationState]);
        subscribe(&[
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::ModeUpdate,
            EventType::CwdChanged,
            EventType::PaneClosed,
            EventType::PermissionRequestResult,
            EventType::PluginConfigurationChanged,
            EventType::BeforeClose,
        ]);
        self.record(
            LogLevel::Info,
            "lifecycle",
            "plugin started; event-driven subscriptions active",
        );
    }

    fn update(&mut self, event: Event) -> bool {
        self.record(LogLevel::Trace, "zellij", event_name(&event));
        match event {
            Event::TabUpdate(tabs) => self.update_tabs(tabs),
            Event::PaneUpdate(panes) => {
                let previous_panes = self.host.terminal_pane_assignments();
                let changed = self.host.set_panes(panes, get_pane_cwd);
                let closed = self.sync_removed_panes(&previous_panes);
                let focused = self.sync_pane_events();
                let replayed = self.retry_pending_events();
                changed || closed || focused || replayed
            }
            Event::ModeUpdate(mode) => {
                let changed = mode != self.mode;
                self.mode = mode;
                changed
            }
            Event::CwdChanged(pane_id, cwd, _) => {
                if cwd.as_os_str().is_empty() {
                    self.record(LogLevel::Warn, "zellij", "ignored empty pane cwd");
                    return false;
                }
                let changed = self.host.cwd_changed(pane_id, cwd);
                self.sync_pane_events() || changed
            }
            Event::PaneClosed(pane_id) => self.close_pane(pane_id),
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.record(
                    LogLevel::Debug,
                    "permission",
                    "read-application-state granted",
                );
                let changed = self.host.seed_cwds(get_pane_cwd);
                let focused = self.sync_pane_events();
                let replayed = self.retry_pending_events();
                changed || focused || replayed
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                self.record(
                    LogLevel::Warn,
                    "permission",
                    "read-application-state denied; directory names use safe fallbacks",
                );
                false
            }
            Event::PluginConfigurationChanged(configuration) => {
                let updated = PluginConfig::from_map(&configuration);
                let changed = updated != self.config;
                self.config = updated;
                self.record(LogLevel::Debug, "configuration", "configuration updated");
                changed
            }
            Event::BeforeClose => {
                self.record(LogLevel::Info, "lifecycle", "plugin shutting down");
                false
            }
            _ => false,
        }
    }

    fn pipe(&mut self, message: PipeMessage) -> bool {
        if message.name != EVENT_PIPE_NAME {
            self.record(
                LogLevel::Trace,
                "pipe",
                format!("ignored unrelated pipe {}", message.name),
            );
            return false;
        }

        let Some(payload) = message.payload else {
            self.record(
                LogLevel::Trace,
                "pipe",
                format!("pipe {} closed", message.name),
            );
            return false;
        };
        self.record(
            LogLevel::Trace,
            "pipe",
            format!("received {} bytes on {}", payload.len(), message.name),
        );

        if payload.len() > MAX_PIPE_PAYLOAD_BYTES {
            self.record(
                LogLevel::Warn,
                "pipe",
                format!(
                    "discarded oversized payload ({} bytes; limit {})",
                    payload.len(),
                    MAX_PIPE_PAYLOAD_BYTES
                ),
            );
            return false;
        }

        match serde_json::from_str::<AgentEvent>(&payload) {
            Ok(event) if is_external_event(&event) => {
                let event = self.target_external_event(event);
                self.apply_pipe_event(event)
            }
            Ok(_) => {
                self.record(
                    LogLevel::Warn,
                    "pipe",
                    "discarded host-authoritative event from external pipe",
                );
                false
            }
            Err(error) => {
                self.record(
                    LogLevel::Warn,
                    "pipe",
                    format!("discarded malformed event: {error}"),
                );
                false
            }
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if rows == 0 || cols == 0 || !self.host.has_tabs() {
            return;
        }

        let labels = self.tab_labels();
        let items = visible_items(
            &labels,
            cols,
            &self.config.separator,
            self.config.show_index,
            self.config.max_tab_width,
        );
        let separator = style_separator(&self.config.separator, &self.mode, self.config.color);
        let output = items
            .iter()
            .map(|item| {
                match fit_item_to_width(
                    item,
                    cols,
                    self.config.show_index,
                    self.config.max_tab_width,
                ) {
                    FittedItem::Tab {
                        text,
                        status,
                        active,
                    } => style_tab(&text, status, active, &self.mode, self.config.color),
                    FittedItem::Indicator(text) => {
                        style_indicator(&text, &self.mode, self.config.color)
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(&separator);
        print!("{}", finish_line(&output, &self.mode, self.config.color));
    }
}

impl AgentTabBar {
    fn update_tabs(&mut self, mut tabs: Vec<TabInfo>) -> bool {
        tabs.sort_by_key(|tab| tab.position);
        if !tabs.is_empty() && !tabs.iter().any(|tab| tab.active) {
            self.record(
                LogLevel::Warn,
                "zellij",
                "tab update had no active tab; rendering first tab as fallback",
            );
        }

        let previous = self.host.tabs().to_vec();
        let shared_changed = self.sync_tab_events(&previous, &tabs);
        let host_changed = self.host.set_tabs(tabs);
        let cwd_changed = self.host.seed_cwds(get_pane_cwd);
        let pane_changed = self.sync_pane_events();
        let replayed = self.retry_pending_events();
        shared_changed || host_changed || cwd_changed || pane_changed || replayed
    }

    fn sync_tab_events(&mut self, previous: &[TabInfo], current: &[TabInfo]) -> bool {
        let mut changed = false;
        for closed in previous
            .iter()
            .filter(|old| !current.iter().any(|new| new.tab_id == old.tab_id))
        {
            changed |= self.apply_host_event(AgentEvent::tab_closed(closed.tab_id.into()));
        }

        for tab in current {
            let tab_id = AgentTabId::from(tab.tab_id);
            if let Some(old) = previous.iter().find(|old| old.tab_id == tab.tab_id) {
                if old.position != tab.position {
                    changed |= self.apply_host_event(AgentEvent::new(AgentEventKind::TabMoved {
                        tab_id,
                        position: tab.position,
                    }));
                }
                let was_automatic = is_automatic_tab_name(&old.name);
                let is_automatic = is_automatic_tab_name(&tab.name);
                if !was_automatic && is_automatic {
                    changed |= self.apply_host_event(AgentEvent::new(
                        AgentEventKind::AutomaticNamingRestored { tab_id },
                    ));
                } else if !is_automatic && old.name != tab.name {
                    changed |= self.apply_host_event(AgentEvent::new(AgentEventKind::TabRenamed {
                        tab_id,
                        name: tab.name.clone(),
                    }));
                }
            } else {
                let automatic = is_automatic_tab_name(&tab.name);
                changed |= self.apply_host_event(AgentEvent::new(AgentEventKind::TabCreated {
                    tab_id,
                    position: tab.position,
                    directory: None,
                    existing_name: (!tab.name.is_empty()).then(|| tab.name.clone()),
                    manual_name: (!automatic).then(|| tab.name.clone()),
                }));
            }
        }
        changed
    }

    fn sync_pane_events(&mut self) -> bool {
        let mut events = Vec::new();
        for candidate in self.host.directory_candidates() {
            let tab_id = AgentTabId::from(candidate.tab_id);
            if let Some(tab_cwd) = candidate.tab_cwd.as_deref() {
                events.push(AgentEvent::new(AgentEventKind::DirectoryChanged {
                    tab_id,
                    pane_id: None,
                    directory: DirectoryPath::from_path_lossy(tab_cwd),
                }));
            }
            for pane in &candidate.panes {
                if pane.key.is_plugin {
                    continue;
                }
                if let Some(cwd) = pane.cwd.as_deref() {
                    events.push(AgentEvent::new(AgentEventKind::DirectoryChanged {
                        tab_id,
                        pane_id: Some(AgentPaneId::from(pane.key.id)),
                        directory: DirectoryPath::from_path_lossy(cwd),
                    }));
                }
            }
            if let Some(pane) = selected_pane(&candidate) {
                let directory = pane.cwd.as_deref().map(DirectoryPath::from_path_lossy);
                events.push(AgentEvent::new(AgentEventKind::PaneFocused {
                    tab_id,
                    pane_id: AgentPaneId::from(pane.key.id),
                    directory,
                    is_terminal: !pane.key.is_plugin,
                }));
            }
        }
        let mut changed = false;
        for event in events {
            changed = self.apply_host_event(event) || changed;
        }
        changed
    }

    fn close_pane(&mut self, pane_id: PaneId) -> bool {
        let key = PaneKey::from(pane_id);
        let tab_id = self.host.tab_id_for_pane(key);
        let host_changed = self.host.pane_closed(pane_id);
        let agent_pane_id = AgentPaneId::from(key.id);
        let pending_len = self.pending_events.len();
        self.pending_events
            .retain(|event| !event_targets_pane(event, agent_pane_id));
        let pending_changed = pending_len != self.pending_events.len();
        let shared_tab_id = tab_id.map(AgentTabId::from).or_else(|| {
            self.agents
                .agents()
                .iter()
                .find(|agent| agent.pane_id() == agent_pane_id)
                .map(|agent| agent.tab_id())
        });
        let shared_changed = !key.is_plugin
            && shared_tab_id.is_some_and(|tab_id| {
                self.apply_host_event(AgentEvent::new(AgentEventKind::PaneExited {
                    tab_id,
                    pane_id: agent_pane_id,
                }))
            });
        host_changed || pending_changed || shared_changed
    }

    fn sync_removed_panes(&mut self, previous: &[(PaneKey, usize)]) -> bool {
        let removed: Vec<(PaneKey, usize)> = previous
            .iter()
            .filter(|(pane, _)| self.host.tab_id_for_pane(*pane).is_none())
            .copied()
            .collect();

        removed.into_iter().fold(false, |changed, (pane, tab_id)| {
            self.apply_host_event(AgentEvent::new(AgentEventKind::PaneExited {
                tab_id: AgentTabId::from(tab_id),
                pane_id: AgentPaneId::from(pane.id),
            })) || changed
        })
    }

    fn target_external_event(&mut self, event: AgentEvent) -> AgentEvent {
        let AgentEvent { version, kind } = event;
        match kind {
            AgentEventKind::StatusChanged {
                tab_id,
                pane_id,
                agent_id,
                status,
                generation,
                sequence,
            } => {
                let host_tab_id = u32::try_from(pane_id.get())
                    .ok()
                    .and_then(|pane_id| {
                        self.host.tab_id_for_pane(PaneKey {
                            id: pane_id,
                            is_plugin: false,
                        })
                    })
                    .map(AgentTabId::from);
                if host_tab_id.is_some() && tab_id.is_some() && host_tab_id != tab_id {
                    self.record(
                        LogLevel::Warn,
                        "pipe",
                        "explicit tab target disagreed with host pane ownership; used host tab",
                    );
                }
                AgentEvent {
                    version,
                    kind: AgentEventKind::StatusChanged {
                        tab_id: host_tab_id.or(tab_id),
                        pane_id,
                        agent_id,
                        status,
                        generation,
                        sequence,
                    },
                }
            }
            kind => AgentEvent { version, kind },
        }
    }

    fn apply_pipe_event(&mut self, event: AgentEvent) -> bool {
        let queued_event = event.clone();
        match self.agents.apply(event) {
            Ok(ApplyOutcome::IgnoredUnknownTarget) => {
                if self.pending_events.len() == PENDING_EVENT_CAPACITY {
                    self.pending_events.pop_front();
                    self.record(
                        LogLevel::Warn,
                        "pipe",
                        "pending-event queue full; dropped oldest event",
                    );
                }
                self.pending_events.push_back(queued_event);
                self.record(
                    LogLevel::Debug,
                    "pipe",
                    "queued event until its tab or pane is observed",
                );
                false
            }
            Ok(outcome) => {
                self.record(
                    LogLevel::Debug,
                    "pipe",
                    format!("accepted event with outcome {outcome:?}"),
                );
                outcome.changed()
            }
            Err(error) => {
                self.record(
                    LogLevel::Error,
                    "pipe",
                    format!("rejected incompatible event: {error}"),
                );
                false
            }
        }
    }

    fn apply_host_event(&mut self, event: AgentEvent) -> bool {
        match self.agents.apply(event) {
            Ok(outcome) => {
                self.record(
                    LogLevel::Debug,
                    "host-adapter",
                    format!("reduced event with outcome {outcome:?}"),
                );
                outcome.changed()
            }
            Err(error) => {
                self.record(
                    LogLevel::Error,
                    "host-adapter",
                    format!("failed to reduce event: {error}"),
                );
                false
            }
        }
    }

    fn retry_pending_events(&mut self) -> bool {
        let pending = std::mem::take(&mut self.pending_events);
        let mut changed = false;
        for event in pending {
            let event = self.target_external_event(event);
            let queued_event = event.clone();
            match self.agents.apply(event) {
                Ok(ApplyOutcome::IgnoredUnknownTarget) => {
                    if self.pending_events.len() < PENDING_EVENT_CAPACITY {
                        self.pending_events.push_back(queued_event);
                    }
                }
                Ok(outcome) => {
                    changed |= outcome.changed();
                    self.record(
                        LogLevel::Debug,
                        "pipe",
                        format!("replayed pending event with outcome {outcome:?}"),
                    );
                }
                Err(error) => self.record(
                    LogLevel::Error,
                    "pipe",
                    format!("discarded pending event: {error}"),
                ),
            }
        }
        changed
    }

    fn tab_labels(&self) -> Vec<TabLabel> {
        let fallback_names = automatic_names(&self.host.directory_candidates());
        let resolved_names = resolve_tab_names(&self.agents);
        self.host
            .tabs()
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let name = if self.config.auto_name && is_automatic_tab_name(&tab.name) {
                    resolved_names
                        .iter()
                        .find(|resolved| resolved.tab_id() == AgentTabId::from(tab.tab_id))
                        .map(|resolved| {
                            if resolved.source() == NameSource::ExistingName && !tab.name.is_empty()
                            {
                                tab.name.clone()
                            } else {
                                resolved.name().to_owned()
                            }
                        })
                        .or_else(|| fallback_names.get(&tab.tab_id).cloned())
                        .unwrap_or_else(|| format!("tab-{}", tab.tab_id))
                } else if tab.name.is_empty() {
                    format!("tab-{}", tab.tab_id)
                } else {
                    tab.name.clone()
                };
                let status = self
                    .agents
                    .aggregate_status_for_tab(AgentTabId::from(tab.tab_id));
                TabLabel {
                    position: tab.position,
                    active: tab.active
                        || (!self.host.tabs().iter().any(|tab| tab.active) && index == 0),
                    badge: self.badge(status).to_owned(),
                    status,
                    name,
                }
            })
            .collect()
    }

    fn badge(&self, status: AgentStatus) -> &str {
        match status {
            AgentStatus::Idle => &self.config.badge_idle,
            AgentStatus::Running => &self.config.badge_running,
            AgentStatus::Complete => &self.config.badge_complete,
            AgentStatus::Error => &self.config.badge_error,
        }
    }

    fn record(&mut self, level: LogLevel, source: &'static str, message: impl Into<String>) {
        let emit =
            self.config.debug || matches!(level, LogLevel::Info | LogLevel::Warn | LogLevel::Error);
        self.event_log.record(level, source, message, emit);
    }
}

fn is_external_event(event: &AgentEvent) -> bool {
    matches!(&event.kind, AgentEventKind::StatusChanged { .. })
}

fn event_targets_pane(event: &AgentEvent, pane_id: AgentPaneId) -> bool {
    matches!(
        &event.kind,
        AgentEventKind::StatusChanged {
            pane_id: target,
            ..
        } if *target == pane_id
    )
}

fn event_name(event: &Event) -> &'static str {
    match event {
        Event::TabUpdate(_) => "tab_update",
        Event::PaneUpdate(_) => "pane_update",
        Event::ModeUpdate(_) => "mode_update",
        Event::CwdChanged(_, _, _) => "cwd_changed",
        Event::PaneClosed(_) => "pane_closed",
        Event::PermissionRequestResult(_) => "permission_request_result",
        Event::PluginConfigurationChanged(_) => "plugin_configuration_changed",
        Event::BeforeClose => "before_close",
        _ => "unsubscribed_event",
    }
}

#[cfg(test)]
mod tests {
    use zellij_agent_shared::{AgentId, Generation, Sequence};

    use super::*;

    #[test]
    fn only_status_events_cross_the_external_pipe_boundary() {
        let status = AgentEvent::status_changed(
            AgentPaneId::new(1),
            AgentStatus::Running,
            Generation::new(1),
            Sequence::new(0),
        );
        let tab = AgentEvent::new(AgentEventKind::TabClosed {
            tab_id: AgentTabId::new(1),
        });

        assert!(is_external_event(&status));
        assert!(!is_external_event(&tab));
        assert!(event_targets_pane(&status, AgentPaneId::new(1)));
        assert!(!event_targets_pane(&status, AgentPaneId::new(2)));
    }

    fn plugin_with_tab() -> AgentTabBar {
        let mut plugin = AgentTabBar::default();
        let tabs = vec![TabInfo {
            tab_id: 7,
            position: 0,
            name: "Tab #1".to_owned(),
            active: true,
            ..TabInfo::default()
        }];
        let shared_changed = plugin.sync_tab_events(&[], &tabs);
        let host_changed = plugin.host.set_tabs(tabs);
        assert!(shared_changed || host_changed);
        plugin
    }

    #[test]
    fn a_piped_status_event_reaches_the_tab_badge() {
        let mut plugin = plugin_with_tab();

        let event = AgentEvent::status_changed_for(
            Some(AgentTabId::new(7)),
            AgentPaneId::new(3),
            AgentId::default(),
            AgentStatus::Complete,
            Generation::new(1),
            Sequence::new(0),
        );
        let payload = serde_json::to_string(&event).unwrap_or_default();
        assert!(!payload.is_empty());
        let changed = plugin.pipe(PipeMessage::new(
            PipeSource::Cli("test-pipe".to_owned()),
            EVENT_PIPE_NAME,
            &Some(payload),
            &None,
            false,
        ));
        assert!(changed);

        let labels = plugin.tab_labels();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].status, AgentStatus::Complete);
        assert_eq!(labels[0].badge, "✅");
    }

    #[test]
    fn rendering_after_a_status_event_does_not_panic() {
        let mut plugin = plugin_with_tab();

        plugin.render(1, 80);
    }

    #[test]
    fn automatic_name_resolves_from_a_focused_pane_cwd() {
        for tab_name in ["Tab #1", "Tab #3", "Tab #9"] {
            let mut plugin = AgentTabBar::default();
            let tabs = vec![TabInfo {
                tab_id: 7,
                position: 0,
                name: tab_name.to_owned(),
                active: true,
                ..TabInfo::default()
            }];
            plugin.sync_tab_events(&[], &tabs);
            plugin.host.set_tabs(tabs);

            let mut manifest = std::collections::HashMap::new();
            manifest.insert(
                0usize,
                vec![PaneInfo {
                    id: 3,
                    is_focused: true,
                    ..PaneInfo::default()
                }],
            );
            plugin
                .host
                .set_panes(PaneManifest { panes: manifest }, |pane_id| match pane_id {
                    PaneId::Terminal(3) => Ok(std::path::PathBuf::from("/Users/oakley/src/my-app")),
                    _ => Err("unknown".to_owned()),
                });
            let changed = plugin.sync_pane_events();
            assert!(changed);

            let labels = plugin.tab_labels();
            assert_eq!(labels.len(), 1);
            assert_eq!(labels[0].name, "my-app");
        }
    }
}
