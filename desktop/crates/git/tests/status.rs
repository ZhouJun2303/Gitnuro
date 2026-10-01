mod common;

use std::fs;

use awegit_git::{status, ChangeKind, FileChange};

#[test]
fn status_splits_staged_and_unstaged_changes() {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "status.showUntrackedFiles", "all"]);
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "initial"]);

    fs::write(repo.path.join("a.txt"), "two\n").unwrap();
    fs::write(repo.path.join("b.txt"), "new\n").unwrap();

    let before_add = status(&repo.path).unwrap();
    assert_eq!(before_add.branch.as_deref(), Some("main"));
    assert!(before_add.staged.is_empty(), "{:?}", before_add.staged);
    assert_has(&before_add.unstaged, "a.txt", ChangeKind::Modified);
    assert_has(&before_add.unstaged, "b.txt", ChangeKind::Untracked);

    repo.git(&["add", "b.txt"]);
    let after_add = status(&repo.path).unwrap();
    assert_has(&after_add.staged, "b.txt", ChangeKind::Added);
    assert_has(&after_add.unstaged, "a.txt", ChangeKind::Modified);
    assert!(!after_add.unstaged.iter().any(|change| change.path == "b.txt"));
}

fn assert_has(changes: &[FileChange], path: &str, kind: ChangeKind) {
    assert!(
        changes.iter().any(|change| change.path == path && change.kind == kind),
        "missing {path} {kind:?} in {changes:?}"
    );
}
