use super::*;
use codesh_app_core::settings::Settings;
use codesh_core::shortcuts::ShortcutId;
use codesh_protocol::{ExecResult, FileContent, FilesAction, FilesReply};

use crate::model::native::Stub;
use crate::views::command_palette::{Kind, PaletteEntry, RECENT_LIMIT};

fn page(view: &Entity<AppModel>, cx: &mut VisualTestContext) -> Option<PaletteEntry> {
    view.read_with(cx, AppModel::palette_page)
}

/// The ids of the open palette's rows, in display order.
fn rows(view: &Entity<AppModel>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |model, cx| {
        model.palette_entity().map_or_else(Vec::new, |palette| {
            palette
                .read(cx)
                .items()
                .iter()
                .map(|item| item.id.to_string())
                .collect()
        })
    })
}

/// A server whose project lists two files and reads every file as `text`.
fn stub_project(text: &'static str) -> Rc<Stub> {
    Rc::new(Stub {
        files: Box::new(move |request| match request.action {
            FilesAction::Read(path) => Ok(FilesReply::Content(FileContent {
                path,
                content: text.into(),
                size: text.len() as u64,
            })),
            FilesAction::List(_) => Ok(FilesReply::Entries(Vec::new())),
            _ => Ok(FilesReply::Done),
        }),
        exec: Box::new(|_| {
            Ok(ExecResult {
                stdout: "src/main.rs\nREADME.md\nsrc/main.rs\n".into(),
                stderr: String::new(),
                exit_code: 0,
                timed_out: false,
                truncated: false,
                cancelled: false,
            })
        }),
    })
}

#[gpui::test]
fn root_search_ranks_commands_and_opens_a_file_hit_in_an_editor(cx: &mut TestAppContext) {
    let (state, project) = git::registered_current_project();
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        model.servers.local.connection = ConnectionState::Ready;
        model.native.stub = Some(stub_project("fn main() {}\n"));
    });
    cx.simulate_keystrokes(ShortcutId::ToggleCommandPalette.definition().keys[0]);
    cx.run_until_parked();
    cx.simulate_input("go to file");
    cx.run_until_parked();
    assert_eq!(
        rows(&view, cx).first().map(String::as_str),
        Some(ShortcutId::GoToFile.name()),
        "the command whose title matches leads"
    );

    view.update(cx, |model, cx| {
        let palette = model.palette_entity().expect("palette");
        palette.update(cx, |palette, cx| palette.set_query("main.rs", cx));
    });
    cx.run_until_parked();
    let file = view
        .read_with(cx, |model, cx| {
            let palette = model.palette_entity()?;
            let palette = palette.read(cx);
            palette
                .items()
                .into_iter()
                .find(|item| {
                    item.title == "main.rs"
                        && item
                            .kind
                            .as_ref()
                            .is_some_and(|kind| kind == Kind::File.label())
                })
                .map(|item| item.id.clone())
        })
        .expect("a file hit");
    assert_eq!(file, "files/src/main.rs");
    view.update(cx, |model, cx| {
        let palette = model.palette_entity().expect("palette");
        palette.update(cx, |palette, cx| {
            let page = palette.page().clone();
            assert!(palette.select(&page, &file, cx));
        });
    });
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.current_project().id, project);
        let pane = model.active_pane().expect("editor pane");
        assert!(
            model.editors.contains_key(&pane),
            "the file opened in an editor"
        );
    });
}

#[gpui::test]
fn go_to_file_opens_over_root_and_escape_walks_back_then_closes(cx: &mut TestAppContext) {
    let (state, _) = git::registered_current_project();
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        model.servers.local.connection = ConnectionState::Ready;
        model.native.stub = Some(stub_project("text"));
    });
    let go_to_file = ShortcutId::GoToFile.definition().keys[0];

    cx.simulate_keystrokes(go_to_file);
    cx.run_until_parked();
    assert_eq!(page(&view, cx), Some(PaletteEntry::Files));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(
        page(&view, cx),
        Some(PaletteEntry::Root),
        "escape returns to root"
    );
    cx.simulate_input("main.rs");
    cx.run_until_parked();
    assert!(rows(&view, cx).contains(&"files/src/main.rs".to_owned()));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, _| model.overlay.is_none()));

    cx.simulate_keystrokes(go_to_file);
    cx.run_until_parked();
    assert_eq!(page(&view, cx), Some(PaletteEntry::Files));
    cx.simulate_keystrokes(go_to_file);
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |model, _| model.overlay.is_none()),
        "the shortcut toggles its page"
    );
}

