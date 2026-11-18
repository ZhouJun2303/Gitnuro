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

#[test]
fn cherry_pick_drops_consecutive_and_trailing_empty_picks_and_restores_local_changes() {
    let repo = repo();
    repo.git(&["checkout", "-b", "topic"]);
    let first = commit(&repo, "first.txt", "first\n", "already applied first");
    repo.git(&["commit", "--allow-empty", "-m", "originally empty"]);
    let empty = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    let middle = commit(&repo, "middle.txt", "middle\n", "new middle");
    let last = commit(&repo, "last.txt", "last\n", "already applied last");
    repo.git(&["checkout", "main"]);
    repo.git(&["cherry-pick", &first, &last]);
    let head = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    let local = dirty(&repo);

    perform(&repo.path, pick(vec![last, middle.clone(), empty, first], true)).unwrap();

    assert_eq!(repo.output(&["rev-list", "--count", &format!("{head}..HEAD")]).trim(), "1");
    assert_eq!(repo.output(&["log", "-1", "--format=%s"]).trim(), "new middle");
    assert!(repo.output(&["log", "-1", "--format=%B"]).contains(&middle));
    assert_settled(&repo, local);
}

#[test]
fn cherry_pick_all_redundant_picks_finish_without_creating_commits() {
    for batch in [false, true] {
        let repo = repo();
        repo.git(&["checkout", "-b", "topic"]);
        let first = commit(&repo, "first.txt", "first\n", "first");
        let last = commit(&repo, "last.txt", "last\n", "last");
        repo.git(&["checkout", "main"]);
        repo.git(&["cherry-pick", &first, &last]);
        let head = repo.output(&["rev-parse", "HEAD"]);
        let local = dirty(&repo);
        let request = if batch {
            pick(vec![last, first], false)
        } else {
            Mutation::CherryPick { rev: first, revs: Vec::new(), record: false }
        };
        perform(&repo.path, request).unwrap();
        assert_eq!(repo.output(&["rev-parse", "HEAD"]), head);
        assert_settled(&repo, local);
    }
}

fn native_empty_sequence() -> (TempRepo, (String, String)) {
    let repo = repo();
    repo.git(&["checkout", "-b", "topic"]);
    let first = commit(&repo, "first.txt", "first\n", "already applied");
    let last = commit(&repo, "last.txt", "last\n", "remaining");
    repo.git(&["checkout", "main"]);
    repo.git(&["cherry-pick", &first]);
    let local = dirty(&repo);
    repo.git(&["stash", "push", "-m", "saved before native cherry-pick"]);
    let stash = repo.output(&["rev-parse", "refs/stash"]);
    repo.git(&["config", "--file", ".git/awegit", "autostash.pending", stash.trim()]);
    let stopped = repo.raw(&["cherry-pick", &first, &last]);
    assert!(!stopped.status.success());
    assert!(String::from_utf8_lossy(&stopped.stderr).contains("The previous cherry-pick is now empty"));
    assert_eq!(in_progress(&repo.path).unwrap(), Some(InProgress::CherryPick));
    assert!(!repo.path.join("last.txt").exists());
    (repo, local)
}

#[test]
fn cherry_pick_continue_recovers_a_native_empty_stop_and_restores_pending_stash() {
    let (repo, local) = native_empty_sequence();
    perform(&repo.path, Mutation::Continue { message: String::new() }).unwrap();
    assert_eq!(repo.output(&["log", "-1", "--format=%s"]).trim(), "remaining");
    assert!(repo.path.join("last.txt").exists());
    assert_settled(&repo, local);
}

#[test]
fn cherry_pick_continue_skips_a_conflict_resolved_to_the_current_tree() {
    let (repo, _, local) = paused_sequence();
    repo.git(&["checkout", "--ours", "--", "shared.txt"]);
    repo.git(&["add", "shared.txt"]);
    perform(&repo.path, Mutation::Continue { message: String::new() }).unwrap();
    assert!(repo.path.join("last.txt").exists());
    assert_eq!(fs::read_to_string(repo.path.join("shared.txt")).unwrap(), "main\n");
    assert!(!repo.output(&["log", "--format=%s"]).contains("conflicting selected"));
    assert_settled(&repo, local);
}

#[test]
fn cherry_pick_skip_drops_the_next_empty_pick_but_keeps_the_next_real_conflict() {
    let repo = repo();
    repo.git(&["checkout", "-b", "topic"]);
    let first = commit(&repo, "shared.txt", "topic\n", "first conflict");
    let empty = commit(&repo, "already.txt", "already\n", "already applied");
    let next = commit(&repo, "shared.txt", "next\n", "next conflict");
    let last = commit(&repo, "last.txt", "last\n", "remaining");
    repo.git(&["checkout", "main"]);
    repo.git(&["cherry-pick", &empty]);
    commit(&repo, "shared.txt", "main\n", "main change");
    let local = dirty(&repo);
    assert!(perform(&repo.path, pick(vec![first, empty, next.clone(), last], false)).is_err());
    let error = perform(&repo.path, Mutation::Skip).unwrap_err();
    assert!(!error.to_string().contains("The previous cherry-pick is now empty"));
    assert_eq!(repo.output(&["rev-parse", "CHERRY_PICK_HEAD"]).trim(), next);
    assert!(!repo.output(&["ls-files", "-u"]).is_empty());
    assert!(!repo.path.join("last.txt").exists());
    assert!(!repo.output(&["stash", "list"]).is_empty());
    perform(&repo.path, Mutation::Abort).unwrap();
    assert_settled(&repo, local);
}

