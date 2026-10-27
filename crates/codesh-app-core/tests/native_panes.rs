use codesh_app_core::{AppState, Layout, PaneContent, PaneId, store};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn assert_equal_row(layout: &Layout, count: usize) {
    let rects = layout.rects();
    assert_eq!(rects.len(), count);
    #[expect(clippy::cast_precision_loss, reason = "a handful of panes")]
    let share = 1.0 / count as f32;
    for (index, (_, rect)) in rects.into_iter().enumerate() {
        #[expect(clippy::cast_precision_loss, reason = "a handful of panes")]
        let x = index as f32 * share;
        assert!((rect[0] - x).abs() < 1e-5, "{rect:?}");
        assert!((rect[2] - share).abs() < 1e-5, "{rect:?}");
    }
    assert!(layout.validate().is_ok());
}

fn content(state: &AppState, pane: PaneId) -> Option<(&str, &PaneContent)> {
    state
        .current_project()
        .tabs
        .iter()
        .flat_map(|tab| &tab.panes)
        .find(|candidate| candidate.id == pane)
        .map(|pane| (pane.title.as_str(), &pane.content))
}

#[test]
fn editor_and_diff_open_as_splits_in_the_single_tab() -> Result {
    let mut state = AppState::bootstrap()?;
    let project =
        state.add_project(codesh_app_core::ServerId::local(), std::env::current_dir()?)?;
    state.select_project(project)?;
    let tab = state.open_terminal_tab(project)?;
    let terminal = state.window().active_pane.ok_or("terminal")?;
    let editor = state.open_editor_pane(project, "src/main.rs")?;
    let diff = state.open_diff_pane(project, "src/main.rs", false)?;
    let staged = state.open_diff_pane(project, "src/main.rs", true)?;

    assert_eq!(state.current_project().tabs.len(), 1);
    assert_eq!(state.current_project().tabs[0].id, tab);
    assert_eq!(state.window().active_pane, Some(staged));
    assert_eq!(
        state.current_project().tabs[0].layout.leaves(),
        vec![terminal, editor, diff, staged]
    );
    assert_equal_row(&state.current_project().tabs[0].layout, 4);
    assert_eq!(
        content(&state, editor),
        Some((
            "main.rs",
            &PaneContent::Editor {
                path: "src/main.rs".into()
            }
        ))
    );
    assert_eq!(
        content(&state, diff).map(|(title, _)| title),
        Some("main.rs")
    );
    assert_eq!(
        content(&state, staged),
        Some((
            "main.rs (staged)",
            &PaneContent::Diff {
                path: "src/main.rs".into(),
                staged: true
            }
        ))
    );
    Ok(())
}

#[test]
fn an_empty_project_gets_its_one_tab_from_the_first_native_pane() -> Result {
    let mut state = AppState::bootstrap()?;
    let project =
        state.add_project(codesh_app_core::ServerId::local(), std::env::current_dir()?)?;
    state.select_project(project)?;
    let editor = state.open_editor_pane(project, "README.md")?;
    assert_eq!(state.current_project().tabs.len(), 1);
    assert_eq!(state.current_project().tabs[0].layout, Layout::Leaf(editor));
    assert_eq!(state.window().active_pane, Some(editor));
    Ok(())
}

#[test]
fn reopening_the_same_file_focuses_the_existing_pane() -> Result {
    let mut state = AppState::bootstrap()?;
    let project =
        state.add_project(codesh_app_core::ServerId::local(), std::env::current_dir()?)?;
    state.select_project(project)?;
    state.open_terminal_tab(project)?;
    let editor = state.open_editor_pane(project, "a.rs")?;
    let diff = state.open_diff_pane(project, "a.rs", false)?;
    let other = state.open_editor_pane(project, "b.rs")?;
    assert_eq!(state.window().active_pane, Some(other));

    assert_eq!(state.open_editor_pane(project, "a.rs")?, editor);
    assert_eq!(state.window().active_pane, Some(editor));
    assert_eq!(state.open_editor_pane(project, "./a.rs")?, editor);
    assert_eq!(state.open_diff_pane(project, "a.rs", false)?, diff);
    assert_eq!(state.window().active_pane, Some(diff));
    assert_ne!(state.open_diff_pane(project, "a.rs", true)?, diff);
    assert_eq!(state.current_project().tabs[0].panes.len(), 5);
    assert_equal_row(&state.current_project().tabs[0].layout, 5);
    Ok(())
}

#[test]
fn paths_outside_the_project_are_rejected() -> Result {
    let mut state = AppState::bootstrap()?;
    let project =
        state.add_project(codesh_app_core::ServerId::local(), std::env::current_dir()?)?;
    state.select_project(project)?;
    for path in ["", "/etc/hosts", "../secret", "a/../../b"] {
        assert!(state.open_editor_pane(project, path).is_err(), "{path}");
        assert!(
            state.open_diff_pane(project, path, false).is_err(),
            "{path}"
        );
    }
    assert!(state.current_project().tabs.is_empty());
    Ok(())
}

#[test]
fn native_panes_survive_a_save_and_load() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("desktop-state.json");
    let mut state = AppState::bootstrap()?;
    let project =
        state.add_project(codesh_app_core::ServerId::local(), std::env::current_dir()?)?;
    state.select_project(project)?;
    state.open_terminal_tab(project)?;
    let editor = state.open_editor_pane(project, "src/lib.rs")?;
    let diff = state.open_diff_pane(project, "src/lib.rs", true)?;
    store::save(&path, &state)?;

    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let panes = &saved["projects"][1]["tabs"][0]["panes"];
    assert_eq!(
        panes[1]["content"],
        serde_json::json!({"type": "editor", "path": "src/lib.rs"})
    );
    assert_eq!(
        panes[2]["content"],
        serde_json::json!({"type": "diff", "path": "src/lib.rs", "staged": true})
    );

    let mut restored = store::load(&path)?;
    assert_eq!(restored, state);
    assert_eq!(restored.open_editor_pane(project, "src/lib.rs")?, editor);
    assert_eq!(restored.open_diff_pane(project, "src/lib.rs", true)?, diff);
    Ok(())
}

#[test]
fn bundled_extension_panes_migrate_and_others_close() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("desktop-state.json");
    std::fs::write(
        &path,
        include_str!("fixtures/desktop-state-extension-panes.json"),
    )?;
    let state = store::load(&path)?;

    let tab = &state.current_project().tabs[0];
    let panes: Vec<_> = tab
        .panes
        .iter()
        .map(|pane| (pane.title.as_str(), &pane.content))
        .collect();
    assert_eq!(
        panes,
        [
            ("Terminal", &PaneContent::Terminal { session: None }),
            (
                "main.rs",
                &PaneContent::Editor {
                    path: "src/main.rs".into()
                }
            ),
            (
                "README.md",
                &PaneContent::Diff {
                    path: "README.md".into(),
                    staged: false
                }
            ),
        ]
    );
    assert_eq!(
        tab.layout.leaves(),
        tab.panes.iter().map(|pane| pane.id).collect::<Vec<_>>()
    );
    assert_equal_row(&tab.layout, 3);
    let active = state.window().active_pane.ok_or("active pane")?;
    assert!(tab.layout.contains(active));

    store::save(&path, &state)?;
    assert!(!std::fs::read_to_string(&path)?.contains("webview"));
    assert_eq!(store::load(&path)?, state);
    Ok(())
}
