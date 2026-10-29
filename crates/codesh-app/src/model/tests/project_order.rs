use super::*;
use codesh_app_core::settings::{AppLayout, ProjectOrder, Settings};

fn fixture() -> (AppState, [ProjectId; 4]) {
    let mut state = AppState::bootstrap().expect("state");
    let ids = ["repo10", "repo2b", "Repo1", "repo2a"].map(|name| {
        let id = state
            .add_project(ServerId::local(), std::env::temp_dir())
            .expect("project");
        state.rename_project(id, name).expect("name");
        id
    });
    let mut stored = serde_json::to_value(&state).expect("state JSON");
    for (index, project) in stored["projects"]
        .as_array_mut()
        .expect("projects")
        .iter_mut()
        .enumerate()
    {
        project["created_at"] =
            serde_json::json!({"secs_since_epoch": index, "nanos_since_epoch": 0});
    }
    let mut state: AppState = serde_json::from_value(stored).expect("dated state");
    state.move_project(ids[0], 4).expect("manual order");
    state.select_project(state.home().id).expect("home");
    (state, ids)
}

fn parent_ids(model: &AppModel) -> Vec<ProjectId> {
    model
        .listed_parents()
        .iter()
        .map(|project| project.id)
        .collect()
}

#[gpui::test]
fn sidebar_saved_project_sort_updates_every_layout_navigation_and_new_tab_picker(
    cx: &mut TestAppContext,
) {
    let (state, [ten, two_b, one, two_a]) = fixture();
    let home = state.home().id;
    let manual = [home, two_b, one, two_a, ten];
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.sidebar_expanded = true;
    boot.settings.appearance.worktree_recent = vec![two_a, two_b];
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    for layout in [
        AppLayout::ProjectFocused,
        AppLayout::TabFocused,
        AppLayout::AgentsFocused,
    ] {
        view.update(cx, |model, cx| model.set_layout(layout, cx));
        for (order, expected) in [
            (ProjectOrder::Name, [home, one, two_a, two_b, ten]),
            (ProjectOrder::NameDescending, [home, ten, two_b, two_a, one]),
            (ProjectOrder::RecentlyActive, [home, two_a, two_b, one, ten]),
            (ProjectOrder::DateAdded, [home, ten, two_b, one, two_a]),
            (ProjectOrder::Manual, manual),
        ] {
            view.update(cx, |model, cx| {
                model.appearance.sidebar_project_order = order;
                model.save_appearance(cx);
                cx.notify();
            });
            view.read_with(cx, |model, _| {
                assert_eq!(model.appearance.sidebar_project_order, order);
                assert_eq!(parent_ids(model), expected);
                assert_eq!(model.navigation_projects(), expected);
                assert_eq!(
                    model
                        .state
                        .projects()
                        .iter()
                        .map(|project| project.id)
                        .collect::<Vec<_>>(),
                    manual
                );
                assert_eq!(
                    Settings::load(&model.path.with_file_name("settings.toml"))
                        .expect("saved settings")
                        .appearance
                        .sidebar_project_order,
                    order
                );
            });
            view.update_in(cx, |model, window, cx| {
                model.toggle_new_tab_list(window, cx);
            });
            cx.run_until_parked();
            let tops: Vec<_> = expected
                .iter()
                .map(|id| {
                    cx.debug_bounds(format!("palette-row-{id}").leak())
                        .expect("project row")
                        .top()
                })
                .collect();
            assert!(tops.windows(2).all(|pair| pair[0] < pair[1]));
            view.update(cx, AppModel::dismiss_overlay);
        }
    }
}

