use std::collections::{HashMap, HashSet};

use codesh_protocol::FileEntry;

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) directory: bool,
    pub(crate) dimmed: bool,
}

#[derive(Default)]
pub(crate) struct Tree {
    directories: HashMap<String, Vec<Node>>,
    expanded: HashSet<String>,
    pending: HashSet<String>,
    refresh_again: HashSet<String>,
    pub(crate) selected: Option<String>,
}

pub(crate) fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

fn within(path: &str, directory: &str) -> bool {
    directory.is_empty()
        || path == directory
        || path
            .strip_prefix(directory)
            .is_some_and(|tail| tail.starts_with('/'))
}

impl Tree {
    pub(crate) fn loaded(&self, path: &str) -> bool {
        self.directories.contains_key(path)
    }

    pub(crate) fn request(&mut self, path: &str) -> bool {
        !self.directories.contains_key(path) && self.pending.insert(path.to_owned())
    }

    /// Returns whether changes received during this listing require another request.
    pub(crate) fn set_entries(&mut self, dir: &str, entries: Vec<FileEntry>) -> bool {
        self.pending.remove(dir);
        if !dir.is_empty() && !self.node(dir).is_some_and(|node| node.directory) {
            self.refresh_again.remove(dir);
            return false;
        }
        let mut nodes: Vec<_> = entries
            .into_iter()
            .filter_map(|entry| {
                let path = String::from_utf8(entry.path.0).ok()?;
                let name = String::from_utf8(entry.name.0).ok()?;
                if path.is_empty() || parent(&path) != dir {
                    return None;
                }
                Some(Node {
                    path,
                    dimmed: entry.is_ignored || name.starts_with('.'),
                    name,
                    directory: entry.is_directory,
                })
            })
            .collect();
        nodes.sort_by_cached_key(|node| {
            (!node.directory, node.name.to_lowercase(), node.name.clone())
        });
        let keys: HashSet<_> = nodes
            .iter()
            .map(|node| (node.path.as_str(), node.directory))
            .collect();
        if let Some(previous) = self.directories.get(dir) {
            let removed: Vec<_> = previous
                .iter()
                .filter(|old| !keys.contains(&(old.path.as_str(), old.directory)))
                .map(|node| node.path.clone())
                .collect();
            for path in removed {
                self.directories.retain(|key, _| !within(key, &path));
                self.expanded.retain(|key| !within(key, &path));
                self.pending.retain(|key| !within(key, &path));
                self.refresh_again.retain(|key| !within(key, &path));
                if self
                    .selected
                    .as_ref()
                    .is_some_and(|selected| within(selected, &path))
                {
                    self.selected = if dir.is_empty() {
                        None
                    } else {
                        Some(dir.to_owned())
                    };
                }
            }
        }
        self.directories.insert(dir.to_owned(), nodes);
        if self.refresh_again.remove(dir) {
            self.pending.insert(dir.to_owned());
            true
        } else {
            false
        }
    }

    pub(crate) fn expanded(&self, path: &str) -> bool {
        self.expanded.contains(path)
    }

    pub(crate) fn expand(&mut self, path: &str) -> bool {
        self.expanded.insert(path.to_owned());
        self.request(path)
    }

    pub(crate) fn collapse(&mut self, path: &str) {
        self.expanded.remove(path);
        if self
            .selected
            .as_ref()
            .is_some_and(|selected| selected != path && within(selected, path))
        {
            self.selected = Some(path.to_owned());
        }
    }

    pub(crate) fn collapse_all(&mut self) {
        self.expanded.clear();
        if let Some(selected) = &self.selected {
            self.selected = selected.split('/').next().map(str::to_owned);
        }
    }

    pub(crate) fn node(&self, path: &str) -> Option<&Node> {
        self.directories
            .get(parent(path))?
            .iter()
            .find(|node| node.path == path)
    }

    pub(crate) fn visible(&self) -> Vec<(&Node, usize)> {
        let mut rows = Vec::new();
        let mut stack: Vec<_> = self
            .directories
            .get("")
            .into_iter()
            .flatten()
            .rev()
            .map(|node| (node, 0))
            .collect();
        while let Some((node, depth)) = stack.pop() {
            rows.push((node, depth));
            if node.directory && self.expanded(&node.path) {
                stack.extend(
                    self.directories
                        .get(&node.path)
                        .into_iter()
                        .flatten()
                        .rev()
                        .map(|child| (child, depth + 1)),
                );
            }
        }
        rows
    }

