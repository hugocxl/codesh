use super::*;
use codesh_protocol::{GitFile, GitFileStatus, GitLineStat, GitSummary, ServerPath};

pub(crate) fn fixture() -> GitStatus {
    let files = [
        ("staged.rs", b'A', b' '),
        ("changed.rs", b' ', b'M'),
        ("both.rs", b'M', b'M'),
        ("new.rs", b'?', b'?'),
        ("conflict.rs", b'U', b'U'),
        ("both-added.rs", b'A', b'A'),
        ("both-deleted.rs", b'D', b'D'),
    ]
    .into_iter()
    .map(|(path, index, worktree)| GitFileStatus {
        file: GitFile {
            path: ServerPath(path.as_bytes().to_vec()),
            original_path: None,
            index,
            worktree,
            added: None,
            removed: None,
        },
        staged: GitLineStat {
            additions: Some(3),
            deletions: Some(1),
            binary: false,
        },
        unstaged: GitLineStat {
            additions: Some(7),
            deletions: Some(2),
            binary: false,
        },
    })
    .collect();
    GitStatus {
        summary: GitSummary::default(),
        default_branch: None,
        branches: Vec::new(),
        files,
        pull_request: None,
    }
}

#[test]
fn git_panel_groups_partial_staging_and_conflicts_without_duplicate_conflicts() {
    let groups = Groups::from_status(&fixture());
    assert_eq!(groups.get(Group::Staged), &[0, 2]);
    assert_eq!(groups.get(Group::Changes), &[1, 2]);
    assert_eq!(groups.get(Group::Untracked), &[3]);
    assert_eq!(groups.get(Group::Conflicts), &[4, 5, 6]);
}

#[test]
fn git_panel_commit_enablement_protects_index_and_allows_amend_without_staging() {
    for amend in [false, true] {
        for staged in [0, 1, 4] {
            for message in ["", " \n\t", "Fix regression"] {
                let enabled = amend || (staged > 0 && !message.trim().is_empty());
                assert_eq!(commit_enabled(message, amend, false, staged, 0), enabled);
                assert!(!commit_enabled(message, amend, true, staged, 0));
                assert!(!commit_enabled(message, amend, false, staged, 2));
            }
        }
    }
}
