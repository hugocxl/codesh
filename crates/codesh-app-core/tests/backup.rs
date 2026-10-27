#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use codesh_app_core::{AppState, Direction, ServerId};

#[test]
fn portable_restore_clears_sessions_and_queues_projects_before_catalog_reconciliation() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = AppState::bootstrap().unwrap();
    let project = state
        .add_project(ServerId::local(), directory.path().into())
        .unwrap();
    state.open_terminal_tab(project).unwrap();
    let pane = state.project(project).unwrap().tabs[0].panes[0].id;
    state.split_pane(pane, Direction::Right).unwrap();
    state
        .set_pane_session(pane, codesh_protocol::SessionId::new(42))
        .unwrap();
    let portable = state.configuration_backup();
    assert!(portable.session_references(ServerId::local()).is_empty());
    assert!(portable.project_intents(ServerId::local()).is_empty());
    let current = AppState::bootstrap().unwrap();
    let mut restored = portable.restore_configuration(&current).unwrap();
    assert_ne!(restored.project(project).unwrap().tabs[0].panes[0].id, pane);
    let creates = restored
        .project_intents(ServerId::local())
        .iter()
        .filter(|intent| matches!(intent.mutation, codesh_protocol::ProjectMutation::Create(_)))
        .count();
    assert_eq!(creates, 1);
    assert_eq!(restored.project(project).unwrap().tabs[0].panes.len(), 2);
    restored
        .apply_catalog(
            ServerId::local(),
            &codesh_protocol::CatalogPage {
                server: codesh_protocol::ServerIdentity::new(),
                revision: 0,
                home: current.home().id,
                projects: vec![current.home().descriptor()],
                next: None,
                legacy_home: None,
            },
        )
        .unwrap();
    assert!(restored.project(project).is_some());
}
