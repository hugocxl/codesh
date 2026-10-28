use codesh_app_core::ProjectId;
use codesh_protocol::FileEntry;
use codesh_ui::components::{IconButton, IconGlyph};
use codesh_ui::icon::Icon;
use codesh_ui::text_input::{InputEvent, InputStyle, TextInput};
use codesh_ui::theme::{Metrics, Theme};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, Pixels, Point, Render, ScrollStrategy,
    Styled, Subscription, UniformListScrollHandle, Window, div, px, uniform_list,
};

use crate::model::files_panel::{Node, Tree, parent};
use crate::views::codesh::style;

pub(crate) enum FilesPanelEvent {
    Close,
    List {
        path: String,
    },
    Open {
        path: String,
    },
    CreateFile {
        path: String,
    },
    CreateFolder {
        path: String,
    },
    Rename {
        from: String,
        to: String,
    },
    Delete {
        path: String,
    },
    /// The app menu needs project metadata for absolute paths and local reveal.
    ContextMenu {
        path: String,
        position: Point<Pixels>,
    },
}

enum Edit {
    Rename(String),
    File(String),
    Folder(String),
}

pub(crate) struct FilesPanel {
    name: String,
    theme: Theme,
    metrics: Metrics,
    focus: FocusHandle,
    tree: Tree,
    rows: Vec<(Node, usize)>,
    scroll: UniformListScrollHandle,
    error: Option<String>,
    edit: Option<Edit>,
    return_focus: bool,
    input: Entity<TextInput>,
    _input_subscription: Subscription,
}

impl EventEmitter<FilesPanelEvent> for FilesPanel {}

