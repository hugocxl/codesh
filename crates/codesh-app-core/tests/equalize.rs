#![allow(
    clippy::many_single_char_names,
    reason = "Pane letters mirror the layout diagrams in each test"
)]

use codesh_app_core::browser::BrowserPane;
use codesh_app_core::{AppState, Branch, Direction, Layout, PaneId};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn assert_row(layout: &Layout, order: &[PaneId], edge: Direction, share: f32) {
    assert_eq!(layout.leaves(), order);
    let dimension = if matches!(edge, Direction::Left | Direction::Right) {
        2
    } else {
        3
    };
    let mut position = 0.0;
    for (_, rect) in layout.rects() {
        assert!((rect[dimension] - share).abs() < 1e-5, "{rect:?}");
        assert!((rect[dimension - 2] - position).abs() < 1e-5, "{rect:?}");
        position += share;
    }
    assert!(layout.validate().is_ok());
}

#[test]
fn insertions_and_closes_equalize_both_axes_and_directions() -> Result {
    for edge in [
        Direction::Left,
        Direction::Right,
        Direction::Up,
        Direction::Down,
    ] {
        let mut state = AppState::bootstrap()?;
        let tab = state.open_terminal_tab(state.home().id)?;
        let first = state.home().tabs[0].panes[0].id;
        let mut order = vec![first];
        let mut anchor = first;
        for share in [0.5, 1.0 / 3.0, 0.25] {
            if order.len() > 1 {
                state.set_ratio(tab, &[], 0.7)?;
            }
            let new = state.split_pane(anchor, edge)?;
            if matches!(edge, Direction::Left | Direction::Up) {
                order.insert(0, new);
            } else {
                order.push(new);
            }
            anchor = new;
            assert_row(&state.home().tabs[0].layout, &order, edge, share);
        }
        state.set_ratio(tab, &[], 0.65)?;
        state.close_pane(order.remove(1))?;
        assert_row(&state.home().tabs[0].layout, &order, edge, 1.0 / 3.0);
        state.set_ratio(tab, &[], 0.3)?;
        state.close_pane(order.remove(0))?;
        assert_row(&state.home().tabs[0].layout, &order, edge, 0.5);
        let restored: AppState = serde_json::from_value(serde_json::to_value(&state)?)?;
        assert_eq!(restored, state);
    }
    Ok(())
}

#[test]
fn mixed_axis_members_keep_internal_ratios_and_unrelated_containers() -> Result {
    let mut state = AppState::bootstrap()?;
    let tab = state.open_terminal_tab(state.home().id)?;
    let a = state.home().tabs[0].panes[0].id;
    let b = state.split_pane(a, Direction::Right)?;
    let c = state.split_pane(b, Direction::Down)?;
    state.set_ratio(tab, &[Branch::Second], 0.3)?;
    let stack = match &state.home().tabs[0].layout {
        Layout::Split { second, .. } => second.as_ref().clone(),
        Layout::Leaf(_) => return Err("missing stack".into()),
    };
    let d = state.split_pane(a, Direction::Right)?;
    let rects = state.home().tabs[0].layout.rects();
    for (_, rect) in &rects {
        assert!((rect[2] - 1.0 / 3.0).abs() < 1e-5);
    }
    assert_eq!(state.home().tabs[0].layout.leaves(), [a, d, b, c]);
    // The opposite-axis member survives a neighbouring insertion and close exactly.
    state.close_pane(d)?;
    if let Layout::Split { second, .. } = &state.home().tabs[0].layout {
        assert_eq!(second.as_ref(), &stack);
    } else {
        return Err("missing stack".into());
    }
    state.set_ratio(tab, &[], 0.65)?;
    let e = state.split_pane(b, Direction::Down)?;
    // Editing the stack leaves the enclosing, manually sized column alone.
    let rects = state.home().tabs[0].layout.rects();
    assert!((rects[0].1[2] - 0.65).abs() < 1e-5);
    state.close_pane(e)?;
    assert!((state.home().tabs[0].layout.rects()[0].1[2] - 0.65).abs() < 1e-5);
    Ok(())
}

