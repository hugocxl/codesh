use super::{Category, Change, PickerKind, SettingsView};
use codesh_ui::{l10n::tr_key, tr};
use gpui::{AnyElement, Context, IntoElement};

pub(super) fn rows(view: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let settings = &view.snapshot.settings.voice;
    let mut rows = Vec::new();
    if view.matches(Category::General, tr_key!("Send after recording")) {
        rows.push(view.row(
            "voice-auto-send",
            tr_key!("Send after recording"),
            view.toggle(
                "voice-auto-send",
                settings.auto_send,
                Change::VoiceAutoSend(!settings.auto_send),
                cx,
            ),
        ));
    }
    if view.matches(Category::General, tr_key!("Dictation language")) {
        let language = if settings.language.is_empty() {
            tr!("System language")
        } else {
            settings.language.clone().into()
        };
        rows.push(view.row(
            "voice-language",
            tr_key!("Dictation language"),
            view.picker(PickerKind::Language, &language, cx),
        ));
        if let Some(reason) = view
            .snapshot
            .capabilities
            .unsupported(codesh_core::capability::Feature::Voice)
        {
            rows.push(view.note(&reason, false).into_any_element());
        }
    }
    if codesh_ui::voice::LOCAL_MODEL
        && view.matches(Category::General, tr_key!("Offline voice model"))
    {
        rows.push(view.row(
            "voice-model",
            tr_key!("Offline voice model"),
            view.field("voice-model"),
        ));
        rows.push(view.note(&tr!("Install whisper-cli and parec, then choose a multilingual GGML model file. Leave blank to use ggml-base.bin in your user data folder under codesh/voice. That folder must also contain ggml-silero-v6.2.0.bin for speech detection. Audio stays on this computer and is transcribed after you finish."), false).into_any_element());
    }
    rows
}
