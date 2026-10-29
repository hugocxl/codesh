//! The GTK thread: the pages, their web views and what the engine reports.
//!
//! A signal handler is given its page's id, never a pointer, and looks the
//! page up again each time, so a signal that arrives after a page closed
//! finds nothing to act on.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_int, c_uint};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc};
use std::time::Instant;

use gpui::{Bounds, CursorStyle, Modifiers, MouseButton, Pixels, Point, point, px, size};

use super::ffi::{
    self, Api, Callback, FALSE, GError, GType, GValue, GdkEventAny, GdkEventButton,
    GdkEventCrossing, GdkEventFocus, GdkEventKey, GdkEventMotion, GdkEventScroll,
    GdkEventSelection, GdkKeymapKey, GdkRGBA, GdkRectangle, TRUE, gboolean, gpointer,
};
use super::{Events, Job, Shared};
use crate::webview::{Event, MenuOption, OptionMenu, Pointer, Snapshot};

/// Casts a handler to the type `GObject` stores, checking it has the signature
/// the signal calls it with.
macro_rules! callback {
    ($handler:ident($($argument:ty),*) $(-> $result:ty)?) => {{
        // SAFETY: GObject calls the handler back with the signature named here.
        #[allow(unused_unsafe, reason = "Also used inside unsafe blocks")]
        let handler = unsafe {
            std::mem::transmute::<unsafe extern "C" fn($($argument),*) $(-> $result)?, Callback>(
                $handler,
            )
        };
        handler
    }};
}

pub(super) mod browser;
mod input;

#[cfg(test)]
mod tests;

/// What a page is created from.
pub(super) struct Setup {
    pub(super) id: u64,
    pub(super) url: String,
    pub(super) browser: Option<browser::Setup>,
    pub(super) transparent: bool,
    pub(super) events: Events,
    pub(super) shared: Arc<Shared>,
}

/// A key as GPUI names it.
pub(super) struct Key {
    pub(super) name: String,
    pub(super) text: Option<String>,
    pub(super) modifiers: Modifiers,
    pub(super) pressed: bool,
}

thread_local! {
    static RUNTIME: OnceCell<Rc<Runtime>> = const { OnceCell::new() };
}

fn runtime() -> Option<Rc<Runtime>> {
    RUNTIME.with(|runtime| runtime.get().cloned())
}

/// Has the GTK thread take the jobs posted to it.
pub(super) fn wake() {
    if let Some(g) = ffi::loaded() {
        // SAFETY: Adding an idle source is safe from any thread, and `drain`
        // takes no data.
        unsafe {
            g.g_idle_add_full(ffi::PRIORITY_DEFAULT, drain, ptr::null_mut(), None);
        }
    }
}

unsafe extern "C" fn drain(_data: gpointer) -> gboolean {
    if let Some(runtime) = runtime() {
        runtime.drain();
    }
    FALSE
}

