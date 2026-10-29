mod ai;
mod app_socket;
mod askpass;
mod backup;
mod boot;
mod cli_install;
mod diagnostics;
mod favicon;
#[cfg(any(target_os = "linux", test))]
mod instance;
mod model;
mod navigation;
mod opener;
#[allow(
    dead_code,
    reason = "The palette's Open Project page is rebuilt on this engine"
)]
mod picker;
mod profiler;
mod proxy;
mod remote_install;
mod repository_actions;
mod server;
mod theme;
mod updater;
mod views {
    pub(crate) mod agent_sessions;
    pub(crate) mod agents_panel;
    pub(crate) mod banners;
    pub(crate) mod browser;
    pub(crate) mod cached;
    pub(crate) mod codesh;
    pub(crate) mod command_palette;
    pub(crate) mod confirm;
    pub(crate) mod diff_pane;
    pub(crate) mod disconnected;
    pub(crate) mod editor;
    pub(crate) mod files_panel;
    pub(crate) mod font_picker;
    pub(crate) mod git;
    pub(crate) mod git_panel;
    pub(crate) mod menu;
    pub(crate) mod native_panels;
    pub(crate) mod overlays;
    pub(crate) mod project_editor;
    pub(crate) mod project_menu;
    pub(crate) mod proxy;
    pub(crate) mod remote_servers;
    pub(crate) mod server_menu;
    pub(crate) mod server_status;
    pub(crate) mod servers_tab;
    pub(crate) mod settings;
    pub(crate) mod shortcut_hints;
    pub(crate) mod sidebar;
    pub(crate) mod splits;
    pub(crate) mod status_bar;
    pub(crate) mod tab_activity;
    pub(crate) mod tab_menu;
    pub(crate) mod tab_sidebar;
    pub(crate) mod tab_strip;
    pub(crate) mod titlebar;
    pub(crate) mod updates;
    pub(crate) mod terminal {
        pub(crate) mod clipboard;
        pub(crate) mod colors;
        pub(crate) mod cursor;
        pub(crate) mod element;
        pub(crate) mod find;
        pub(crate) mod ime;
        pub(crate) mod input;
        pub(crate) mod links;
        pub(crate) mod pane;
        pub(crate) mod scroll;
        pub(crate) mod scrollbar;
        pub(crate) mod selection;
        pub(crate) mod typography;
    }
    pub(crate) mod voice;
    pub(crate) mod workspace;
}

use std::cell::Cell;
use std::io::{self, Write};
use std::process::ExitCode;
use std::rc::Rc;

use gpui::{
    App, AppContext, Application, Bounds, Menu, MenuItem, OsAction, SystemMenuType,
    TitlebarOptions, WindowBackgroundAppearance, WindowBounds, WindowOptions, point, px, size,
};

use model::AppModel;
use views::workspace::{
    AddProject, CheckForUpdates, ClosePane, CloseTab, DecreaseFontSize, EndAllSessionsAndQuit,
    EqualizeSplits, Find, FindNext, FindPrevious, FocusPaneDown, FocusPaneLeft, FocusPaneRight,
    FocusPaneUp, HideApp, HideOthers, IncreaseFontSize, InstallCommandLineTool, Minimize,
    NewBrowserTab, NewHomeTab, NewTab, NewTabInProject, NewWorktree, NextProject, NextPrompt,
    NextTab, OpenSettings, PreviousProject, PreviousPrompt, PreviousTab, Quit, SelectCommandOutput,
    SelectProject, SelectTab, ShowAll, SplitDown, SplitRight, SwitchWorktree, ToggleAgentSessions,
    ToggleAgentsPanel, ToggleCommandPalette, ToggleFullScreen, ToggleSidebar, ToggleThemePicker,
    ToggleVoiceRecording, ToggleZoomPane, Zoom, bind_keys,
};
use views::workspace::{FindInFiles, GoToFile, ToggleFiles, ToggleSourceControl};

fn main() -> ExitCode {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == "--build-info")
    {
        return match serde_json::to_writer(
            io::stdout().lock(),
            &codesh_protocol::BuildInfo::current(),
        ) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == proxy::TUNNEL)
    {
        return proxy::run_tunnel(std::env::args_os().skip(2));
    }
    run()
}

fn run_askpass() -> Option<ExitCode> {
    let socket = std::env::var_os(askpass::SOCKET)?;
    let prompt = std::env::args_os()
        .nth(1)
        .map(|prompt| prompt.to_string_lossy().into_owned())
        .unwrap_or_default();
    Some(askpass::run(&socket, &prompt))
}

