use codesh_ui::picker::{Picker, PickerConfig, PickerEvent, PickerItem, PickerRow, PickerStatus};
use codesh_ui::theme::{Metrics, Theme};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Subscription,
};

pub(crate) enum ThemeEvent {
    Selected(String),
    Dismiss,
}

pub(crate) struct ThemeDropdown {
    entries: Vec<crate::theme::Entry>,
    active: String,
    picker: Entity<Picker>,
    _subscription: Subscription,
}

impl EventEmitter<ThemeEvent> for ThemeDropdown {}

impl Focusable for ThemeDropdown {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.picker.read(cx).input().focus_handle(cx)
    }
}

impl ThemeDropdown {
    pub(crate) fn new(
        mut entries: Vec<crate::theme::Entry>,
        active: String,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        entries.sort_by_cached_key(|entry| (entry.name.to_lowercase(), entry.name.clone()));
        let picker = cx.new(|cx| {
            Picker::new(
                PickerConfig::dropdown("theme-browser", "Search themes…"),
                theme,
                metrics,
                cx,
            )
        });
        let subscription = cx.subscribe(&picker, |browser: &mut Self, _, event, cx| match event {
            PickerEvent::QueryChanged { query, .. } => browser.sync_picker(query, cx),
            PickerEvent::Confirmed(selection) | PickerEvent::SecondaryConfirmed(selection) => {
                if let Some(entry) = selection
                    .id
                    .strip_prefix("theme-")
                    .and_then(|index| index.parse::<usize>().ok())
                    .and_then(|index| browser.entries.get(index))
                {
                    cx.emit(ThemeEvent::Selected(entry.name.clone()));
                }
            }
            PickerEvent::Dismissed => cx.emit(ThemeEvent::Dismiss),
            _ => {}
        });
        let browser = Self {
            entries,
            active,
            picker,
            _subscription: subscription,
        };
        browser.sync_picker("", cx);
        browser
    }

    pub(crate) fn set_appearance(&self, theme: Theme, metrics: Metrics, cx: &mut Context<Self>) {
        self.picker
            .update(cx, |picker, cx| picker.set_appearance(theme, metrics, cx));
    }

    pub(crate) fn set_current(&mut self, name: &str, cx: &mut Context<Self>) {
        if self.active == name {
            return;
        }
        name.clone_into(&mut self.active);
        let id = self
            .entries
            .iter()
            .position(|entry| entry.name == name)
            .map_or_else(String::new, |index| format!("theme-{index}"));
        self.picker
            .update(cx, |picker, cx| picker.set_current_row(&id, cx));
    }

    fn sync_picker(&self, query: &str, cx: &mut Context<Self>) {
        let query = query.trim().to_lowercase();
        let items: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.name.to_lowercase().contains(&query))
            .map(|(index, entry)| {
                let mut row = PickerRow::new(format!("theme-{index}"), entry.name.clone());
                row.current = entry.name == self.active;
                row.swatches = entry.swatches();
                PickerItem::Row(row)
            })
            .collect();
        let status = if items.is_empty() {
            PickerStatus::Empty("No themes found".into())
        } else {
            PickerStatus::Ready
        };
        self.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
        });
    }
}

impl Render for ThemeDropdown {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
        self.picker.clone()
    }
}
