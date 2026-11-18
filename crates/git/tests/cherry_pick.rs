mod common;

use std::fs;

use awegit_git::{in_progress, perform, InProgress, Mutation};
use common::TempRepo;

fn repo() -> TempRepo {
    let repo = TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    for (key, value) in [
        ("user.name", "Test"),
        ("user.email", "test@example.com"),
        ("core.autocrlf", "false"),
        ("core.hooksPath", ".git/no-hooks"),
        ("commit.gpgsign", "false"),
    ] {
        repo.git(&["config", key, value]);
    }
    commit(&repo, "local.txt", "base\n", "base");
    commit(&repo, "shared.txt", "base\n", "shared base");
    repo
}

fn commit(repo: &TempRepo, file: &str, text: &str, message: &str) -> String {
    fs::write(repo.path.join(file), text).unwrap();
    repo.git(&["add", "--", file]);
    repo.git(&["commit", "-m", message]);
    repo.output(&["rev-parse", "HEAD"]).trim().into()
}

fn pick(revs: Vec<String>, record: bool) -> Mutation {
    Mutation::CherryPick {
        rev: String::new(),
        revs,
        record,
    }
}

fn dirty(repo: &TempRepo) -> (String, String) {
    fs::write(repo.path.join("local.txt"), "staged\n").unwrap();
    repo.git(&["add", "local.txt"]);
    fs::write(repo.path.join("local.txt"), "unstaged\n").unwrap();
    (repo.output(&["diff", "--cached"]), repo.output(&["diff"]))
}

#[test]
fn cherry_pick_batch_orders_non_adjacent_commits_and_records_each_source_once() {
    let repo = repo();
    repo.git(&["checkout", "-b", "topic"]);
    let first = commit(&repo, "picked.txt", "one\n", "first selected");
    commit(&repo, "skip.txt", "skip\n", "not selected");
    let last = commit(&repo, "picked.txt", "two\n", "last selected");
    repo.git(&["checkout", "main"]);
    commit(&repo, "main.txt", "main\n", "main change");
    let local = dirty(&repo);

    perform(
        &repo.path,
        pick(vec![last.clone(), first.clone(), last.clone()], true),
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(repo.path.join("picked.txt")).unwrap(),
        "two\n"
    );
    assert!(!repo.path.join("skip.txt").exists());
    assert_eq!(
        repo.output(&["log", "-2", "--reverse", "--format=%s"]),
        "first selected\nlast selected\n"
    );
    for (rev, source) in [("HEAD~1", first), ("HEAD", last)] {
        assert!(repo
            .output(&["log", "-1", "--format=%B", rev])
            .contains(&format!("(cherry picked from commit {source})")));
    }
    assert_eq!(
        (repo.output(&["diff", "--cached"]), repo.output(&["diff"])),
        local
    );
    assert!(repo.output(&["stash", "list"]).is_empty());
}

#[test]
fn cherry_pick_batch_validates_every_revision_before_changing_head_or_local_files() {
    let repo = repo();
    repo.git(&["checkout", "-b", "topic"]);
    let first = commit(&repo, "picked.txt", "one\n", "selected");
    repo.git(&["checkout", "main"]);
    let head = repo.output(&["rev-parse", "HEAD"]);
    let local = dirty(&repo);

    for invalid in ["missing-commit", "--abort", "HEAD..topic"] {
        assert!(perform(&repo.path, pick(vec![first.clone(), invalid.into()], false)).is_err());
        assert_eq!(repo.output(&["rev-parse", "HEAD"]), head);
        assert!(!repo.path.join("picked.txt").exists());
        assert_eq!(
            (repo.output(&["diff", "--cached"]), repo.output(&["diff"])),
            local
        );
        assert!(repo.output(&["stash", "list"]).is_empty());
    }
}

fn paused_sequence() -> (TempRepo, String, (String, String)) {
    let repo = repo();
    repo.git(&["checkout", "-b", "topic"]);
    let first = commit(&repo, "first.txt", "first\n", "first selected");
    let conflict = commit(&repo, "shared.txt", "topic\n", "conflicting selected");
    let last = commit(&repo, "last.txt", "last\n", "last selected");
    repo.git(&["checkout", "main"]);
    let head = commit(&repo, "shared.txt", "main\n", "main change");
    let local = dirty(&repo);
    assert!(perform(&repo.path, pick(vec![last, conflict, first], false)).is_err());
    assert_eq!(
        in_progress(&repo.path).unwrap(),
        Some(InProgress::CherryPick)
    );
    assert!(repo.path.join("first.txt").exists());
    assert!(!repo.path.join("last.txt").exists());
    assert_eq!(
        repo.output(&["stash", "list", "--format=%s"])
            .lines()
            .count(),
        1
    );
    (repo, head, local)
}

fn assert_settled(repo: &TempRepo, local: (String, String)) {
    assert_eq!(in_progress(&repo.path).unwrap(), None);
    assert_eq!(
        (repo.output(&["diff", "--cached"]), repo.output(&["diff"])),
        local
    );
    assert!(repo.output(&["stash", "list"]).is_empty());
}

#[test]
fn cherry_pick_batch_continue_applies_remaining_commits_and_restores_local_changes() {
    let (repo, _, local) = paused_sequence();
    fs::write(repo.path.join("shared.txt"), "resolved\n").unwrap();
    repo.git(&["add", "shared.txt"]);
    perform(
        &repo.path,
        Mutation::Continue {
            message: String::new(),
        },
    )
    .unwrap();
    assert!(repo.path.join("last.txt").exists());
    assert_eq!(
        fs::read_to_string(repo.path.join("shared.txt")).unwrap(),
        "resolved\n"
    );
    assert_settled(&repo, local);
}

#[test]
fn cherry_pick_batch_skip_keeps_remaining_commits() {
    let (repo, _, local) = paused_sequence();
    perform(&repo.path, Mutation::Skip).unwrap();
    assert!(repo.path.join("first.txt").exists());
    assert!(repo.path.join("last.txt").exists());
    assert_eq!(
        fs::read_to_string(repo.path.join("shared.txt")).unwrap(),
        "main\n"
    );
    assert_settled(&repo, local);
}

#[test]
fn cherry_pick_batch_abort_restores_the_head_before_the_entire_sequence() {
    let (repo, head, local) = paused_sequence();
    perform(&repo.path, Mutation::Abort).unwrap();
    assert_eq!(repo.output(&["rev-parse", "HEAD"]).trim(), head);
    assert!(!repo.path.join("first.txt").exists());
    assert!(!repo.path.join("last.txt").exists());
    assert_settled(&repo, local);
}

#[test]
fn cherry_pick_requests_accept_single_revision_and_batch_payloads() {
    for payload in [
        r#"{"action":"cherryPick","rev":"topic","record":true}"#,
        r#"{"action":"cherryPick","revs":["first","last"]}"#,
    ] {
        let request: Mutation = serde_json::from_str(payload).unwrap();
        match request {
            Mutation::CherryPick { rev, revs, record } if revs.is_empty() => {
                assert_eq!(rev, "topic");
                assert!(record);
            }
            Mutation::CherryPick { rev, revs, record } => {
                assert!(rev.is_empty());
                assert_eq!(revs, ["first", "last"]);
                assert!(!record);
            }
            _ => panic!("unexpected mutation"),
        }
    }
}
