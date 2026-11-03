mod common;

use std::fs;

use awegit_git::{
    blame_file, branch_commits, commit_detail, commit_files, commit_log, commits_from, locate_ref, repository_refs, ChangeKind,
};

fn repo() -> common::TempRepo {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.git(&["config", "core.hooksPath", ".git/no-hooks"]);
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
    assert!(log.iter().all(|commit| commit.at > 0), "{log:?}");

    let detail = commit_detail(&repo.path, "HEAD").unwrap();
    assert_eq!(detail.author, "Test");
    assert_eq!(detail.committer, "Test");
    assert_eq!(detail.committer_email, "test@example.com");
    assert!(detail.author_at > 0, "{detail:?}");
    assert!(detail.body.is_empty(), "{detail:?}");
    assert_eq!(detail.summary, "rename");

    let refs = repository_refs(&repo.path).unwrap();
    assert!(refs.branches.iter().any(|branch| branch.name == "main" && branch.current));
    assert!(refs.branches.iter().any(|branch| branch.name == "topic" && !branch.current));
    assert!(refs.tags.iter().any(|tag| tag.name == "v1"));
    assert_eq!(refs.worktrees.len(), 1, "{:?}", refs.worktrees);
}

#[test]
fn branch_log_hides_other_branches() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "first"]);
    repo.git(&["checkout", "-b", "other"]);
    fs::write(repo.path.join("b.txt"), "two\n").unwrap();
    repo.git(&["add", "b.txt"]);
    repo.git(&["commit", "-m", "only-other"]);
    repo.git(&["checkout", "main"]);

    let current = branch_commits(&repo.path, 20).unwrap();
    assert!(current.iter().all(|commit| commit.summary != "only-other"), "{current:?}");
    let all = commit_log(&repo.path, 20).unwrap();
    assert!(all.iter().any(|commit| commit.summary == "only-other"), "{all:?}");
}

#[test]
fn stash_commit_lists_its_changes() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "first"]);
    fs::write(repo.path.join("a.txt"), "two\n").unwrap();
    repo.git(&["stash", "push", "-m", "wip"]);

    let refs = repository_refs(&repo.path).unwrap();
    let stash = refs.stashes.first().expect("stash");
    let files = commit_files(&repo.path, &stash.id).unwrap();
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(files[0].path, "a.txt");
    let diff = awegit_git::show_commit_file(&repo.path, &stash.id, "a.txt").unwrap();
    assert!(diff.lines.iter().any(|line| line.text.contains("two")), "{diff:?}");
}

#[test]
fn locate_ref_finds_a_branch_buried_under_later_commits() {
    let repo = repo();
    repo.git(&["commit", "--allow-empty", "-m", "base"]);
    repo.git(&["branch", "buried"]);
    let buried = repo.output(&["rev-parse", "buried"]).trim().to_string();
    for i in 0..40 {
        repo.git(&["commit", "--allow-empty", "-m", &format!("feat: later {i}")]);
    }
    let found = locate_ref(&repo.path, "buried", false).unwrap();
    assert_eq!(found.id, buried);
    assert!(found.commits.iter().any(|commit| commit.id == buried), "{} commits", found.commits.len());
}

#[test]
fn current_branch_log_still_draws_remote_forks() {
    let repo = repo();
    repo.git(&["commit", "--allow-empty", "-m", "base"]);
    repo.git(&["checkout", "-b", "fork"]);
    repo.git(&["commit", "--allow-empty", "-m", "feat: only on the fork"]);
    let tip = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    repo.git(&["update-ref", "refs/remotes/origin/fork", &tip]);
    repo.git(&["checkout", "main"]);
    repo.git(&["branch", "-D", "fork"]);
    let rows = commits_from(&repo.path, 20, &["main".into()], false).unwrap();
    let found = rows.iter().find(|commit| commit.id == tip).expect("remote fork tip");
    assert!(found.refs.iter().any(|name| name == "origin/fork"), "{:?}", found.refs);
}
