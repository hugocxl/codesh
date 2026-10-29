use codesh_app_core::ServerId;
use codesh_app_core::settings::TerminalEdit;
use gpui::{
    AppContext, Bounds, Context, Entity, TitlebarOptions, Window, WindowBounds, WindowHandle,
    WindowOptions, point, px, size,
};

use super::{AppModel, Overlay, Quitting};
use crate::boot::Work;
use crate::views::settings::window::SettingsWindow;
use crate::views::settings::{BrowserChange, Change, SettingsView, Snapshot};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Default)]
pub(super) struct ServerPreferences {
    pub(super) document: Option<codesh_protocol::ServerSettingsDoc>,
    pub(super) busy: bool,
    pub(super) control_busy: bool,
    pub(super) row: String,
    pub(super) pending: std::collections::VecDeque<Change>,
}

pub(crate) struct SettingsWindowState {
    pub(crate) window: WindowHandle<SettingsWindow>,
    pub(crate) view: Entity<SettingsView>,
}

pub(super) struct TerminalPreview {
    settings: codesh_app_core::settings::TerminalSettings,
    zoom: std::collections::HashMap<super::PaneId, f32>,
}

#[derive(Default)]
pub(super) struct TerminalBackground {
    pub(super) available: bool,
    #[cfg(all(target_os = "macos", not(test)))]
    native: Option<codesh_ui::vibrancy::TerminalVibrancy>,
    /// Whether the window was last made see-through.
    #[cfg(all(target_os = "macos", not(test)))]
    window_transparent: Option<bool>,
}

impl AppModel {
    pub(crate) fn open_settings(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.show_settings(cx);
    }

