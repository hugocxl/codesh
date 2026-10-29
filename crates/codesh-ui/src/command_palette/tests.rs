use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription,
    TestAppContext, VisualTestContext, Window, div,
};

use super::*;
use crate::theme::ColorScheme;

type Event = PaletteEvent<&'static str>;

struct Host {
    palette: Entity<Palette<&'static str>>,
    events: Vec<Event>,
    _subscription: Subscription,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().child(self.palette.clone())
    }
}

fn open(cx: &mut TestAppContext) -> (Entity<Host>, &mut VisualTestContext) {
    cx.update(|cx| {
        cx.bind_keys(text_input::key_bindings());
        cx.bind_keys(crate::picker::key_bindings());
        cx.bind_keys(key_bindings());
    });
    let (host, cx) = cx.add_window_view(|_, cx| {
        let palette = cx.new(|cx| {
            Palette::new(
                PaletteConfig::new("palette", PageSpec::new("root", "Search")),
                Theme::from_scheme(&ColorScheme::default()),
                Metrics::new(1.0),
                cx,
            )
        });
        let subscription = cx.subscribe(&palette, |host: &mut Host, _, event: &Event, _| {
            host.events.push(event.clone());
        });
        Host {
            palette,
            events: Vec::new(),
            _subscription: subscription,
        }
    });
    cx.run_until_parked();
    (host, cx)
}

fn take(host: &Entity<Host>, cx: &mut VisualTestContext) -> Vec<Event> {
    cx.run_until_parked();
    host.update(cx, |host, _| std::mem::take(&mut host.events))
}

fn root_items() -> PageContent<&'static str> {
    PageContent::sections(vec![Section::new(
        "Commands",
        vec![
            Item::run("alpha", "Alpha", "run alpha").actions(vec![
                ItemAction::new("open", "Open"),
                ItemAction::new("rename", "Rename"),
                ItemAction::new("delete", "Delete").destructive(),
            ]),
            Item::page("files", "Go to File…", "files"),
        ],
    )])
}

fn query(page: &str, query: &str) -> Event {
    PaletteEvent::QueryChanged {
        page: page.to_owned().into(),
        query: query.to_owned().into(),
    }
}

/// Pushes the "files" page and fills it, as a host answering `OpenPage` does.
fn push_files(
    host: &Entity<Host>,
    palette: &Entity<Palette<&'static str>>,
    cx: &mut VisualTestContext,
) {
    palette.update(cx, |palette, cx| {
        palette.push_page(
            PageSpec::new("files", "Go to file…").breadcrumb("Files"),
            cx,
        );
        palette.set_content(
            "files",
            PageContent::items(vec![Item::run("main.rs", "main.rs", "open main")]),
            cx,
        );
    });
    assert_eq!(
        take(host, cx),
        [
            query("files", ""),
            PaletteEvent::SelectionChanged {
                page: "files".into(),
                item: Some("main.rs".into()),
            },
        ]
    );
}

fn open_with_root_items(
    cx: &mut TestAppContext,
) -> (
    Entity<Host>,
    Entity<Palette<&'static str>>,
    &mut VisualTestContext,
) {
    let (host, cx) = open(cx);
    let palette = host.read_with(cx, |host, _| host.palette.clone());
    assert_eq!(take(&host, cx), [query("root", "")]);
    palette.update(cx, |palette, cx| {
        palette.set_content("root", root_items(), cx);
    });
    assert_eq!(
        take(&host, cx),
        [PaletteEvent::SelectionChanged {
            page: "root".into(),
            item: Some("alpha".into()),
        }]
    );
    (host, palette, cx)
}

