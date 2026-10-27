use std::cell::RefCell;

use super::*;
use codesh_app_core::PaneContent;
use serde_json::{Value, json};

/// Sends a `browser.*` request as the CLI does and waits for the reply.
fn call(
    view: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    verb: &str,
    args: &Value,
) -> std::result::Result<Value, String> {
    let args = args.as_object().cloned().unwrap_or_default();
    let reply = view.update_in(cx, |model, window, cx| {
        model.browser_call(verb, &args, window, cx)
    });
    let result = Rc::new(RefCell::new(None));
    let slot = Rc::clone(&result);
    cx.update(|_, cx| {
        cx.spawn(async move |_| {
            *slot.borrow_mut() = Some(reply.await);
        })
        .detach();
    });
    cx.run_until_parked();
    result.borrow_mut().take().expect("reply")
}

fn browser_pane(model: &AppModel) -> Option<PaneId> {
    model.browser_panes().first().map(|(_, _, pane, _)| *pane)
}

#[gpui::test]
fn sidebar_footer_opens_browser_panes_after_layout_and_group_requests(cx: &mut TestAppContext) {
    use codesh_app_core::{Direction, settings::AppLayout};

    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), std::env::current_dir().expect("cwd"))
        .expect("project");
    state.select_project(project).expect("select");
    let first = state.open_terminal_tab(project).expect("first tab");
    let second = state.open_terminal_tab(project).expect("second tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for grouped in [false, true] {
        if grouped {
            view.update(cx, |model, cx| {
                model
                    .state
                    .split_tab(second, first, Direction::Right)
                    .expect("split tabs into groups");
                model.changed(cx);
            });
        }
        for layout in [
            AppLayout::ProjectFocused,
            AppLayout::TabFocused,
            AppLayout::AgentsFocused,
        ] {
            view.update(cx, |model, cx| model.set_layout(layout, cx));
            cx.run_until_parked();
            let (count, previous) = view.read_with(cx, |model, _| {
                (
                    model.state.current_project().tabs[0].panes.len(),
                    model.active_pane(),
                )
            });
            let button = cx
                .debug_bounds("browser-button")
                .unwrap_or_else(|| panic!("browser unavailable in {layout:?}, grouped={grouped}"));
            cx.simulate_click(button.center(), Modifiers::default());
            cx.run_until_parked();
            view.read_with(cx, |model, _| {
                let project = model.state.current_project();
                assert_eq!(project.tabs.len(), 1);
                assert_eq!(project.tabs[0].panes.len(), count + 1);
                assert_eq!(model.active_tab(), Some(first));
                let pane = model.active_pane().expect("new pane selected");
                assert_ne!(Some(pane), previous);
                assert!(model.state.browser(pane).is_some());
                assert!(project.groups().is_none());
                assert_eq!(
                    project.tabs[0]
                        .layout
                        .neighbor(previous.expect("previous pane"), Direction::Right),
                    Some(pane)
                );
                assert!(model.error.is_none(), "{:?}", model.error);
            });
        }
    }
}

#[gpui::test]
fn new_browser_tabs_open_on_the_start_page_and_requests_drive_them(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, AppModel::new_browser_tab);
    cx.run_until_parked();
    let pane = view.read_with(cx, |model, _| {
        let pane = browser_pane(model).expect("browser pane");
        assert_eq!(model.active_pane(), Some(pane));
        assert_eq!(model.state.browser(pane).and_then(|b| b.url.clone()), None);
        pane
    });
    let tab = pane.to_string();

    let listed = call(&view, cx, "browser.list", &json!({})).expect("list");
    assert_eq!(listed[0]["tabId"], json!(tab));
    assert_eq!(listed[0]["url"], json!(""));
    assert_eq!(listed[0]["active"], json!(true));
    assert_eq!(listed[0]["profile"], json!("Default"));

    call(
        &view,
        cx,
        "browser.navigate",
        &json!({"tabId": tab, "url": "example.com/docs"}),
    )
    .expect("navigate");
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.state.browser(pane).and_then(|b| b.url.as_deref()),
            Some("https://example.com/docs")
        );
    });

    assert_eq!(
        call(&view, cx, "browser.click", &json!({"tabId": tab})),
        Err("missing argument 'selector'".into())
    );
    assert_eq!(
        call(
            &view,
            cx,
            "browser.click",
            &json!({"tabId": "nope", "selector": "a"})
        ),
        Err("browser tab not found nope".into())
    );
    assert_eq!(
        call(&view, cx, "browser.teleport", &json!({"tabId": tab})),
        Err("unknown command browser.teleport".into())
    );
    assert_eq!(
        call(
            &view,
            cx,
            "browser.navigate",
            &json!({"tabId": tab, "url": " "})
        ),
        Err("invalid url".into())
    );

    call(&view, cx, "browser.close", &json!({"tabId": tab})).expect("close");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(browser_pane(model).is_none()));
}

