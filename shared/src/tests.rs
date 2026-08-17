use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;
use unicode_width::UnicodeWidthStr;

use crate::{
    basename, render_tab_bar, resolve_tab_names, truncate_to_width, AgentId, AgentStatus,
    ApplyOutcome, Config, DirectoryPath, Event, EventKind, Generation, NameSource, PaneId,
    RenderModel, Sequence, State, StateError, TabId, EVENT_SCHEMA_VERSION,
};

fn apply(state: &mut State, kind: EventKind) -> ApplyOutcome {
    state
        .apply(Event::new(kind))
        .unwrap_or_else(|error| panic!("valid event failed: {error}"))
}

fn create_tab(state: &mut State, tab_id: u64, position: usize, directory: Option<&str>) {
    let outcome = apply(
        state,
        EventKind::TabCreated {
            tab_id: TabId::from(tab_id),
            position,
            directory: directory.map(DirectoryPath::from),
            existing_name: None,
            manual_name: None,
        },
    );
    assert_eq!(outcome, ApplyOutcome::Applied);
}

fn focus_pane(state: &mut State, tab_id: u64, pane_id: u64, directory: Option<&str>) {
    let outcome = apply(
        state,
        EventKind::PaneFocused {
            tab_id: TabId::from(tab_id),
            pane_id: PaneId::from(pane_id),
            directory: directory.map(DirectoryPath::from),
            is_terminal: true,
        },
    );
    assert_eq!(outcome, ApplyOutcome::Applied);
}

#[test]
fn extracts_portable_basenames_without_panicking() {
    let cases = [
        ("/workspace/api", Some("api")),
        ("/workspace/api///", Some("api")),
        (r"C:\workspace\api", Some("api")),
        (r"C:\workspace\api\\", Some("api")),
        ("relative/path", Some("path")),
        ("résumé/数据", Some("数据")),
        ("/", Some("/")),
        ("\\", Some("\\")),
        ("", None),
    ];

    for (path, expected) in cases {
        assert_eq!(basename(path).as_deref(), expected, "path: {path:?}");
    }
}

#[test]
fn duplicate_basenames_use_the_shortest_unique_parent_suffix() {
    let mut state = State::default();
    create_tab(&mut state, 1, 0, Some("/workspace/team/api"));
    create_tab(&mut state, 2, 1, Some("/srv/team/api"));
    create_tab(&mut state, 3, 2, Some("/opt/web"));

    let names = resolve_tab_names(&state);
    assert_eq!(names[0].name(), "workspace/team/api");
    assert_eq!(names[1].name(), "srv/team/api");
    assert_eq!(names[2].name(), "web");
}

#[test]
fn identical_directories_receive_stable_tab_suffixes() {
    let mut state = State::default();
    create_tab(&mut state, 7, 0, Some("/workspace/api"));
    create_tab(&mut state, 9, 1, Some("/workspace/api"));

    let names = resolve_tab_names(&state);
    assert_eq!(names[0].name(), "api #7");
    assert_eq!(names[1].name(), "api #9");
}

#[test]
fn manual_names_override_directories_and_are_never_disambiguated() {
    let mut state = State::default();
    create_tab(&mut state, 1, 0, Some("/workspace/api"));
    create_tab(&mut state, 2, 1, Some("/srv/api"));
    assert_eq!(
        apply(
            &mut state,
            EventKind::TabRenamed {
                tab_id: TabId::from(1_u64),
                name: "api".to_owned(),
            },
        ),
        ApplyOutcome::Applied
    );

    let names = resolve_tab_names(&state);
    assert_eq!(names[0].name(), "api");
    assert_eq!(names[0].source(), NameSource::Manual);
    assert_eq!(names[1].name(), "srv/api");

    assert_eq!(
        apply(
            &mut state,
            EventKind::AutomaticNamingRestored {
                tab_id: TabId::from(1_u64),
            },
        ),
        ApplyOutcome::Applied
    );
    let restored = resolve_tab_names(&state);
    assert_eq!(restored[0].name(), "workspace/api");
    assert_eq!(restored[0].source(), NameSource::TabDirectory);
}