    pub(crate) fn step(&mut self, forward: bool) {
        let rows = self.visible();
        let index = self
            .selected
            .as_ref()
            .and_then(|path| rows.iter().position(|(node, _)| &node.path == path));
        let next = index.map_or(0, |index| {
            if forward {
                (index + 1).min(rows.len().saturating_sub(1))
            } else {
                index.saturating_sub(1)
            }
        });
        self.selected = rows.get(next).map(|(node, _)| node.path.clone());
    }

    pub(crate) fn left(&mut self) {
        let Some(path) = self.selected.clone() else {
            return;
        };
        if self.expanded(&path) {
            self.collapse(&path);
        } else if !parent(&path).is_empty() {
            self.selected = Some(parent(&path).to_owned());
        }
    }

    pub(crate) fn right(&mut self) -> Option<String> {
        let path = self.selected.clone()?;
        if !self.node(&path).is_some_and(|node| node.directory) {
            return None;
        }
        if self.expanded(&path) {
            if let Some(child) = self.directories.get(&path).and_then(|nodes| nodes.first()) {
                self.selected = Some(child.path.clone());
            }
            None
        } else if self.expand(&path) {
            Some(path)
        } else {
            None
        }
    }

    fn visible_directory(&self, mut directory: &str) -> bool {
        while !directory.is_empty() {
            if !self.expanded(directory) {
                return false;
            }
            directory = parent(directory);
        }
        true
    }

    pub(crate) fn refresh(&mut self, paths: &[String], rescan: bool) -> Vec<String> {
        let mut affected: HashSet<_> = self
            .directories
            .keys()
            .chain(self.pending.iter())
            .chain(self.expanded.iter())
            .filter(|dir| {
                rescan
                    || paths
                        .iter()
                        .any(|path| parent(path) == dir.as_str() || within(dir, path))
            })
            .cloned()
            .collect();
        if rescan && !self.loaded("") {
            affected.insert(String::new());
        }
        let mut requests = Vec::new();
        for dir in affected {
            if self.visible_directory(&dir) {
                if self.pending.insert(dir.clone()) {
                    requests.push(dir);
                } else {
                    self.refresh_again.insert(dir);
                }
            } else {
                self.directories.remove(&dir);
            }
        }
        requests.sort();
        requests
    }