#[gpui::test]
fn opening_in_another_project_leaves_the_view_and_split_opens_beside(cx: &mut TestAppContext) {
    let (state, first, second, first_pane, _) = projects::two_projects();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| model.select_project(first, cx));
    cx.run_until_parked();

    let opened = call(
        &view,
        cx,
        "browser.open",
        &json!({"url": "https://example.com", "project": second.to_string()}),
    )
    .expect("open");
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, first);
        assert_eq!(model.active_pane(), Some(first_pane));
        let pane: PaneId = opened.as_str().expect("tab ID").parse().expect("pane");
        assert_eq!(model.state.pane_project(pane), Some(second));
    });
    let listed = call(
        &view,
        cx,
        "browser.list",
        &json!({"project": first.to_string()}),
    )
    .expect("list");
    assert_eq!(listed, json!([]));

    let split = call(&view, cx, "browser.open", &json!({"split": true})).expect("split");
    view.read_with(cx, |model, _| {
        let pane: PaneId = split.as_str().expect("tab ID").parse().expect("pane");
        assert_eq!(model.active_pane(), Some(pane));
        let tab = model.tab(model.active_tab().expect("tab")).expect("tab");
        assert_eq!(tab.panes.len(), 2);
        assert!(tab.panes.iter().any(|candidate| candidate.id == first_pane));
        assert!(matches!(
            model.state.browser(pane),
            Some(browser) if browser.url.is_none()
        ));
    });
}

#[gpui::test]
fn a_disabled_browser_opens_nothing_and_lists_nothing(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let (_, pane) = state
        .open_browser(home, codesh_app_core::browser::BrowserPane::default(), true)
        .expect("browser");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.browser.enabled = false;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, AppModel::new_browser_tab);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.browser_panes().len(), 1);
        assert!(model.browser_view(pane).is_none());
        assert_eq!(
            model.browser_unavailable(pane).as_deref(),
            Some("Built-in browser is disabled")
        );
    });
    assert_eq!(call(&view, cx, "browser.list", &json!({})), Ok(json!([])));
    assert_eq!(
        call(&view, cx, "browser.open", &json!({})),
        Err("the built-in browser is disabled".into())
    );
}

#[gpui::test]
fn urls_open_in_a_new_browser_tab_of_the_current_project(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.open_url_in_browser("https://example.com/".into(), cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        let panes = model.browser_panes();
        assert_eq!(panes.len(), 1);
        let PaneContent::Browser(browser) = &model.state.home().tabs[0].panes[0].content else {
            panic!("opened pane should be a browser");
        };
        assert_eq!(browser.url.as_deref(), Some("https://example.com/"));
    });
}

fn saved_profile(model: &AppModel, pane: PaneId) -> Option<String> {
    model
        .state
        .browser(pane)
        .and_then(|browser| browser.profile.clone())
}

#[gpui::test]
fn tabs_keep_the_profile_they_opened_with_when_the_default_changes(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let opened = call(&view, cx, "browser.open", &json!({})).expect("open");
    let pane: PaneId = opened.as_str().expect("tab ID").parse().expect("pane");
    view.update(cx, |model, _| {
        let default = model
            .browser_profile(None)
            .expect("default")
            .id()
            .to_string();
        assert_eq!(saved_profile(model, pane), Some(default.clone()));
        model.add_browser_profile("Work").expect("profile");
        let work = model.browser_profile(Some("Work")).expect("work");
        model.settings.browser.default_profile = work.id().to_string();
        assert_eq!(saved_profile(model, pane), Some(default.clone()));
        assert_eq!(
            model
                .pane_profile(saved_profile(model, pane).as_deref())
                .expect("tab profile")
                .name(),
            "Default"
        );
    });
}

#[gpui::test]
fn browser_cancellation_does_not_show_a_copy_notice(cx: &mut TestAppContext) {
    use crate::views::browser::BrowserViewEvent;

    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        model.new_browser_tab(cx);
        let pane = browser_pane(model).expect("browser pane");
        model.browser_event(pane, &BrowserViewEvent::Escape, window, cx);
        assert!(model.notice.is_none());
        model.browser_event(pane, &BrowserViewEvent::Copied, window, cx);
        assert_eq!(model.notice.as_deref(), Some("Copied!"));
    });
}