/// Whether the system lets `WebKit` confine its web processes. It refuses to
/// run a page at all when asked for a sandbox it cannot build, so this asks
/// the same helper first.
fn sandbox_available() -> bool {
    use std::process::{Command, Stdio};
    Path::new("/usr/bin/xdg-dbus-proxy").exists()
        && Command::new("bwrap")
            .args(["--ro-bind", "/", "/", "true"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
}

pub(super) struct Runtime {
    g: &'static Api,
    queue: OnceCell<mpsc::Receiver<Job>>,
    sandboxed: bool,
    /// The engine and its processes, made for the first page and let go
    /// with the last, so an app with no page open runs none of them.
    pages: RefCell<HashMap<u64, Rc<Page>>>,
    started: Instant,
}

impl Runtime {
    /// Starts GTK and `WebKit` on this thread, which keeps them from then on.
    pub(super) fn start() -> Result<Rc<Self>, String> {
        let g = ffi::load()?;
        let sandboxed = sandbox_available();
        // SAFETY: This is the one thread that uses GTK. Every pointer passed
        // is a live object made here or a static string.
        let runtime = unsafe {
            g.g_set_prgname(c"codesh-app".as_ptr());
            g.gtk_disable_setlocale();
            if g.gtk_init_check(ptr::null_mut(), ptr::null_mut()) == FALSE {
                return Err("Browser pages need a desktop session to render in.".into());
            }
            // On X11, GTK asks the server about windows that are on no
            // screen, and GDK ends the whole app over the server's refusal
            // unless errors are trapped. The trap stays for good.
            g.gdk_error_trap_push();
            g.gdk_event_handler_set(on_event, ptr::null_mut(), None);
            Rc::new(Self {
                g,
                queue: OnceCell::new(),
                sandboxed,
                pages: RefCell::default(),
                started: Instant::now(),
            })
        };
        let _ = RUNTIME.with(|cell| cell.set(runtime.clone()));
        Ok(runtime)
    }

    /// Runs the jobs in `queue`, and GTK, until the app exits.
    pub(super) fn serve(self: &Rc<Self>, queue: mpsc::Receiver<Job>) {
        let _ = self.queue.set(queue);
        self.drain();
        // SAFETY: GTK was started on this thread.
        unsafe { self.g.gtk_main() };
    }

    fn drain(self: &Rc<Self>) {
        while let Some(job) = self.queue.get().and_then(|queue| queue.try_recv().ok()) {
            job(Ok(self));
        }
    }

    pub(super) fn page(&self, id: u64) -> Option<Rc<Page>> {
        self.pages.borrow().get(&id).cloned()
    }

    /// Milliseconds on the clock events are stamped with.
    #[allow(clippy::cast_possible_truncation)]
    fn time(&self) -> u32 {
        self.started.elapsed().as_millis() as u32
    }

    pub(super) fn open(self: &Rc<Self>, setup: Setup) {
        let g = self.g;
        #[allow(clippy::cast_possible_truncation, reason = "Linux desktops are 64-bit")]
        let data = setup.id as usize;
        // SAFETY: Each object is made here and kept by the page until it closes.
        let page = unsafe {
            let browser = match setup
                .browser
                .map(|setup| browser::State::new(self, &setup))
                .transpose()
            {
                Ok(browser) => browser,
                Err(error) => {
                    setup.events.browser(polybrowser::BrowserEvent::LoadFailed {
                        url: setup.url,
                        error: error.to_string(),
                    });
                    return;
                }
            };
            let Some(context) = browser.as_ref().map(browser::State::context) else {
                setup.events.browser(polybrowser::BrowserEvent::LoadFailed {
                    url: setup.url,
                    error: unloadable(),
                });
                return;
            };
            let (manager, view) = web_view(g, context, data, browser.is_some());
            let window = offscreen_window(g, view, setup.transparent, data);
            Rc::new(Page {
                id: setup.id,
                token: token(),
                browser,
                events: setup.events,
                shared: setup.shared,
                window,
                view,
                manager,
                sequence: Cell::new(0),
                menu: Cell::new(ptr::null_mut()),
                committed: Cell::new(false),
                failed: Cell::new(false),
                visible: Cell::new(false),
                focused: Cell::new(false),
                input: input::State::default(),
                unit: Cell::new(1.0),
                scale: Cell::new(1.0),
                size: Cell::new((0, 0)),
                in_flight: Cell::new(false),
                dirty: Cell::new(false),
                cursor: Cell::new(CursorStyle::Arrow),
            })
        };
        self.pages.borrow_mut().insert(page.id, page.clone());
        if page.browser.is_some()
            && let Err(failure) = page.install_browser(self)
        {
            page.events.browser(polybrowser::BrowserEvent::LoadFailed {
                url: setup.url,
                error: failure.to_string(),
            });
            self.close(page.id);
            return;
        }
        let url = cstring(&setup.url);
        // SAFETY: The view is live and the URL a valid string.
        unsafe { g.webkit_web_view_load_uri(page.view, url.as_ptr()) };
    }

    pub(super) fn close(&self, id: u64) {
        let Some(page) = self.pages.borrow_mut().remove(&id) else {
            return;
        };
        let g = self.g;
        page.retire(g);
        // SAFETY: The page owned these objects, and nothing uses them after.
        unsafe {
            let menu = page.menu.replace(ptr::null_mut());
            if !menu.is_null() {
                g.webkit_option_menu_close(menu);
                g.g_object_unref(menu);
            }
            g.webkit_web_view_stop_loading(page.view);
            g.gtk_widget_destroy(page.window);
            g.g_object_unref(page.manager);
        }
    }

    #[cfg(test)]
    pub(super) fn live(&self) -> usize {
        self.pages.borrow().len()
    }
}

/// Passes GDK's events on to GTK, except the news that the desktop's
/// primary selection went to someone else. A page nobody sees can never
/// hold it, and `WebKit` answers that news by dropping the selection the
/// user just made in the page.
unsafe extern "C" fn on_event(event: gpointer, _data: gpointer) {
    let Some(g) = ffi::loaded() else {
        return;
    };
    // SAFETY: GDK passes a live event, which starts with these fields
    // whenever it is of this type.
    unsafe {
        let selection = &*event.cast::<GdkEventSelection>();
        if selection.kind == ffi::EVENT_SELECTION_CLEAR
            && selection.selection as usize == ffi::SELECTION_PRIMARY
        {
            return;
        }
        g.gtk_main_do_event(event);
    }
}

/// A web context that keeps what pages store under `data`.
unsafe fn web_context(g: &Api, data: Option<&Path>) -> gpointer {
    let Some(data) = data else {
        // SAFETY: Takes no arguments.
        return unsafe { g.webkit_web_context_new() };
    };
    let path = |name: &str| {
        CString::new(data.join(name).as_os_str().as_bytes()).unwrap_or_else(|_| CString::default())
    };
    let directories = [path("data"), path("cache")];
    let names = [
        c"base-data-directory".as_ptr(),
        c"base-cache-directory".as_ptr(),
    ];
    let mut values = [GValue::EMPTY, GValue::EMPTY];
    // SAFETY: Names and values are parallel arrays of two string properties.
    unsafe {
        for (value, directory) in values.iter_mut().zip(&directories) {
            g.g_value_init(value, ffi::TYPE_STRING);
            g.g_value_set_string(value, directory.as_ptr());
        }
        let manager = g.g_object_new_with_properties(
            g.webkit_website_data_manager_get_type(),
            2,
            names.as_ptr(),
            values.as_ptr(),
        );
        for value in &mut values {
            g.g_value_unset(value);
        }
        let context = g.webkit_web_context_new_with_website_data_manager(manager);
        g.g_object_unref(manager);
        context
    }
}

/// A web view in `context` whose signals carry `data`, and the manager of
/// what its pages are given.
unsafe fn web_view(g: &Api, context: gpointer, data: usize, browser: bool) -> (gpointer, gpointer) {
    // SAFETY: The context is live, and the handlers have the signatures
    // `WebKitGTK` documents for their signals.
    unsafe {
        let manager = g.webkit_user_content_manager_new();
        let handler = cstring("codesh");
        g.webkit_user_content_manager_register_script_message_handler_with_reply(
            manager,
            handler.as_ptr(),
            if browser {
                c"codesh-browser".as_ptr()
            } else {
                ptr::null()
            },
        );
        let signal = cstring(&format!("script-message-with-reply-received::{}", "codesh"));
        connect(
            g,
            manager,
            &signal,
            callback!(on_message(gpointer, gpointer, gpointer, gpointer) -> gboolean),
            data,
        );
        let view = construct(
            g,
            g.webkit_web_view_get_type(),
            &[
                (c"web-context", g.webkit_web_context_get_type(), context),
                (
                    c"user-content-manager",
                    g.webkit_user_content_manager_get_type(),
                    manager,
                ),
            ],
        );
        g.webkit_settings_set_hardware_acceleration_policy(
            g.webkit_web_view_get_settings(view),
            ffi::HARDWARE_ACCELERATION_NEVER,
        );
        connect_view(g, view, data);
        // Starting a drag makes GTK ask the desktop to move a window that is
        // on no screen, and GDK ends the app over the refusal. So nothing in
        // a page, or in a frame of it, can be dragged.
        let sheet = g.webkit_user_style_sheet_new(
            c"*{-webkit-user-drag:none!important}".as_ptr(),
            ffi::INJECT_ALL_FRAMES,
            ffi::STYLE_LEVEL_USER,
            ptr::null(),
            ptr::null(),
        );
        g.webkit_user_content_manager_add_style_sheet(manager, sheet);
        g.webkit_user_style_sheet_unref(sheet);
        // The app hears the desktop's input method and passes on the text
        // it composes. A page nobody sees has no input method to ask, and
        // one that answers late makes WebKit skip shortcuts.
        g.webkit_web_view_set_input_method_context(view, ptr::null_mut());
        (manager, view)
    }
}

/// The window on no screen that `view` renders into.
unsafe fn offscreen_window(g: &Api, view: gpointer, transparent: bool, data: usize) -> gpointer {
    // SAFETY: The view is live and not yet in a window.
    unsafe {
        let window = g.gtk_offscreen_window_new();
        let visual = g.gdk_screen_get_rgba_visual(g.gtk_widget_get_screen(window));
        if !visual.is_null() {
            g.gtk_widget_set_visual(window, visual);
        }
        g.gtk_widget_set_app_paintable(window, TRUE);
        if transparent {
            let clear = GdkRGBA {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 0.0,
            };
            g.webkit_web_view_set_background_color(view, &raw const clear);
        }
        connect(
            g,
            window,
            c"damage-event",
            callback!(on_damage(gpointer, gpointer, gpointer) -> gboolean),
            data,
        );
        g.gtk_container_add(window, view);
        g.gtk_widget_show(window);
        // The view stays hidden until the app shows the page, but WebKit
        // asks where it is from the start.
        g.gtk_widget_realize(view);
        window
    }
}

/// Makes an object of `kind` with object-valued construction properties.
unsafe fn construct(g: &Api, kind: GType, properties: &[(&CStr, GType, gpointer)]) -> gpointer {
    let names: Vec<*const c_char> = properties.iter().map(|(name, ..)| name.as_ptr()).collect();
    let mut values: Vec<GValue> = properties.iter().map(|_| GValue::EMPTY).collect();
    // SAFETY: Names and values are parallel, and each value holds an object
    // of the type it was initialized with.
    unsafe {
        for (value, (_, kind, object)) in values.iter_mut().zip(properties) {
            g.g_value_init(value, *kind);
            g.g_value_set_object(value, *object);
        }
        let object = g.g_object_new_with_properties(
            kind,
            c_uint::try_from(names.len()).unwrap_or(0),
            names.as_ptr(),
            values.as_ptr(),
        );
        for value in &mut values {
            g.g_value_unset(value);
        }
        object
    }
}

/// Connects `handler` to `signal`, passing it `data` in place of a pointer.
unsafe fn connect(g: &Api, instance: gpointer, signal: &CStr, handler: Callback, data: usize) {
    // SAFETY: The instance is live, and the handler only reads `data` as a number.
    unsafe {
        g.g_signal_connect_data(
            instance,
            signal.as_ptr(),
            handler,
            data as gpointer,
            ptr::null_mut(),
            0,
        );
    }
}

unsafe fn connect_view(g: &Api, view: gpointer, data: usize) {
    // A page gets no dialogs, print panels, tooltips, context menus or
    // notifications of its own: each would open a window, and a page nobody
    // sees has no window for one to belong to.
    let handlers: [(&CStr, Callback); 16] = [
        (
            c"decide-policy",
            callback!(on_policy(gpointer, gpointer, c_int, gpointer) -> gboolean),
        ),
        (
            c"load-changed",
            callback!(on_load_changed(gpointer, c_int, gpointer)),
        ),
        (
            c"load-failed",
            callback!(
                on_load_failed(gpointer, c_int, *const c_char, *const GError, gpointer) -> gboolean
            ),
        ),
        (
            c"web-process-terminated",
            callback!(on_terminated(gpointer, c_int, gpointer)),
        ),
        (
            c"create",
            callback!(on_create(gpointer, gpointer, gpointer) -> gpointer),
        ),
        (
            c"mouse-target-changed",
            callback!(on_target(gpointer, gpointer, c_uint, gpointer)),
        ),
        (
            c"show-option-menu",
            callback!(
                on_menu(gpointer, gpointer, gpointer, *const GdkRectangle, gpointer) -> gboolean
            ),
        ),
        (
            c"permission-request",
            callback!(on_permission(gpointer, gpointer, gpointer) -> gboolean),
        ),
        (
            c"run-file-chooser",
            callback!(on_file_chooser(gpointer, gpointer, gpointer) -> gboolean),
        ),
        (
            c"run-color-chooser",
            callback!(on_color_chooser(gpointer, gpointer, gpointer) -> gboolean),
        ),
        (
            c"print",
            callback!(on_declined(gpointer, gpointer, gpointer) -> gboolean),
        ),
        (
            c"show-notification",
            callback!(on_declined(gpointer, gpointer, gpointer) -> gboolean),
        ),
        (
            c"script-dialog",
            callback!(on_dialog(gpointer, gpointer, gpointer) -> gboolean),
        ),
        (
            c"enter-fullscreen",
            callback!(on_fullscreen(gpointer, gpointer) -> gboolean),
        ),
        (
            c"query-tooltip",
            callback!(on_tooltip(gpointer, c_int, c_int, gboolean, gpointer, gpointer) -> gboolean),
        ),
        (
            c"context-menu",
            callback!(on_context_menu(gpointer, gpointer, gpointer, gpointer, gpointer) -> gboolean),
        ),
    ];
    for (signal, handler) in handlers {
        // SAFETY: Each handler has the signature `WebKitGTK` documents for
        // its signal.
        unsafe { connect(g, view, signal, handler, data) };
    }
}

pub(super) struct Page {
    id: u64,
    /// What the page's own script adds to a request. Nothing in another
    /// frame can read it, so a request without it is not the page's.
    token: String,
    browser: Option<browser::State>,
    events: Events,
    shared: Arc<Shared>,
    window: gpointer,
    view: gpointer,
    manager: gpointer,
    sequence: Cell<u64>,
    menu: Cell<gpointer>,
    committed: Cell<bool>,
    failed: Cell<bool>,
    visible: Cell<bool>,
    focused: Cell<bool>,
    input: input::State,
    /// Device pixels in one GTK unit.
    unit: Cell<f64>,
    /// Device pixels in one of the app's points.
    scale: Cell<f64>,
    /// The size the app draws the page at, in device pixels.
    size: Cell<(u32, u32)>,
    /// A frame was sent and the app has not taken it yet.
    in_flight: Cell<bool>,
    /// The page changed since the last frame was sent.
    dirty: Cell<bool>,
    cursor: Cell<CursorStyle>,
}

impl Page {
    fn next(&self) -> u64 {
        let id = self.sequence.get() + 1;
        self.sequence.set(id);
        id
    }

    fn generation(&self) -> u64 {
        self.shared.generation.load(Ordering::Acquire)
    }

    /// Voids the requests of a document that is gone.
    fn retire(&self, g: &Api) {
        if let Some(browser) = &self.browser {
            browser.cancel(g);
        }
    }

    pub(super) fn evaluate(&self, runtime: &Runtime, script: &str) {
        let Ok(script) = CString::new(script) else {
            return;
        };
        // SAFETY: The view is live, and no result is asked for.
        unsafe {
            runtime.g.webkit_web_view_evaluate_javascript(
                self.view,
                script.as_ptr(),
                -1,
                if self.browser.is_some() {
                    c"codesh-browser".as_ptr()
                } else {
                    ptr::null()
                },
                ptr::null(),
                ptr::null_mut(),
                None,
                ptr::null_mut(),
            );
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    pub(super) fn place(&self, runtime: &Runtime, width: u32, height: u32, scale: f32) {
        let g = runtime.g;
        // SAFETY: The window and view are live.
        unsafe {
            let unit = f64::from(g.gtk_widget_get_scale_factor(self.window).max(1));
            let units = |pixels: u32| (f64::from(pixels) / unit).ceil() as c_int;
            self.unit.set(unit);
            self.scale.set(f64::from(scale));
            self.size.set((width, height));
            g.gtk_widget_set_size_request(self.view, units(width), units(height));
            g.gtk_window_resize(self.window, units(width), units(height));
            g.webkit_web_view_set_zoom_level(
                self.view,
                f64::from(scale) / unit
                    * self
                        .browser
                        .as_ref()
                        .map_or(1.0, |browser| browser.zoom.get()),
            );
        }
    }

    pub(super) fn set_visible(&self, runtime: &Runtime, visible: bool) {
        if self.visible.replace(visible) == visible {
            return;
        }
        let g = runtime.g;
        if let Some(browser) = &self.browser
            && let Some(native) = browser.native.borrow().as_ref()
        {
            if !visible {
                self.focus(runtime, false);
            }
            native.set_visible(visible);
            return;
        }
        // SAFETY: The view is live. Hiding it has WebKit stop drawing the page.
        unsafe {
            if visible {
                g.gtk_widget_show(self.view);
            } else {
                self.focus(runtime, false);
                g.gtk_widget_hide(self.view);
            }
        }
    }

    pub(super) fn focus(&self, runtime: &Runtime, focused: bool) {
        if !focused {
            self.input.release();
        }
        if (focused && !self.visible.get()) || self.focused.replace(focused) == focused {
            return;
        }
        let g = runtime.g;
        // SAFETY: The window is live. GTK reads focus from the window, which
        // WebKit takes as the page being active.
        unsafe {
            let window = g.gtk_widget_get_window(self.window);
            if window.is_null() {
                return;
            }
            Self::dispatch::<GdkEventFocus>(
                g,
                ffi::EVENT_FOCUS_CHANGE,
                window,
                keyboard(g),
                |event| event.focused = i16::from(focused),
            );
            if focused {
                g.gtk_widget_grab_focus(self.input_focus(g));
            }
        }
    }

    pub(super) fn snapshot(&self, runtime: &Runtime) {
        self.dirty.set(true);
        self.send_frame(runtime.g);
    }

    pub(super) fn snapshot_taken(&self, runtime: &Runtime) {
        self.in_flight.set(false);
        self.send_frame(runtime.g);
    }

    /// Sends the page as it is now, unless the app still holds the last frame.
    fn send_frame(&self, g: &Api) {
        if !self.visible.get() || self.in_flight.get() || !self.dirty.get() {
            return;
        }
        let Some(image) = self.capture(g) else {
            return;
        };
        let id = self.next();
        self.shared.snapshot.store(id, Ordering::Release);
        let frame = Event::Snapshot {
            id,
            generation: self.generation(),
            image,
        };
        if self.events.try_send(frame).is_ok() {
            self.in_flight.set(true);
            self.dirty.set(false);
        }
    }

    #[allow(clippy::cast_sign_loss)]
    fn capture(&self, g: &Api) -> Option<Snapshot> {
        let (wanted_width, wanted_height) = self.size.get();
        // SAFETY: The window is live. The mapped image is read within its
        // own bounds and unmapped before returning.
        unsafe {
            let surface = g.gtk_offscreen_window_get_surface(self.window);
            if surface.is_null() {
                return None;
            }
            g.cairo_surface_flush(surface);
            let image = g.cairo_surface_map_to_image(surface, ptr::null_mut());
            let format = g.cairo_image_surface_get_format(image);
            let width = (g.cairo_image_surface_get_width(image).max(0) as u32).min(wanted_width);
            let height = (g.cairo_image_surface_get_height(image).max(0) as u32).min(wanted_height);
            let stride = g.cairo_image_surface_get_stride(image).max(0) as usize;
            let data = g.cairo_image_surface_get_data(image);
            let readable = g.cairo_surface_status(image) == 0
                && matches!(format, ffi::CAIRO_FORMAT_ARGB32 | ffi::CAIRO_FORMAT_RGB24)
                && width > 0
                && height > 0
                && !data.is_null();
            let snapshot = readable.then(|| {
                let row = width as usize * 4;
                let mut bgra = Vec::with_capacity(row * height as usize);
                for y in 0..height as usize {
                    bgra.extend_from_slice(std::slice::from_raw_parts(data.add(y * stride), row));
                }
                if format == ffi::CAIRO_FORMAT_RGB24 {
                    for pixel in bgra.chunks_exact_mut(4) {
                        pixel[3] = 0xff;
                    }
                }
                Snapshot {
                    width,
                    height,
                    bgra,
                }
            });
            g.cairo_surface_unmap_image(surface, image);
            snapshot
        }
    }

    /// Builds a GDK event for `window`, lets `fill` set its fields and has
    /// GTK deliver it as if the desktop had sent it.
    unsafe fn dispatch<T>(
        g: &Api,
        kind: c_int,
        window: gpointer,
        device: gpointer,
        fill: impl FnOnce(&mut T),
    ) {
        // SAFETY: `gdk_event_new` returns a zeroed event large enough for
        // any event type, `T` is the layout of `kind`, and the event owns
        // the window reference taken here until it is freed.
        unsafe {
            let event = g.gdk_event_new(kind);
            let any = &mut *event.cast::<GdkEventAny>();
            any.window = g.g_object_ref(window);
            any.send_event = 1;
            fill(&mut *event.cast::<T>());
            if !device.is_null() {
                g.gdk_event_set_device(event, device);
            }
            g.gtk_main_do_event(event);
            g.gdk_event_free(event);
        }
    }

    /// A position in the app's points, in GTK's units.
    fn units(&self, position: Point<Pixels>) -> (f64, f64) {
        let factor = self.scale.get() / self.unit.get();
        (
            f64::from(f32::from(position.x)) * factor,
            f64::from(f32::from(position.y)) * factor,
        )
    }

    pub(super) fn pointer(&self, runtime: &Runtime, event: Pointer) {
        let g = runtime.g;
        let time = runtime.time();
        let (view, x, y) = self.pointer_target(g, event);
        // SAFETY: The view is live, and each event is filled as its type.
        unsafe {
            let window = g.gtk_widget_get_window(view);
            if window.is_null() {
                return;
            }
            let device = g.gdk_seat_get_pointer(seat(g));
            match event {
                Pointer::Moved {
                    pressed, modifiers, ..
                } => {
                    let state = state(modifiers) | pressed.map_or(0, button_mask);
                    Self::dispatch::<GdkEventMotion>(
                        g,
                        ffi::EVENT_MOTION_NOTIFY,
                        window,
                        device,
                        |event| {
                            event.time = time;
                            event.x = x;
                            event.y = y;
                            event.state = state;
                            event.device = device;
                        },
                    );
                }
                Pointer::Pressed {
                    button, modifiers, ..
                }
                | Pointer::Released {
                    button, modifiers, ..
                } => {
                    let Some(number) = button_number(button) else {
                        return;
                    };
                    let pressed = matches!(event, Pointer::Pressed { .. });
                    let held = if pressed { 0 } else { button_mask(button) };
                    Self::dispatch::<GdkEventButton>(
                        g,
                        if pressed {
                            ffi::EVENT_BUTTON_PRESS
                        } else {
                            ffi::EVENT_BUTTON_RELEASE
                        },
                        window,
                        device,
                        |event| {
                            event.time = time;
                            event.x = x;
                            event.y = y;
                            event.state = state(modifiers) | held;
                            event.button = number;
                            event.device = device;
                        },
                    );
                }
                Pointer::Scrolled {
                    delta, modifiers, ..
                } => {
                    let (delta_x, delta_y) = self.units(delta);
                    Self::dispatch::<GdkEventScroll>(
                        g,
                        ffi::EVENT_SCROLL,
                        window,
                        device,
                        |event| {
                            event.time = time;
                            event.x = x;
                            event.y = y;
                            event.state = state(modifiers);
                            event.direction = ffi::SCROLL_SMOOTH;
                            event.device = device;
                            event.delta_x = -delta_x / SCROLL_STEP;
                            event.delta_y = -delta_y / SCROLL_STEP;
                        },
                    );
                }
                Pointer::Left => {
                    Self::dispatch::<GdkEventCrossing>(
                        g,
                        ffi::EVENT_LEAVE_NOTIFY,
                        window,
                        device,
                        |event| event.time = time,
                    );
                }
            }
        }
    }

    pub(super) fn key(&self, runtime: &Runtime, key: &Key) {
        let g = runtime.g;
        // SAFETY: The view is live, and the keymap entries GDK allocates are
        // freed once the first is read.
        unsafe {
            let window = g.gtk_widget_get_window(self.input_focus(g));
            let keyval = keyval(g, key);
            if window.is_null() || keyval == 0 {
                return;
            }
            let mut entries: *mut GdkKeymapKey = ptr::null_mut();
            let mut count = 0;
            let keymap = g.gdk_keymap_get_for_display(g.gdk_display_get_default());
            let found = g.gdk_keymap_get_entries_for_keyval(
                keymap,
                keyval,
                &raw mut entries,
                &raw mut count,
            ) != FALSE
                && count > 0;
            let (keycode, group) = if found {
                ((*entries).keycode, (*entries).group)
            } else {
                (0, 0)
            };
            if !entries.is_null() {
                g.g_free(entries.cast());
            }
            let time = runtime.time();
            Self::dispatch::<GdkEventKey>(
                g,
                if key.pressed {
                    ffi::EVENT_KEY_PRESS
                } else {
                    ffi::EVENT_KEY_RELEASE
                },
                window,
                keyboard(g),
                |event| {
                    event.time = time;
                    event.state = state(key.modifiers);
                    event.keyval = keyval;
                    event.hardware_keycode = u16::try_from(keycode).unwrap_or(0);
                    event.group = u8::try_from(group).unwrap_or(0);
                },
            );
        }
    }

    /// Offers the page `text` for the paste key that follows. The page's
    /// guard pastes it when that key arrives, so it lands in order with what
    /// was typed before: a script would overtake keys `WebKit` still holds.
    pub(super) fn offer_paste(&self, runtime: &Runtime, text: &str) {
        if self.input_focus(runtime.g) != self.view {
            return;
        }
        self.evaluate(
            runtime,
            &format!("window.__codeshPaste&&__codeshPaste({})", quoted(text)),
        );
    }

    pub(super) fn choose(&self, runtime: &Runtime, option: Option<usize>) {
        let menu = self.menu.replace(ptr::null_mut());
        if menu.is_null() {
            return;
        }
        let g = runtime.g;
        // SAFETY: The menu was retained when the page opened it.
        unsafe {
            if let Some(index) = option.and_then(|index| c_uint::try_from(index).ok())
                && index < g.webkit_option_menu_get_n_items(menu)
            {
                g.webkit_option_menu_activate_item(menu, index);
            }
            g.webkit_option_menu_close(menu);
            g.g_object_unref(menu);
        }
    }
}

/// How long an offered paste waits for its key, in milliseconds.
const PASTE_PATIENCE_MS: u32 = 5000;
/// How far `WebKit` scrolls for one step of a wheel, in GTK's units.
const SCROLL_STEP: f64 = 40.0;

/// The script run before the page's own. It stamps the page's requests
/// with `token`; a frame has a handler of its own, which stamps nothing.
///
/// A page nobody sees owns no clipboard, so the guard also stands in for
/// one. It passes on a copy the engine itself reports, which `WebKit` makes
/// only while the user is acting on the page; an event the page made up is
/// not one. On the paste key it gives the page the text the app offered, as
/// the paste event and then as typed text.
/// It also undoes and redoes on their keys, which `WebKitGTK` leaves to the
/// app. A page that handles one of these keys itself keeps it.
fn guard(token: &str) -> String {
    format!(
        "(()=>{{const handler=window.webkit&&window.webkit.messageHandlers&&\
         window.webkit.messageHandlers.{name};if(!handler)return;\
         const post=handler.postMessage.bind(handler);const stamp='{token}';\
         const copy=text=>{{if(text)post({{document:stamp,copied:String(text)}}).catch(()=>{{}})}};\
         const copied=event=>{{if(!event.isTrusted)return;const field=document.activeElement;\
         const chosen=event.clipboardData&&event.clipboardData.getData('text/plain');\
         if(chosen)copy(chosen);\
         else if(!field||field.type!=='password')copy(String(getSelection())||\
         (field&&typeof field.selectionStart==='number'?\
         field.value.slice(field.selectionStart,field.selectionEnd):''))}};\
         addEventListener('copy',copied);addEventListener('cut',copied);\
         if(navigator.clipboard)navigator.clipboard.writeText=text=>\
         navigator.userActivation&&navigator.userActivation.isActive?\
         (copy(text),Promise.resolve()):\
         Promise.reject(new DOMException('The request is not allowed','NotAllowedError'));\
         let offered=null;\
         Object.defineProperty(window,'__codeshPaste',\
         {{value:text=>{{offered=[String(text),performance.now()]}}}});\
         addEventListener('keydown',event=>{{\
         if(event.defaultPrevented||event.altKey||event.metaKey)return;\
         const key=event.key.toLowerCase();const control=event.ctrlKey,shift=event.shiftKey;\
         const edit=!control?null:key==='z'?(shift?'redo':'undo'):key==='y'&&!shift?'redo':null;\
         if(edit){{event.preventDefault();document.execCommand(edit);return}}\
         const pastes=control&&!shift&&key==='v'||shift&&!control&&key==='insert';\
         if(!pastes||!offered)return;const [text,since]=offered;offered=null;\
         if(performance.now()-since>{patience})return;\
         event.preventDefault();const target=document.activeElement||document.body;\
         const data=new DataTransfer();data.setData('text/plain',text);\
         if(target.dispatchEvent(new ClipboardEvent('paste',\
         {{clipboardData:data,bubbles:true,cancelable:true}})))\
         document.execCommand('insertText',false,text)}});}})();",
        name = "codesh",
        patience = PASTE_PATIENCE_MS,
    )
}

/// Sixteen random bytes as hex.
fn token() -> String {
    use std::fmt::Write;
    use std::io::Read;
    let mut bytes = [0_u8; 16];
    let random =
        std::fs::File::open("/dev/urandom").and_then(|mut file| file.read_exact(&mut bytes));
    if random.is_err() {
        // No page is trusted with a token that could be guessed.
        return String::new();
    }
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// `text` as a JavaScript string literal.
pub(super) fn quoted(text: &str) -> String {
    use std::fmt::Write;
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push('"');
    for character in text.chars() {
        match character {
            '"' => literal.push_str("\\\""),
            '\\' => literal.push_str("\\\\"),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            '\t' => literal.push_str("\\t"),
            character if character < ' ' || matches!(character, '\u{2028}' | '\u{2029}') => {
                let _ = write!(literal, "\\u{:04x}", u32::from(character));
            }
            character => literal.push(character),
        }
    }
    literal.push('"');
    literal
}

/// What the app shows in place of a page whose files could not be read.
fn unloadable() -> String {
    "The browser page could not be loaded.".to_string()
}

fn cstring(text: &str) -> CString {
    CString::new(text).unwrap_or_default()
}

/// Copies a string `GLib` owns.
unsafe fn text(pointer: *const c_char) -> Option<String> {
    // SAFETY: A non-null pointer is a NUL-terminated string from the library.
    (!pointer.is_null()).then(|| {
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned()
    })
}

/// Copies and frees a string `GLib` allocated for the caller.
unsafe fn owned(g: &Api, pointer: *mut c_char) -> Option<String> {
    // SAFETY: The caller owns the string, which is freed once copied.
    unsafe {
        let copy = text(pointer);
        g.g_free(pointer.cast());
        copy
    }
}

unsafe fn seat(g: &Api) -> gpointer {
    // SAFETY: GTK is running, so there is a default display.
    unsafe { g.gdk_display_get_default_seat(g.gdk_display_get_default()) }
}

unsafe fn keyboard(g: &Api) -> gpointer {
    // SAFETY: As for `seat`.
    unsafe { g.gdk_seat_get_keyboard(seat(g)) }
}

fn state(modifiers: Modifiers) -> c_uint {
    [
        (modifiers.shift, ffi::SHIFT_MASK),
        (modifiers.control, ffi::CONTROL_MASK),
        (modifiers.alt, ffi::ALT_MASK),
        (modifiers.platform, ffi::SUPER_MASK),
    ]
    .into_iter()
    .filter(|(held, _)| *held)
    .fold(0, |state, (_, mask)| state | mask)
}

fn button_number(button: MouseButton) -> Option<c_uint> {
    match button {
        MouseButton::Left => Some(1),
        MouseButton::Middle => Some(2),
        MouseButton::Right => Some(3),
        MouseButton::Navigate(_) => None,
    }
}

fn button_mask(button: MouseButton) -> c_uint {
    match button {
        MouseButton::Left => ffi::BUTTON1_MASK,
        MouseButton::Middle => ffi::BUTTON2_MASK,
        MouseButton::Right => ffi::BUTTON3_MASK,
        MouseButton::Navigate(_) => 0,
    }
}

/// The GDK key value for a key as GPUI names it, or zero for one GDK has no
/// value for.
unsafe fn keyval(g: &Api, key: &Key) -> c_uint {
    const VOID: c_uint = 0x00ff_ffff;
    let single = |text: &str| {
        let mut characters = text.chars();
        characters.next().filter(|_| characters.next().is_none())
    };
    // SAFETY: These take a character or a NUL-terminated name.
    unsafe {
        if let Some(character) = key.text.as_deref().and_then(single)
            && !character.is_control()
        {
            return g.gdk_unicode_to_keyval(u32::from(character));
        }
        let named = match key.name.as_str() {
            "enter" => 0xff0d,
            "tab" if key.modifiers.shift => 0xfe20,
            "tab" => 0xff09,
            "space" => 0x20,
            "backspace" => 0xff08,
            "escape" => 0xff1b,
            "delete" => 0xffff,
            "insert" => 0xff63,
            "home" => 0xff50,
            "left" => 0xff51,
            "up" => 0xff52,
            "right" => 0xff53,
            "down" => 0xff54,
            "pageup" => 0xff55,
            "pagedown" => 0xff56,
            "end" => 0xff57,
            _ => 0,
        };
        if named != 0 {
            return named;
        }
        if let Some(character) = single(&key.name) {
            let character = if key.modifiers.shift {
                character.to_uppercase().next().unwrap_or(character)
            } else {
                character
            };
            return g.gdk_unicode_to_keyval(u32::from(character));
        }
        if let Some(number) = key
            .name
            .strip_prefix('f')
            .and_then(|number| number.parse::<c_uint>().ok())
            .filter(|number| (1..=35).contains(number))
        {
            return 0xffbe + number - 1;
        }
        let name = cstring(&key.name);
        match g.gdk_keyval_from_name(name.as_ptr()) {
            VOID => 0,
            keyval => keyval,
        }
    }
}

/// The runtime and the page a handler was connected for, while it is open.
fn target(data: gpointer) -> Option<(Rc<Runtime>, Rc<Page>)> {
    let runtime = runtime()?;
    let page = runtime.page(data as usize as u64)?;
    Some((runtime, page))
}

unsafe extern "C" fn on_message(
    _manager: gpointer,
    value: gpointer,
    reply: gpointer,
    data: gpointer,
) -> gboolean {
    let Some(g) = ffi::loaded() else {
        return FALSE;
    };
    let reject = |message: &str| {
        let message = cstring(message);
        // SAFETY: WebKit passes a live reply for the handler to answer.
        unsafe { g.webkit_script_message_reply_return_error_message(reply, message.as_ptr()) };
        TRUE
    };
    let Some((_, page)) = target(data) else {
        return reject("document retired");
    };
    // SAFETY: WebKit passes a live value, and each property read from it is
    // released here.
    unsafe {
        let stamped = !page.token.is_empty() && g.jsc_value_is_object(value) != FALSE && {
            let document = g.jsc_value_object_get_property(value, c"document".as_ptr());
            let stamp = (g.jsc_value_is_string(document) != FALSE)
                .then(|| owned(g, g.jsc_value_to_string(document)))
                .flatten();
            g.g_object_unref(document);
            stamp.as_deref() == Some(page.token.as_str())
        };
        if !stamped {
            return reject("message is not from the main document");
        }
        let copied = g.jsc_value_object_get_property(value, c"copied".as_ptr());
        let copy = (g.jsc_value_is_string(copied) != FALSE)
            .then(|| owned(g, g.jsc_value_to_string(copied)))
            .flatten();
        g.g_object_unref(copied);
        if let Some(copy) = copy {
            if copy.len() <= 1024 * 1024 {
                let _ = page.events.try_send(Event::Copied(copy));
            }
            let done = g.jsc_value_new_from_json(g.jsc_value_get_context(value), c"null".as_ptr());
            g.webkit_script_message_reply_return_value(reply, done);
            g.g_object_unref(done);
            return TRUE;
        }
        reject("unsupported browser message")
    }
}

unsafe extern "C" fn on_policy(
    _view: gpointer,
    decision: gpointer,
    kind: c_int,
    data: gpointer,
) -> gboolean {
    let Some(g) = ffi::loaded() else {
        return FALSE;
    };
    // SAFETY: WebKit passes a live decision of the type `kind` names.
    unsafe {
        if let Some((runtime, page)) = target(data)
            && page.browser.is_some()
        {
            return page.browser_policy(&runtime, decision, kind);
        }
        g.webkit_policy_decision_ignore(decision);
    }
    TRUE
}

unsafe extern "C" fn on_load_changed(_view: gpointer, event: c_int, data: gpointer) {
    let Some((runtime, page)) = target(data) else {
        return;
    };
    match event {
        ffi::LOAD_STARTED => {
            if let Some(browser) = &page.browser {
                browser.cancel(runtime.g);
            }
            page.browser_load(runtime.g, event);
            page.committed.set(false);
            page.failed.set(false);
        }
        ffi::LOAD_COMMITTED => {
            page.retire(runtime.g);
            page.shared.generation.fetch_add(1, Ordering::AcqRel);
            page.committed.set(true);
            page.browser_load(runtime.g, event);
            let _ = page.events.try_send(Event::Navigated);
        }
        // A navigation that was refused or failed finishes too.
        ffi::LOAD_FINISHED if page.committed.get() && !page.failed.get() => {
            page.browser_load(runtime.g, event);
            let _ = page.events.try_send(Event::Loaded);
        }
        _ => {}
    }
}

unsafe extern "C" fn on_load_failed(
    _view: gpointer,
    _event: c_int,
    url: *const c_char,
    error: *const GError,
    data: gpointer,
) -> gboolean {
    let (Some(g), Some((_, page))) = (ffi::loaded(), target(data)) else {
        return TRUE;
    };
    page.failed.set(true);
    // SAFETY: WebKit passes a live error.
    unsafe {
        let error = &*error;
        let stopped = (error.domain == g.webkit_network_error_quark()
            && error.code == ffi::NETWORK_ERROR_CANCELLED)
            || (error.domain == g.webkit_policy_error_quark()
                && error.code == ffi::POLICY_ERROR_INTERRUPTED);
        if !stopped {
            page.events.browser(polybrowser::BrowserEvent::LoadFailed {
                url: text(url).unwrap_or_default(),
                error: text(error.message).unwrap_or_default(),
            });
            let _ = page
                .events
                .try_send(Event::Failed(text(error.message).unwrap_or_default()));
        }
    }
    // The app shows the failure, so WebKit's own error page is not loaded.
    TRUE
}

unsafe extern "C" fn on_terminated(_view: gpointer, _reason: c_int, data: gpointer) {
    let Some((runtime, page)) = target(data) else {
        return;
    };
    page.retire(runtime.g);
    page.events.browser(polybrowser::BrowserEvent::LoadFailed {
        url: page.browser_url(runtime.g),
        error: "Web content process terminated".to_string(),
    });
    let _ = page
        .events
        .try_send(Event::Failed("Web content process terminated".to_string()));
}

unsafe extern "C" fn on_create(_view: gpointer, _action: gpointer, _data: gpointer) -> gpointer {
    ptr::null_mut()
}

unsafe extern "C" fn on_declined(_view: gpointer, _request: gpointer, _data: gpointer) -> gboolean {
    TRUE
}

/// Answers a dialog the page asked for without showing one. A page that
/// asks before it is left is left, as on macOS.
unsafe extern "C" fn on_dialog(_view: gpointer, dialog: gpointer, _data: gpointer) -> gboolean {
    if let Some(g) = ffi::loaded() {
        // SAFETY: WebKit passes a live dialog.
        unsafe {
            if g.webkit_script_dialog_get_dialog_type(dialog) == ffi::DIALOG_BEFORE_UNLOAD {
                g.webkit_script_dialog_confirm_set_confirmed(dialog, TRUE);
            }
        }
    }
    TRUE
}

unsafe extern "C" fn on_fullscreen(_view: gpointer, _data: gpointer) -> gboolean {
    TRUE
}

/// Stops the tooltip before the engine's own handler fills it in.
unsafe extern "C" fn on_tooltip(
    view: gpointer,
    _x: c_int,
    _y: c_int,
    _keyboard: gboolean,
    _tooltip: gpointer,
    _data: gpointer,
) -> gboolean {
    if let Some(g) = ffi::loaded() {
        // SAFETY: GTK passes the live view the signal is being emitted on.
        unsafe { g.g_signal_stop_emission_by_name(view, c"query-tooltip".as_ptr()) };
    }
    FALSE
}

unsafe extern "C" fn on_context_menu(
    _view: gpointer,
    menu: gpointer,
    event: gpointer,
    _hit: gpointer,
    data: gpointer,
) -> gboolean {
    if let Some((runtime, page)) = target(data)
        && page.visible.get()
    {
        let Events::Browser(sender, _) = &page.events;
        if unsafe { runtime.g.webkit_context_menu_get_n_items(menu) } == 0 {
            return TRUE;
        }
        let (mut x, mut y) = (0.0, 0.0);
        if !event.is_null() {
            unsafe {
                runtime
                    .g
                    .gdk_event_get_coords(event, &raw mut x, &raw mut y)
            };
        }
        #[allow(
            clippy::cast_possible_truncation,
            reason = "Coordinates in page points"
        )]
        let position = point(
            px((x * page.unit.get() / page.scale.get()) as f32),
            px((y * page.unit.get() / page.scale.get()) as f32),
        );
        let _ = sender.try_send(crate::browser::Event::ContextMenu(position));
    }
    TRUE
}

unsafe extern "C" fn on_permission(
    _view: gpointer,
    request: gpointer,
    _data: gpointer,
) -> gboolean {
    if let Some(g) = ffi::loaded() {
        // SAFETY: WebKit passes a live request.
        unsafe { g.webkit_permission_request_deny(request) };
    }
    TRUE
}

unsafe extern "C" fn on_file_chooser(
    _view: gpointer,
    request: gpointer,
    _data: gpointer,
) -> gboolean {
    if let Some(g) = ffi::loaded() {
        // SAFETY: WebKit passes a live request.
        unsafe { g.webkit_file_chooser_request_cancel(request) };
    }
    TRUE
}

unsafe extern "C" fn on_color_chooser(
    _view: gpointer,
    request: gpointer,
    _data: gpointer,
) -> gboolean {
    if let Some(g) = ffi::loaded() {
        // SAFETY: WebKit passes a live request.
        unsafe { g.webkit_color_chooser_request_cancel(request) };
    }
    TRUE
}

unsafe extern "C" fn on_damage(_window: gpointer, _event: gpointer, data: gpointer) -> gboolean {
    if let Some((runtime, page)) = target(data) {
        page.dirty.set(true);
        page.send_frame(runtime.g);
    }
    FALSE
}

unsafe extern "C" fn on_target(_view: gpointer, hit: gpointer, _modifiers: c_uint, data: gpointer) {
    let Some((runtime, page)) = target(data) else {
        return;
    };
    // SAFETY: WebKit passes a live hit test result.
    let context = unsafe { runtime.g.webkit_hit_test_result_get_context(hit) };
    let cursor = if context & ffi::HIT_EDITABLE != 0 {
        CursorStyle::IBeam
    } else if context & ffi::HIT_LINK != 0 {
        CursorStyle::PointingHand
    } else {
        CursorStyle::Arrow
    };
    if page.cursor.replace(cursor) != cursor {
        let _ = page.events.try_send(Event::Cursor(cursor));
    }
}

unsafe extern "C" fn on_menu(
    _view: gpointer,
    menu: gpointer,
    _event: gpointer,
    rectangle: *const GdkRectangle,
    data: gpointer,
) -> gboolean {
    let Some((runtime, page)) = target(data) else {
        return FALSE;
    };
    let g = runtime.g;
    page.choose(&runtime, None);
    // SAFETY: WebKit passes a live menu and the control's rectangle. The
    // menu is retained until the app answers.
    unsafe {
        let options = (0..g.webkit_option_menu_get_n_items(menu))
            .map(|index| {
                let item = g.webkit_option_menu_get_item(menu, index);
                MenuOption {
                    label: text(g.webkit_option_menu_item_get_label(item)).unwrap_or_default(),
                    enabled: g.webkit_option_menu_item_is_enabled(item) != FALSE,
                    selected: g.webkit_option_menu_item_is_selected(item) != FALSE,
                    heading: g.webkit_option_menu_item_is_group_label(item) != FALSE,
                }
            })
            .collect();
        let rectangle = &*rectangle;
        let points = |units: c_int| {
            #[allow(clippy::cast_possible_truncation)]
            px((f64::from(units) * page.unit.get() / page.scale.get()) as f32)
        };
        let anchor = Bounds::new(
            point(points(rectangle.x), points(rectangle.y)),
            size(points(rectangle.width), points(rectangle.height)),
        );
        page.menu.set(g.g_object_ref(menu));
        connect(
            g,
            menu,
            c"close",
            callback!(on_menu_closed(gpointer, gpointer)),
            data as usize,
        );
        let _ = page
            .events
            .try_send(Event::Menu(Some(OptionMenu { options, anchor })));
    }
    TRUE
}

/// The page closed its own menu, as when the control is removed.
unsafe extern "C" fn on_menu_closed(menu: gpointer, data: gpointer) {
    let Some((runtime, page)) = target(data) else {
        return;
    };
    if page.menu.get() == menu {
        page.menu.set(ptr::null_mut());
        // SAFETY: Releases the reference taken when the menu opened.
        unsafe { runtime.g.g_object_unref(menu) };
        let _ = page.events.try_send(Event::Menu(None));
    }
}