    pub(crate) fn retry(&mut self) {
        self.pending.clear();
        self.refresh_again.clear();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use codesh_protocol::ServerPath;

    fn entry(path: &str, directory: bool) -> FileEntry {
        FileEntry {
            name: ServerPath(path.rsplit('/').next().unwrap().as_bytes().to_vec()),
            path: ServerPath(path.as_bytes().to_vec()),
            is_directory: directory,
            is_ignored: false,
        }
    }

    #[test]
    fn files_panel_lazy_navigation_and_sorting() {
        let mut tree = Tree::default();
        tree.set_entries(
            "",
            vec![
                entry("z", false),
                entry("Beta", false),
                entry("src", true),
                entry(".env", false),
            ],
        );
        assert_eq!(
            tree.visible()
                .iter()
                .map(|(node, _)| node.name.as_str())
                .collect::<Vec<_>>(),
            ["src", ".env", "Beta", "z"]
        );
        assert!(tree.node(".env").unwrap().dimmed);
        tree.step(true);
        assert_eq!(tree.selected.as_deref(), Some("src"));
        assert!(tree.expand("src"));
        assert!(!tree.expand("src"));
        tree.set_entries("src", vec![entry("src/main.rs", false)]);
        tree.step(true);
        assert_eq!(tree.selected.as_deref(), Some("src/main.rs"));
        tree.collapse("src");
        assert_eq!(tree.selected.as_deref(), Some("src"));
        assert!(!tree.expand("src"));
        tree.step(false);
        assert_eq!(tree.selected.as_deref(), Some("src"));
    }

    #[test]
    fn files_panel_refresh_preserves_expansion_and_removes_deleted_or_renamed_subtrees() {
        let mut tree = Tree::default();
        tree.set_entries("", vec![entry("src", true), entry("other", true)]);
        tree.expand("src");
        tree.set_entries("src", vec![entry("src/old", true)]);
        tree.expand("src/old");
        tree.set_entries("src/old", vec![entry("src/old/a", false)]);
        tree.selected = Some("src/old/a".into());
        assert_eq!(tree.refresh(&["src/old/a".into()], false), ["src/old"]);
        assert!(tree.expanded("src/old"));
        tree.set_entries("src", vec![entry("src/new", true)]);
        assert_eq!(tree.selected.as_deref(), Some("src"));
        assert!(!tree.expanded("src/old"));
        tree.set_entries("", vec![entry("other", true)]);
        assert!(!tree.expanded("src"));
        assert!(tree.selected.is_none());
    }
    #[test]
    fn files_panel_watch_during_list_queues_a_fresh_listing() {
        let mut tree = Tree::default();
        assert!(tree.request(""));
        assert!(tree.refresh(&["new".into()], false).is_empty());
        assert!(tree.set_entries("", vec![]));
        assert!(!tree.set_entries("", vec![entry("new", false)]));
        tree.set_entries("", vec![entry("src", true)]);
        tree.expand("src");
        tree.set_entries("", vec![]);
        assert!(!tree.set_entries("src", vec![entry("src/stale", false)]));
        assert!(tree.visible().is_empty());
    }
    #[test]
    fn files_panel_horizontal_navigation_does_not_leave_empty_folder() {
        let mut tree = Tree::default();
        tree.set_entries("", vec![entry("a", true), entry("b", true)]);
        tree.step(true);
        assert_eq!(tree.right().as_deref(), Some("a"));
        tree.set_entries("a", vec![]);
        assert!(tree.right().is_none());
        assert_eq!(tree.selected.as_deref(), Some("a"));
        tree.set_entries("a", vec![entry("a/file", false)]);
        tree.right();
        assert_eq!(tree.selected.as_deref(), Some("a/file"));
        tree.left();
        assert_eq!(tree.selected.as_deref(), Some("a"));
        tree.left();
        assert!(!tree.expanded("a"));
        tree.right();
        tree.right();
        tree.collapse_all();
        assert_eq!(tree.selected.as_deref(), Some("a"));
        assert_eq!(tree.visible().len(), 2);
    }
    #[test]
    fn files_panel_refresh_only_touches_affected_expanded_directories() {
        let mut tree = Tree::default();
        tree.set_entries("", vec![entry("a", true), entry("b", true)]);
        tree.expand("a");
        tree.expand("b");
        tree.set_entries("a", vec![entry("a/x", false)]);
        tree.set_entries("b", vec![entry("b/y", false)]);
        assert_eq!(tree.refresh(&["a/x".into()], false), ["a"]);
        tree.set_entries("a", vec![entry("a/x", false)]);
        assert!(tree.expanded("a") && tree.expanded("b"));
        tree.collapse("a");
        assert!(tree.refresh(&["a/x".into()], false).is_empty());
        assert!(tree.expand("a"));
        assert!(!tree.expand("b"));
    }
    #[test]
    fn files_panel_rescan_retries_failed_initial_listing() {
        let mut tree = Tree::default();
        tree.request("");
        tree.retry();
        assert_eq!(tree.refresh(&[], true), [""]);
        assert!(tree.refresh(&[], true).is_empty());
        assert!(tree.set_entries("", vec![]));
    }
    #[test]
    fn files_panel_reconnect_reissues_root_and_unloaded_expanded_folders() {
        let mut tree = Tree::default();
        tree.set_entries("", vec![entry("src", true)]);
        assert!(tree.expand("src"));
        tree.retry();
        assert_eq!(tree.refresh(&[], true), ["", "src"]);
        tree.retry();
        assert_eq!(tree.refresh(&[], true), ["", "src"]);
        assert!(tree.refresh(&[], true).is_empty());
    }

    #[test]
    fn files_panel_collapsed_ancestors_do_not_refresh_hidden_descendants() {
        let mut tree = Tree::default();
        tree.set_entries("", vec![entry("a", true)]);
        tree.expand("a");
        tree.set_entries("a", vec![entry("a/sub", true)]);
        tree.expand("a/sub");
        tree.set_entries("a/sub", vec![entry("a/sub/file", false)]);
        tree.collapse("a");
        assert!(tree.refresh(&["a/sub/file".into()], false).is_empty());
        assert!(!tree.expand("a"));
        assert!(tree.expanded("a/sub"));
        assert!(tree.request("a/sub"));
    }
}