#[test]
fn directory_resolution_obeys_pane_priority() {
    let mut state = State::default();
    create_tab(&mut state, 1, 0, Some("/tab/root"));
    apply(
        &mut state,
        EventKind::DirectoryChanged {
            tab_id: TabId::from(1_u64),
            pane_id: Some(PaneId::from(10_u64)),
            directory: DirectoryPath::from("/first/terminal"),
        },
    );
    focus_pane(&mut state, 1, 11, Some("/last/focused"));
    focus_pane(&mut state, 1, 12, Some("/now/focused"));

    let focused = resolve_tab_names(&state);
    assert_eq!(focused[0].name(), "focused");
    assert_eq!(focused[0].source(), NameSource::FocusedPane);

    focus_pane(&mut state, 1, 13, None);
    let last_focused = resolve_tab_names(&state);
    assert_eq!(last_focused[0].name(), "focused");
    assert_eq!(last_focused[0].source(), NameSource::LastFocusedPane);

    apply(
        &mut state,
        EventKind::PaneExited {
            tab_id: TabId::from(1_u64),
            pane_id: PaneId::from(12_u64),
        },
    );
    apply(
        &mut state,
        EventKind::PaneExited {
            tab_id: TabId::from(1_u64),
            pane_id: PaneId::from(11_u64),
        },
    );
    let first_terminal = resolve_tab_names(&state);
    assert_eq!(first_terminal[0].name(), "terminal");
    assert_eq!(first_terminal[0].source(), NameSource::FirstTerminalPane);
}

#[test]
fn truncation_is_grapheme_and_display_width_safe() {
    assert_eq!(truncate_to_width("e\u{301}clair", 2, "…"), "e\u{301}…");
    assert_eq!(truncate_to_width("你好世界", 5, "…"), "你好…");
    assert_eq!(truncate_to_width("unchanged", 20, "…"), "unchanged");
    assert_eq!(truncate_to_width("anything", 0, "…"), "");

    let family = "👨‍👩‍👧‍👦 family";
    let result = truncate_to_width(family, 3, "…");
    assert!(UnicodeWidthStr::width(result.as_str()) <= 3);
    assert!(!result.ends_with('\u{200d}'));
}

#[test]
fn renderer_respects_the_available_display_width() {
    let model = RenderModel {
        tabs: vec![
            crate::RenderTab {
                tab_id: TabId::from(1_u64),
                position: 1,
                name: "数据服务".to_owned(),
                status: AgentStatus::Running,
                is_active: true,
            },
            crate::RenderTab {
                tab_id: TabId::from(2_u64),
                position: 2,
                name: "infrastructure".to_owned(),
                status: AgentStatus::Error,
                is_active: false,
            },
        ],
    };
    let output = render_tab_bar(&model, &Config::default(), 18);
    assert!(UnicodeWidthStr::width(output.text.as_str()) <= 18);
    assert!(output.truncated);
    assert_eq!(output.visible_tabs, 2);
}

#[test]
fn renderer_removes_terminal_control_characters_from_all_configurable_text() {
    let model = RenderModel {
        tabs: vec![crate::RenderTab {
            tab_id: TabId::from(1_u64),
            position: 1,
            name: "api\u{1b}[31m\nspoof".to_owned(),
            status: AgentStatus::Running,
            is_active: true,
        }],
    };
    let mut config = Config::default();
    config.badges.running = "\u{1b}[1m●\r".to_owned();
    config.layout.separator = "\t".to_owned();
    config.layout.truncation_marker = "\u{1b}…".to_owned();

    let output = render_tab_bar(&model, &config, 80);
    assert!(!output.text.contains('\u{1b}'));
    assert!(!output.text.chars().any(char::is_control));
}

#[test]
fn aggregation_keeps_concurrent_running_agents_visible() {
    let mut state = State::default();
    create_tab(&mut state, 1, 0, None);
    focus_pane(&mut state, 1, 10, Some("/api"));
    focus_pane(&mut state, 1, 11, Some("/api"));

    let complete = Event::status_changed_for(
        None,
        PaneId::from(10_u64),
        AgentId::from("codex"),
        AgentStatus::Complete,
        Generation::from(50_u64),
        Sequence::from(2_u64),
    );
    let running = Event::status_changed_for(
        None,
        PaneId::from(11_u64),
        AgentId::from("claude"),
        AgentStatus::Running,
        Generation::from(1_u64),
        Sequence::from(1_u64),
    );
    assert_eq!(state.apply(complete), Ok(ApplyOutcome::Applied));
    assert_eq!(state.apply(running), Ok(ApplyOutcome::Applied));
    assert_eq!(
        state.aggregate_status_for_tab(TabId::from(1_u64)),
        AgentStatus::Running
    );

    assert_eq!(
        AgentStatus::aggregate([AgentStatus::Idle, AgentStatus::Complete, AgentStatus::Error,]),
        AgentStatus::Error
    );
}