#[test]
fn cherry_pick_empty_recovery_keeps_unstaged_edits_and_the_saved_work() {
    let (repo, _) = native_empty_sequence();
    fs::write(repo.path.join("shared.txt"), "edited while paused\n").unwrap();
    let head = repo.output(&["rev-parse", "HEAD"]);
    let todo = fs::read(repo.path.join(".git/sequencer/todo")).unwrap();
    assert!(perform(&repo.path, Mutation::Continue { message: String::new() }).is_err());
    assert_eq!(repo.output(&["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(repo.path.join(".git/sequencer/todo")).unwrap(), todo);
    assert_eq!(fs::read_to_string(repo.path.join("shared.txt")).unwrap(), "edited while paused\n");
    assert!(!repo.output(&["stash", "list"]).is_empty());
}

#[test]
fn cherry_pick_commit_hook_failures_keep_the_pick_and_remaining_queue() {
    let (repo, _, _) = paused_sequence();
    fs::write(repo.path.join("shared.txt"), "resolved\n").unwrap();
    repo.git(&["add", "shared.txt"]);
    let current = repo.output(&["rev-parse", "CHERRY_PICK_HEAD"]);
    let hooks = repo.path.join(".git/test-hooks");
    fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("pre-commit");
    fs::write(&hook, "#!/bin/sh\necho 'fixture: commit hook rejected' >&2\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    }
    repo.git(&["config", "core.hooksPath", ".git/test-hooks"]);
    let head = repo.output(&["rev-parse", "HEAD"]);
    let error = perform(&repo.path, Mutation::Continue { message: String::new() }).unwrap_err();
    assert!(error.to_string().contains("fixture: commit hook rejected"));
    assert_eq!(repo.output(&["rev-parse", "HEAD"]), head);
    assert_eq!(repo.output(&["rev-parse", "CHERRY_PICK_HEAD"]), current);
    assert!(!repo.output(&["diff", "--cached"]).is_empty());
    assert!(!repo.path.join("last.txt").exists());
}

#[test]
fn cherry_pick_continue_keeps_a_queue_after_an_external_resolution_commit() {
    let (repo, _, local) = paused_sequence();
    fs::write(repo.path.join("shared.txt"), "resolved externally\n").unwrap();
    repo.git(&["add", "shared.txt"]);
    repo.git(&["commit", "--no-edit"]);
    assert!(!repo.path.join(".git/CHERRY_PICK_HEAD").exists());
    assert_eq!(in_progress(&repo.path).unwrap(), Some(InProgress::CherryPick));
    awegit_git::flow::restore_pending(&repo.path).unwrap();
    assert!(!repo.output(&["stash", "list"]).is_empty());
    assert!(repo.output(&["diff"]).is_empty());
    perform(&repo.path, Mutation::Continue { message: String::new() }).unwrap();
    assert!(repo.path.join("last.txt").exists());
    assert_settled(&repo, local);
}

#[test]
fn unreadable_or_invalid_sequencer_metadata_keeps_pending_local_changes_saved() {
    for contents in [None, Some(""), Some("unsupported-step HEAD\n"), Some("\u{fffd}")] {
        let (repo, _, _) = paused_sequence();
        fs::write(repo.path.join("shared.txt"), "resolved externally\n").unwrap();
        repo.git(&["add", "shared.txt"]);
        repo.git(&["commit", "--no-edit"]);
        let todo = repo.path.join(".git/sequencer/todo");
        match contents {
            Some(text) => fs::write(&todo, text).unwrap(),
            None => { fs::remove_file(&todo).unwrap(); }
        }
        let pending = fs::read(repo.path.join(".git/awegit")).unwrap();
        let stash = repo.output(&["stash", "list"]);
        assert!(in_progress(&repo.path).is_err());
        assert!(awegit_git::flow::restore_pending(&repo.path).is_err());
        assert_eq!(fs::read(repo.path.join(".git/awegit")).unwrap(), pending);
        assert_eq!(repo.output(&["stash", "list"]), stash);
        assert!(repo.output(&["diff"]).is_empty());
    }
}

#[test]
fn revert_continue_keeps_a_queue_after_an_external_resolution_commit() {
    let repo = repo();
    let first = commit(&repo, "first.txt", "first\n", "first");
    let second = commit(&repo, "shared.txt", "second\n", "second");
    commit(&repo, "shared.txt", "current\n", "current");
    assert_ne!(repo.code(&["revert", "--no-edit", &second, &first]), 0);
    repo.git(&["checkout", "--ours", "--", "shared.txt"]);
    fs::write(repo.path.join("shared.txt"), "resolved revert\n").unwrap();
    repo.git(&["add", "shared.txt"]);
    repo.git(&["commit", "--no-edit"]);
    assert!(!repo.path.join(".git/REVERT_HEAD").exists());
    assert_eq!(in_progress(&repo.path).unwrap(), Some(InProgress::Revert));
    perform(&repo.path, Mutation::Continue { message: String::new() }).unwrap();
    assert!(!repo.path.join("first.txt").exists());
    assert_eq!(in_progress(&repo.path).unwrap(), None);
}
