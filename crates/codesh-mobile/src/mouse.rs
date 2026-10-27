//! Taps and scrolling for programs that read the mouse, such as editors and pagers.

use codesh_protocol::{MouseAction, MouseEvent};

use crate::keys::Modifiers;

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum ScrollDirection {
    Up,
    Down,
}

/// A press and then a release at one cell.
pub(crate) fn click(
    button: MouseButton,
    row: u16,
    column: u16,
    modifiers: Modifiers,
) -> [MouseEvent; 2] {
    let press = MouseEvent {
        action: MouseAction::Press,
        button: Some(match button {
            MouseButton::Left => codesh_protocol::MouseButton::Left,
            MouseButton::Middle => codesh_protocol::MouseButton::Middle,
            MouseButton::Right => codesh_protocol::MouseButton::Right,
        }),
        column,
        row,
        scroll: None,
        modifiers: codesh_protocol::Modifiers {
            shift: modifiers.shift,
            alt: modifiers.alt,
            ctrl: modifiers.control,
        },
    };
    [
        press,
        MouseEvent {
            action: MouseAction::Release,
            ..press
        },
    ]
}

/// One mouse-wheel step at a cell.
pub(crate) fn scroll(direction: ScrollDirection, row: u16, column: u16) -> MouseEvent {
    MouseEvent {
        action: MouseAction::Scroll,
        button: None,
        column,
        row,
        scroll: Some(match direction {
            ScrollDirection::Up => codesh_protocol::ScrollDirection::Up,
            ScrollDirection::Down => codesh_protocol::ScrollDirection::Down,
        }),
        modifiers: codesh_protocol::Modifiers::default(),
    }
}