#[test]
fn stale_sequences_and_generations_are_rejected_per_agent() {
    let mut state = State::default();
    create_tab(&mut state, 1, 0, None);
    focus_pane(&mut state, 1, 10, None);
    let update = |status: AgentStatus, generation: u64, sequence: u64| {
        Event::status_changed_for(
            None,
            PaneId::from(10_u64),
            AgentId::from("codex"),
            status,
            Generation::from(generation),
            Sequence::from(sequence),
        )
    };

    assert_eq!(
        state.apply(update(AgentStatus::Running, 4, 7)),
        Ok(ApplyOutcome::Applied)
    );
    assert_eq!(
        state.apply(update(AgentStatus::Complete, 4, 7)),
        Ok(ApplyOutcome::IgnoredStale)
    );
    assert_eq!(
        state.apply(update(AgentStatus::Complete, 4, 6)),
        Ok(ApplyOutcome::IgnoredStale)
    );
    assert_eq!(
        state.apply(update(AgentStatus::Complete, 3, 100)),
        Ok(ApplyOutcome::IgnoredStale)
    );
    assert_eq!(
        state.aggregate_status_for_tab(TabId::from(1_u64)),
        AgentStatus::Running
    );
    assert_eq!(
        state.apply(update(AgentStatus::Complete, 5, 0)),
        Ok(ApplyOutcome::Applied)
    );
    assert_eq!(
        state.aggregate_status_for_tab(TabId::from(1_u64)),
        AgentStatus::Complete
    );
}

#[test]
fn reorder_preserves_identity_and_close_removes_dependent_state() {
    let mut state = State::default();
    create_tab(&mut state, 1, 0, None);
    create_tab(&mut state, 2, 1, None);
    focus_pane(&mut state, 1, 10, None);
    assert_eq!(
        state.apply(Event::status_changed(
            PaneId::from(10_u64),
            AgentStatus::Error,
            Generation::from(1_u64),
            Sequence::from(1_u64),
        )),
        Ok(ApplyOutcome::Applied)
    );

    assert_eq!(
        apply(
            &mut state,
            EventKind::TabMoved {
                tab_id: TabId::from(1_u64),
                position: 1,
            },
        ),
        ApplyOutcome::Applied
    );
    assert_eq!(state.tab_order(), &[TabId::from(2_u64), TabId::from(1_u64)]);
    assert_eq!(
        state.aggregate_status_for_tab(TabId::from(1_u64)),
        AgentStatus::Error
    );

    assert_eq!(
        state.apply(Event::tab_closed(TabId::from(1_u64))),
        Ok(ApplyOutcome::Applied)
    );
    assert!(state.tab(TabId::from(1_u64)).is_none());
    assert!(state.agents().is_empty());
    assert_eq!(state.pane_tab(PaneId::from(10_u64)), None);
}

#[test]
fn repeated_host_snapshot_events_are_idempotent() {
    let mut state = State::default();
    create_tab(&mut state, 1, 0, Some("/workspace/api"));
    focus_pane(&mut state, 1, 10, Some("/workspace/api"));

    assert_eq!(
        apply(
            &mut state,
            EventKind::DirectoryChanged {
                tab_id: TabId::from(1_u64),
                pane_id: Some(PaneId::from(10_u64)),
                directory: DirectoryPath::from("/workspace/api"),
            },
        ),
        ApplyOutcome::NoChange
    );
    assert_eq!(
        apply(
            &mut state,
            EventKind::PaneFocused {
                tab_id: TabId::from(1_u64),
                pane_id: PaneId::from(10_u64),
                directory: Some(DirectoryPath::from("/workspace/api")),
                is_terminal: true,
            },
        ),
        ApplyOutcome::NoChange
    );
    assert_eq!(
        apply(
            &mut state,
            EventKind::TabMoved {
                tab_id: TabId::from(1_u64),
                position: 0,
            },
        ),
        ApplyOutcome::NoChange
    );
    assert_eq!(
        apply(
            &mut state,
            EventKind::TabRenamed {
                tab_id: TabId::from(1_u64),
                name: "api".to_owned(),
            },
        ),
        ApplyOutcome::Applied
    );
    assert_eq!(
        apply(
            &mut state,
            EventKind::TabRenamed {
                tab_id: TabId::from(1_u64),
                name: "api".to_owned(),
            },
        ),
        ApplyOutcome::NoChange
    );
}

