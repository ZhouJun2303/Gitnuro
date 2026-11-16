mod common;

use std::fs;

use awegit_git::{capture_output, perform, Mutation};
use common::TempRepo;

fn repo(conflict: bool) -> TempRepo {
    let repo = TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    for (key, value) in [
        ("core.autocrlf", "false"), ("core.hooksPath", ".git/no-hooks"),
        ("commit.gpgsign", "false"), ("user.name", "Test"), ("user.email", "test@example.com"),
    ] {
        repo.git(&["config", key, value]);
    }
    fs::write(repo.path.join("note.txt"), "one\ntwo\nthree\nfour\nfive\n").unwrap();
    fs::write(repo.path.join("keep.txt"), "base\n").unwrap();
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("keep.txt"), "older stash\n").unwrap();
    repo.git(&["stash", "push", "-m", "older unrelated work"]);
    repo.git(&["checkout", "-b", "topic"]);
    fs::write(repo.path.join("note.txt"), if conflict {
        "one\ntwo\nthree\nfour\nremote five\n"
    } else {
        "one\nremote two\nthree\nfour\nfive\n"
    }).unwrap();
    repo.git(&["commit", "-am", "topic"]);
    repo.git(&["checkout", "main"]);
    fs::write(repo.path.join("note.txt"), "one\ntwo\nthree\nfour\nlocal five\n").unwrap();
    fs::write(repo.path.join("keep.txt"), "staged\n").unwrap();
    repo.git(&["add", "."]);
    fs::write(repo.path.join("keep.txt"), "unstaged\n").unwrap();
    fs::write(repo.path.join("loose.txt"), "untracked\n").unwrap();
    repo
}

#[test]
fn checkout_restores_work_when_the_saved_index_patch_no_longer_applies() {
    let repo = repo(false);
    let older = repo.output(&["rev-parse", "refs/stash"]);
    let (result, log) = capture_output(|| perform(&repo.path, Mutation::Checkout { name: "topic".into() }));
    result.unwrap();
    assert!(log.contains("conflicts in index"), "must exercise the failed index patch: {log}");
    assert_eq!(fs::read_to_string(repo.path.join("note.txt")).unwrap(), "one\nremote two\nthree\nfour\nlocal five\n");
    assert_eq!(fs::read_to_string(repo.path.join("keep.txt")).unwrap(), "unstaged\n");
    assert_eq!(fs::read_to_string(repo.path.join("loose.txt")).unwrap(), "untracked\n");
    assert_eq!(repo.output(&["rev-parse", "stash@{1}"]), older);
    assert_eq!(repo.output(&["show", "stash@{0}^2:keep.txt"]), "staged\n");
    assert!(repo.output(&["stash", "list"]).contains("AweGit autostash"));
    assert!(repo.output(&["ls-files", "--unmerged"]).is_empty());
}

#[test]
fn pull_restores_work_when_the_saved_index_patch_no_longer_applies() {
    for rebase in [false, true] {
        let repo = repo(false);
        repo.git(&["remote", "add", "origin", "."]);
        perform(&repo.path, Mutation::PullRef {
            remote: "origin".into(), branch: "topic".into(), rebase, autostash: true,
        }).unwrap();
        assert_eq!(fs::read_to_string(repo.path.join("note.txt")).unwrap(), "one\nremote two\nthree\nfour\nlocal five\n");
        assert_eq!(fs::read_to_string(repo.path.join("keep.txt")).unwrap(), "unstaged\n");
        assert_eq!(fs::read_to_string(repo.path.join("loose.txt")).unwrap(), "untracked\n");
        assert_eq!(repo.output(&["show", "stash@{0}^2:keep.txt"]), "staged\n");
        assert!(repo.output(&["ls-files", "--unmerged"]).is_empty());
    }
}

#[test]
fn merge_restores_other_files_when_the_saved_index_has_a_real_conflict() {
    let repo = repo(true);
    perform(&repo.path, Mutation::Merge {
        name: "topic".into(), squash: false, no_ff: false, autostash: true,
        summary: String::new(), description: String::new(),
    }).unwrap();
    let text = fs::read_to_string(repo.path.join("note.txt")).unwrap();
    assert!(text.contains("<<<<<<<") && text.contains("local five") && text.contains("remote five"), "{text}");
    assert_eq!(fs::read_to_string(repo.path.join("keep.txt")).unwrap(), "unstaged\n");
    assert_eq!(fs::read_to_string(repo.path.join("loose.txt")).unwrap(), "untracked\n");
    assert_eq!(repo.output(&["show", "stash@{0}^2:keep.txt"]), "staged\n");
    assert_eq!(repo.output(&["stash", "list", "--format=%H"]).lines().count(), 2);
}

#[test]
fn failed_checkout_preserves_the_index_and_does_not_pop_an_older_stash() {
    let repo = repo(false);
    let status = repo.output(&["status", "--porcelain=v1"]);
    let staged = repo.output(&["diff", "--cached"]);
    let unstaged = repo.output(&["diff"]);
    let stashes = repo.output(&["stash", "list", "--format=%H"]);
    assert!(perform(&repo.path, Mutation::Checkout { name: "missing".into() }).is_err());
    assert_eq!(repo.output(&["branch", "--show-current"]), "main\n");
    assert_eq!(repo.output(&["status", "--porcelain=v1"]), status);
    assert_eq!(repo.output(&["diff", "--cached"]), staged);
    assert_eq!(repo.output(&["diff"]), unstaged);
    assert_eq!(repo.output(&["stash", "list", "--format=%H"]), stashes);
    assert_eq!(fs::read_to_string(repo.path.join("loose.txt")).unwrap(), "untracked\n");
}

#[test]
fn checkout_keeps_the_pending_backup_when_an_untracked_file_cannot_be_restored() {
    let repo = repo(false);
    let older = repo.output(&["rev-parse", "refs/stash"]);
    // The destination tracks a file that is untracked in our saved local work.
    repo.git(&["stash", "push", "--include-untracked"]);
    repo.git(&["checkout", "topic"]);
    fs::write(repo.path.join("note.txt"), repo.output(&["show", "main:note.txt"])).unwrap();
    fs::write(repo.path.join("loose.txt"), "destination content\n").unwrap();
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "track the previously untracked path"]);
    repo.git(&["checkout", "main"]);
    repo.git(&["stash", "pop", "--index"]);

    let (result, log) = capture_output(|| perform(&repo.path, Mutation::Checkout { name: "topic".into() }));
    assert!(result.is_err(), "{log}");
    // This failure can happen after tracked files were restored. Do not retry
    // without --index or silently discard the outstanding recovery record.
    assert_eq!(log.lines().filter(|line| line.starts_with("$ git stash apply ")).count(), 1, "{log}");
    assert_eq!(fs::read_to_string(repo.path.join("keep.txt")).unwrap(), "unstaged\n");
    assert_eq!(fs::read_to_string(repo.path.join("loose.txt")).unwrap(), "destination content\n");
    assert_eq!(repo.output(&["show", "stash@{0}^3:loose.txt"]), "untracked\n");
    assert_eq!(repo.output(&["show", "stash@{0}^2:keep.txt"]), "staged\n");
    assert_eq!(repo.output(&["rev-parse", "stash@{1}"]), older);
    assert_eq!(repo.output(&["config", "--file", ".git/awegit", "autostash.pending"]), repo.output(&["rev-parse", "refs/stash"]));
    assert!(repo.output(&["ls-files", "--unmerged"]).is_empty());
}
