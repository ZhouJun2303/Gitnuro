mod common;

use std::fs;

use awegit_git::{perform, Mutation};
use common::TempRepo;

fn repo() -> TempRepo {
    let repo = TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.git(&["config", "core.hooksPath", ".git/no-hooks"]);
    repo.git(&["config", "user.name", "Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    repo
}

fn pair() -> (TempRepo, TempRepo) {
    let upstream = repo();
    for name in ["deleted.txt", "keep.txt", "remote.txt"] {
        fs::write(upstream.path.join(name), "base\n").unwrap();
    }
    upstream.git(&["add", "."]);
    upstream.git(&["commit", "-m", "base"]);
    let local = repo();
    local.git(&["remote", "add", "origin", upstream.path.to_str().unwrap()]);
    local.git(&["fetch", "origin"]);
    local.git(&["reset", "--hard", "origin/main"]);
    local.git(&["branch", "--set-upstream-to", "origin/main", "main"]);
    (upstream, local)
}

fn pull(local: &TempRepo, rebase: bool, explicit: bool) -> Result<String, awegit_git::Error> {
    perform(
        &local.path,
        if explicit {
            Mutation::PullRef {
                remote: "origin".into(),
                branch: "main".into(),
                rebase,
                autostash: true,
            }
        } else {
            Mutation::Pull {
                rebase,
                autostash: true,
            }
        },
    )
}

#[test]
fn pull_preserves_local_deletions_and_the_index_with_fakeignore() {
    for rebase in [false, true] {
        for staged in [false, true] {
            let (upstream, local) = pair();
            fs::write(upstream.path.join("remote.txt"), "remote\n").unwrap();
            upstream.git(&["commit", "-am", "remote change"]);
            fs::remove_file(local.path.join("deleted.txt")).unwrap();
            if staged {
                local.git(&["add", "-u"]);
            }
            fs::write(local.path.join("keep.txt"), "staged\n").unwrap();
            local.git(&["add", "keep.txt"]);
            fs::write(local.path.join("keep.txt"), "unstaged\n").unwrap();
            fs::write(local.path.join(".fakeignore"), "local-only\n").unwrap();
            let before = local.output(&["status", "--porcelain"]);

            pull(&local, rebase, staged).unwrap();

            assert_eq!(local.output(&["status", "--porcelain"]), before);
            assert!(!local.path.join("deleted.txt").exists());
            assert_eq!(
                fs::read_to_string(local.path.join("remote.txt")).unwrap(),
                "remote\n"
            );
            assert_eq!(
                fs::read_to_string(local.path.join("keep.txt")).unwrap(),
                "unstaged\n"
            );
            assert_eq!(local.output(&["show", ":keep.txt"]), "staged\n");
            assert_eq!(
                fs::read_to_string(local.path.join(".fakeignore")).unwrap(),
                "local-only\n"
            );
            assert!(local.output(&["stash", "list"]).is_empty());
        }
    }
}

#[test]
fn pull_accepts_a_file_deleted_both_locally_and_upstream() {
    for rebase in [false, true] {
        for staged in [false, true] {
            let (upstream, local) = pair();
            upstream.git(&["rm", "deleted.txt"]);
            upstream.git(&["commit", "-m", "remote deletion"]);
            fs::remove_file(local.path.join("deleted.txt")).unwrap();
            if staged {
                local.git(&["add", "-u"]);
            }
            fs::write(local.path.join(".fakeignore"), "local-only\n").unwrap();

            pull(&local, rebase, staged).unwrap();

            assert!(!local.path.join("deleted.txt").exists());
            assert!(local.output(&["diff", "HEAD"]).is_empty());
            assert!(local.output(&["stash", "list"]).is_empty());
        }
    }
}

#[test]
fn pull_preserves_a_staged_addition_deleted_from_the_worktree() {
    let (upstream, local) = pair();
    fs::write(upstream.path.join("remote.txt"), "remote\n").unwrap();
    upstream.git(&["commit", "-am", "remote change"]);
    fs::write(local.path.join("new.txt"), "staged new file\n").unwrap();
    local.git(&["add", "new.txt"]);
    fs::remove_file(local.path.join("new.txt")).unwrap();
    fs::write(local.path.join(".fakeignore"), "local-only\n").unwrap();
    let before = local.output(&["status", "--porcelain"]);

    pull(&local, false, false).unwrap();

    assert_eq!(local.output(&["status", "--porcelain"]), before);
    assert!(!local.path.join("new.txt").exists());
    assert_eq!(local.output(&["show", ":new.txt"]), "staged new file\n");
    assert!(local.output(&["stash", "list"]).is_empty());
}

#[test]
fn pull_keeps_a_real_modify_delete_conflict_and_its_backup() {
    let (upstream, local) = pair();
    fs::write(upstream.path.join("deleted.txt"), "remote edit\n").unwrap();
    upstream.git(&["commit", "-am", "remote change"]);
    fs::remove_file(local.path.join("deleted.txt")).unwrap();

    pull(&local, false, false).unwrap();

    assert!(!local.output(&["ls-files", "--unmerged"]).is_empty());
    assert!(!local.output(&["stash", "list"]).is_empty());
    assert_eq!(
        fs::read_to_string(local.path.join("deleted.txt")).unwrap(),
        "remote edit\n"
    );
}

#[test]
fn pull_abort_restores_index_only_deletions_after_a_paused_rebase() {
    let (upstream, local) = pair();
    fs::write(upstream.path.join("remote.txt"), "remote\n").unwrap();
    upstream.git(&["commit", "-am", "remote change"]);
    fs::write(local.path.join("remote.txt"), "local commit\n").unwrap();
    local.git(&["commit", "-am", "local change"]);
    let name = "新文件 [draft].txt";
    fs::write(local.path.join(name), "staged\n").unwrap();
    local.git(&["add", "--", name]);
    fs::remove_file(local.path.join(name)).unwrap();
    fs::write(local.path.join(".fakeignore"), "local-only\n").unwrap();
    let before = local.output(&["status", "--porcelain"]);

    assert!(pull(&local, true, true).is_err());
    assert_eq!(
        awegit_git::in_progress(&local.path).unwrap(),
        Some(awegit_git::InProgress::Rebase)
    );
    assert_eq!(local.output(&["stash", "list"]).lines().count(), 1);
    assert!(fs::read_to_string(local.path.join(".git/awegit"))
        .unwrap()
        .contains("indexOnly"));
    perform(&local.path, Mutation::Abort).unwrap();

    assert_eq!(local.output(&["status", "--porcelain"]), before);
    assert!(!local.path.join(name).exists());
    assert_eq!(local.output(&["show", &format!(":{name}")]), "staged\n");
    assert!(local.output(&["stash", "list"]).is_empty());
    assert!(!fs::read_to_string(local.path.join(".git/awegit"))
        .unwrap()
        .contains("indexOnly"));
}

#[cfg(windows)]
fn lock_file(local: &TempRepo, name: &str) -> fs::File {
    use std::os::windows::fs::OpenOptionsExt;
    // Allow Git to read, but deny writes and deletion like an editor does.
    fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(local.path.join(name))
        .unwrap()
}

#[cfg(windows)]
#[test]
fn pull_retries_transient_windows_file_locks() {
    for during_stash in [false, true] {
        let (upstream, local) = pair();
        fs::write(upstream.path.join("remote.txt"), "remote\n").unwrap();
        upstream.git(&["commit", "-am", "remote change"]);
        if during_stash {
            fs::write(local.path.join("keep.txt"), "local\n").unwrap();
        }
        let file = lock_file(
            &local,
            if during_stash {
                "keep.txt"
            } else {
                "remote.txt"
            },
        );
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            drop(file);
        });

        let result = pull(&local, false, during_stash);
        release.join().unwrap();
        result.unwrap();

        assert_eq!(
            fs::read_to_string(local.path.join("remote.txt")).unwrap(),
            "remote\n"
        );
        assert_eq!(
            fs::read_to_string(local.path.join("keep.txt")).unwrap(),
            if during_stash { "local\n" } else { "base\n" }
        );
        assert!(local.output(&["stash", "list"]).is_empty());
        assert!(awegit_git::in_progress(&local.path).unwrap().is_none());
    }
}