#[test]
fn pane_exit_removes_agents_even_when_pane_was_never_registered() {
    let mut state = State::default();
    create_tab(&mut state, 1, 0, None);
    assert_eq!(
        state.apply(Event::status_changed_for(
            Some(TabId::from(1_u64)),
            PaneId::from(99_u64),
            AgentId::from("codex"),
            AgentStatus::Running,
            Generation::from(1_u64),
            Sequence::from(1_u64),
        )),
        Ok(ApplyOutcome::Applied)
    );
    assert_eq!(state.agents().len(), 1);

    assert_eq!(
        apply(
            &mut state,
            EventKind::PaneExited {
                tab_id: TabId::from(1_u64),
                pane_id: PaneId::from(99_u64),
            },
        ),
        ApplyOutcome::Applied
    );
    assert!(state.agents().is_empty());
    assert_eq!(
        state.aggregate_status_for_tab(TabId::from(1_u64)),
        AgentStatus::Idle
    );
}

#[test]
fn moving_a_pane_between_tabs_preserves_status_and_unique_membership() {
    let mut state = State::default();
    create_tab(&mut state, 1, 0, None);
    create_tab(&mut state, 2, 1, None);
    focus_pane(&mut state, 1, 10, Some("/workspace/api"));
    assert_eq!(
        state.apply(Event::status_changed(
            PaneId::from(10_u64),
            AgentStatus::Running,
            Generation::from(1_u64),
            Sequence::from(1_u64),
        )),
        Ok(ApplyOutcome::Applied)
    );

    assert_eq!(
        apply(
            &mut state,
            EventKind::PaneFocused {
                tab_id: TabId::from(2_u64),
                pane_id: PaneId::from(10_u64),
                directory: Some(DirectoryPath::from("/workspace/api")),
                is_terminal: true,
            },
        ),
        ApplyOutcome::Applied
    );
    assert!(state
        .tab(TabId::from(1_u64))
        .is_some_and(|tab| tab.pane(PaneId::from(10_u64)).is_none()));
    assert!(state
        .tab(TabId::from(2_u64))
        .is_some_and(|tab| tab.pane(PaneId::from(10_u64)).is_some()));
    assert_eq!(
        state.pane_tab(PaneId::from(10_u64)),
        Some(TabId::from(2_u64))
    );
    assert_eq!(
        state.aggregate_status_for_tab(TabId::from(1_u64)),
        AgentStatus::Idle
    );
    assert_eq!(
        state.aggregate_status_for_tab(TabId::from(2_u64)),
        AgentStatus::Running
    );
    assert_eq!(state.agents()[0].tab_id(), TabId::from(2_u64));
}

#[test]
fn reduce_is_immutable_and_versions_are_checked() {
    let state = State::default();
    let event = Event::new(EventKind::TabCreated {
        tab_id: TabId::from(1_u64),
        position: 0,
        directory: None,
        existing_name: None,
        manual_name: None,
    });
    let reduction = state
        .reduce(&event)
        .unwrap_or_else(|error| panic!("current event version must reduce: {error}"));
    assert!(state.tab_order().is_empty());
    assert_eq!(reduction.state().tab_order(), &[TabId::from(1_u64)]);

    let incompatible = Event {
        version: EVENT_SCHEMA_VERSION.saturating_add(1),
        kind: EventKind::TabClosed {
            tab_id: TabId::from(1_u64),
        },
    };
    assert!(matches!(
        state.reduce(&incompatible),
        Err(StateError::UnsupportedEventVersion { .. })
    ));
}

