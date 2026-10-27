//! Browser surface types and platform hosting.
#[cfg(target_os = "macos")]
pub(crate) mod host;
#[cfg(target_os = "macos")]
pub(crate) use host::parent_view;
#[cfg(target_os = "linux")]
pub(crate) mod linux;
use gpui::{Bounds, CursorStyle, Keystroke, Modifiers, MouseButton, Pixels, Point};
/// Premultiplied BGRA pixels of a page at the display's scale.
pub struct Snapshot {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Snapshot({}x{})", self.width, self.height)
    }
}

/// What the pointer did over a page the app draws, in points from the page's
/// top left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pointer {
    Moved {
        position: Point<Pixels>,
        pressed: Option<MouseButton>,
        modifiers: Modifiers,
    },
    Pressed {
        position: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    },
    Released {
        position: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    },
    /// `delta` is how far the content moves, as GPUI reports it.
    Scrolled {
        position: Point<Pixels>,
        delta: Point<Pixels>,
        modifiers: Modifiers,
    },
    Left,
}

/// One row of a `<select>` a page asked to open.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuOption {
    pub label: String,
    pub enabled: bool,
    pub selected: bool,
    /// The title of a group of options, which cannot be chosen.
    pub heading: bool,
}

/// A `<select>` a page asked to open, anchored to the control in points from
/// the page's top left corner.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionMenu {
    pub options: Vec<MenuOption>,
    pub anchor: Bounds<Pixels>,
}

#[derive(Debug)]
pub enum Event {
    Navigated,
    Loaded,
    Failed(String),
    Escape,
    Shortcut(Keystroke),
    Snapshot {
        id: u64,
        generation: u64,
        image: Snapshot,
    },
    /// The pointer's shape over a page the app draws changed.
    Cursor(CursorStyle),
    /// A page the app draws put text on the clipboard.
    Copied(String),
    /// A page the app draws opened a `<select>`, or closed the one it had open.
    Menu(Option<OptionMenu>),
}

#[cfg(target_os = "macos")]
pub const CAPABILITY: codesh_core::capability::Capability =
    codesh_core::capability::Capability::Supported;
#[cfg(target_os = "linux")]
pub use linux::CAPABILITY;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub const CAPABILITY: codesh_core::capability::Capability =
    codesh_core::capability::Capability::Unsupported;
pub fn probe() -> codesh_core::capability::Capability {
    #[cfg(target_os = "linux")]
    {
        linux::probe()
    }
    #[cfg(not(target_os = "linux"))]
    {
        CAPABILITY
    }
}
