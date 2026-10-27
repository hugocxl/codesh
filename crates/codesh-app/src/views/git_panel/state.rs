use codesh_protocol::GitStatus;

#[cfg(test)]
#[path = "state/tests.rs"]
pub(super) mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Group {
    Staged,
    Changes,
    Untracked,
    Conflicts,
}

impl Group {
    pub(super) const ALL: [Self; 4] = [
        Self::Staged,
        Self::Changes,
        Self::Untracked,
        Self::Conflicts,
    ];

    pub(super) const fn index(self) -> usize {
        match self {
            Self::Staged => 0,
            Self::Changes => 1,
            Self::Untracked => 2,
            Self::Conflicts => 3,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Staged => "Staged Changes",
            Self::Changes => "Changes",
            Self::Untracked => "Untracked",
            Self::Conflicts => "Conflicts",
        }
    }
}

#[derive(Default)]
pub(super) struct Groups([Vec<usize>; 4]);

impl Groups {
    pub(super) fn from_status(status: &GitStatus) -> Self {
        let mut groups = Self::default();
        for (index, entry) in status.files.iter().enumerate() {
            let file = &entry.file;
            if file.conflicted() {
                groups.0[Group::Conflicts.index()].push(index);
            } else if file.untracked() {
                groups.0[Group::Untracked.index()].push(index);
            } else {
                if file.staged() {
                    groups.0[Group::Staged.index()].push(index);
                }
                if file.unstaged() {
                    groups.0[Group::Changes.index()].push(index);
                }
            }
        }
        groups
    }

    pub(super) fn get(&self, group: Group) -> &[usize] {
        &self.0[group.index()]
    }
}

#[derive(Clone, Copy)]
pub(super) enum Row {
    Header(Group),
    File(Group, usize),
}

pub(super) fn commit_enabled(
    message: &str,
    amend: bool,
    busy: bool,
    staged: usize,
    conflicts: usize,
) -> bool {
    !busy && conflicts == 0 && (amend || (!message.trim().is_empty() && staged > 0))
}