#[gpui::test]
fn project_sort_recent_activity_follows_worktrees_and_keeps_workspace_and_server_groups(
    cx: &mut TestAppContext,
) {
    let mut state = AppState::bootstrap().expect("state");
    let local_home = state.home().id;
    let first_server = ServerId::new();
    let second_server = ServerId::new();
    let [first_home, ten, two, child, second_home, other] =
        std::array::from_fn(|_| ProjectId::new());
    state
        .apply_catalog(
            second_server,
            &servers::page(3, second_home, &[(other, "Alpha")]),
        )
        .expect("second server");
    let mut first_page = servers::page(2, first_home, &[(ten, "repo10"), (two, "Repo2")]);
    let mut worktree = servers::descriptor(child, false, "feature");
    worktree.parent_id = Some(ten);
    worktree.kind = Some(codesh_app_core::ProjectKind::Worktree);
    first_page.projects.push(worktree);
    state
        .apply_catalog(first_server, &first_page)
        .expect("first server");
    state.move_project(ten, 4).expect("manual order");
    let workspace = state.create_workspace("Work").expect("workspace");
    for id in [ten, two, other] {
        state
            .set_workspace_member(workspace, id, true)
            .expect("member");
    }
    let (boot, _requests, _remotes) = remote_boot(
        state,
        vec![
            servers::entry(first_server, "First"),
            servers::entry(second_server, "Second"),
        ],
    );
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        acknowledge_catalog(model, cx);
        model.appearance.sidebar_project_order = ProjectOrder::Name;
        assert_eq!(parent_ids(model), [local_home, two, ten, other]);
        model.appearance.sidebar_project_order = ProjectOrder::RecentlyActive;
        for id in [two, other, child] {
            model.select_project(id, cx);
            model.sync_tab_sidebar(cx);
        }
        assert_eq!(parent_ids(model), [local_home, ten, two, other]);
        model.appearance.layout = AppLayout::TabFocused;
        assert_eq!(
            model
                .sidebar_projects()
                .iter()
                .map(|project| project.id)
                .collect::<Vec<_>>(),
            [local_home, ten, child, two, other]
        );
        model
            .state
            .select_workspace(Some(workspace))
            .expect("workspace");
        model
            .state
            .set_workspace_member(workspace, two, false)
            .expect("filter");
        assert_eq!(parent_ids(model), [local_home, ten, other]);
        model.save_appearance(cx);
        model.appearance = Settings::load(&model.path.with_file_name("settings.toml"))
            .expect("saved settings")
            .appearance;
        assert_eq!(parent_ids(model), [local_home, ten, other]);
        model.state.select_remote_servers();
        assert_eq!(parent_ids(model), [first_home, second_home]);
        model.state.select_workspace(None).expect("all projects");
        for id in [ten, two, child, local_home] {
            model.select_project(id, cx);
            model.sync_tab_sidebar(cx);
        }
        assert_eq!(parent_ids(model), [local_home, ten, two, other]);
        assert!(
            model
                .appearance
                .worktree_recent
                .starts_with(&[local_home, child, two, ten])
        );
        first_page.projects.retain(|project| project.id != child);
        first_page.revision += 1;
        model.receive_catalog(first_server, Ok(first_page), cx);
        assert!(model.state.project(child).is_none());
        assert_eq!(parent_ids(model), [local_home, ten, two, other]);
        model.appearance = Settings::load(&model.path.with_file_name("settings.toml"))
            .expect("saved parent activity")
            .appearance;
        assert_eq!(parent_ids(model), [local_home, ten, two, other]);
    });
}

#[gpui::test]
fn project_sort_date_added_keeps_undated_projects_stable_ahead_of_new_projects(
    cx: &mut TestAppContext,
) {
    let (state, [ten, two_b, one, two_a]) = fixture();
    let mut stored = serde_json::to_value(&state).expect("state JSON");
    for project in stored["projects"].as_array_mut().expect("projects") {
        if [two_a.to_string(), one.to_string()]
            .iter()
            .any(|id| project["id"].as_str() == Some(id))
        {
            project
                .as_object_mut()
                .expect("project")
                .remove("created_at");
        }
    }
    let (boot, _requests) = stub_boot(serde_json::from_value(stored).expect("legacy state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        model.appearance.sidebar_project_order = ProjectOrder::DateAdded;
        assert_eq!(
            parent_ids(model),
            [model.state.home().id, one, two_a, ten, two_b]
        );
    });
}

#[gpui::test]
fn sidebar_host_filter_keeps_matching_projects(cx: &mut TestAppContext) {
    use codesh_app_core::settings::SidebarHost;

    let mut state = AppState::bootstrap().expect("state");
    let local = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("local");
    state.rename_project(local, "Local tools").expect("name");
    let server = ServerId::new();
    let other = ServerId::new();
    let [home, parent, child, other_home, other_project] =
        std::array::from_fn(|_| ProjectId::new());
    let mut page = servers::page(2, home, &[(parent, "Remote tools")]);
    let mut worktree = servers::descriptor(child, false, "Feature-ÄBC");
    worktree.parent_id = Some(parent);
    worktree.kind = Some(codesh_app_core::ProjectKind::Worktree);
    page.projects.push(worktree);
    state.apply_catalog(server, &page).expect("remote");
    state
        .apply_catalog(
            other,
            &servers::page(3, other_home, &[(other_project, "Other tools")]),
        )
        .expect("other");
    let (boot, _, _remotes) = remote_boot(
        state,
        vec![
            servers::entry(server, "Dev"),
            servers::entry(other, "Other"),
        ],
    );
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        acknowledge_catalog(model, cx);
        let local_home = model.state.home().id;
        for (host, expected) in [
            (
                SidebarHost::All,
                vec![local_home, local, parent, other_project],
            ),
            (SidebarHost::Local, vec![local_home, local]),
            (SidebarHost::Server(server), vec![parent]),
            (SidebarHost::Server(ServerId::new()), vec![]),
        ] {
            model.appearance.sidebar_host = host;
            assert_eq!(parent_ids(model), expected);
        }
        model.appearance.sidebar_host = SidebarHost::Server(server);
        model.save_appearance(cx);
        assert_eq!(
            Settings::load(&model.path.with_file_name("settings.toml"))
                .expect("saved")
                .appearance
                .sidebar_host,
            SidebarHost::Server(server)
        );
    });
}