#[allow(
    clippy::too_many_lines,
    reason = "Startup wiring reads best in one place; Codesh adds a debug settings hook"
)]
fn run() -> ExitCode {
    #[cfg(target_os = "macos")]
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == codesh_client::local::host::FLAG)
    {
        return codesh_client::local::host::run(std::env::args_os().skip(2));
    }
    if let Some(exit) = run_askpass() {
        return exit;
    }
    #[cfg(target_os = "linux")]
    let _installation_lease = match codesh_core::executable::current_path()
        .and_then(|executable| codesh_client::local::bundle::acquire_runtime(&executable))
    {
        Ok(lease) => lease,
        Err(error) => {
            let _ = writeln!(io::stderr(), "codesh-app: {error}");
            return ExitCode::FAILURE;
        }
    };
    let (folders, opened_folders) = async_channel::unbounded();
    #[cfg(target_os = "linux")]
    let _instance = match instance::claim(std::env::args_os().skip(1), &folders) {
        Ok(Some(instance)) => instance,
        Ok(None) => return ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "codesh-app: {error}");
            return ExitCode::FAILURE;
        }
    };
    codesh_ui::browser::polybrowser::init();
    let profile = profiler::init();
    let boot = {
        let _span = profiler::span(profiler::Metric::BootLoad);
        match boot::Boot::load() {
            Ok(boot) => boot,
            Err(error) => {
                let _ = writeln!(io::stderr(), "codesh-app: {error}");
                return ExitCode::FAILURE;
            }
        }
    };
    #[cfg(target_os = "macos")]
    codesh_ui::native_scroll::use_overlay_scrollers();
    let application = Application::new().with_assets(codesh_ui::assets::Assets);
    application.on_open_urls(move |urls| {
        let _ = folders.try_send(urls);
    });
    let exit_code = Rc::new(Cell::new(ExitCode::SUCCESS));
    let startup_exit_code = exit_code.clone();
    application.run(move |cx: &mut App| {
        if let Some(profile) = profile {
            profile.finish_on_quit(cx);
        }
        bind_keys(&boot.settings.keymap, cx);
        cx.on_action(|_: &HideApp, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
        cx.set_menus(menus());
        let bounds = restored_bounds(&boot, cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(640.0), px(400.0))),
            window_background: WindowBackgroundAppearance::Transparent,
            #[cfg(target_os = "linux")]
            window_decorations: Some(gpui::WindowDecorations::Client),
            // Empty titlebar space explicitly starts native movement, not tab presses.
            is_movable: !cfg!(target_os = "macos"),
            titlebar: Some(TitlebarOptions {
                title: Some(app_name().into()),
                appears_transparent: true,
                traffic_light_position: Some(views::codesh::style::traffic_lights()),
            }),
            ..window_options()
        };
        if let Err(error) = cx.open_window(options, |window, cx| {
            #[cfg(target_os = "linux")]
            window.set_window_title(app_name());
            let model = cx.new(|cx| AppModel::new(boot, window, cx));
            model.update(cx, |_, cx| AppModel::open_folders_from(opened_folders, cx));
            model.update(cx, |model, cx| {
                model.start_app_socket(window, cx);
                model.open_requested_settings(cx);
            });
            let configuration = model.downgrade();
            cx.on_action(move |_: &views::workspace::ReloadConfiguration, cx| {
                let _ = configuration.update(cx, AppModel::reload_configuration);
            });
            let weak = model.downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                if weak.update(cx, AppModel::quit).is_err() {
                    cx.quit();
                }
                false
            });
            model
        }) {
            let _ = writeln!(io::stderr(), "codesh-app: could not open window: {error}");
            startup_exit_code.set(ExitCode::FAILURE);
            cx.quit();
        }
        cx.activate(true);
    });
    exit_code.get()
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Saved coordinates are checked against the GPUI pixel range"
)]
fn restored_bounds(boot: &boot::Boot, cx: &App) -> Bounds<gpui::Pixels> {
    if let Some(bounds) = boot.state.window().bounds
        && [bounds.x, bounds.y, bounds.width, bounds.height]
            .into_iter()
            .all(|value| value.abs() <= f64::from(f32::MAX))
    {
        return Bounds::new(
            point(px(bounds.x as f32), px(bounds.y as f32)),
            size(px(bounds.width as f32), px(bounds.height as f32)),
        );
    }
    let [width, height] = boot.settings.window.default_size;
    Bounds::centered(None, size(px(width), px(height)), cx)
}

fn app_name() -> &'static str {
    codesh_core::release::Channel::current().app_name()
}

fn window_options() -> WindowOptions {
    WindowOptions {
        #[cfg(target_os = "linux")]
        app_id: Some(
            codesh_core::release::Channel::current()
                .bundle_identifier()
                .into(),
        ),
        ..WindowOptions::default()
    }
}

