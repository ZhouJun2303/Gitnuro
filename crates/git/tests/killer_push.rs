mod common;

use std::fs;

use awegit_git::{in_progress, perform, status, Mutation};
use common::TempRepo;

fn repo() -> TempRepo {
    let repo = TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    for (key, value) in [
        ("core.autocrlf", "false"),
        ("core.hooksPath", ".git/no-hooks"),
        ("commit.gpgsign", "false"),
        ("user.name", "Test"),
        ("user.email", "test@example.com"),
    ] {
        repo.git(&["config", key, value]);
    }
    repo
}

fn bare() -> TempRepo {
    let repo = TempRepo::new();
    repo.git(&["init", "--bare", "-b", "main"]);
    repo
}

fn file_url(path: &std::path::Path) -> String {
    let text = path.display().to_string().replace('\\', "/");
    if let Some(rest) = text.strip_prefix("//") {
        format!("file://{rest}")
    } else {
        format!("file:///{text}")
    }
}

fn save(repo: &TempRepo, file: &str, body: &str, message: &str) {
    fs::write(repo.path.join(file), body).unwrap();
    repo.git(&["add", "--", file]);
    repo.git(&["commit", "-m", message]);
}

fn add_origin(local: &TempRepo, origin: &TempRepo) {
    perform(
        &local.path,
        Mutation::AddRemote {
            name: "origin".into(),
            url: file_url(&origin.path),
        },
    )
    .unwrap();
}

fn push(local: &TempRepo, killer: bool, rebase: bool, force_with_lease: bool) -> Result<(), awegit_git::Error> {
    perform(
        &local.path,
        Mutation::Push {
            remote: Some("origin".into()),
            set_upstream: true,
            tags: false,
            force_with_lease,
            force: false,
            killer_push: killer,
            rebase,
        },
    )
    .map(|_| ())
}

/// A second checkout that pushes new commits into the same bare remote.
fn other_clone(origin: &TempRepo) -> TempRepo {
    let other = TempRepo::new();
    other.git(&["clone", &file_url(&origin.path), "."]);
    for (key, value) in [
        ("core.hooksPath", ".git/no-hooks"),
        ("commit.gpgsign", "false"),
        ("user.name", "Test"),
        ("user.email", "test@example.com"),
    ] {
        other.git(&["config", key, value]);
    }
    other
}

/// A bare remote, a local clone tracking its main, and another clone that has
/// pushed one new commit. The local branch has one commit of its own.
fn diverged(conflict: bool) -> (TempRepo, TempRepo, TempRepo) {
    let origin = bare();
    let local = repo();
    save(&local, "base.txt", "base\n", "base");
    add_origin(&local, &origin);
    push(&local, false, false, false).unwrap();

    let other = other_clone(&origin);
    if conflict {
        save(&other, "shared.txt", "remote\n", "remote change");
        save(&local, "shared.txt", "local\n", "local change");
    } else {
        save(&other, "remote.txt", "remote\n", "remote change");
        save(&local, "local.txt", "local\n", "local change");
    }
    other.git(&["push", "origin", "HEAD:main"]);
    (origin, local, other)
}

#[test]
fn killer_push_merges_remote_commits_then_pushes_once() {
    let (origin, local, _other) = diverged(false);

    push(&local, true, false, false).unwrap();

    let log = local.output(&["log", "--format=%s"]);
    for subject in ["local change", "remote change", "base"] {
        assert!(log.contains(subject), "local history is missing {subject:?}: {log}");
    }
    assert_ne!(local.code(&["rev-parse", "--verify", "--quiet", "MERGE_HEAD"]), 0);
    let remote_log = origin.output(&["log", "--format=%s", "main"]);
    for subject in ["local change", "remote change"] {
        assert!(remote_log.contains(subject), "remote is missing {subject:?}: {remote_log}");
    }
    assert!(in_progress(&local.path).unwrap().is_none());
    assert!(status(&local.path).unwrap().staged.is_empty());
    assert!(status(&local.path).unwrap().unstaged.is_empty());
}

#[test]
fn killer_push_aborts_a_conflicting_merge_and_keeps_local_history() {
    let (origin, local, _other) = diverged(true);
    let head = local.output(&["rev-parse", "HEAD"]);
    let remote_tip = origin.output(&["rev-parse", "refs/heads/main"]);
    let before = fs::read_to_string(local.path.join("shared.txt")).unwrap();

    let error = push(&local, true, false, false).unwrap_err().to_string();
    assert!(error.contains("killer push"), "{error}");
    assert!(error.contains("aborted") || error.contains("已中止"), "{error}");

    assert_eq!(local.output(&["rev-parse", "HEAD"]), head);
    assert_eq!(origin.output(&["rev-parse", "refs/heads/main"]), remote_tip);
    assert!(in_progress(&local.path).unwrap().is_none());
    let after = status(&local.path).unwrap();
    assert!(after.staged.is_empty(), "{after:?}");
    assert!(after.unstaged.is_empty(), "{after:?}");
    assert_eq!(fs::read_to_string(local.path.join("shared.txt")).unwrap(), before);
    assert_ne!(local.code(&["rev-parse", "--verify", "--quiet", "MERGE_HEAD"]), 0);
}