#[gpui::test]
fn manual_order_lists_local_projects_before_remote_ones(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let local_home = state.home().id;
    let server = ServerId::new();
    let [remote_home, remote] = std::array::from_fn(|_| ProjectId::new());
    state
        .apply_catalog(
            server,
            &servers::page(2, remote_home, &[(remote, "Remote")]),
        )
        .expect("remote");
    let local = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("local");
    let (boot, _requests, _remotes) = remote_boot(state, vec![servers::entry(server, "Server")]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        acknowledge_catalog(model, cx);
        model.appearance.sidebar_project_order = ProjectOrder::Manual;
        assert_eq!(parent_ids(model), [local_home, local, remote]);
    });
}

#[gpui::test]
fn project_palette_search_switches_projects_and_keeps_root_hit_ids(cx: &mut TestAppContext) {
    use crate::views::command_palette::{Kind, PaletteEntry};

    let (mut state, [ten, _, _, _]) = fixture();
    for name in ["repo-extra-a", "repo-extra-b"] {
        let id = state
            .add_project(ServerId::local(), std::env::temp_dir())
            .expect("project");
        state.rename_project(id, name).expect("name");
    }
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        model.open_palette(PaletteEntry::Root, window, cx);
        let palette = model.palette_entity().expect("palette");
        palette.update(cx, |palette, cx| palette.set_query("repo10", cx));
    });
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        let palette = model.palette_entity().expect("palette");
        let items = palette.read(cx).items();
        let hit = items
            .iter()
            .find(|item| {
                item.kind
                    .as_ref()
                    .is_some_and(|kind| kind == Kind::Project.label())
            })
            .expect("project hit");
        assert_eq!(hit.id, format!("projects/{ten}"));
        assert!(!hit.highlights.is_empty());
        assert!(hit.subtitle.is_some());
    });
    view.update(cx, |model, cx| {
        let palette = model.palette_entity().expect("palette");
        palette.update(cx, |palette, cx| palette.set_query("repo", cx));
    });
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        let palette = model.palette_entity().expect("palette");
        let items = palette.read(cx).items();
        assert_eq!(
            items
                .iter()
                .filter(|item| item
                    .kind
                    .as_ref()
                    .is_some_and(|kind| kind == Kind::Project.label()))
                .count(),
            5
        );
        assert!(items.iter().any(|item| item.id == "see-all:projects"));
    });
    view.update(cx, |model, cx| {
        model.push_palette_page(PaletteEntry::Projects, Some("repo10"), cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        let palette = model.palette_entity().expect("palette");
        let items = palette.read(cx).items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, ten.to_string());
    });
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, ten);
        assert!(model.overlay.is_none());
    });
}

#[gpui::test]
fn workspace_palette_switches_and_marks_remote_servers_separately(cx: &mut TestAppContext) {
    use crate::views::command_palette::PaletteEntry;

    let mut state = AppState::bootstrap().expect("state");
    let workspace = state.create_workspace("Work").expect("workspace");
    let server = ServerId::new();
    let home = ProjectId::new();
    state
        .apply_catalog(server, &servers::page(2, home, &[]))
        .expect("remote catalog");
    state.select_remote_servers();
    let (boot, _requests, _remotes) = remote_boot(state, vec![servers::entry(server, "Dev")]);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        acknowledge_catalog(model, cx);
        model.open_palette(PaletteEntry::Workspaces, window, cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        let palette = model.palette_entity().expect("palette");
        let items = palette.read(cx).items();
        assert_eq!(
            items
                .iter()
                .map(|item| item.id.as_ref())
                .collect::<Vec<_>>(),
            ["all", "remote_servers", &workspace.to_string()]
        );
        assert_eq!(items.iter().filter(|item| item.current).count(), 1);
        assert!(items[1].current);
    });
    cx.simulate_input("Work");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.state.active_workspace().map(|workspace| workspace.id),
            Some(workspace)
        );
        assert!(!model.state.remote_servers_selected());
        assert!(model.overlay.is_none());
    });
}
