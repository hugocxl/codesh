//! Structural pane edits equalize only the maximal same-axis run containing
//! the leaf's parent. Remember its path before removal; if it dissolves, use
//! the enclosing run, including any same-axis neighbours exposed by collapse.
//! Docking's collector treats opposite-axis subtrees as indivisible members;
//! its balanced rebuild gives those members equal shares without deep chains.

use super::{Axis, Branch, Layout, docking::equalize_container};
use crate::PaneId;

impl Layout {
    pub(crate) fn equalize_parent(&mut self, pane: PaneId) {
        let mut path = Vec::new();
        if find(self, pane, &mut path) && !path.is_empty() {
            self.equalize_run(&path);
        }
    }

    pub(crate) fn remove_equalized(&mut self, pane: PaneId) {
        let mut path = Vec::new();
        if !find(self, pane, &mut path) || path.is_empty() {
            return;
        }
        let depth = run_start(&path);
        let axis = path[depth].1;
        self.remove(pane);
        let survives = matches!(at_mut(self, &path[..depth]),
            Some(Self::Split { axis: current, .. }) if *current == axis);
        if survives {
            self.equalize_run(&path);
        } else if depth > 0 {
            // The dissolved container's survivor now belongs to its enclosing
            // row or column. Flatten any newly adjacent same-axis splits.
            self.equalize_run(&path[..depth]);
        }
    }

    fn equalize_run(&mut self, path: &[(Branch, Axis)]) {
        if let Some(container) = at_mut(self, &path[..run_start(path)])
            && let Some(equalized) = equalize_container(container)
        {
            *container = equalized;
        }
    }
}

fn run_start(path: &[(Branch, Axis)]) -> usize {
    let Some((_, axis)) = path.last() else {
        return 0;
    };
    path.len() - path.iter().rev().take_while(|(_, a)| a == axis).count()
}

fn find(layout: &Layout, pane: PaneId, path: &mut Vec<(Branch, Axis)>) -> bool {
    match layout {
        Layout::Leaf(id) => *id == pane,
        Layout::Split {
            axis,
            first,
            second,
            ..
        } => {
            for (branch, child) in [(Branch::First, first), (Branch::Second, second)] {
                path.push((branch, *axis));
                if find(child, pane, path) {
                    return true;
                }
                path.pop();
            }
            false
        }
    }
}

fn at_mut<'a>(mut layout: &'a mut Layout, path: &[(Branch, Axis)]) -> Option<&'a mut Layout> {
    for (branch, _) in path {
        let Layout::Split { first, second, .. } = layout else {
            return None;
        };
        layout = match branch {
            Branch::First => first,
            Branch::Second => second,
        };
    }
    Some(layout)
}