    /// Debug builds: opens Settings at the category `CODESH_OPEN_SETTINGS`
    /// names, so a section can be brought up for screenshots.
    pub(crate) fn open_requested_settings(&mut self, cx: &mut Context<Self>) {
        #[cfg(debug_assertions)]
        if let Ok(label) = std::env::var("CODESH_OPEN_SETTINGS") {
            self.show_settings(cx);
            if let Some(settings) = &self.settings_window {
                settings
                    .view
                    .update(cx, |view, cx| view.show_labelled(&label, cx));
            }
        }
        // Debug builds: open a file or diff of the current project once it
        // has loaded, so editor and diff panes can be screenshotted.
        #[cfg(debug_assertions)]
        {
            if let Some(directory) =
                std::env::var_os("CODESH_OPEN_PROJECT").map(std::path::PathBuf::from)
                && directory.is_absolute()
                && directory.is_dir()
            {
                let existing = self
                    .state
                    .projects()
                    .iter()
                    .find(|project| project.server_id.is_local() && project.directory == directory)
                    .map(|project| project.id);
                if let Some(project) = existing.or_else(|| self.add_project(directory, cx)) {
                    self.select_project(project, cx);
                }
            }
            let file = std::env::var("CODESH_OPEN_FILE").ok();
            let diff = std::env::var("CODESH_OPEN_DIFF").ok();
            if file.is_some() || diff.is_some() {
                cx.spawn(async move |model, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(4))
                        .await;
                    let _ = model.update(cx, |model, cx| {
                        let project = model.state.current_project().id;
                        if let Some(diff) = &diff {
                            model.open_diff(project, diff, false, cx);
                        }
                        if let Some(file) = &file {
                            model.open_project_file(project, file, None, cx);
                        }
                    });
                })
                .detach();
            }
        }
        #[cfg(not(debug_assertions))]
        let _ = cx;
    }

    /// Opens Settings at Browser, where profiles are managed.
    pub(crate) fn show_browser_settings(&mut self, cx: &mut Context<Self>) {
        self.show_settings(cx);
        if let Some(settings) = &self.settings_window {
            settings.view.update(cx, SettingsView::show_browser);
        }
    }

    pub(super) fn show_settings(&mut self, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle || self.close_prompt.is_some() {
            return;
        }
        self.refresh_browser_profiles();
        self.browsers.import_sources = Self::browser_import_sources();
        if let Some(settings) = &self.settings_window
            && settings
                .window
                .update(cx, |root, window, cx| {
                    window.activate_window();
                    root.focus(window, cx);
                })
                .is_ok()
        {
            return;
        }
        self.settings_window = None;
        let snapshot = self.preferences_snapshot();
        self.refresh_theme(cx);
        let view = cx.new(|cx| {
            let mut view = SettingsView::new(
                snapshot,
                self.theme.clone(),
                codesh_ui::theme::Metrics::new(1.0),
                cx,
            );
            view.set_backup_pending(self.path.with_file_name("pending-import.codesh").exists());
            view
        });
        let weak = cx.weak_entity();
        let content = view.clone();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1040.0), px(780.0)),
                cx,
            ))),
            window_min_size: Some(size(px(740.0), px(480.0))),
            #[cfg(target_os = "linux")]
            window_decorations: Some(gpui::WindowDecorations::Client),
            #[cfg(target_os = "linux")]
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            is_movable: !cfg!(target_os = "macos"),
            titlebar: Some(TitlebarOptions {
                title: Some("Codesh Settings".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(14.0), px(16.0))),
            }),
            ..crate::window_options()
        };
        match cx.open_window(options, move |window, cx| {
            #[cfg(target_os = "linux")]
            window.set_window_title("Codesh Settings");
            let model = weak.clone();
            window.on_window_should_close(cx, move |_, cx| {
                let _ = model.update(cx, |model, cx| {
                    model.settings_closed(cx);
                });
                true
            });
            cx.new(|cx| SettingsWindow::new(weak, content, window, cx))
        }) {
            Ok(window) => {
                self.settings_window = Some(SettingsWindowState { window, view });
                self.dismiss_overlay(cx);
                self.read_server_settings(cx);
                self.read_remote_access(cx);
            }
            Err(error) => {
                self.fail(format!("Could not open Settings: {error}"), cx);
            }
        }
    }

    pub(crate) fn settings_closed(&mut self, cx: &mut Context<Self>) {
        self.flush_preferences(cx);
        self.withdraw_pairing(cx);
        self.settings_window = None;
        cx.notify();
    }

    fn preferences_snapshot(&self) -> Snapshot {
        let mut settings = self.settings.clone();
        settings.appearance = self.appearance.clone();
        Snapshot {
            capabilities: self.capabilities.clone(),
            settings,
            terminal: self.terminal.clone(),
            server: self.server_preferences.document.clone(),
            server_update: self.server_update_description(),
            connection: self.connection(ServerId::local()),
            server_busy: self.server_preferences.busy
                || self.server_preferences.control_busy
                || self.updates.replacing(),
            pending_server_fields: self.pending_server_fields(),
            mobile: self.mobile.state.clone(),
            mobile_pairing: self.mobile.pairing.clone(),
            mobile_busy: self.mobile.busy,
            mobile_error: self.mobile.error.clone(),
            ai_installed: self
                .ai
                .installed
                .iter()
                .map(|provider| provider.id)
                .collect(),
            browser_profiles: self.settings_browser_profiles(),
            browser_sources: self
                .browsers
                .import_sources
                .iter()
                .map(|source| {
                    (
                        format!("{} — {}", source.browser(), source.name()),
                        source.browser().supports_cookies(),
                    )
                })
                .collect(),
        }
    }

    pub(super) fn pending_server_fields(&self) -> std::collections::HashSet<String> {
        self.server_preferences
            .pending
            .iter()
            .map(|change| change_id(change).to_owned())
            .chain(
                self.server_preferences
                    .busy
                    .then(|| self.server_preferences.row.clone()),
            )
            .collect()
    }

    pub(crate) fn flush_preferences(&mut self, cx: &mut Context<Self>) {
        let changes = self
            .settings_window
            .as_ref()
            .map_or_else(Vec::new, |settings| {
                settings.view.update(cx, |view, cx| view.take_changes(cx))
            });
        for change in changes {
            self.change_preference(change, cx);
        }
    }

    pub(super) fn preferences_before_quit(&mut self, cx: &mut Context<Self>) -> bool {
        self.flush_preferences(cx);
        if self.server_preferences.busy || self.server_preferences.control_busy {
            self.fail(
                "Wait for the server settings operation to finish before quitting".to_string(),
                cx,
            );
            return false;
        }
        true
    }

    pub(super) fn sync_preferences(&self, cx: &mut Context<Self>) {
        if let Some(settings) = &self.settings_window {
            settings.view.update(cx, |view, cx| {
                view.sync(self.preferences_snapshot(), self.theme.clone(), cx);
            });
        }
    }

    pub(crate) fn preference_result(&self, id: &str, error: Option<&str>, cx: &mut Context<Self>) {
        if let Some(settings) = &self.settings_window {
            settings
                .view
                .update(cx, |view, cx| view.set_error(id, error, cx));
        }
    }

    pub(crate) fn configuration_path(&self, filename: &str) -> std::path::PathBuf {
        self.path.with_file_name(filename)
    }

    pub(crate) fn change_preference(&mut self, change: Change, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle {
            return;
        }
        if let Change::Field(id, prompt) = &change
            && let Some(action) = crate::views::settings::prompt_action(id)
        {
            self.set_ai_prompt(action, prompt, cx);
            return;
        }
        let id = change_id(&change).to_owned();
        if matches!(
            &change,
            Change::MobileAccess(_) | Change::Field("mobile-port", _)
        ) {
            let result = self.write_mobile_preference(change, cx);
            self.preference_result(&id, result.err().as_deref(), cx);
        } else if matches!(
            &change,
            Change::ShellIntegration(_) | Change::Field("default-shell" | "history-budget", _)
        ) {
            if self.server_preferences.busy {
                self.server_preferences
                    .pending
                    .retain(|pending| change_id(pending) != id);
                self.server_preferences.pending.push_back(change);
            } else if let Err(error) = self.write_server_preference(change, cx) {
                self.preference_result(&id, Some(&error.to_string()), cx);
            }
        } else {
            let theme_change = matches!(change, Change::Theme(..));
            let result = self.apply_preference(change, cx);
            let error = result.err().map(|error| error.to_string());
            self.preference_result(&id, error.as_deref(), cx);
            if theme_change && let Some(error) = error {
                self.fail(format!("Could not save theme: {error}"), cx);
            }
        }
        self.sync_preferences(cx);
        cx.notify();
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Exhaustive preference change dispatch"
    )]
    fn apply_preference(&mut self, change: Change, cx: &mut Context<Self>) -> Result<()> {
        let path = self.path.with_file_name("settings.toml");
        let mut settings = self.settings.clone();
        settings.appearance = self.appearance.clone();
        let theme_changed = matches!(change, Change::Theme(..));
        let bindings_changed = matches!(
            change,
            Change::Binding(..) | Change::Command(_) | Change::RemoveCommand(_)
        );
        match change {
            Change::VoiceAutoSend(value) => {
                settings.voice.auto_send = value;
                settings.save_voice(&path)?;
            }
            Change::Field("voice-language", value) => {
                settings.voice.language = value;
                settings.save_voice(&path)?;
            }
            Change::Field("voice-model", value) => {
                settings.voice.model = value;
                settings.save_voice(&path)?;
            }
            Change::Browser(change) => {
                if !self.apply_browser_profile_change(&change, cx)? {
                    settings.browser = browser_settings(settings.browser, change);
                    settings.save_browser(&path)?;
                }
            }
            Change::Field("browser-home-page", value) => {
                settings.browser.home_page = value;
                settings.save_browser(&path)?;
            }
            Change::Field("browser-profile-name", _) => return Ok(()),
            Change::Theme(dark, name) => {
                if dark {
                    settings.appearance.dark_theme = name;
                } else {
                    settings.appearance.light_theme = name;
                }
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Worktrees(key, value) => {
                match key {
                    "auto-expand-worktrees" => settings.appearance.auto_expand_worktrees = value,
                    "worktree-unread" => settings.appearance.worktree_show_unread = value,
                    "worktree-recent-order" => settings.appearance.worktree_recent_order = value,
                    _ => return Ok(()),
                }
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Sidebar(value) => {
                settings.appearance.sidebar_expanded = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::SidebarCollapsedStyle(style) => {
                settings.appearance.sidebar_collapsed_style = style;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::AgentsPanel(value) => {
                settings.appearance.agents_panel_visible = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
                if !settings.appearance.agents_panel_visible {
                    self.agents_panel.drag = None;
                    self.focus_requested = true;
                    if matches!(self.overlay, Some(Overlay::Agents)) {
                        self.dismiss_overlay(cx);
                    }
                }
            }
            Change::AgentsPanelGrouping(grouping) => {
                settings.appearance.agents_panel_grouping = grouping;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::SidebarVibrancy(value) => {
                settings.appearance.sidebar_vibrancy = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }

            Change::Field("sidebar-vibrancy-level", value) => {
                let level: u8 = value
                    .parse()
                    .map_err(|_| "Enter a whole number from 0 to 100".to_string())?;
                if level > 100 {
                    return Err("Enter a whole number from 0 to 100".into());
                }
                settings.appearance.sidebar_vibrancy_level = level;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::StatusBar(value) => {
                settings.appearance.status_bar_visible = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Tips(value) => {
                settings.appearance.tips_visible = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::ConfirmProcess(value) => {
                settings.window.confirm_running_process = value;
                settings.save_window(&path)?;
            }
            Change::CloseBehavior(value) => {
                settings.window.close_behavior = value;
                settings.save_window(&path)?;
            }
            Change::FileOpener(value) => {
                settings.openers.file = value;
                settings.save_openers(&path)?;
            }
            Change::Directory(value) => {
                settings.panes.new_pane_directory = value;
                settings.save_panes(&path)?;
            }
            Change::Command(command) => {
                command.validate()?;
                if let Some(existing) = settings
                    .commands
                    .iter_mut()
                    .find(|existing| existing.id == command.id)
                {
                    *existing = command;
                } else {
                    settings.commands.push(command);
                }
                settings.validate_command_shortcuts(&self.terminal)?;
                settings.save_commands(&path)?;
            }
            Change::RemoveCommand(id) => {
                settings.commands.retain(|command| command.id != id);
                settings.keymap = settings
                    .keymap
                    .with_binding(&format!("command.{id}"), None)?;
                settings.save_commands(&path)?;
            }
            Change::Binding(id, chord) => {
                if let Some(chord) = &chord
                    && let Some(conflict) = self.custom_shortcut_conflict(&id, chord)
                {
                    return Err(conflict.into());
                }
                settings.keymap = settings.keymap.with_binding(&id, chord)?;
                settings.validate_command_shortcuts(&self.terminal)?;
                settings.keymap.save(&path)?;
                cx.set_menus(crate::menus());
            }

            Change::Field(id @ ("worktree-template" | "worktree-folder"), value) => {
                if id == "worktree-template" {
                    settings.worktrees.default_location.path_template = value;
                } else {
                    settings.worktrees.default_location.parent_path = value;
                }
                settings.save_worktrees(&path)?;
            }
            Change::Field(id @ ("width" | "height"), value) => {
                let index = usize::from(id == "height");
                settings.window.default_size[index] = value.parse()?;
                settings.save_window(&path)?;
            }
            Change::Terminal(id, value)
            | Change::Field(
                id @ ("font-family"
                | "font-size"
                | "adjust-cell-height"
                | "adjust-cell-width"
                | "background-opacity"
                | "window-padding-x"
                | "window-padding-y"
                | "adjust-cursor-thickness"
                | "scroll-precision"
                | "scroll-discrete"),
                value,
            ) => {
                self.save_terminal(cx, |terminal| terminal.set_preference(id, &value))?;
            }
            Change::Field(id, value) if terminal_field(id) => {
                self.save_terminal(cx, |terminal| terminal.set_preference(id, &value))?;
            }
            Change::TerminalEdit(edit) => {
                self.save_terminal(cx, |terminal| terminal.edit(edit))?;
            }
            Change::ResetTerminal => {
                self.save_terminal(cx, |terminal| {
                    *terminal = codesh_app_core::settings::TerminalSettings::default();
                    Ok(())
                })?;
                let directory = codesh_app_core::settings::NewPaneDirectory::default();
                if settings.panes.new_pane_directory != directory {
                    settings.panes.new_pane_directory = directory;
                    settings.save_panes(&path)?;
                }
            }
            _ => return Err("Unknown app setting".into()),
        }
        let theme_changed = theme_changed
            || self.appearance.dark_theme != settings.appearance.dark_theme
            || self.appearance.light_theme != settings.appearance.light_theme;
        self.appearance = settings.appearance.clone();
        self.settings = settings;
        self.sync_agents_panel(cx);
        self.sync_agent_sessions(cx);
        self.filter_agent_sessions(cx);
        self.invalidate_browser_shortcuts();
        if bindings_changed {
            self.bind_command_keys(cx);
        }
        if theme_changed {
            self.refresh_theme(cx);
        }

        for pane in self.grids.values() {
            pane.view.update(cx, |pane, cx| {
                pane.copy_on_select = self.settings.clipboard.copy_on_select;
                cx.notify();
            });
        }
        Ok(())
    }

    #[cfg(all(target_os = "macos", not(test)))]
    pub(crate) fn sync_background_effects(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.sync_sidebar_vibrancy(window);
        self.sync_terminal_vibrancy(window, cx);
        self.sync_window_transparency(window);
    }

    /// Keeps the window opaque unless something must show through it. Over a
    /// transparent window without a native effect behind them, the window
    /// buttons blend with the desktop and look blurred, most visibly in light
    /// themes.
    #[cfg(all(target_os = "macos", not(test)))]
    fn sync_window_transparency(&mut self, window: &Window) {
        let transparent = self.sidebar_vibrancy.is_some()
            || self.terminal_background.native.is_some()
            || self
                .terminal
                .options
                .background_opacity
                .is_some_and(|opacity| opacity < 1.0);
        if self.terminal_background.window_transparent == Some(transparent) {
            return;
        }
        self.terminal_background.window_transparent = Some(transparent);
        window.set_background_appearance(if transparent {
            gpui::WindowBackgroundAppearance::Transparent
        } else {
            gpui::WindowBackgroundAppearance::Opaque
        });
    }

    #[cfg(all(target_os = "macos", not(test)))]
    fn sync_terminal_vibrancy(&mut self, window: &Window, cx: &mut Context<Self>) {
        let options = &self.terminal.options;
        if options.background_vibrancy == 0 || !codesh_ui::accessibility::transparency_allowed() {
            self.terminal_background.native = None;
            self.set_terminal_vibrancy_available(false, cx);
            return;
        }
        let offset = self.sidebar_width();
        let width = (f32::from(window.viewport_size().width) - offset).max(0.0);
        let background = self.palette.background;
        let background = gpui::rgb(background).into();
        if let Some(effect) = &mut self.terminal_background.native {
            effect.update(offset, width, background);
        } else {
            self.terminal_background.native =
                codesh_ui::vibrancy::TerminalVibrancy::new(window, offset, width, background);
        }
        self.set_terminal_vibrancy_available(self.terminal_background.native.is_some(), cx);
    }

    #[cfg(all(target_os = "macos", not(test)))]
    pub(super) fn set_terminal_vibrancy_available(
        &mut self,
        available: bool,
        cx: &mut Context<Self>,
    ) {
        if self.terminal_background.available == available {
            return;
        }
        self.terminal_background.available = available;
        for pane in self.grids.values() {
            let enabled = available;
            pane.view.update(cx, |pane, cx| {
                if pane.native_vibrancy != enabled {
                    pane.native_vibrancy = enabled;
                    cx.notify();
                }
            });
        }
    }

    pub(crate) fn confirm_terminal_reset(
        &mut self,
        window: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.quitting != Quitting::Idle || self.close_prompt.is_some() {
            return;
        }
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = crate::views::confirm::prompt_reset_terminal(window, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                match response {
                    Ok(true) => model.change_preference(Change::ResetTerminal, cx),
                    Ok(false) => {}
                    Err(error) => model.preference_result("terminal-reset", Some(&error), cx),
                }
                cx.notify();
            });
        }));
    }

    pub(crate) fn preview_terminal_preference(
        &mut self,
        id: &str,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        let mut next = self.terminal.clone();
        if let Err(error) = next.set_preference(id, value) {
            self.preference_result(id, Some(&error.to_string()), cx);
            return;
        }
        if next == self.terminal {
            return;
        }
        self.terminal_preview
            .get_or_insert_with(|| TerminalPreview {
                settings: self.terminal.clone(),
                zoom: self
                    .grids
                    .iter()
                    .map(|(id, pane)| (*id, pane.view.read(cx).terminal.font_size))
                    .collect(),
            });
        self.apply_terminal_preferences(next, false, cx);
    }

    fn save_terminal(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(
            &mut codesh_app_core::settings::TerminalSettings,
        ) -> codesh_app_core::settings::Result<()>,
    ) -> Result<()> {
        let result = (|| {
            let path = self.path.with_file_name("terminal.toml");
            let mut requested = codesh_app_core::settings::TerminalSettings::load_native(&path)?;
            edit(&mut requested)?;
            self.settings.validate_command_shortcuts(&requested)?;
            requested.save_native(&path)?;
            Ok(requested)
        })();
        let preview = self.terminal_preview.take();
        match result {
            Ok(requested) => {
                let original = preview
                    .as_ref()
                    .map_or(&self.terminal, |preview| &preview.settings);
                let changed_size = original.font_size.to_bits() != requested.font_size.to_bits();
                self.apply_terminal_preferences(requested, changed_size, cx);
                if !changed_size && let Some(preview) = preview {
                    self.restore_terminal_zoom(preview.zoom, cx);
                }
                self.set_configuration_error(None);
                Ok(())
            }
            Err(error) => {
                if let Some(preview) = preview {
                    self.apply_terminal_preferences(preview.settings, false, cx);
                    self.restore_terminal_zoom(preview.zoom, cx);
                }
                Err(error)
            }
        }
    }

    fn restore_terminal_zoom(
        &mut self,
        zoom: std::collections::HashMap<super::PaneId, f32>,
        cx: &mut Context<Self>,
    ) {
        for (id, size) in zoom {
            if let Some(pane) = self.grids.get(&id) {
                pane.view.update(cx, |pane, cx| {
                    pane.terminal.font_size = size;
                    cx.notify();
                });
            }
        }
    }

    fn apply_terminal_preferences(
        &mut self,
        effective: codesh_app_core::settings::TerminalSettings,
        reset_zoom: bool,
        cx: &mut Context<Self>,
    ) {
        let changed_size =
            reset_zoom || self.terminal.font_size.to_bits() != effective.font_size.to_bits();
        self.terminal = effective;
        if reset_zoom {
            self.font_sizes.clear();
        }
        for pane in self.grids.values() {
            pane.view.update(cx, |pane, cx| {
                let zoom = pane.terminal.font_size;
                pane.terminal = self.terminal.clone();
                pane.configured_font_size = self.terminal.font_size;
                if !changed_size {
                    pane.terminal.font_size = zoom;
                }
                cx.notify();
            });
        }
        self.refresh_theme(cx);
    }

    pub(crate) fn read_server_settings(&mut self, cx: &mut Context<Self>) {
        if self.quitting == Quitting::Idle
            && self.ready(ServerId::local())
            && !self.server_preferences.busy
            && !self.server_preferences.control_busy
        {
            self.server_preferences.busy =
                self.send(ServerId::local(), Work::ReadServerSettings, cx);
            self.server_preferences.row = "server".into();
            self.sync_preferences(cx);
        }
    }

    fn write_server_preference(&mut self, change: Change, cx: &mut Context<Self>) -> Result<()> {
        if !self.ready(ServerId::local()) || self.server_preferences.control_busy {
            return Err("Connect to the server before editing its settings"
                .to_string()
                .into());
        }
        let mut settings = self
            .server_preferences
            .document
            .clone()
            .ok_or_else(|| "Load server settings first".to_string())?;
        let id = change_id(&change).to_owned();
        match change {
            Change::ShellIntegration(value) => settings.shell_integration = value,
            Change::Field("default-shell", value) => {
                settings.default_shell =
                    (!value.is_empty()).then(|| codesh_protocol::ServerPath(value.into_bytes()));
            }
            Change::Field("history-budget", value) => {
                settings.history_budget_bytes = value
                    .parse::<u64>()?
                    .checked_mul(1024 * 1024)
                    .ok_or_else(|| "History budget is too large".to_string())?;
            }
            _ => return Err("Unknown server setting".into()),
        }
        settings.validate().map_err(|_| {
            "Use an absolute shell path and a history budget between 0 and 65536 MiB".to_string()
        })?;
        self.server_preferences.busy =
            self.send(ServerId::local(), Work::WriteServerSettings(settings), cx);
        self.server_preferences.row = id;
        if !self.server_preferences.busy {
            return Err("Could not send server settings".into());
        }
        Ok(())
    }

    pub(super) fn receive_server_settings(
        &mut self,
        result: std::result::Result<codesh_protocol::ServerSettingsDoc, codesh_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.server_preferences.busy = false;
        match result {
            Ok(settings) => {
                self.server_preferences.document = Some(settings);
                self.preference_result(&self.server_preferences.row, None, cx);
            }
            Err(error) => {
                self.preference_result(&self.server_preferences.row, Some(&error.to_string()), cx);
            }
        }
        while !self.server_preferences.busy {
            let Some(change) = self.server_preferences.pending.pop_front() else {
                break;
            };
            self.change_preference(change, cx);
        }
        self.sync_preferences(cx);
    }

    /// A stopped server counts as disconnected. Restarting connects again,
    /// which starts it, over SSH too.
    pub(super) fn receive_server_stopped(
        &mut self,
        server: ServerId,
        restart: bool,
        result: std::result::Result<(), codesh_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        if server.is_local() {
            self.server_preferences.control_busy = false;
            self.server_update_control_finished(&result, cx);
        } else if let Some(runtime) = self.servers.get_mut(server) {
            runtime.stopping = false;
        }
        match result {
            Ok(()) => {
                self.disconnect(server, cx);
                if restart {
                    self.connect_server(server, cx);
                } else if let Some(runtime) = self.servers.get_mut(server) {
                    runtime.retry.held = true;
                }
            }
            Err(error) => {
                let message =
                    self.server_message(server, &format!("Could not stop server: {error}"));
                if server.is_local() {
                    self.preference_result("server", Some(&message), cx);
                }
                self.fail(message, cx);
            }
        }
        self.sync_preferences(cx);
    }

    /// Asks before stopping or restarting `server`, which ends its sessions.
    pub(crate) fn confirm_server_control(
        &mut self,
        server: ServerId,
        restart: bool,
        window: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !self.control_enabled(server) {
            return;
        }
        self.dismiss_overlay(cx);
        let generation = self.generation(server);
        let name = (!server.is_local()).then(|| self.server_label(server));
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response =
                crate::views::confirm::prompt_server(window, restart, name.as_deref(), cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                model.focus_requested = true;
                if generation == model.generation(server) {
                    match response {
                        Ok(true) => model.stop_server(server, restart, cx),
                        Ok(false) => {}
                        Err(error) if server.is_local() => {
                            model.preference_result("server", Some(&error), cx);
                        }
                        Err(error) => model.fail(error, cx),
                    }
                }
                cx.notify();
            });
        }));
    }

    fn stop_server(&mut self, server: ServerId, restart: bool, cx: &mut Context<Self>) {
        let sent = self.send(server, Work::StopServer { restart }, cx);
        if server.is_local() {
            self.server_preferences.control_busy = sent;
            if sent {
                self.server_update_control_started(restart);
            }
        } else if let Some(runtime) = self.servers.get_mut(server) {
            runtime.stopping = sent;
        }
        self.sync_preferences(cx);
    }
}