#[allow(clippy::too_many_lines, reason = "One native menu registration list")]
fn menus() -> Vec<Menu> {
    use codesh_core::capability::Feature;
    use codesh_ui::text_input;
    let backends = model::capabilities::backends();
    let mut window_items = vec![
        MenuItem::action("Minimize", Minimize),
        MenuItem::action("Zoom", Zoom),
        MenuItem::separator(),
        MenuItem::action("Split Right", SplitRight),
        MenuItem::action("Split Down", SplitDown),
        MenuItem::action("Focus Pane Left", FocusPaneLeft),
        MenuItem::action("Focus Pane Right", FocusPaneRight),
        MenuItem::action("Focus Pane Up", FocusPaneUp),
        MenuItem::action("Focus Pane Down", FocusPaneDown),
        MenuItem::action("Toggle Pane Zoom", ToggleZoomPane),
        MenuItem::action("Equalize Splits", EqualizeSplits),
        MenuItem::action("Close Pane", ClosePane),
        MenuItem::action("Close Tab", CloseTab),
        MenuItem::separator(),
        MenuItem::action("Next Tab", NextTab),
        MenuItem::action("Previous Tab", PreviousTab),
        MenuItem::separator(),
        MenuItem::action("Previous Project", PreviousProject),
        MenuItem::action("Next Project", NextProject),
        MenuItem::separator(),
    ];
    window_items.extend(
        (0..9).map(|index| MenuItem::action(format!("Tab {}", index + 1), SelectTab { index })),
    );
    window_items.push(MenuItem::separator());
    window_items.extend(
        (0..9).map(|index| {
            MenuItem::action(format!("Project {}", index + 1), SelectProject { index })
        }),
    );
    let mut app_items = vec![MenuItem::action("Settings…", OpenSettings)];
    if backends.has_backend(Feature::Updates) {
        app_items.push(MenuItem::action("Check for Updates…", CheckForUpdates));
    }
    if backends.has_backend(Feature::CommandLineTool) {
        app_items.push(MenuItem::action(
            "Install Command Line Tool…",
            InstallCommandLineTool,
        ));
    }
    app_items.extend([
        MenuItem::separator(),
        MenuItem::os_submenu("Services", SystemMenuType::Services),
        MenuItem::separator(),
        MenuItem::action(format!("Hide {}", app_name()), HideApp),
        MenuItem::action("Hide Others", HideOthers),
        MenuItem::action("Show All", ShowAll),
        MenuItem::separator(),
        MenuItem::action(format!("Quit {}", app_name()), Quit),
        MenuItem::action("End All Sessions and Quit", EndAllSessionsAndQuit),
    ]);
    let mut view_items = vec![
        MenuItem::action("Command Palette…", ToggleCommandPalette),
        MenuItem::separator(),
    ];
    if backends.has_backend(Feature::Voice) {
        view_items.push(MenuItem::action(
            "Toggle Voice Recording",
            ToggleVoiceRecording,
        ));
    }
    view_items.extend([
        MenuItem::separator(),
        MenuItem::action("Toggle Sidebar", ToggleSidebar),
        MenuItem::action("Toggle Agents Panel", ToggleAgentsPanel),
        MenuItem::action("Toggle Agent Sessions", ToggleAgentSessions),
        MenuItem::action("Toggle Files", ToggleFiles),
        MenuItem::action("Toggle Source Control", ToggleSourceControl),
        MenuItem::action("Go to File…", GoToFile),
        MenuItem::action("Find in Files…", FindInFiles),
        MenuItem::action("Toggle Full Screen", ToggleFullScreen),
        MenuItem::separator(),
        MenuItem::action("Theme Picker", ToggleThemePicker),
        MenuItem::separator(),
        MenuItem::action("Increase Font Size", IncreaseFontSize),
        MenuItem::action("Decrease Font Size", DecreaseFontSize),
    ]);
    vec![
        Menu {
            name: app_name().into(),
            items: app_items,
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::os_action("Cut", text_input::Cut, OsAction::Cut),
                MenuItem::os_action("Copy", text_input::Copy, OsAction::Copy),
                MenuItem::os_action("Paste", text_input::Paste, OsAction::Paste),
                MenuItem::os_action("Select All", text_input::SelectAll, OsAction::SelectAll),
                MenuItem::separator(),
                MenuItem::action("Find…", Find),
                MenuItem::action("Find Next", FindNext),
                MenuItem::action("Find Previous", FindPrevious),
                MenuItem::separator(),
                MenuItem::action("Previous Prompt", PreviousPrompt),
                MenuItem::action("Next Prompt", NextPrompt),
                MenuItem::action("Select Command Output", SelectCommandOutput),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("New Terminal", NewTab),
                MenuItem::action("New Browser Pane", NewBrowserTab),
                MenuItem::action("New Home Terminal", NewHomeTab),
                MenuItem::action("New Terminal in Project…", NewTabInProject),
                MenuItem::action("New Worktree…", NewWorktree),
                MenuItem::action("Switch Worktree…", SwitchWorktree),
                MenuItem::action("Open Project…", AddProject),
                MenuItem::separator(),
                MenuItem::action("Close Tab", CloseTab),
            ],
        },
        Menu {
            name: "View".into(),
            items: view_items,
        },
        Menu {
            // GPUI hands macOS the window list only for a menu named "Window".
            name: "Window".into(),
            items: window_items,
        },
    ]
}
