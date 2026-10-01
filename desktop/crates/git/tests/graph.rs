mod common;

use std::fs;

use awegit_git::{blame_file, commit_files, commit_log, repository_refs, ChangeKind};

fn repo() -> common::TempRepo {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.git(&["config", "user.name", "Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    repo
}

#[test]
fn empty_repository_has_no_commits() {
    let repo = repo();
    let log = commit_log(&repo.path, 20).unwrap();
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn log_lists_commits_branches_and_tags() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "first"]);
    repo.git(&["branch", "topic"]);

    fs::rename(repo.path.join("a.txt"), repo.path.join("c.txt")).unwrap();
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-m", "rename"]);
    repo.git(&["tag", "v1"]);

    let log = commit_log(&repo.path, 20).unwrap();
    assert_eq!(log.len(), 2, "{log:?}");
    assert!(log[0].refs.iter().any(|name| name == "main"), "{:?}", log[0].refs);
    assert!(log[0].refs.iter().any(|name| name == "v1"), "{:?}", log[0].refs);
    assert!(log.iter().any(|commit| commit.refs.iter().any(|name| name == "topic")), "{log:?}");
    assert!(log.iter().all(|commit| commit.lane < 8));

    let files = commit_files(&repo.path, "HEAD").unwrap();
    let renamed = files.iter().find(|file| file.path == "c.txt").expect("renamed file");
    assert_eq!(renamed.kind, ChangeKind::Renamed);
    assert_eq!(renamed.previous_path.as_deref(), Some("a.txt"));

    let blame = blame_file(&repo.path, "c.txt").unwrap();
    assert!(blame.iter().any(|line| line.text.contains("one")), "{blame:?}");

    let refs = repository_refs(&repo.path).unwrap();
    assert!(refs.branches.iter().any(|branch| branch.name == "main" && branch.current));
    assert!(refs.branches.iter().any(|branch| branch.name == "topic" && !branch.current));
    assert!(refs.tags.iter().any(|tag| tag.name == "v1"));
    assert_eq!(refs.worktrees.len(), 1, "{:?}", refs.worktrees);
}