fn terminal_field(id: &str) -> bool {
    matches!(
        id,
        "font-feature"
            | "background"
            | "foreground"
            | "cursor-color"
            | "cursor-text"
            | "selection-foreground"
            | "selection-background"
    ) || id.starts_with("palette-")
}

fn change_id(change: &Change) -> &str {
    match change {
        Change::Command(_) | Change::RemoveCommand(_) => "commands",
        Change::VoiceAutoSend(_) => "voice-auto-send",
        Change::Theme(false, _) => "light-theme",
        Change::Theme(true, _) => "dark-theme",
        Change::Sidebar(_) => "sidebar",
        Change::SidebarVibrancy(_) => "sidebar-vibrancy",
        Change::Worktrees(key, _) => key,
        Change::SidebarCollapsedStyle(_) => "sidebar-collapsed-style",
        Change::AgentsPanel(_) => "agents-panel",
        Change::AgentsPanelGrouping(_) => "agents-panel-grouping",
        Change::StatusBar(_) => "status-bar",
        Change::Tips(_) => "tips",
        Change::ConfirmProcess(_) => "confirm-process",
        Change::CloseBehavior(_) => "close-behavior",
        Change::FileOpener(_) => "file-opener",
        Change::Directory(_) => "directory",
        Change::Terminal("padding-left" | "padding-right", _) => "window-padding-x",
        Change::Terminal("padding-top" | "padding-bottom", _) => "window-padding-y",
        Change::Terminal("dismiss-import-notes", _) => "terminal-import-notes",
        Change::Terminal(id, _) | Change::Field(id, _) => id,
        Change::ResetTerminal => "terminal-reset",
        Change::TerminalEdit(edit) => match edit {
            TerminalEdit::Fallbacks(_) => "font-fallbacks",
            TerminalEdit::CodepointMap { .. } => "font-codepoint-map",
            TerminalEdit::Binding { .. } => "terminal-keybindings",
        },
        Change::Binding(id, _) => id,
        Change::ShellIntegration(_) => "shell-integration",
        Change::MobileAccess(_) => "mobile-access",
        Change::Browser(change) => match change {
            BrowserChange::Enabled(_) => "browser-enabled",
            BrowserChange::TerminalLinks(_) => "browser-terminal-links",
            BrowserChange::SearchEngine(_) => "browser-search-engine",
            BrowserChange::Import { .. } => "browser-import",
            BrowserChange::DefaultProfile(_)
            | BrowserChange::AddProfile(_)
            | BrowserChange::RenameProfile(..)
            | BrowserChange::ClearProfile(_)
            | BrowserChange::DeleteProfile(_) => "browser-profiles",
        },
    }
}

/// `browser` with a change that lives in `settings.toml`.
fn browser_settings(
    mut browser: codesh_app_core::settings::BrowserSettings,
    change: BrowserChange,
) -> codesh_app_core::settings::BrowserSettings {
    match change {
        BrowserChange::Enabled(enabled) => browser.enabled = enabled,
        BrowserChange::TerminalLinks(enabled) => browser.open_terminal_links = enabled,
        BrowserChange::SearchEngine(engine) => browser.search_engine = engine,
        BrowserChange::DefaultProfile(id) => browser.default_profile = id,
        _ => {}
    }
    browser
}