#[test]
fn killer_push_aborts_a_conflicting_rebase_and_keeps_local_history() {
    let (_origin, local, _other) = diverged(true);
    let head = local.output(&["rev-parse", "HEAD"]);

    let error = push(&local, true, true, false).unwrap_err().to_string();
    assert!(error.contains("killer push"), "{error}");
    assert_eq!(local.output(&["rev-parse", "HEAD"]), head);
    assert!(in_progress(&local.path).unwrap().is_none());
    let after = status(&local.path).unwrap();
    assert!(after.staged.is_empty(), "{after:?}");
    assert!(after.unstaged.is_empty(), "{after:?}");
}

#[test]
fn killer_push_restores_uncommitted_changes_after_a_conflicting_pull() {
    let (_origin, local, _other) = diverged(true);
    fs::write(local.path.join("base.txt"), "dirty\n").unwrap();
    let head = local.output(&["rev-parse", "HEAD"]);

    let error = push(&local, true, false, false).unwrap_err().to_string();
    assert!(error.contains("killer push"), "{error}");
    assert_eq!(local.output(&["rev-parse", "HEAD"]), head);
    assert!(local.output(&["stash", "list"]).is_empty(), "autostash should be restored");
    assert_eq!(fs::read_to_string(local.path.join("base.txt")).unwrap(), "dirty\n");
    let after = status(&local.path).unwrap();
    assert!(after.staged.is_empty(), "{after:?}");
    assert!(after.unstaged.iter().any(|change| change.path == "base.txt"), "{after:?}");
}

#[test]
fn killer_push_off_keeps_the_plain_rejection() {
    let (origin, local, _other) = diverged(false);
    let head = local.output(&["rev-parse", "HEAD"]);
    let remote_tip = origin.output(&["rev-parse", "refs/heads/main"]);

    let error = push(&local, false, false, false).unwrap_err().to_string();
    assert!(error.contains("rejected") || error.contains("non-fast-forward") || error.contains("fetch first"), "{error}");

    assert_eq!(local.output(&["rev-parse", "HEAD"]), head);
    assert_eq!(origin.output(&["rev-parse", "refs/heads/main"]), remote_tip);
    assert!(in_progress(&local.path).unwrap().is_none());
}

#[test]
fn killer_push_recovers_a_stale_force_with_lease() {
    let (origin, local, _other) = diverged(false);

    push(&local, true, false, true).unwrap();

    let remote_log = origin.output(&["log", "--format=%s", "main"]);
    for subject in ["local change", "remote change"] {
        assert!(remote_log.contains(subject), "remote is missing {subject:?}: {remote_log}");
    }
    assert!(in_progress(&local.path).unwrap().is_none());
}

#[test]
fn push_ref_killer_recovers_the_current_branch() {
    let (origin, local, _other) = diverged(false);

    perform(
        &local.path,
        Mutation::PushRef {
            remote: "origin".into(),
            branch: "main".into(),
            source_commit: None,
            remote_branch: "main".into(),
            force_with_lease: false,
            force: false,
            set_upstream: true,
            tags: false,
            killer_push: true,
            rebase: false,
        },
    )
    .unwrap();

    let remote_log = origin.output(&["log", "--format=%s", "main"]);
    assert!(remote_log.contains("local change"), "{remote_log}");
    assert!(remote_log.contains("remote change"), "{remote_log}");
}

#[test]
fn push_ref_killer_replaces_a_commit_push_with_the_integrated_branch() {
    let (origin, local, _other) = diverged(false);
    let head = local.output(&["rev-parse", "HEAD"]).trim().to_string();

    perform(
        &local.path,
        Mutation::PushRef {
            remote: "origin".into(),
            branch: "main".into(),
            source_commit: Some(head),
            remote_branch: "main".into(),
            force_with_lease: false,
            force: false,
            set_upstream: false,
            tags: false,
            killer_push: true,
            rebase: false,
        },
    )
    .unwrap();

    let remote_log = origin.output(&["log", "--format=%s", "main"]);
    assert!(remote_log.contains("local change"), "{remote_log}");
    assert!(remote_log.contains("remote change"), "{remote_log}");
}