#[test]
fn events_and_state_round_trip_through_json() {
    let event = Event::status_changed_for(
        None,
        PaneId::from(42_u64),
        AgentId::default(),
        AgentStatus::Running,
        Generation::from(3_u64),
        Sequence::from(9_u64),
    );
    let json = serde_json::to_string(&event)
        .unwrap_or_else(|error| panic!("event must serialize: {error}"));
    assert!(json.contains(r#""type":"status_changed""#));
    assert!(!json.contains("tab_id"));
    let decoded: Event = serde_json::from_str(&json)
        .unwrap_or_else(|error| panic!("event must deserialize: {error}"));
    assert_eq!(decoded, event);

    let missing_version = r#"{"type":"tab_closed","tab_id":1}"#;
    let decoded: Event = serde_json::from_str(missing_version)
        .unwrap_or_else(|error| panic!("missing version uses v1 compatibility default: {error}"));
    assert_eq!(decoded.version, EVENT_SCHEMA_VERSION);

    let mut state = State::default();
    create_tab(&mut state, 1, 0, Some("/workspace/api"));
    let encoded_state = serde_json::to_string(&state)
        .unwrap_or_else(|error| panic!("state must serialize: {error}"));
    let decoded_state: State = serde_json::from_str(&encoded_state)
        .unwrap_or_else(|error| panic!("state must deserialize: {error}"));
    assert_eq!(decoded_state, state);
}

#[test]
fn flat_config_map_is_tolerant_and_validated() {
    let values = BTreeMap::from([
        ("badge_running".to_owned(), "▶".to_owned()),
        ("layout_max_tab_width".to_owned(), "18".to_owned()),
        ("layout_show_index".to_owned(), "false".to_owned()),
        ("behavior_auto_name".to_owned(), "no".to_owned()),
        ("theme_color".to_owned(), "off".to_owned()),
        ("debug".to_owned(), "yes".to_owned()),
        ("future.setting".to_owned(), "accepted".to_owned()),
    ]);
    let config =
        Config::from_map(&values).unwrap_or_else(|error| panic!("valid map must parse: {error}"));
    assert_eq!(config.badges.running, "▶");
    assert_eq!(config.layout.max_name_width, 18);
    assert!(!config.layout.show_index);
    assert!(!config.behavior.automatic_naming);
    assert!(!config.theme.use_color);
    assert!(config.debug.enabled);
}

#[test]
fn malformed_flat_config_values_keep_safe_defaults() {
    let values = BTreeMap::from([
        ("badge_error".to_owned(), String::new()),
        ("layout_max_tab_width".to_owned(), "9999".to_owned()),
        ("layout_show_index".to_owned(), "sometimes".to_owned()),
        ("theme_color".to_owned(), "maybe".to_owned()),
    ]);
    let config = Config::from_map(&values)
        .unwrap_or_else(|error| panic!("flat plugin values must degrade safely: {error}"));
    assert_eq!(config.badges.error, "❌");
    assert_eq!(config.layout.max_name_width, 32);
    assert!(config.layout.show_index);
    assert!(config.theme.use_color);
}

#[derive(Debug, Clone)]
enum Operation {
    Create(u8, u8, String),
    Close(u8),
    Move(u8, u8),
    Rename(u8, String),
    Restore(u8),
    Directory(u8, u8, String),
    Focus(u8, u8, String, bool),
    Exit(u8, u8),
    Status(u8, u8, u8, AgentStatus, u8, u8),
}

fn text_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 0..16)
        .prop_map(|characters| characters.into_iter().collect())
}

fn operation_strategy() -> impl Strategy<Value = Operation> {
    let status = prop_oneof![
        Just(AgentStatus::Idle),
        Just(AgentStatus::Running),
        Just(AgentStatus::Complete),
        Just(AgentStatus::Error),
    ];
    prop_oneof![
        (0_u8..12, 0_u8..16, text_strategy())
            .prop_map(|(tab, position, path)| { Operation::Create(tab, position, path) }),
        (0_u8..12).prop_map(Operation::Close),
        (0_u8..12, 0_u8..16).prop_map(|(tab, position)| Operation::Move(tab, position)),
        (0_u8..12, text_strategy()).prop_map(|(tab, name)| Operation::Rename(tab, name)),
        (0_u8..12).prop_map(Operation::Restore),
        (0_u8..12, 0_u8..24, text_strategy())
            .prop_map(|(tab, pane, path)| Operation::Directory(tab, pane, path)),
        (0_u8..12, 0_u8..24, text_strategy(), any::<bool>())
            .prop_map(|(tab, pane, path, terminal)| Operation::Focus(tab, pane, path, terminal)),
        (0_u8..12, 0_u8..24).prop_map(|(tab, pane)| Operation::Exit(tab, pane)),
        (0_u8..12, 0_u8..24, 0_u8..4, status, 0_u8..8, 0_u8..16).prop_map(
            |(tab, pane, agent, status, generation, sequence)| {
                Operation::Status(tab, pane, agent, status, generation, sequence)
            },
        ),
    ]
}

