mod common;

use std::fs;

use awegit_git::{
    commit, file_diff, stage_all, stage_paths, status, unstage_paths, ChangeKind, CommitRequest, DiffLineKind,
};

#[test]
fn stage_commit_and_diff_round_trip() {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.git(&["config", "user.name", "Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "initial"]);

    fs::write(repo.path.join("a.txt"), "two\n").unwrap();
    fs::write(repo.path.join("b.txt"), "new\n").unwrap();

    let untracked_diff = file_diff(&repo.path, "b.txt", false).unwrap();
    assert!(untracked_diff.lines.iter().any(|line| line.text == "+new"), "{untracked_diff:?}");

    stage_paths(&repo.path, &["b.txt".to_string()]).unwrap();
    let staged = status(&repo.path).unwrap();
    assert_has(&staged, "b.txt", ChangeKind::Added, true);
    assert_has(&staged, "a.txt", ChangeKind::Modified, false);

    let unstaged_diff = file_diff(&repo.path, "a.txt", false).unwrap();
    assert!(unstaged_diff.lines.iter().any(|line| line.kind == DiffLineKind::Delete && line.text == "-one"));
    assert!(unstaged_diff.lines.iter().any(|line| line.kind == DiffLineKind::Add && line.text == "+two"));

    unstage_paths(&repo.path, &["b.txt".to_string()]).unwrap();
    let unstaged = status(&repo.path).unwrap();
    assert_has(&unstaged, "b.txt", ChangeKind::Untracked, false);

    stage_all(&repo.path).unwrap();
    let ready = status(&repo.path).unwrap();
    assert!(ready.unstaged.is_empty(), "{:?}", ready.unstaged);
    assert_has(&ready, "a.txt", ChangeKind::Modified, true);
    assert_has(&ready, "b.txt", ChangeKind::Added, true);

    let error = commit(
        &repo.path,
        CommitRequest {
            summary: "   ".to_string(),
            description: String::new(),
            amend: false,
            sign_off: false,
            sign_off_format: String::new(),
            skip_hooks: false,
        },
    );
    assert!(error.is_err());
    assert!(repo.output(&["log", "-1", "--format=%s"]).contains("initial"));

    commit(
        &repo.path,
        CommitRequest {
            summary: "stage both".to_string(),
            description: "body".to_string(),
            amend: false,
            sign_off: true,
            sign_off_format: String::new(),
            skip_hooks: false,
        },
    )
    .unwrap();
    let message = repo.output(&["log", "-1", "--format=%B"]);
    assert!(message.contains("stage both"), "{message}");
    assert!(message.contains("body"), "{message}");
    assert!(message.contains("Signed-off-by:"), "{message}");
    let clean = status(&repo.path).unwrap();
    assert!(clean.staged.is_empty() && clean.unstaged.is_empty(), "{clean:?}");
}

#[test]
fn stage_rejects_a_path_outside_the_work_tree() {
    let repo = common::TempRepo::new();
    let error = stage_paths(&repo.path, &["../outside.txt".to_string()]).unwrap_err();
    assert!(error.to_string().contains("outside.txt"), "{error}");
}

fn assert_has(snapshot: &awegit_git::StatusSnapshot, path: &str, kind: ChangeKind, staged: bool) {
    let list = if staged { &snapshot.staged } else { &snapshot.unstaged };
    assert!(
        list.iter().any(|change| change.path == path && change.kind == kind),
        "missing staged={staged} {path} {kind:?} in {snapshot:?}"
    );
}
