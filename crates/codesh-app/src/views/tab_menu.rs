use codesh_app_core::{Color, Project, TabCloseScope, TabId};

use super::menu::{Command, Item, color_items};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Action {
    New,
    Rename,
    ResetTitle,
    /// Sets the tab's color to `TAB_COLORS[index]`.
    SetColor(usize),
    ResetColor,
    TogglePin,
    Close,
    CloseTabs(TabCloseScope),
}

pub(crate) fn items(project: &Project, id: TabId) -> Vec<Item> {
    let Some(tab) = project.tabs.iter().find(|tab| tab.id == id) else {
        return Vec::new();
    };
    let item = |label: gpui::SharedString, action| Item::action(label, Command::Tab(id, action));
    let mut items = vec![
        item("New Terminal".into(), Action::New),
        item("Rename Tab".into(), Action::Rename).separated(),
    ];
    if tab.custom_title.is_some() {
        items.push(item("Reset Title".into(), Action::ResetTitle));
    }
    let mut palette = color_items(tab.color.as_ref().map(Color::as_str), |index| {
        Command::Tab(id, Action::SetColor(index))
    })
    .into_iter();
    let mut colors =
        vec![item("Default".into(), Action::ResetColor).checked_if(tab.color.is_none())];
    colors.extend(palette.next().map(Item::separated));
    colors.extend(palette);
    items.push(Item::submenu("Color", colors));
    items.push(
        item(
            if tab.pinned {
                "Unpin Tab".into()
            } else {
                "Pin Tab".into()
            },
            Action::TogglePin,
        )
        .separated(),
    );
    if !tab.pinned {
        items.push(item("Close Tab".into(), Action::Close).separated());
    }
    for (label, scope) in [
        ("Close Other Tabs", TabCloseScope::Other),
        ("Close Tabs to the Left", TabCloseScope::Left),
        ("Close Tabs to the Right", TabCloseScope::Right),
    ] {
        let action = item(label.into(), Action::CloseTabs(scope));
        let action = if tab.pinned && scope == TabCloseScope::Other {
            action.separated()
        } else {
            action
        };
        items.push(if project.closable_tabs(id, scope).is_empty() {
            action.disabled()
        } else {
            action
        });
    }
    items
}