#[gpui::test]
fn a_run_command_leads_the_suggestions_and_is_remembered(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let sidebar = ShortcutId::ToggleSidebar.name();
    let older: Vec<String> = [
        ShortcutId::NewTab,
        ShortcutId::NewHomeTab,
        ShortcutId::ToggleFiles,
        ShortcutId::ToggleSourceControl,
        ShortcutId::ToggleAgentsPanel,
        ShortcutId::Minimize,
        ShortcutId::Zoom,
        ShortcutId::OpenSettings,
    ]
    .iter()
    .map(|id| id.name().to_owned())
    .collect();
    view.update(cx, |model, _| {
        model.appearance.palette_recent.clone_from(&older);
    });
    let palette = ShortcutId::ToggleCommandPalette.definition().keys[0];

    cx.simulate_keystrokes(palette);
    cx.run_until_parked();
    cx.simulate_input("toggle sidebar");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();

    view.read_with(cx, |model, _| {
        let recent = &model.appearance.palette_recent;
        assert_eq!(recent.len(), RECENT_LIMIT, "the list keeps its limit");
        assert_eq!(recent[0], sidebar);
        assert_eq!(recent[1..], older[..RECENT_LIMIT - 1]);
        let saved = Settings::load(&model.path.with_file_name("settings.toml"))
            .expect("saved settings")
            .appearance
            .palette_recent;
        assert_eq!(&saved, recent, "recent commands persist");
    });

    cx.simulate_keystrokes(palette);
    cx.run_until_parked();
    assert_eq!(
        rows(&view, cx).first().map(String::as_str),
        Some(sidebar),
        "the last command run leads the suggestions"
    );
}

#[gpui::test]
fn root_file_and_branch_with_the_same_name_run_distinct_targets(cx: &mut TestAppContext) {
    use codesh_protocol::{GitAction, GitBranch, GitReply, GitRequest};
    let (state, project) = git::registered_current_project();
    let (boot, requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        model.servers.local.connection = ConnectionState::Ready;
        model.native.stub = Some(stub_project("text"));
    });
    cx.simulate_keystrokes(ShortcutId::ToggleCommandPalette.definition().keys[0]);
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        model.git.projects.entry(project).or_default().disconnect();
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::Branches,
            },
            Ok(GitReply::Branches(vec![GitBranch {
                name: "src/main.rs".into(),
                current: false,
                checked_out: false,
                default: false,
            }])),
            cx,
        );
    });
    cx.simulate_input("src/main.rs");
    cx.run_until_parked();
    let ids = rows(&view, cx);
    assert_eq!(
        ids.iter().filter(|id| *id == "files/src/main.rs").count(),
        1
    );
    assert!(ids.contains(&"branches/branch:src/main.rs".to_owned()));
    for id in ["files/src/main.rs", "branches/branch:src/main.rs"] {
        view.update(cx, |model, cx| {
            let palette = model.palette_entity().expect("palette");
            palette.update(cx, |palette, cx| {
                assert!(palette.select("root", id, cx));
            });
        });
        requests.try_iter().for_each(drop);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        if id.starts_with("files/") {
            assert!(view.read_with(cx, |model, _| {
                model
                    .active_pane()
                    .is_some_and(|pane| model.editors.contains_key(&pane))
            }));
            cx.simulate_keystrokes(ShortcutId::ToggleCommandPalette.definition().keys[0]);
            cx.run_until_parked();
            cx.simulate_input("src/main.rs");
            cx.run_until_parked();
        } else {
            assert!(requests.try_iter().any(|(_, work)| matches!(work,
                Work::Git(GitRequest { action: GitAction::SwitchBranch(name), .. }) if name == "src/main.rs")));
        }
    }
}