#[cfg(windows)]
#[test]
fn pull_bounds_persistent_windows_locks_and_preserves_local_work() {
    let (upstream, local) = pair();
    fs::write(upstream.path.join("remote.txt"), "remote\n").unwrap();
    upstream.git(&["commit", "-am", "remote change"]);
    fs::write(local.path.join("keep.txt"), "staged\n").unwrap();
    local.git(&["add", "keep.txt"]);
    fs::write(local.path.join("keep.txt"), "unstaged\n").unwrap();
    let before = local.output(&["rev-parse", "HEAD"]);
    let file = lock_file(&local, "remote.txt");
    let started = std::time::Instant::now();

    let error = pull(&local, false, false).unwrap_err().to_string();

    assert!(started.elapsed() < std::time::Duration::from_secs(20));
    assert!(error.contains("File: remote.txt"), "{error}");
    assert!(error.contains("unable to unlink"), "{error}");
    assert_eq!(local.output(&["rev-parse", "HEAD"]), before);
    assert_eq!(
        fs::read_to_string(local.path.join("keep.txt")).unwrap(),
        "unstaged\n"
    );
    assert_eq!(local.output(&["show", ":keep.txt"]), "staged\n");
    assert!(local.output(&["stash", "list"]).is_empty());

    drop(file);
    pull(&local, false, false).unwrap();
    assert_eq!(
        fs::read_to_string(local.path.join("remote.txt")).unwrap(),
        "remote\n"
    );
    assert_eq!(
        fs::read_to_string(local.path.join("keep.txt")).unwrap(),
        "unstaged\n"
    );
    assert_eq!(local.output(&["show", ":keep.txt"]), "staged\n");
}
