mod common;

use awegit_git::{flow, perform, Mutation};
use std::{
    fs,
    sync::{Arc, Mutex},
};

fn repo() -> common::TempRepo {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    for (key, value) in [
        ("user.name", "Flow Author"),
        ("user.email", "flow@example.com"),
        ("commit.gpgsign", "false"),
        ("core.autocrlf", "false"),
        ("core.hooksPath", ".git/no-hooks"),
    ] {
        repo.git(&["config", key, value]);
    }
    repo
}

fn save(repo: &common::TempRepo, file: &str, text: &str, message: &str) {
    fs::write(repo.path.join(file), text).unwrap();
    repo.git(&["add", "--", file]);
    repo.git(&["commit", "-m", message]);
}

#[test]
fn preview_replays_commits_even_when_final_tree_hides_a_conflict() {
    let repo = repo();
    save(&repo, "note", "base\n", "base");
    repo.git(&["branch", "upstream"]);
    save(&repo, "note", "local\n", "first");
    let first = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    save(&repo, "note", "base\n", "undo first");
    repo.git(&["checkout", "upstream"]);
    save(&repo, "note", "remote\n", "remote");
    repo.git(&["checkout", "main"]);
    fs::write(repo.path.join("untracked"), "keep me").unwrap();
    let head = repo.output(&["rev-parse", "HEAD"]);
    let status = repo.output(&["status", "--porcelain=v1"]);
    let preview = flow::preview(&repo.path, "upstream", true, &[]).unwrap();
    assert_eq!(preview.steps.len(), 2);
    assert_eq!(preview.steps[0].commit.id, first);
    assert_eq!(preview.steps[0].state, "conflict");
    assert_eq!(preview.steps[0].files, ["note"]);
    assert_eq!(preview.steps[1].state, "conditional");
    assert_eq!(repo.output(&["rev-parse", "HEAD"]), head);
    assert_eq!(repo.output(&["status", "--porcelain=v1"]), status);
    assert_eq!(repo.output(&["stash", "list"]), "");
    assert!(!repo.path.join(".git/rebase-merge").exists());
    assert_eq!(repo.code(&["rebase", "upstream"]), 1);
    assert_eq!(
        flow::checkpoint(&repo.path).unwrap().replay.unwrap().id,
        first
    );
}

#[test]
fn frozen_preview_keeps_the_original_head_after_the_branch_moves() {
    let repo = repo();
    save(&repo, "note", "base\n", "base");
    repo.git(&["branch", "upstream"]);
    save(&repo, "note", "local\n", "local");
    let head = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    repo.git(&["checkout", "upstream"]);
    save(&repo, "note", "remote\n", "remote");
    let preview = flow::preview_at(&repo.path, "upstream", &head, true, &[]).unwrap();
    assert_eq!(preview.target.unwrap().id, head);
    assert_eq!(preview.steps[0].state, "conflict");
}

#[test]
fn custom_merge_driver_is_never_executed_by_a_preview() {
    let repo = repo();
    save(&repo, "note", "base\n", "base");
    repo.git(&["branch", "other"]);
    repo.git(&["config", "merge.test.driver", "touch should-not-exist"]);
    let preview = flow::preview(&repo.path, "other", true, &[]).unwrap();
    assert_eq!(preview.detail, "custom-merge-driver");
    assert!(!repo.path.join("should-not-exist").exists());
}

#[test]
fn revision_names_are_not_misidentified_as_write_commands() {
    let repo = repo();
    save(&repo, "note", "base\n", "base");
    repo.git(&["branch", "rebase"]);
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = events.clone();
    flow::observe(
        Arc::new(move |event| recorded.lock().unwrap().push(event)),
        || {
            flow::commit(&repo.path, "rebase").unwrap();
            flow::checkpoint(&repo.path).unwrap();
        },
    );
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn stash_pop_failure_keeps_the_exact_stash_and_observes_the_failure() {
    let repo = repo();
    save(&repo, "note", "base\n", "base");
    fs::write(repo.path.join("note"), "saved\n").unwrap();
    repo.git(&["stash", "push", "-m", "saved"]);
    let stash = repo.output(&["rev-parse", "refs/stash"]).trim().to_string();
    save(&repo, "note", "new baseline\n", "advance");
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = events.clone();
    let result = flow::observe(
        Arc::new(move |event| recorded.lock().unwrap().push(event)),
        || {
            perform(
                &repo.path,
                Mutation::StashPop {
                    name: stash.clone(),
                },
            )
        },
    );
    assert!(result.is_err());
    assert_eq!(repo.output(&["rev-parse", "refs/stash"]).trim(), stash);
    assert_eq!(flow::checkpoint(&repo.path).unwrap().files.len(), 1);
    assert!(events
        .lock()
        .unwrap()
        .iter()
        .any(|event| event.kind == "stash-pop" && event.state == "failed"));
}

#[test]
fn autostash_emits_once_and_keeps_restore_conflicts_after_rebase() {
    let repo = repo();
    save(&repo, "note", "base\n", "base");
    repo.git(&["branch", "other"]);
    save(&repo, "clean", "local\n", "clean local commit");
    repo.git(&["checkout", "other"]);
    save(&repo, "note", "remote\n", "remote");
    repo.git(&["checkout", "main"]);
    fs::write(repo.path.join("note"), "uncommitted\n").unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = events.clone();
    flow::observe(
        Arc::new(move |event| recorded.lock().unwrap().push(event)),
        || {
            perform(
                &repo.path,
                Mutation::Rebase {
                    onto: "other".into(),
                    autostash: true,
                },
            )
            .unwrap();
        },
    );
    let events = events.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == "local-saved")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == "local-restore" && event.state == "conflict")
            .count(),
        1
    );
    let point = flow::checkpoint(&repo.path).unwrap();
    assert!(point.progress.is_none());
    assert_eq!(point.files.len(), 1);
    assert!(!repo.output(&["stash", "list"]).is_empty());
    assert!(perform(
        &repo.path,
        Mutation::StashApply {
            name: "stash@{0}".into()
        }
    )
    .is_err());
}

#[test]
fn native_pull_records_its_fetch_and_integration_phases() {
    let upstream = repo();
    save(&upstream, "note", "base\n", "base");
    let local = repo();
    local.git(&["remote", "add", "origin", upstream.path.to_str().unwrap()]);
    local.git(&["fetch", "origin"]);
    local.git(&["reset", "--hard", "origin/main"]);
    local.git(&["branch", "--set-upstream-to", "origin/main", "main"]);
    save(&upstream, "note", "remote\n", "advance");
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = events.clone();
    flow::observe(
        Arc::new(move |event| recorded.lock().unwrap().push(event)),
        || {
            perform(
                &local.path,
                Mutation::Pull {
                    rebase: false,
                    autostash: true,
                },
            )
            .unwrap();
        },
    );
    let events = events.lock().unwrap();
    for verb in ["fetch", "merge"] {
        assert!(
            events
                .iter()
                .any(|event| event.kind == verb && event.state == "running"),
            "missing {verb} start: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|event| event.kind == verb && event.state == "completed"),
            "missing {verb} completion"
        );
    }
}