#[test]
fn collapse_merges_exposed_same_axis_neighbours() -> Result {
    let mut state = AppState::bootstrap()?;
    let tab = state.open_terminal_tab(state.home().id)?;
    let a = state.home().tabs[0].panes[0].id;
    let b = state.split_pane(a, Direction::Right)?;
    let c = state.split_pane(b, Direction::Down)?;
    let d = state.split_pane(c, Direction::Right)?;
    let e = state.split_pane(d, Direction::Right)?;
    state.set_ratio(tab, &[], 0.7)?;
    state.set_ratio(tab, &[Branch::Second, Branch::Second], 0.6)?;
    state.close_pane(b)?;
    assert_row(
        &state.home().tabs[0].layout,
        &[a, c, d, e],
        Direction::Right,
        0.25,
    );
    Ok(())
}

#[test]
fn dissolving_root_keeps_surviving_opposite_axis_layout() -> Result {
    let mut state = AppState::bootstrap()?;
    let tab = state.open_terminal_tab(state.home().id)?;
    let a = state.home().tabs[0].panes[0].id;
    let b = state.split_pane(a, Direction::Right)?;
    let c = state.split_pane(b, Direction::Down)?;
    state.set_ratio(tab, &[Branch::Second], 0.3)?;
    state.close_pane(a)?;
    assert_eq!(state.home().tabs[0].layout.leaves(), [b, c]);
    assert!((state.home().tabs[0].layout.rects()[0].1[3] - 0.3).abs() < 1e-5);
    Ok(())
}

#[test]
fn browser_insertions_equalize_long_rows_without_changing_focus() -> Result {
    let mut state = AppState::bootstrap()?;
    let tab = state.open_terminal_tab(state.home().id)?;
    let first = state.home().tabs[0].panes[0].id;
    let mut order = vec![first, state.split_pane(first, Direction::Right)?];
    state.focus_pane(first)?;
    for _ in 0..10 {
        state.set_ratio(tab, &[], 0.75)?;
        let anchor = *order.last().ok_or("anchor")?;
        order.push(state.split_browser(anchor, BrowserPane::default(), false)?);
        #[allow(clippy::cast_precision_loss, reason = "At most twelve panes")]
        let share = 1.0 / order.len() as f32;
        assert_row(
            &state.home().tabs[0].layout,
            &order,
            Direction::Right,
            share,
        );
        assert_eq!(state.window().active_pane, Some(first));
    }
    let restored: AppState = serde_json::from_value(serde_json::to_value(&state)?)?;
    assert_eq!(restored, state);
    Ok(())
}

#[test]
fn editing_one_stack_preserves_its_resized_sibling() -> Result {
    let mut state = AppState::bootstrap()?;
    let tab = state.open_terminal_tab(state.home().id)?;
    let a = state.home().tabs[0].panes[0].id;
    let b = state.split_pane(a, Direction::Right)?;
    state.split_pane(a, Direction::Down)?;
    state.split_pane(b, Direction::Down)?;
    state.set_ratio(tab, &[], 0.6)?;
    state.set_ratio(tab, &[Branch::Second], 0.35)?;
    let before = state.home().tabs[0].layout.rects();
    let new = state.split_pane(a, Direction::Down)?;
    for (id, rect) in &before[2..] {
        assert_eq!(
            state.home().tabs[0]
                .layout
                .rects()
                .iter()
                .find(|(pane, _)| pane == id),
            Some(&(*id, *rect))
        );
    }
    state.close_pane(new)?;
    for (id, rect) in &before[2..] {
        assert_eq!(
            state.home().tabs[0]
                .layout
                .rects()
                .iter()
                .find(|(pane, _)| pane == id),
            Some(&(*id, *rect))
        );
    }
    Ok(())
}

#[test]
fn long_terminal_runs_remain_equal_and_valid_on_both_axes() -> Result {
    for edge in [Direction::Right, Direction::Down] {
        let mut state = AppState::bootstrap()?;
        state.open_terminal_tab(state.home().id)?;
        let first = state.home().tabs[0].panes[0].id;
        let mut order = vec![first];
        let mut anchor = first;
        for _ in 0..23 {
            anchor = state.split_pane(anchor, edge)?;
            order.push(anchor);
        }
        assert_row(&state.home().tabs[0].layout, &order, edge, 1.0 / 24.0);
    }
    Ok(())
}
