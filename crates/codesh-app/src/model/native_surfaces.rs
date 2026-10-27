//! Native browser surfaces, occlusions and resize grips shared with panels.
use codesh_app_core::{PaneContent, PaneId, Tab};
use gpui::{Context, Window};

use super::AppModel;
type Occlusion = (Option<gpui::EntityId>, gpui::Bounds<gpui::Pixels>);
type Grip = (
    Option<gpui::EntityId>,
    gpui::Bounds<gpui::Pixels>,
    gpui::CursorStyle,
);
#[derive(Default)]
pub(crate) struct Webviews {
    pub occlusions: std::rc::Rc<std::cell::RefCell<Vec<Occlusion>>>,
    grips: std::rc::Rc<std::cell::RefCell<Vec<Grip>>>,
}

impl AppModel {
    pub(crate) fn sync_native_surfaces(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.webviews.occlusions.borrow_mut().clear();
        self.webviews.grips.borrow_mut().clear();
        let blocked = self.overlay.is_some() || self.close_prompt.is_some();
        let resizing = self.sidebar_resize.is_some()
            || self.agents_panel.resize.is_active()
            || self.agent_sessions.resize.is_active()
            || self.native.resize.is_active()
            || self.split_resize.active()
            || self.layout_drag_pending();
        self.spinners
            .set_blocked(resizing || self.overlay_unmarked());
        self.sync_browsers(blocked, resizing, window, cx);
    }

    fn overlay_unmarked(&self) -> bool {
        self.overlay
            .as_ref()
            .is_some_and(|overlay| !overlay.marks_bounds())
    }

    pub(super) fn pane_drag_available(&self, pane: PaneId) -> bool {
        self.pane_tab(pane)
            .and_then(|tab| self.tab(tab))
            .is_some_and(|tab| tab.zoomed.is_none() && tab.panes.len() > 1)
    }

    pub(crate) fn webview_occlusion(&self) -> gpui::AnyElement {
        self.webview_occlusion_except(None)
    }

    pub(crate) fn webview_occlusion_except(
        &self,
        except: Option<gpui::EntityId>,
    ) -> gpui::AnyElement {
        use gpui::{IntoElement, Styled};
        let occlusions = self.webviews.occlusions.clone();
        gpui::canvas(
            move |bounds, _, _| occlusions.borrow_mut().push((except, bounds)),
            |_, (), _, _| (),
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element()
    }

    pub(crate) fn webview_grip(
        &self,
        owner: Option<gpui::EntityId>,
        cursor: gpui::CursorStyle,
    ) -> gpui::AnyElement {
        use gpui::{IntoElement, Styled};
        let grips = self.webviews.grips.clone();
        gpui::canvas(
            move |bounds, window, _| {
                let visible = bounds.intersect(&window.content_mask().bounds);
                grips.borrow_mut().push((owner, visible, cursor));
            },
            |_, (), _, _| (),
        )
        .absolute()
        .size_full()
        .into_any_element()
    }

    pub(crate) fn blur_webviews(&self, cx: &gpui::App) {
        for surface in self.browsers.views.values() {
            surface.view.read(cx).native.blur();
        }
    }

    pub(crate) fn webview_title<'a>(&'a self, tab: &'a Tab, cx: &'a gpui::App) -> &'a str {
        if tab.custom_title.is_none()
            && let Some(pane) = tab.displayed_pane(self.state.shown_pane(tab))
            && matches!(pane.content, PaneContent::Browser(_))
            && let Some(title) = self.browser_title(pane.id, cx)
        {
            return title;
        }
        tab.title(self.state.shown_pane(tab))
    }

    pub(crate) fn webview_glyph(
        &self,
        tab: &Tab,
        size: gpui::Pixels,
        foreground: gpui::Hsla,
        cx: &gpui::App,
    ) -> Option<gpui::AnyElement> {
        use gpui::IntoElement;
        let pane = tab.displayed_pane(self.state.shown_pane(tab))?;
        if tab.pinned {
            return None;
        }
        if matches!(pane.content, PaneContent::Browser(_)) {
            return Some(self.browser_glyph(pane.id, size, foreground, cx));
        }
        if pane.content == PaneContent::Servers {
            return Some(
                codesh_ui::components::IconGlyph::new(
                    codesh_ui::icon::Icon::Network,
                    size,
                    foreground,
                )
                .into_any_element(),
            );
        }
        None
    }

    pub(crate) fn apply_webview_regions(&self, cx: &gpui::App) -> gpui::AnyElement {
        use gpui::{IntoElement, Styled};
        let browsers: Vec<_> = self
            .browsers
            .views
            .values()
            .map(|surface| {
                (
                    surface.view.entity_id(),
                    surface.view.read(cx).native.clone(),
                )
            })
            .collect();
        let occlusions = self.webviews.occlusions.clone();
        let covers = self.webviews.occlusions.clone();
        let grips = self.webviews.grips.clone();
        let spinners = self.spinners.clone();
        gpui::canvas(
            move |_, _, _| {
                for (id, view) in &browsers {
                    view.occlude(&occlusions_for(*id, &occlusions.borrow()));
                    view.set_grips(&grips_for(*id, true, &grips.borrow()));
                }
            },
            // Painting starts after every element, overlays included, is laid out.
            move |_, (), _, _| {
                spinners.present(&covers.borrow());
            },
        )
        .absolute()
        .size_full()
        .into_any_element()
    }

    pub(crate) fn place_panel(
        &mut self,
        placement: codesh_ui::panel::PanelPlacement,
        cx: &mut Context<Self>,
    ) {
        if let Some(displacement) = self.panels.place(placement) {
            let id = displacement.displaced.id;
            if id.as_str() == crate::model::agents_panel::PANEL {
                self.agents_panel.drag = None;
                self.agents_panel.resize.end();
                self.appearance.agents_panel_visible = false;
                self.save_appearance(cx);
            } else if id.as_str() == crate::model::agent_sessions::PANEL {
                self.appearance.agent_sessions_visible = false;
                self.sync_agent_sessions(cx);
                self.save_appearance(cx);
            } else if [
                crate::model::native::FILES_PANEL,
                crate::model::native::GIT_PANEL,
            ]
            .contains(&id.as_str())
            {
                self.side_panel_displaced(cx);
            }
        }
        cx.notify();
    }
}

pub(super) fn dispatch_shortcut(key: gpui::Keystroke, window: &Window, cx: &mut gpui::App) {
    window.defer(cx, move |window, cx| {
        window.dispatch_keystroke(key, cx);
    });
}

fn grips_for(
    id: gpui::EntityId,
    pane: bool,
    grips: &[Grip],
) -> Vec<(gpui::Bounds<gpui::Pixels>, gpui::CursorStyle)> {
    grips
        .iter()
        .filter(|(owner, _, _)| *owner == Some(id) || (pane && owner.is_none()))
        .map(|(_, bounds, cursor)| (*bounds, *cursor))
        .collect()
}

fn occlusions_for(id: gpui::EntityId, occlusions: &[Occlusion]) -> Vec<gpui::Bounds<gpui::Pixels>> {
    let start = occlusions
        .iter()
        .position(|(owner, _)| *owner == Some(id))
        .map_or(0, |index| index + 1);
    occlusions[start..]
        .iter()
        .map(|(_, bounds)| *bounds)
        .collect()
}
