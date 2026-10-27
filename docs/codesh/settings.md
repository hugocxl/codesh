# Settings window

Settings is a separate window laid out like macOS System Settings: a sidebar of
categories with a search field, and a scrolling page of grouped, inset cards on
the right. The categories are General, Appearance, Terminal, Keyboard,
Commands, Browser, Server, Mobile, AI, and Backup & Restore. Search matches
setting labels across every category. The user guide describes each setting in
[Settings](../user-guide/settings.md).

## Controls

Numeric options use the shared slider, which previews while dragging and saves
on release or when Settings closes. On/off options use switches and fixed
choices use pop-up menus. The sidebar vibrancy level (0–100) in Appearance uses
the same slider.

## Keyboard

Keyboard shortcuts form one inset-separated card rendered through a virtual
list. Labels use sentence case. macOS bindings show key glyphs, and **Reset**
appears only when a binding differs from its default. Recording, cancellation,
and conflict checks go through the keymap.

## Voice

Dictation preferences live in General → Voice and in the `[voice]` table of
`settings.toml`. See [Dictation](../features/dictation.md).

## Files

App preferences are in `settings.toml`, terminal preferences in
`terminal.toml`, and server settings in `server.toml`. Sections and shortcut
overrides for removed features are ignored when loading and omitted when
saving; every other value is validated. Mobile access has no file of its own in Settings; it is
managed only from the Mobile page or `codesh mobile`.
