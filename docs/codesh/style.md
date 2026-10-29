# Desktop style

The design system for the desktop app: tokens, pane cards, split gutters, and
sidebar chrome.

The window and opaque sidebar share `Theme::bg`. The bundled Codesh dark theme
uses `#111111` and warm `#d5cec7` text. Fresh profiles use the project-focused
sidebar, expanded to 270px, with tips hidden and no sidebar vibrancy. Saved
appearance choices apply, except the sidebar always stays opaque. There is no
status bar.

## Tokens and geometry

`views/codesh/style.rs` owns one 8px spacing (`GUTTER`: window inset, gaps between cards, sidebar and panel padding, card header and body insets), 10px card radius, 1px border,
36px header. Headers use 13px semibold
text, 16px glyphs and 8px control gaps. Cards blend 2.5% theme foreground over
background (about `#161616`); borders blend 95% theme border over background
(about `#282827`). Pills use theme surface/hover. No shadows or permanent split
or sidebar separators are painted.

For extent `E`, ratio `r` and display scale `s`, the pure gutter helper computes:

```text
g = min(max(E, 0), 12)
A = max(E, 0) - g
first = clamp(round(A * r * s) / s, 0, A)
second_origin = first + g
second_size = max(E, 0) - second_origin
```

Only the shared edge is snapped, including for extents smaller than the gutter.
Resize divides pointer delta by `max(E - 12, 1)`. The 10px grip is centred in the
gutter, which shows a subtle 1px accent line on hover/drag. Both pane-drop
walkers reuse `split_bounds()`; nested ratios remain owned by the layout model.
The whole grid receives one 8px inset, inside auxiliary panel composition.

## Command palette

`codesh_ui::command_palette` draws the palette 740pt wide, at most 480pt tall,
72pt from the top, with a 14pt radius, 1px border and modal shadow over a 30%
black backdrop. The header is 56pt with an 18/24pt search field; section
headers are 28pt; rows are 40pt with 8pt side insets and radius, a 24pt
rounded-square tile (6pt radius) with a white glyph over a hue gradient, title,
muted subtitle, badge, keycaps, kind label and chevron; the footer is 40pt with
the primary action and **Actions ⌘K**. Status messages centre in a 96pt body.
Tile hues: red, orange, amber, green, teal, blue, indigo, purple, pink, gray and
the theme accent. Heights are computed from row counts, never measured.

## Hooks

- `splits::pane_element()` wraps every pane kind exactly once in `pane_card`;
  zoom adds no extra frame. Header buttons focus their own PaneId before acting;
  close uses `AppModel::close_pane()`. Headers focus but never initiate drags.
  Actions retain their layout space and appear only on the active pane or while
  its card is hovered. Browser page clicks also set the active pane.
- The body shrinks below the header with real layout padding. Workspace terminal
  padding adds only `max(configured - 2, 0)` per edge: the default 2px is
  absorbed into the card inset.
  Palette construction and the opaque workspace terminal root share the card
  blend; explicit terminal background overrides stay user overrides. Terminals
  draw no inner focus border; the card shows focus.
- Native browser canvases measure their frames inside the body below
  the header and sync the shared 10px radius through the native host.

- The workspace has no header or tab strip; each project shows one split
  container (see [Single tab per project](single-tab.md)). Sidebar chrome
  reserves native titlebar space and keeps window dragging; a collapsed sidebar
  reserves top space for the toggle.
- Sidebar project headers keep selection, menus and drag ordering; their
  worktrees are always expanded. Worktrees and primary checkouts use compact
  activity cards with 8px radii, 8px padding and 4px gaps. The first line shows
  the worktree name, agent glyph/activity indicator and relative age; the second
  prefers a running agent title, latest local session, open PR, then last commit;
  the third shows the branch, PRIMARY badge and accent PR number. Git metadata
  refreshes on worktree sync events, and local sessions share one batched cached
  catalog scan. Relative ages invalidate only the sidebar cache once a minute.
  The top row shares the native traffic-light row: sidebar toggle on the left,
  then the native Files and Source Control toggles on the right, tinted while
  their panel is open. Each opens its panel pinned on the right with the
  sidebar's chrome, for the current project; only one is open at a time.
- The footer has 16px side padding and 8px vertical padding: host selector,
  28px Filter pill, Add Project, New Browser. Collapsed sidebars stack the icon
  controls and hide the text input. Host choice persists in Appearance (All,
  Local, Server); the name query lives only in AppModel and Escape clears it.
  Case-insensitive name filtering keeps parent headers with matching worktrees
  and reveals matching rows. The
  host menu also opens remote server management.
- The agents panel is hidden by default. `Cmd+Opt+A` opens the left placement
  as a popup; right and detached placements dock or float as usual. Saved
  visibility is restored without docking into the sidebar.

Terminal `PaneView::element()` and the sidebar are cached. Titles and icons
use cached state; there are no native title/process queries, animations, new
dependencies or layout feedback loops. The terminal card-padding flag changes
once when a terminal joins the workspace.

## Validation

Style changes are validated with core tests, GPUI sidebar/filter tests,
Clippy with warnings denied, and documentation with rustdoc warnings denied.
Workspace integration tests also exercise sockets, subprocesses and SSH; those
need a session that permits those operations. The debug app is built for manual
visual review. The local validation commands are:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps
cargo run -p codesh-app
```

Compare one pane, nested splits and zoom to the references. Check inactive-card
controls, confirmations, find, selection, IME, native scrolling/webviews, resizing
and pane drops, collapsed/narrow sidebars, alternate themes and Linux controls.
Measure idle CPU/memory and resize responsiveness with several active panes.