impl Operation {
    fn into_event(self) -> Event {
        match self {
            Self::Create(tab, position, path) => Event::new(EventKind::TabCreated {
                tab_id: TabId::from(u64::from(tab)),
                position: usize::from(position),
                directory: Some(DirectoryPath::from(path)),
                existing_name: None,
                manual_name: None,
            }),
            Self::Close(tab) => Event::new(EventKind::TabClosed {
                tab_id: TabId::from(u64::from(tab)),
            }),
            Self::Move(tab, position) => Event::new(EventKind::TabMoved {
                tab_id: TabId::from(u64::from(tab)),
                position: usize::from(position),
            }),
            Self::Rename(tab, name) => Event::new(EventKind::TabRenamed {
                tab_id: TabId::from(u64::from(tab)),
                name,
            }),
            Self::Restore(tab) => Event::new(EventKind::AutomaticNamingRestored {
                tab_id: TabId::from(u64::from(tab)),
            }),
            Self::Directory(tab, pane, path) => Event::new(EventKind::DirectoryChanged {
                tab_id: TabId::from(u64::from(tab)),
                pane_id: Some(PaneId::from(u64::from(pane))),
                directory: DirectoryPath::from(path),
            }),
            Self::Focus(tab, pane, path, is_terminal) => Event::new(EventKind::PaneFocused {
                tab_id: TabId::from(u64::from(tab)),
                pane_id: PaneId::from(u64::from(pane)),
                directory: Some(DirectoryPath::from(path)),
                is_terminal,
            }),
            Self::Exit(tab, pane) => Event::new(EventKind::PaneExited {
                tab_id: TabId::from(u64::from(tab)),
                pane_id: PaneId::from(u64::from(pane)),
            }),
            Self::Status(tab, pane, agent, status, generation, sequence) => {
                Event::status_changed_for(
                    Some(TabId::from(u64::from(tab))),
                    PaneId::from(u64::from(pane)),
                    AgentId::from(format!("agent-{agent}")),
                    status,
                    Generation::from(u64::from(generation)),
                    Sequence::from(u64::from(sequence)),
                )
            }
        }
    }
}

proptest! {
    #[test]
    fn arbitrary_event_sequences_never_panic_or_corrupt_tab_identity(
        operations in prop::collection::vec(operation_strategy(), 0..300),
        width in 0_usize..160,
    ) {
        let mut state = State::default();
        for operation in operations {
            let result = state.apply(operation.into_event());
            prop_assert!(result.is_ok());

            let unique_order: BTreeSet<_> = state.tab_order().iter().copied().collect();
            prop_assert_eq!(unique_order.len(), state.tab_order().len());
            prop_assert!(state.tab_order().iter().all(|tab_id| state.tab(*tab_id).is_some()));
            prop_assert_eq!(state.tabs_in_order().count(), state.tab_order().len());
            prop_assert!(state.agents().iter().all(|agent| state.tab(agent.tab_id()).is_some()));
            let mut unique_panes = BTreeSet::new();
            prop_assert!(state
                .tabs_in_order()
                .flat_map(|tab| tab.panes())
                .all(|pane| unique_panes.insert(pane.id())));

            let resolved = resolve_tab_names(&state);
            prop_assert_eq!(resolved.len(), state.tab_order().len());
            prop_assert!(resolved
                .iter()
                .zip(state.tab_order())
                .all(|(name, tab_id)| name.tab_id() == *tab_id));

            let model = RenderModel::from_state(&state, state.tab_order().first().copied());
            let output = render_tab_bar(&model, &Config::default(), width);
            prop_assert!(UnicodeWidthStr::width(output.text.as_str()) <= width);
            prop_assert!(!output.text.chars().any(char::is_control));
        }
    }

    #[test]
    fn truncation_always_returns_valid_text_within_width(
        input in text_strategy(),
        marker in text_strategy(),
        width in 0_usize..80,
    ) {
        let truncated = truncate_to_width(&input, width, &marker);
        prop_assert!(UnicodeWidthStr::width(truncated.as_str()) <= width);
        prop_assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }
}