impl FilesPanel {
    pub(crate) fn new(
        _project: ProjectId,
        theme: &Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::new(InputStyle::compact(theme, &metrics), cx));
        let subscription = cx.subscribe(&input, |panel, _, event, cx| match event {
            InputEvent::Submitted => panel.submit(cx),
            InputEvent::Cancelled => {
                panel.edit = None;
                panel.return_focus = true;
                panel.error = None;
                cx.notify();
            }
            InputEvent::Changed => {
                panel.error = None;
                cx.notify();
            }
        });
        let mut tree = Tree::default();
        tree.request("");
        // Defer until the owner has installed its event subscription.
        let owner = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |_, cx| {
                cx.emit(FilesPanelEvent::List {
                    path: String::new(),
                });
            });
        });
        Self {
            name: "Files".into(),
            theme: theme.clone(),
            metrics,
            focus: cx.focus_handle(),
            tree,
            rows: Vec::new(),
            scroll: UniformListScrollHandle::new(),
            error: None,
            edit: None,
            return_focus: false,
            input,
            _input_subscription: subscription,
        }
    }

    pub(crate) fn set_project_name(&mut self, name: String, cx: &mut Context<Self>) {
        self.name = name;
        cx.notify();
    }

    pub(crate) fn set_entries(
        &mut self,
        dir: &str,
        entries: Vec<FileEntry>,
        cx: &mut Context<Self>,
    ) {
        self.error = None;
        if self.tree.set_entries(dir, entries) {
            cx.emit(FilesPanelEvent::List {
                path: dir.to_owned(),
            });
        }
        self.changed(cx);
    }

    pub(crate) fn refresh(&mut self, paths: &[String], rescan: bool, cx: &mut Context<Self>) {
        for path in self.tree.refresh(paths, rescan) {
            cx.emit(FilesPanelEvent::List { path });
        }
        self.changed(cx);
    }

    pub(crate) fn reconnect(&mut self, cx: &mut Context<Self>) {
        self.tree.retry();
        self.error = None;
        self.refresh(&[], true, cx);
    }

    pub(crate) fn ensure_root(&mut self, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            self.refresh(&[], true, cx);
        }
    }

    pub(crate) fn set_error(&mut self, message: Option<String>, cx: &mut Context<Self>) {
        if message.is_some() {
            self.tree.retry();
        }
        self.error = message
            .filter(|error| !crate::model::native::connecting(error))
            .map(|error| crate::model::native::friendly_error(&error, true));
        cx.notify();
    }

    pub(crate) fn set_theme(&mut self, theme: &Theme, cx: &mut Context<Self>) {
        self.theme = theme.clone();
        self.input.update(cx, |input, cx| {
            input.set_style(InputStyle::compact(theme, &self.metrics), cx);
        });
        cx.notify();
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.rows = self
            .tree
            .visible()
            .into_iter()
            .map(|(node, depth)| (node.clone(), depth))
            .collect();
        for (node, _) in &self.rows {
            if node.directory && self.tree.expanded(&node.path) && self.tree.request(&node.path) {
                cx.emit(FilesPanelEvent::List {
                    path: node.path.clone(),
                });
            }
        }
        cx.notify();
    }

    fn expand(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.tree.expand(path) {
            cx.emit(FilesPanelEvent::List {
                path: path.to_owned(),
            });
        }
    }

    pub(crate) fn open(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.tree.node(path).is_some_and(|node| node.directory) {
            if self.tree.expanded(path) {
                self.tree.collapse(path);
            } else {
                self.expand(path, cx);
            }
            self.changed(cx);
        } else {
            cx.emit(FilesPanelEvent::Open {
                path: path.to_owned(),
            });
            cx.notify();
        }
    }

    fn begin_edit(&mut self, edit: Edit, window: &mut Window, cx: &mut Context<Self>) {
        match &edit {
            Edit::File(dir) | Edit::Folder(dir) => {
                if self.tree.request(dir) {
                    cx.emit(FilesPanelEvent::List { path: dir.clone() });
                }
            }
            Edit::Rename(_) => {}
        }
        let text = match &edit {
            Edit::Rename(path) => path.rsplit('/').next().unwrap_or_default(),
            _ => "",
        };
        self.input.update(cx, |input, cx| {
            input.set_text(text.to_owned(), cx);
            input.select_all_text(cx);
        });
        self.edit = Some(edit);
        self.error = None;
        self.input.focus_handle(cx).focus(window);
        cx.notify();
    }

    pub(crate) fn rename(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.tree.selected = Some(path.to_owned());
        self.begin_edit(Edit::Rename(path.to_owned()), window, cx);
    }

    pub(crate) fn new_file(
        &mut self,
        directory: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_edit(Edit::File(directory.to_owned()), window, cx);
    }

    pub(crate) fn new_folder(
        &mut self,
        directory: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_edit(Edit::Folder(directory.to_owned()), window, cx);
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let name = self.input.read(cx).text().to_owned();
        if name.trim().is_empty() || name == "." || name == ".." || name.contains(['/', '\\', '\0'])
        {
            self.error = Some("Enter a name without path separators.".into());
            cx.notify();
            return;
        }
        let Some(edit) = self.edit.take() else {
            return;
        };
        let dir = match &edit {
            Edit::Rename(path) => parent(path),
            Edit::File(dir) | Edit::Folder(dir) => dir,
        };
        if !self.tree.loaded(dir) {
            self.error = Some("Wait for the folder listing before choosing a name.".into());
            self.edit = Some(edit);
            cx.notify();
            return;
        }
        let path = if dir.is_empty() {
            name
        } else {
            format!("{dir}/{name}")
        };
        if self.tree.node(&path).is_some() && !matches!(&edit, Edit::Rename(from) if from == &path)
        {
            self.error = Some("A file or folder with this name already exists.".into());
            self.edit = Some(edit);
            cx.notify();
            return;
        }
        self.return_focus = true;
        self.error = None;
        match edit {
            Edit::Rename(from) if from != path => {
                cx.emit(FilesPanelEvent::Rename { from, to: path });
            }
            Edit::Rename(_) => {}
            Edit::File(_) => cx.emit(FilesPanelEvent::CreateFile { path }),
            Edit::Folder(_) => cx.emit(FilesPanelEvent::CreateFolder { path }),
        }
        cx.notify();
    }

    pub(crate) fn is_directory(&self, path: &str) -> bool {
        self.tree.node(path).is_some_and(|node| node.directory)
    }

    pub(crate) fn selected_directory(&self) -> String {
        self.tree
            .selected
            .as_ref()
            .map_or_else(String::new, |path| {
                if self.tree.node(path).is_some_and(|node| node.directory) {
                    path.clone()
                } else {
                    parent(path).to_owned()
                }
            })
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_some() {
            if event.keystroke.key == "escape" {
                self.edit = None;
                self.focus.focus(window);
                cx.stop_propagation();
                cx.notify();
            }
            return;
        }
        let path = self.tree.selected.clone();
        match event.keystroke.key.as_str() {
            "up" => self.tree.step(false),
            "down" => self.tree.step(true),
            "left" => self.tree.left(),
            "right" => {
                if let Some(path) = self.tree.right() {
                    cx.emit(FilesPanelEvent::List { path });
                }
            }
            "enter" => {
                if let Some(path) = path {
                    self.open(&path, cx);
                }
            }
            "f2" => {
                if let Some(path) = path {
                    self.begin_edit(Edit::Rename(path), window, cx);
                }
            }
            "backspace" if event.keystroke.modifiers.platform => {
                if let Some(path) = path {
                    cx.emit(FilesPanelEvent::Delete { path });
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        if matches!(event.keystroke.key.as_str(), "left" | "right") {
            self.changed(cx);
        } else {
            cx.notify();
        }
        if let Some(index) = self
            .rows
            .iter()
            .position(|(node, _)| Some(&node.path) == self.tree.selected.as_ref())
        {
            self.scroll.scroll_to_item(index, ScrollStrategy::Top);
        }
    }

    fn row(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (node, depth) = &self.rows[index];
        let path = node.path.clone();
        let click_path = path.clone();
        let selected = self.tree.selected.as_ref() == Some(&path);
        let rename = matches!(&self.edit, Some(Edit::Rename(edit_path)) if edit_path == &path);
        let glyph = if node.directory {
            Icon::Folder
        } else {
            Icon::File
        };
        let chevron = if self.tree.expanded(&path) {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        };
        div()
            .id(index)
            .h(px(24.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .pl(px(4.0 + indentation(*depth)))
            .pr(px(4.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(if selected {
                style::border(&self.theme)
            } else {
                gpui::transparent_black()
            })
            .when(selected, |row| row.bg(self.theme.surface))
            .hover(|row| row.bg(self.theme.hover))
            .when(node.dimmed, |row| row.text_color(self.theme.fg_muted))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |panel, _, window, cx| {
                    if matches!(&panel.edit, Some(Edit::Rename(path)) if path == &click_path) {
                        return;
                    }
                    panel.edit = None;
                    panel.focus.focus(window);
                    panel.tree.selected = Some(click_path.clone());
                    panel.open(&click_path, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |panel, event: &gpui::MouseDownEvent, _, cx| {
                    panel.tree.selected = Some(path.clone());
                    cx.emit(FilesPanelEvent::ContextMenu {
                        path: path.clone(),
                        position: event.position,
                    });
                    cx.notify();
                }),
            )
            .child(div().w(px(12.0)).flex_none().when(node.directory, |slot| {
                slot.child(IconGlyph::new(chevron, px(12.0), self.theme.fg_muted))
            }))
            .child(IconGlyph::new(
                glyph,
                px(14.0),
                if node.dimmed {
                    self.theme.fg_muted
                } else {
                    self.theme.fg
                },
            ))
            .child(div().flex_1().min_w_0().overflow_hidden().when_else(
                rename,
                |label| label.child(self.input.clone()),
                |label| label.child(node.name.clone()),
            ))
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "Indentation is a pixel coordinate"
)]
fn indentation(depth: usize) -> f32 {
    depth as f32 * 12.0
}

impl Focusable for FilesPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FilesPanel {
    #[allow(
        clippy::too_many_lines,
        reason = "Declarative panel header, inline edit, and virtualized tree layout"
    )]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.return_focus) {
            self.focus.focus(window);
        }
        let mut header = div()
            .flex()
            .items_center()
            .h(px(style::HEADER))
            .flex_none()
            .gap(style::spacing())
            .text_color(self.theme.fg_muted)
            .child(IconGlyph::new(
                Icon::Folder,
                self.metrics.font_footnote(),
                self.theme.fg_muted,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(self.name.clone()),
            );
        for (id, icon, label) in [
            ("new-file", Icon::File, "New File"),
            ("new-folder", Icon::Folder, "New Folder"),
            ("collapse", Icon::ChevronUp, "Collapse All"),
            ("refresh", Icon::Refresh, "Refresh"),
            ("close", Icon::X, "Close"),
        ] {
            header = header.child(
                IconButton::new(
                    id,
                    icon,
                    px(16.0),
                    px(20.0),
                    self.theme.fg_muted,
                    self.theme.fg,
                )
                .tooltip(
                    label,
                    self.theme.raised(),
                    self.theme.fg,
                    self.theme.border,
                    self.theme.bg,
                )
                .on_click(cx.listener(move |panel, _, window, cx| match id {
                    "close" => cx.emit(FilesPanelEvent::Close),
                    "new-file" => {
                        panel.begin_edit(Edit::File(panel.selected_directory()), window, cx);
                    }
                    "new-folder" => {
                        panel.begin_edit(Edit::Folder(panel.selected_directory()), window, cx);
                    }
                    "collapse" => {
                        panel.tree.collapse_all();
                        panel.changed(cx);
                    }
                    _ => {
                        panel.error = None;
                        panel.refresh(&[], true, cx);
                        if panel.rows.is_empty() && panel.tree.request("") {
                            cx.emit(FilesPanelEvent::List {
                                path: String::new(),
                            });
                        }
                    }
                })),
            );
        }
        let owner = cx.weak_entity();
        div()
            .id("files-panel")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .px(style::spacing())
            .pt(style::spacing())
            .text_color(self.theme.fg)
            .text_size(self.metrics.font_footnote())
            .on_key_down(cx.listener(Self::key_down))
            .child(header)
            .when(
                matches!(self.edit, Some(Edit::File(_) | Edit::Folder(_))),
                |view| {
                    view.child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_color(self.theme.fg_muted)
                                    .child(match &self.edit {
                                        Some(Edit::File(dir)) => format!(
                                            "New file in {}",
                                            if dir.is_empty() { "/" } else { dir }
                                        ),
                                        Some(Edit::Folder(dir)) => format!(
                                            "New folder in {}",
                                            if dir.is_empty() { "/" } else { dir }
                                        ),
                                        _ => String::new(),
                                    }),
                            )
                            .child(div().h(px(24.0)).child(self.input.clone())),
                    )
                },
            )
            .when(!self.tree.loaded("") && self.error.is_none(), |view| {
                view.child(
                    div()
                        .text_color(self.theme.fg_muted)
                        .child(codesh_ui::tr!("Loading…")),
                )
            })
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_color(self.theme.danger).child(error))
            })
            .child(
                uniform_list("files-tree", self.rows.len(), move |range, _, cx| {
                    owner
                        .update(cx, |panel, cx| {
                            range
                                .filter(|index| *index < panel.rows.len())
                                .map(|index| panel.row(index, cx).into_any_element())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .track_scroll(self.scroll.clone())
                .flex_1()
                .min_h_0(),
            )
    }
}
