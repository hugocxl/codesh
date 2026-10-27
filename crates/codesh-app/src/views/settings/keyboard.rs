use super::{Category, Change, SettingsEvent, SettingsView};
use codesh_ui::{controls, tr};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, Keystroke, ParentElement, Styled, div,
    prelude::FluentBuilder, px,
};

pub(super) fn matching(pane: &SettingsView) -> Vec<usize> {
    pane.shortcut_names
        .iter()
        .enumerate()
        .filter_map(|(index, name)| pane.matches(Category::Keyboard, name).then_some(index))
        .collect()
}

pub(super) fn row(pane: &SettingsView, index: usize, cx: &mut Context<SettingsView>) -> AnyElement {
    let shortcut = &codesh_core::shortcuts::ALL[index];
    let id = shortcut.id;
    let unavailable = shortcut
        .feature()
        .and_then(|feature| pane.snapshot.capabilities.reason(feature));
    let recording = pane.recording.as_deref() == Some(id);
    let chord = pane.snapshot.settings.keymap.binding(id);
    let label = if recording {
        tr!("Press a shortcut…")
    } else {
        chord.map_or_else(
            || tr!("Not assigned"),
            |chord| codesh_core::shortcuts::display_chord(chord.as_str()).into(),
        )
    };
    let control = div()
        .flex()
        .flex_wrap()
        .gap(px(6.0))
        .child(
            controls::button(
                pane.style(),
                id,
                &label,
                true,
                cx.listener(move |pane, _, window, cx| {
                    pane.begin_recording(id, window, cx);
                }),
            )
            .debug_selector(move || format!("settings-shortcut-{id}"))
            .h(px(22.0))
            .text_size(px(12.0))
            .track_focus(&pane.results.shortcut_focus[index][0]),
        )
        .when(!pane.snapshot.settings.keymap.is_default(id), |control| {
            control.child(
                controls::button(
                    pane.style(),
                    &format!("reset-{id}"),
                    "↺",
                    true,
                    cx.listener(move |pane, _, _, cx| {
                        pane.recording = None;
                        cx.emit(SettingsEvent::Change(Change::Binding(id.into(), None)));
                    }),
                )
                .bg(gpui::transparent_black())
                .border_0()
                .debug_selector(move || format!("settings-reset-{id}"))
                .track_focus(&pane.results.shortcut_focus[index][1]),
            )
        });
    pane.row_with_description(
        id,
        &pane.shortcut_names[index],
        unavailable.as_deref(),
        control.into_any_element(),
        pane.compact,
    )
}

impl SettingsView {
    pub(crate) fn begin_recording(
        &mut self,
        id: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.results.dirty = true;
        self.recording = Some(id.into());
        self.errors.remove(id);
        self.focus.focus(window);
        cx.notify();
    }

    pub(super) fn record_key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) {
        let Some(id) = self.recording.take() else {
            return;
        };
        self.results.dirty = true;
        cx.stop_propagation();
        if keystroke.key == "escape" && !keystroke.modifiers.modified() {
            cx.notify();
            return;
        }
        match recorded_chord(keystroke) {
            Ok(chord) if id == super::terminal_lists::BINDING_CHORD => {
                self.record_terminal_chord(chord);
            }
            Ok(chord) => cx.emit(SettingsEvent::Change(Change::Binding(id, Some(chord)))),
            Err(error) => {
                self.errors.insert(id, format!("{error}"));
            }
        }
        cx.notify();
    }
}

fn recorded_chord(
    keystroke: &Keystroke,
) -> codesh_app_core::settings::Result<codesh_app_core::settings::KeyChord> {
    let modifiers = keystroke.modifiers;
    let mut value = String::new();
    for (name, enabled) in [
        ("cmd", modifiers.platform),
        ("ctrl", modifiers.control),
        ("alt", modifiers.alt),
        ("shift", modifiers.shift),
        ("fn", modifiers.function),
    ] {
        if enabled {
            value.push_str(name);
            value.push('-');
        }
    }
    value.push_str(&keystroke.key);
    value.parse()
}