#[gpui::test]
fn keyboard_flow_through_pages(cx: &mut TestAppContext) {
    let (host, palette, cx) = open_with_root_items(cx);

    cx.simulate_input("al");
    assert_eq!(take(&host, cx), [query("root", "a"), query("root", "al")]);

    cx.simulate_keystrokes("enter");
    assert_eq!(
        take(&host, cx),
        [PaletteEvent::Run {
            page: "root".into(),
            item: "alpha".into(),
            payload: "run alpha",
        }]
    );

    cx.simulate_keystrokes("down enter");
    assert_eq!(
        take(&host, cx),
        [
            PaletteEvent::SelectionChanged {
                page: "root".into(),
                item: Some("files".into()),
            },
            PaletteEvent::OpenPage {
                page: "files".into(),
                parent: "files".into(),
            },
        ]
    );

    push_files(&host, &palette, cx);
    assert_eq!(palette.read_with(cx, |palette, _| palette.depth()), 2);
    cx.simulate_keystrokes("escape");
    assert_eq!(
        take(&host, cx),
        [PaletteEvent::PagePopped {
            page: "files".into()
        }]
    );
    assert_eq!(
        palette.read_with(cx, |palette, _| (
            palette.depth(),
            palette.query().to_owned()
        )),
        (1, "al".to_owned())
    );

    push_files(&host, &palette, cx);
    cx.simulate_input("x");
    cx.simulate_keystrokes("backspace");
    assert_eq!(take(&host, cx), [query("files", "x"), query("files", "")]);
    cx.simulate_keystrokes("backspace");
    assert_eq!(
        take(&host, cx),
        [PaletteEvent::PagePopped {
            page: "files".into()
        }]
    );
    assert!(palette.read_with(cx, |palette, _| palette.is_page("root")));

    cx.simulate_keystrokes("escape");
    assert_eq!(take(&host, cx), [PaletteEvent::Dismissed]);
}

#[gpui::test]
fn actions_panel_runs_actions_and_arms_destructive_ones(cx: &mut TestAppContext) {
    let (host, palette, cx) = open_with_root_items(cx);
    let toggle = if cfg!(target_os = "macos") {
        "cmd-k"
    } else {
        "ctrl-k"
    };

    cx.simulate_keystrokes(toggle);
    assert!(palette.read_with(cx, |palette, _| palette.actions_open()));
    cx.simulate_keystrokes("down enter");
    assert_eq!(
        take(&host, cx),
        [PaletteEvent::RunAction {
            page: "root".into(),
            item: "alpha".into(),
            action: "rename".into(),
            payload: "run alpha",
        }]
    );
    assert!(!palette.read_with(cx, |palette, _| palette.actions_open()));

    cx.simulate_keystrokes(toggle);
    cx.simulate_keystrokes("up enter");
    assert!(take(&host, cx).is_empty());
    assert!(palette.read_with(cx, |palette, _| palette.actions_open()));
    cx.simulate_keystrokes("enter");
    assert_eq!(
        take(&host, cx),
        [PaletteEvent::RunAction {
            page: "root".into(),
            item: "alpha".into(),
            action: "delete".into(),
            payload: "run alpha",
        }]
    );

    cx.simulate_keystrokes(toggle);
    cx.simulate_keystrokes("escape");
    assert!(take(&host, cx).is_empty());
    assert!(!palette.read_with(cx, |palette, _| palette.actions_open()));
    cx.simulate_keystrokes("escape");
    assert_eq!(take(&host, cx), [PaletteEvent::Dismissed]);
}

#[gpui::test]
fn actions_panel_refresh_preserves_highlight_and_destructive_confirmation(cx: &mut TestAppContext) {
    let (host, palette, cx) = open_with_root_items(cx);
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-k"
    } else {
        "ctrl-k"
    });
    cx.simulate_keystrokes("up enter");
    assert!(take(&host, cx).is_empty());
    palette.update(cx, |palette, cx| {
        let mut content = root_items();
        content.sections[0].items[0]
            .actions
            .insert(1, ItemAction::new("copy", "Copy"));
        palette.set_content("root", content, cx);
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(
        take(&host, cx),
        [PaletteEvent::RunAction {
            page: "root".into(),
            item: "alpha".into(),
            action: "delete".into(),
            payload: "run alpha",
        }]
    );
}
