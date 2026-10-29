//! Reusable GPUI components. Shared shortcut definitions come from codesh-core;
//! app settings resolve them through the registration interface.

pub mod assets;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod bitmap;
pub mod command_palette;
pub mod components;
pub mod controls;
#[cfg(target_os = "linux")]
mod dbus;
#[cfg_attr(not(target_os = "macos"), path = "dialog/portable.rs")]
pub mod dialog;
#[path = "dialog/types.rs"]
mod dialog_types;
#[cfg(target_os = "linux")]
pub mod file_manager;
pub mod font;
pub mod form;
pub mod icon;
#[cfg(target_os = "macos")]
pub mod keyboard;
pub mod motion;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod native_scroll;
pub mod navigation;
pub mod panel;
pub mod picker;
pub mod popover;
pub mod scrollbar;
pub mod shortcuts;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod spinner;
pub mod text_input;
pub mod theme;
pub mod toast;
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "Main-thread native sidebar background view")]
pub mod vibrancy;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod window_drag;

#[allow(
    unsafe_code,
    reason = "Native notification delegate and Objective-C initialization"
)]
pub mod notifications;

pub mod pasteboard;

#[allow(unsafe_code, reason = "Main-thread WebKit and native view integration")]
pub mod webview;

#[allow(
    unsafe_code,
    reason = "The browser view is built inside a native container"
)]
pub mod browser;

pub mod transcript;

pub mod accessibility;

#[allow(
    unsafe_code,
    reason = "Native on-device dictation capture and transcription"
)]
pub mod voice;
