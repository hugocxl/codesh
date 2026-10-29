mod ai;
mod appearance;
mod backup;
pub(crate) mod browser;
mod catalog;
mod commands;
mod keyboard;
mod languages;
mod layout;
pub(crate) mod mobile;
mod pickers;
mod results;
mod server;
mod terminal;
mod terminal_lists;
mod themes;
mod voice;
pub(crate) mod window;

pub(crate) use ai::prompt_action;
pub(crate) use browser::BrowserChange;
pub(crate) use pickers::{PickerAnchor, PickerKind, PickerRequest};

use std::collections::{HashMap, HashSet};

use super::codesh::style;
use codesh_app_core::settings::{Settings, TerminalSettings};
use codesh_ui::controls::{self, Style};
use codesh_ui::form;
use codesh_ui::text_input::{InputEvent, InputStyle, TextInput};
use codesh_ui::theme::{Metrics, Theme};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Window, canvas, div, px,
};

pub(crate) fn register_commands(
    registry: &mut codesh_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &crate::model::AppModel,
) {
    use super::command_palette::action;
    use codesh_core::shortcuts::ShortcutId;
    registry.register(action(
        model,
        ShortcutId::OpenSettings,
        "Open Settings",
        super::workspace::OpenSettings,
    ));
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Category {
    General,
    Ai,
    Appearance,
    Terminal,
    Keyboard,
    Commands,
    Browser,
    Server,
    Mobile,
    Backup,
}

impl Category {
    pub(crate) const ALL: [Self; 10] = [
        Self::General,
        Self::Appearance,
        Self::Terminal,
        Self::Keyboard,
        Self::Commands,
        Self::Browser,
        Self::Server,
        Self::Mobile,
        Self::Ai,
        Self::Backup,
    ];

    /// The English name; also the key of its translation.
    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Ai => "AI",
            Self::Appearance => "Appearance",
            Self::Terminal => "Terminal",
            Self::Keyboard => "Keyboard",
            Self::Commands => "Commands",
            Self::Browser => "Browser",
            Self::Server => "Server",
            Self::Mobile => "Mobile",
            Self::Backup => "Backup & Restore",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Change {
    VoiceAutoSend(bool),
    Theme(bool, String),
    Sidebar(bool),
    SidebarVibrancy(bool),

    Worktrees(&'static str, bool),
    SidebarCollapsedStyle(codesh_app_core::settings::SidebarCollapsedStyle),
    AgentsPanel(bool),
    AgentsPanelGrouping(codesh_app_core::settings::AgentsPanelGrouping),
    StatusBar(bool),
    Tips(bool),
    ConfirmProcess(bool),
    CloseBehavior(codesh_app_core::settings::CloseBehavior),
    Terminal(&'static str, String),
    TerminalEdit(codesh_app_core::settings::TerminalEdit),
    ResetTerminal,

    FileOpener(String),
    Directory(codesh_app_core::settings::NewPaneDirectory),
    Field(&'static str, String),
    Binding(String, Option<codesh_app_core::settings::KeyChord>),
    Command(codesh_app_core::settings::CustomCommand),
    RemoveCommand(String),

    ShellIntegration(bool),
    MobileAccess(bool),
    Browser(BrowserChange),
}

pub(crate) enum SettingsEvent {
    Backup(backup::Action),
    Change(Change),
    PreviewTerminal(&'static str, String),
    ResetTerminal,
    Picker(PickerKind, PickerAnchor),
    ServerControl { restart: bool },
    ReadServer,
    ReadMobile,
    PairPhone,
    CancelPairing,
    RevokeDevice(codesh_protocol::DeviceId),
    Connect,
    OpenConfiguration(&'static str),
    DismissProblem,
}

#[derive(Clone)]
pub(crate) struct Snapshot {
    pub(crate) settings: Settings,
    pub(crate) capabilities: codesh_core::capability::Capabilities,
    pub(crate) terminal: TerminalSettings,
    pub(crate) server: Option<codesh_protocol::ServerSettingsDoc>,
    pub(crate) connection: crate::model::ConnectionState,
    pub(crate) server_busy: bool,
    pub(crate) server_update: Option<String>,
    pub(crate) pending_server_fields: HashSet<String>,
    pub(crate) mobile: Option<codesh_protocol::RemoteAccessState>,
    pub(crate) mobile_pairing: Option<mobile::Pairing>,
    pub(crate) mobile_busy: bool,
    pub(crate) mobile_error: Option<String>,
    pub(crate) ai_installed: Vec<&'static str>,

    pub(crate) browser_profiles: Vec<browser::Profile>,
    /// Other browsers' profiles that can be imported, by name, and whether
    /// their cookies can be.
    pub(crate) browser_sources: Vec<(String, bool)>,
}

pub(crate) struct SettingsView {
    backup_busy: bool,
    backup_import: Option<crate::backup::PreparedImport>,
    backup_status: String,
    navigation_view: Entity<super::cached::CachedView<Self>>,
    content_view: Entity<super::cached::CachedView<Self>>,
    terminal_slider: Option<terminal::Drag>,
    codepoint_editor: Option<terminal_lists::CodepointEditor>,
    binding_editor: Option<terminal_lists::BindingEditor>,
    pub(crate) focus: FocusHandle,
    snapshot: Snapshot,
    theme: Theme,
    metrics: Metrics,
    search: Entity<TextInput>,
    query: String,
    results: results::Results,
    scrollbar: codesh_ui::scrollbar::ListScrollbar,
    shortcut_names: Vec<String>,
    category: Category,
    section: Option<&'static str>,
    expanded: HashSet<Category>,
    navbar_focus: FocusHandle,
    fields: HashMap<&'static str, Entity<TextInput>>,
    dirty: HashSet<&'static str>,
    pub(crate) errors: HashMap<String, String>,
    recording: Option<String>,
    command_editor: Option<codesh_app_core::settings::CustomCommand>,
    focus_initialized: bool,
    compact: bool,
    picker_anchors: HashMap<PickerKind, PickerAnchor>,
    subscriptions: Vec<Subscription>,
    #[cfg(test)]
    pub(crate) render_count: usize,
    #[cfg(test)]
    pub(crate) shortcut_row_count: usize,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl SettingsView {
    pub(crate) fn new(
        snapshot: Snapshot,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(InputStyle::field(&theme, &metrics), cx)
                .with_placeholder("Search Settings")
        });
        let search_subscription = cx.subscribe(&search, |pane: &mut Self, _, event, cx| {
            if matches!(event, InputEvent::Changed) {
                pane.query = pane.search.read(cx).text().trim().to_lowercase();
                pane.results.reset();
            }
            cx.notify();
        });
        let focus = cx.focus_handle();
        let weak = cx.weak_entity();
        let recorder = cx.intercept_keystrokes(move |event, window, cx| {
            let _ = weak.update(cx, |pane: &mut Self, cx| {
                if pane.focus.is_focused(window) {
                    pane.record_key(&event.keystroke, cx);
                }
            });
        });
        let mut pane = Self {
            backup_busy: false,
            backup_import: None,
            backup_status: String::new(),
            navigation_view: super::cached::CachedView::new(
                |view, window, cx| view.navigation(window, cx),
                cx,
            ),
            content_view: super::cached::CachedView::new(|view, _, cx| view.content(cx), cx),
            terminal_slider: None,
            codepoint_editor: None,
            binding_editor: None,
            focus,
            snapshot,
            theme,
            metrics,
            search,
            query: String::new(),
            results: results::Results::new(cx),
            scrollbar: codesh_ui::scrollbar::ListScrollbar::default(),
            shortcut_names: codesh_core::shortcuts::ALL
                .iter()
                .map(codesh_core::shortcuts::Shortcut::label)
                .collect(),
            category: Category::General,
            section: None,
            expanded: HashSet::new(),
            navbar_focus: cx.focus_handle().tab_stop(true),
            fields: HashMap::new(),
            dirty: HashSet::new(),
            errors: HashMap::new(),
            recording: None,
            command_editor: None,
            focus_initialized: false,
            compact: false,
            picker_anchors: [
                PickerKind::Theme(false),
                PickerKind::Theme(true),
                PickerKind::AiProvider(crate::repository_actions::Action::Commit),
                PickerKind::AiProvider(crate::repository_actions::Action::CreatePullRequest),
                PickerKind::FileOpener,
                PickerKind::Language,
            ]
            .into_iter()
            .chain(pickers::FontSlot::ALL.map(PickerKind::Font))
            .chain(terminal::PICKERS.iter().copied().map(PickerKind::Terminal))
            .map(|kind| (kind, PickerAnchor::default()))
            .collect(),
            subscriptions: vec![search_subscription, recorder],
            #[cfg(test)]
            render_count: 0,
            #[cfg(test)]
            shortcut_row_count: 0,
        };
        pane.create_fields(cx);
        pane.sync_fields(cx);
        pane
    }

    fn create_fields(&mut self, cx: &mut Context<Self>) {
        let (theme, metrics) = (self.theme.clone(), self.metrics);
        for (id, multiline) in [
            "voice-model",
            "command-name",
            "command-text",
            "worktree-template",
            "worktree-folder",
            "width",
            "height",
            "default-shell",
            "history-budget",
            "mobile-port",
            "browser-home-page",
            "browser-profile-name",
        ]
        .into_iter()
        .chain(terminal::fields())
        .chain(terminal_lists::EDITOR_FIELDS)
        .map(|id| (id, false))
        .chain(ai::PROMPTS.iter().map(|(_, id)| (*id, true)))
        {
            let input = cx.new(|cx| {
                let input = TextInput::new(InputStyle::field(&theme, &metrics), cx);
                if multiline { input.multiline() } else { input }
            });
            let changed = cx.subscribe(&input, move |pane: &mut Self, _, event, cx| {
                if matches!(id, "command-name" | "command-text") {
                    match event {
                        InputEvent::Submitted => pane.save_command(cx),
                        InputEvent::Cancelled => pane.cancel_command(cx),
                        InputEvent::Changed => {
                            cx.notify();
                        }
                    }
                    return;
                }
                if terminal_lists::EDITOR_FIELDS.contains(&id) {
                    match event {
                        InputEvent::Submitted => pane.save_terminal_editor(id, cx),
                        InputEvent::Cancelled => pane.cancel_terminal_editor(id, cx),
                        InputEvent::Changed => cx.notify(),
                    }
                    return;
                }
                match event {
                    InputEvent::Changed => {
                        pane.dirty.insert(id);
                        pane.results.dirty = true;
                        cx.notify();
                    }
                    InputEvent::Submitted => pane.commit_field(id, cx),
                    InputEvent::Cancelled => {
                        pane.dirty.remove(id);
                        pane.errors.remove(id);
                        pane.results.dirty = true;
                        pane.sync_fields(cx);
                        cx.notify();
                    }
                }
            });
            self.fields.insert(id, input);
            self.subscriptions.push(changed);
        }
    }

    #[cfg(test)]
    pub(crate) fn server_connection(&self) -> crate::model::ConnectionState {
        self.snapshot.connection
    }

    #[cfg(test)]
    pub(crate) fn browser_profiles(&self) -> &[browser::Profile] {
        &self.snapshot.browser_profiles
    }

    pub(crate) fn sync(&mut self, snapshot: Snapshot, theme: Theme, cx: &mut Context<Self>) {
        self.results.dirty = true;
        self.snapshot = snapshot;
        self.theme = theme;

        let style = InputStyle::field(&self.theme, &self.metrics);
        self.search
            .update(cx, |input, cx| input.set_style(style, cx));
        for input in self.fields.values() {
            input.update(cx, |input, cx| input.set_style(style, cx));
        }
        self.sync_fields(cx);
        cx.notify();
    }

    fn sync_fields(&self, cx: &mut Context<Self>) {
        let settings = &self.snapshot.settings;
        let server = self.snapshot.server.as_ref();
        let shell = server
            .and_then(|server| server.default_shell.as_ref())
            .map_or_else(String::new, |path| {
                String::from_utf8_lossy(&path.0).into_owned()
            });
        let budget = server.map_or_else(String::new, |server| {
            (server.history_budget_bytes / (1024 * 1024)).to_string()
        });
        let mobile_port = self
            .snapshot
            .mobile
            .as_ref()
            .map_or_else(String::new, |mobile| mobile.settings.port.to_string());
        for (id, value) in [
            ("voice-model", settings.voice.model.clone()),
            (
                "worktree-template",
                settings.worktrees.default_location.path_template.clone(),
            ),
            (
                "worktree-folder",
                settings.worktrees.default_location.parent_path.clone(),
            ),
            ("width", settings.window.default_size[0].to_string()),
            ("height", settings.window.default_size[1].to_string()),
            ("default-shell", shell),
            ("history-budget", budget),
            ("mobile-port", mobile_port),
            ("browser-home-page", settings.browser.home_page.clone()),
        ]
        .into_iter()
        .chain(terminal::field_values(&self.snapshot.terminal))
        .chain(ai::PROMPTS.iter().map(|(action, id)| {
            (
                *id,
                settings
                    .ai
                    .prompts
                    .get(action.key())
                    .filter(|prompt| !prompt.trim().is_empty())
                    .cloned()
                    .unwrap_or_else(|| action.default_prompt().into()),
            )
        })) {
            if !self.dirty.contains(id)
                && !self.errors.contains_key(id)
                && !self.snapshot.pending_server_fields.contains(id)
            {
                self.fields[id].update(cx, |input, cx| {
                    if input.text() != value {
                        input.set_text(value, cx);
                    }
                });
            }
        }
    }

    fn initialize_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus_initialized {
            return;
        }
        self.focus_initialized = true;
        self.subscriptions
            .push(cx.on_focus_out(&self.focus, window, |pane, _, _, cx| {
                pane.recording = None;
                pane.results.dirty = true;
                cx.notify();
            }));
        for input in std::iter::once(&self.search).chain(self.fields.values()) {
            self.subscriptions
                .push(cx.on_focus(&input.focus_handle(cx), window, |pane, _, cx| {
                    pane.recording = None;
                    pane.results.dirty = true;
                    cx.notify();
                }));
        }
        for (&id, input) in &self.fields {
            self.subscriptions.push(cx.on_blur(
                &input.focus_handle(cx),
                window,
                move |pane, _, cx| {
                    pane.results.dirty = true;
                    pane.commit_field(id, cx);
                    cx.notify();
                },
            ));
        }
    }

    pub(crate) fn take_changes(&mut self, cx: &Context<Self>) -> Vec<Change> {
        self.recording = None;
        self.results.dirty = true;
        self.dirty
            .drain()
            .map(|id| Change::Field(id, self.fields[id].read(cx).text().trim().to_owned()))
            .chain(self.terminal_slider.take().and_then(terminal::Drag::change))
            .collect()
    }

    pub(crate) fn set_error(&mut self, id: &str, error: Option<&str>, cx: &mut Context<Self>) {
        if let Some(error) = error {
            self.errors.insert(id.into(), error.to_owned());
        } else {
            self.errors.remove(id);
            match id {
                "commands" => self.command_editor = None,
                "font-codepoint-map" => self.codepoint_editor = None,
                "terminal-keybindings" => self.binding_editor = None,
                "terminal-reset" => {
                    self.codepoint_editor = None;
                    self.binding_editor = None;
                    self.errors.retain(|id, _| !terminal::owns(id));
                }
                _ => (),
            }
        }
        self.results.dirty = true;
        cx.notify();
    }

    /// Clears a page-level error; the application error is dismissed everywhere.
    fn dismiss_error(&mut self, id: &str, cx: &mut Context<Self>) {
        self.set_error(id, None, cx);
        if id == "application" {
            cx.emit(SettingsEvent::DismissProblem);
        }
    }

    fn commit_field(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if self.dirty.remove(id) {
            let value = self.fields[id].read(cx).text().trim().to_owned();
            cx.emit(SettingsEvent::Change(Change::Field(id, value)));
        }
    }

    pub(crate) fn show_browser(&mut self, cx: &mut Context<Self>) {
        self.category = Category::Browser;
        self.section = None;
        self.query.clear();
        self.results.reset();
        self.search.update(cx, |input, cx| input.set_text("", cx));
        cx.notify();
    }

    /// Debug builds: opens the category labelled `label` (any case), so
    /// `CODESH_OPEN_SETTINGS` can bring a section up for screenshots.
    #[cfg(debug_assertions)]
    pub(crate) fn show_labelled(&mut self, label: &str, cx: &mut Context<Self>) {
        if let Some(category) = Category::ALL
            .into_iter()
            .find(|category| category.label().eq_ignore_ascii_case(label))
        {
            self.category = category;
            self.section = None;
            cx.notify();
        }
    }

    fn matches(&self, category: Category, label: &str) -> bool {
        let setting = catalog::SETTINGS
            .iter()
            .find(|setting| setting.category == category && setting.label == label);
        if setting.is_some_and(|setting| {
            matches!(
                setting.id,
                "status-bar" | "sidebar-collapsed-style" | "tips"
            )
        }) {
            return false;
        }
        if self.query.is_empty() {
            category == self.category
                && self
                    .section
                    .is_none_or(|section| setting.is_none_or(|setting| setting.section == section))
        } else {
            let text = setting.map_or_else(
                || {
                    format!(
                        "{} {}",
                        category.label().to_lowercase(),
                        label.to_lowercase()
                    )
                },
                |setting| {
                    format!(
                        "{} {} {} {} {}",
                        category.label().to_lowercase(),
                        setting.section.to_lowercase(),
                        setting.id,
                        label.to_lowercase(),
                        setting.description.to_lowercase()
                    )
                },
            );
            self.query
                .split_whitespace()
                .all(|word| text.contains(word))
        }
    }

    /// The file behind this category's settings; mobile access is managed only here.
    fn configuration_file(&self) -> Option<&'static str> {
        match self.category {
            Category::Server => Some("server.toml"),
            Category::Terminal => Some("terminal.toml"),
            Category::Mobile => None,
            _ => Some("settings.toml"),
        }
    }

    fn style(&self) -> Style<'_> {
        Style {
            theme: &self.theme,
            metrics: &self.metrics,
        }
    }

    fn layout_style(&self) -> Style<'_> {
        const METRICS: Metrics = Metrics::new(1.0);
        Style {
            theme: &self.theme,
            metrics: &METRICS,
        }
    }

    /// A setting row. `label` is the English key it is listed and searched
    /// by; an explicit `description` is already translated.
    fn row(&self, id: &str, label: &str, control: AnyElement) -> AnyElement {
        self.row_with(id, label, control, self.compact)
    }

    fn row_with(&self, id: &str, label: &str, control: AnyElement, stacked: bool) -> AnyElement {
        self.row_with_description(id, label, None, control, stacked)
    }

    fn row_with_description(
        &self,
        id: &str,
        label: &str,
        description: Option<&str>,
        control: AnyElement,
        stacked: bool,
    ) -> AnyElement {
        self.text_row(id, label, description, control, stacked)
    }

    fn visible_catalog_row(&self, setting: &catalog::Setting) -> bool {
        let available = match setting.id {
            "voice-model" => codesh_ui::voice::LOCAL_MODEL,
            "terminal-import-notes" => !self.snapshot.terminal.diagnostics.is_empty(),
            _ => true,
        };
        available && self.matches(setting.category, setting.label)
    }

    /// Match the visible catalog section while keeping rows independently focusable.
    fn row_group(&self, id: &str) -> (Option<&'static str>, bool) {
        if let Some(index) = codesh_core::shortcuts::ALL
            .iter()
            .position(|shortcut| shortcut.id == id)
        {
            let matching = keyboard::matching(self);
            return (None, matching.last() == Some(&index));
        }
        let setting = catalog::setting(id);
        let section = setting
            .filter(|setting| {
                !catalog::SETTINGS
                    .iter()
                    .take_while(|previous| previous.id != id)
                    .any(|previous| {
                        previous.category == setting.category
                            && previous.section == setting.section
                            && self.visible_catalog_row(previous)
                    })
            })
            .map(|setting| setting.section);
        let last = setting.is_none_or(|setting| {
            !catalog::SETTINGS
                .iter()
                .skip_while(|next| next.id != id)
                .skip(1)
                .any(|next| {
                    next.category == setting.category
                        && next.section == setting.section
                        && self.visible_catalog_row(next)
                })
        });
        (section, last)
    }

    fn row_start(&self, id: &str, catalog: bool, section: bool) -> (bool, bool) {
        let shortcut_index = codesh_core::shortcuts::ALL
            .iter()
            .position(|shortcut| shortcut.id == id);
        let first = shortcut_index.map_or(section || !catalog, |index| {
            keyboard::matching(self).first() == Some(&index)
        });
        (first, !catalog && shortcut_index.is_none())
    }

    /// A row whose label is shown as given, such as a custom command's name.
    fn text_row(
        &self,
        id: &str,
        label: &str,
        description: Option<&str>,
        control: AnyElement,
        stacked: bool,
    ) -> AnyElement {
        let setting = catalog::setting(id);
        let catalog_description = setting.map(|setting| setting.description);
        let description = description.or(catalog_description);
        let (section, last) = self.row_group(id);
        let help = description.map(str::to_owned);
        let help_theme = self.theme.clone();
        let (first, standalone) = self.row_start(id, setting.is_some(), section.is_some());
        let focus = self.results.row_focus[id].clone();
        let reveal = self.results.reveal_focus.clone();
        div()
            .relative()
            .track_focus(&focus)
            .debug_selector({
                let id = id.to_owned();
                move || format!("settings-row-{id}")
            })
            .min_w(px(0.0))
            .when(standalone, |row| row.mb(px(style::GUTTER)))
            .when_some(section, |row, section| {
                row.child(self.subsection_heading(section))
            })
            .child(
                div()
                    .id(gpui::SharedString::from(format!("settings-card-row-{id}")))
                    .relative()
                    .bg(self.theme.fg_alpha(0.065))
                    .border_x_1()
                    .border_color(self.theme.fg_alpha(0.06))
                    .when(first, |row| row.border_t_1().rounded_t(px(style::RADIUS)))
                    .when(last, |row| row.border_b_1().rounded_b(px(style::RADIUS)))
                    .child(form::row(
                        self.layout_style(),
                        label,
                        None,
                        control,
                        stacked,
                    ))
                    .when(!last, |row| {
                        row.child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left(px(style::GUTTER * 1.5))
                                .right(px(style::GUTTER * 1.5))
                                .h(px(1.0))
                                .bg(self.theme.fg_alpha(0.10)),
                        )
                    })
                    .when_some(help, |row, help| {
                        row.tooltip(move |_, cx| {
                            cx.new(|_| {
                                codesh_ui::components::Tooltip::new(
                                    help.clone(),
                                    help_theme.raised(),
                                    help_theme.fg,
                                    help_theme.border,
                                    help_theme.bg,
                                )
                            })
                            .into()
                        })
                    }),
            )
            .when_some(self.errors.get(id), |row, error| {
                row.child(self.note(error, true).pt_0())
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        if reveal.get() && focus.contains_focused(window, cx) {
                            reveal.set(false);
                            window.request_autoscroll(bounds);
                        }
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .into_any_element()
    }

    fn note(&self, text: &str, error: bool) -> gpui::Div {
        form::note(self.style(), text, error)
    }

    fn field(&self, id: &'static str) -> AnyElement {
        let selector = format!("settings-field-{id}");
        div()
            .flex()
            .min_w(px(0.0))
            .debug_selector(move || selector.clone())
            .child(controls::text_field(
                self.style(),
                id,
                &self.fields[id],
                (!self.compact).then_some(210.0),
            ))
            .into_any_element()
    }

    fn toggle(
        &self,
        id: &'static str,
        value: bool,
        change: Change,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        controls::toggle(
            self.style(),
            id,
            value,
            cx.listener(move |_, _, _, cx| {
                cx.emit(SettingsEvent::Change(change.clone()));
            }),
        )
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(test)]
        {
            self.render_count += 1;
        }
        self.initialize_focus(window, cx);
        let view = cx.entity();
        let reveal = self.results.reveal_focus.clone();
        div()
            .id("settings-view")
            .on_mouse_move(cx.listener(|view, event: &gpui::MouseMoveEvent, _, cx| {
                view.move_terminal_slider(event.position, cx);
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.finish_terminal_slider(cx);
                }),
            )
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.finish_terminal_slider(cx);
                }),
            )
            .debug_selector(|| "settings-view".into())
            .track_focus(&self.focus)
            .tab_group()
            .font_family(".SystemUIFont")
            .line_height(gpui::relative(1.3))
            .on_action(cx.listener(|view, _: &SearchSettings, window, cx| {
                view.search.focus_handle(cx).focus(window);
            }))
            .on_action(cx.listener(|view, _: &FocusSettingsNavbar, window, _| {
                view.navbar_focus.focus(window);
            }))
            .on_action(cx.listener(|view, _: &NextSettingsControl, window, cx| {
                view.advance_focus(false, window, cx);
            }))
            .on_action(
                cx.listener(|view, _: &PreviousSettingsControl, window, cx| {
                    view.advance_focus(true, window, cx);
                }),
            )
            .size_full()
            .relative()
            .flex()
            .overflow_hidden()
            .bg(self.theme.bg)
            .map(|view| super::titlebar::top_corners(view, window, true, true))
            .text_color(self.theme.fg)
            .text_size(px(13.0))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let mut content = view
                            .update(cx, |pane, cx| pane.layout_content(bounds.size, window, cx));
                        content.layout_as_root(bounds.size.into(), window, cx);
                        content.prepaint_at(bounds.origin, window, cx);
                        reveal.set(false);
                        content
                    },
                    |_, mut content, window, cx| content.paint(window, cx),
                )
                .size_full(),
            )
    }
}

gpui::actions!(
    settings,
    [
        CloseSettings,
        SearchSettings,
        FocusSettingsNavbar,
        NextSettingsControl,
        PreviousSettingsControl
    ]
);

pub(crate) fn register_shortcuts(registry: &mut codesh_ui::shortcuts::Registry<'_>) {
    use codesh_core::shortcuts::ShortcutId;
    registry.register(ShortcutId::CloseSettings, &CloseSettings);
    registry.register(ShortcutId::SearchSettings, &SearchSettings);
    registry.register(ShortcutId::FocusSettingsNavbar, &FocusSettingsNavbar);
    registry.register(ShortcutId::NextSettingsControl, &NextSettingsControl);
    registry.register(
        ShortcutId::PreviousSettingsControl,
        &PreviousSettingsControl,
    );
}
