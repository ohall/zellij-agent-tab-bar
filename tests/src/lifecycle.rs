use zellij_agent_shared::{
    resolve_tab_names, AgentId, AgentStatus, ApplyOutcome, DirectoryPath, Event, EventKind,
    Generation, PaneId, Sequence, State, TabId,
};

fn apply(state: &mut State, kind: EventKind) -> ApplyOutcome {
    match state.apply(Event::new(kind)) {
        Ok(outcome) => outcome,
        Err(error) => panic!("valid lifecycle event failed: {error}"),
    }
}

#[test]
fn complete_tab_and_agent_lifecycle_remains_consistent() {
    let api_tab = TabId::from(10_u64);
    let infra_tab = TabId::from(20_u64);
    let api_pane = PaneId::from(100_u32);
    let infra_pane = PaneId::from(200_u32);
    let mut state = State::default();

    assert_eq!(
        apply(
            &mut state,
            EventKind::TabCreated {
                tab_id: api_tab,
                position: 0,
                directory: None,
                existing_name: Some("Tab #1".to_owned()),
                manual_name: None,
            },
        ),
        ApplyOutcome::Applied
    );
    assert_eq!(
        apply(
            &mut state,
            EventKind::TabCreated {
                tab_id: infra_tab,
                position: 1,
                directory: None,
                existing_name: Some("Tab #2".to_owned()),
                manual_name: None,
            },
        ),
        ApplyOutcome::Applied
    );
    assert_eq!(
        apply(
            &mut state,
            EventKind::PaneFocused {
                tab_id: api_tab,
                pane_id: api_pane,
                directory: Some(DirectoryPath::from("/workspace/api")),
                is_terminal: true,
            },
        ),
        ApplyOutcome::Applied
    );
    assert_eq!(
        apply(
            &mut state,
            EventKind::PaneFocused {
                tab_id: infra_tab,
                pane_id: infra_pane,
                directory: Some(DirectoryPath::from("/srv/api")),
                is_terminal: true,
            },
        ),
        ApplyOutcome::Applied
    );

    let names = resolve_tab_names(&state);
    assert_eq!(names[0].name(), "workspace/api");
    assert_eq!(names[1].name(), "srv/api");

    assert_eq!(
        state
            .apply(Event::status_changed_for(
                None,
                api_pane,
                AgentId::from("codex"),
                AgentStatus::Running,
                Generation::from(1_u64),
                Sequence::from(1_u64),
            ))
            .unwrap_or_else(|error| panic!("running status applies: {error}")),
        ApplyOutcome::Applied
    );
    assert_eq!(
        state.aggregate_status_for_tab(api_tab),
        AgentStatus::Running
    );
    assert_eq!(
        state
            .apply(Event::status_changed_for(
                None,
                api_pane,
                AgentId::from("codex"),
                AgentStatus::Complete,
                Generation::from(1_u64),
                Sequence::from(0_u64),
            ))
            .unwrap_or_else(|error| panic!("stale status is safely classified: {error}")),
        ApplyOutcome::IgnoredStale
    );
    assert_eq!(
        state.aggregate_status_for_tab(api_tab),
        AgentStatus::Running
    );
    assert_eq!(
        state
            .apply(Event::status_changed_for(
                None,
                api_pane,
                AgentId::from("codex"),
                AgentStatus::Complete,
                Generation::from(1_u64),
                Sequence::from(2_u64),
            ))
            .unwrap_or_else(|error| panic!("completion status applies: {error}")),
        ApplyOutcome::Applied
    );
    assert_eq!(
        state.aggregate_status_for_tab(api_tab),
        AgentStatus::Complete
    );

    assert_eq!(
        apply(
            &mut state,
            EventKind::TabRenamed {
                tab_id: api_tab,
                name: "control-plane".to_owned(),
            },
        ),
        ApplyOutcome::Applied
    );
    assert_eq!(resolve_tab_names(&state)[0].name(), "control-plane");
    assert_eq!(
        apply(
            &mut state,
            EventKind::AutomaticNamingRestored { tab_id: api_tab },
        ),
        ApplyOutcome::Applied
    );
    assert_eq!(resolve_tab_names(&state)[0].name(), "workspace/api");

    assert_eq!(
        apply(
            &mut state,
            EventKind::TabMoved {
                tab_id: infra_tab,
                position: 0,
            },
        ),
        ApplyOutcome::Applied
    );
    assert_eq!(state.tab_order(), [infra_tab, api_tab]);
    assert_eq!(
        state.aggregate_status_for_tab(api_tab),
        AgentStatus::Complete
    );

    assert_eq!(
        apply(&mut state, EventKind::TabClosed { tab_id: api_tab }),
        ApplyOutcome::Applied
    );
    assert!(state.tab(api_tab).is_none());
    assert!(state.agents().is_empty());
}

#[test]
fn a_new_generation_can_report_failure_after_success() {
    let tab_id = TabId::from(1_u64);
    let pane_id = PaneId::from(2_u32);
    let mut state = State::default();
    apply(
        &mut state,
        EventKind::TabCreated {
            tab_id,
            position: 0,
            directory: None,
            existing_name: None,
            manual_name: None,
        },
    );
    apply(
        &mut state,
        EventKind::PaneFocused {
            tab_id,
            pane_id,
            directory: None,
            is_terminal: true,
        },
    );

    for (generation, status) in [(1_u64, AgentStatus::Complete), (2_u64, AgentStatus::Error)] {
        let result = state.apply(Event::status_changed_for(
            None,
            pane_id,
            AgentId::default(),
            status,
            Generation::from(generation),
            Sequence::from(0_u64),
        ));
        assert!(result.is_ok());
    }

    assert_eq!(state.aggregate_status_for_tab(tab_id), AgentStatus::Error);
}
