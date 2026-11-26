mod common;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use awegit_git::worktree::{self, AddMode};
use awegit_git::{
    commit_log, commit_log_page, perform, repository_refs, status, status_fingerprint, Mutation,
};

fn repository() -> common::TempRepo {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.hooksPath", ".git/no-hooks"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    fs::write(repo.path.join(".gitignore"), "ignored/\n").unwrap();
    repo.git(&["add", ".gitignore"]);
    repo.git(&["commit", "-m", "base"]);
    repo
}

fn add(repo: &common::TempRepo, path: &str, branch: &str, mode: AddMode, start: Option<&str>) {
    perform(
        &repo.path,
        Mutation::AddWorktree {
            path: path.into(),
            branch: branch.into(),
            mode,
            start: start.map(str::to_string),
        },
    )
    .unwrap();
}

fn merge_worktree(repo: &common::TempRepo, path: &str) -> Mutation {
    Mutation::MergeWorktree {
        path: path.into(),
        head: repo.output(&["-C", path, "rev-parse", "HEAD"]).trim().into(),
        into: "main".into(),
        no_ff: false,
        summary: "chore: Merge worktree".into(),
    }
}

#[test]
fn worktree_merge_preserves_local_changes_and_is_repeatable() {
    let repo = repository();
    let path = "ignored/topic";
    add(&repo, path, "topic", AddMode::New, None);
    fs::write(repo.path.join(path).join("topic.txt"), "incoming\n").unwrap();
    repo.git(&["-C", path, "add", "topic.txt"]);
    repo.git(&["-C", path, "commit", "-m", "incoming"]);
    let merge = merge_worktree(&repo, path);
    let incoming = repo.output(&["-C", path, "rev-parse", "HEAD"]);

    fs::write(repo.path.join(".gitignore"), "ignored/\n# staged\n").unwrap();
    repo.git(&["add", ".gitignore"]);
    let local = "ignored/\n# staged\n# unstaged\n";
    fs::write(repo.path.join(".gitignore"), local).unwrap();
    let staged = repo.output(&["show", ":.gitignore"]);

    perform(&repo.path, merge.clone()).unwrap();
    perform(&repo.path, merge).unwrap();
    assert_eq!(repo.output(&["rev-parse", "HEAD"]), incoming);
    assert_eq!(repo.output(&["-C", path, "rev-parse", "HEAD"]), incoming);
    assert_eq!(repo.output(&["-C", path, "status", "--porcelain"]), "");
    assert_eq!(repo.output(&["show", ":.gitignore"]), staged);
    assert_eq!(fs::read_to_string(repo.path.join(".gitignore")).unwrap().replace("\r\n", "\n"), local);
    assert!(repo.output(&["stash", "list"]).is_empty());
}

#[test]
fn worktree_merge_rejects_dirty_missing_and_changed_checkouts() {
    let repo = repository();
    let path = "ignored/task";
    add(&repo, path, "task", AddMode::New, None);
    repo.git(&["-C", path, "commit", "--allow-empty", "-m", "incoming"]);
    let merge = merge_worktree(&repo, path);
    let main_head = repo.output(&["rev-parse", "HEAD"]);
    let local = repo.path.join(path).join("unfinished.txt");
    fs::write(&local, "unfinished\n").unwrap();
    assert!(perform(&repo.path, merge.clone()).unwrap_err().to_string().contains("请先提交源工作树"));
    assert!(local.is_file());
    fs::remove_file(local).unwrap();

    repo.git(&["switch", "-c", "other"]);
    assert!(perform(&repo.path, merge.clone()).unwrap_err().to_string().contains("主工作树的分支已变化"));
    repo.git(&["switch", "main"]);
    repo.git(&["-C", path, "commit", "--allow-empty", "-m", "new tip"]);
    assert!(perform(&repo.path, merge).unwrap_err().to_string().contains("源工作树的 HEAD 已变化"));

    let merge = merge_worktree(&repo, path);
    fs::rename(repo.path.join(path), repo.path.join("ignored/relocated")).unwrap();
    assert!(perform(&repo.path, merge).unwrap_err().to_string().contains("源工作树不可用"));
    assert_eq!(repo.output(&["rev-parse", "HEAD"]), main_head);
    assert!(repo.output(&["status", "--porcelain"]).is_empty());
}

#[test]
fn worktree_merge_conflict_can_abort_and_restore_main_changes() {
    let repo = repository();
    let file = repo.path.join("shared.txt");
    fs::write(&file, "base\n").unwrap();
    repo.git(&["add", "shared.txt"]);
    repo.git(&["commit", "-m", "shared"]);
    let path = "ignored/detached";
    add(&repo, path, "", AddMode::Detached, None);
    fs::write(repo.path.join(path).join("shared.txt"), "source\n").unwrap();
    repo.git(&["-C", path, "commit", "-am", "source"]);
    fs::write(&file, "target\n").unwrap();
    repo.git(&["commit", "-am", "target"]);
    let main_head = repo.output(&["rev-parse", "HEAD"]);
    let incoming = repo.output(&["-C", path, "rev-parse", "HEAD"]);
    let local = repo.path.join("staged.txt");
    fs::write(&local, "local\n").unwrap();
    repo.git(&["add", "staged.txt"]);
    let merge = merge_worktree(&repo, path);
    assert!(perform(&repo.path, merge.clone()).is_err());
    assert_eq!(repo.output(&["rev-parse", "MERGE_HEAD"]), incoming);
    assert!(repo.output(&["ls-files", "-u"]).contains("shared.txt"));
    assert!(perform(&repo.path, merge).unwrap_err().to_string().contains("merge is in progress"));
    perform(&repo.path, Mutation::Abort).unwrap();
    assert_eq!(repo.output(&["rev-parse", "HEAD"]), main_head);
    assert!(repo.output(&["diff", "--cached", "--name-only"]).contains("staged.txt"));
    assert!(local.is_file());
    assert!(repo.output(&["stash", "list"]).is_empty());
    assert_eq!(repo.output(&["-C", path, "rev-parse", "HEAD"]), incoming);
}

#[test]
fn worktree_modes_history_and_shared_registry() {
    let repo = repository();
    assert!(!worktree::is_linked(&repo.path).unwrap());
    let base = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    let before = status_fingerprint(&repo.path).unwrap();
    repo.git(&["branch", "existing"]);
    add(
        &repo,
        "existing with space",
        "existing",
        AddMode::Existing,
        None,
    );
    let existing = repo.path.join("existing with space");
    assert!(worktree::is_linked(&existing).unwrap());
    let cached = repository_refs(&existing).unwrap();
    assert_eq!(cached.worktrees.len(), 2);
    assert_eq!(cached.worktrees[1].branch.as_deref(), Some("existing"));
    assert_ne!(
        status_fingerprint(&repo.path).unwrap().worktree_sum,
        before.worktree_sum
    );

    repo.git(&["remote", "add", "origin", "."]);
    repo.git(&["update-ref", "refs/remotes/origin/topic", &base]);
    add(
        &repo,
        "new",
        "tracking",
        AddMode::New,
        Some("refs/remotes/origin/topic"),
    );
    let refs = repository_refs(&existing).unwrap();
    assert_eq!(refs.worktrees.len(), 3);
    assert!(refs
        .branches
        .iter()
        .any(|row| row.name == "tracking" && row.upstream.as_deref() == Some("origin/topic")));

    let before = status_fingerprint(&repo.path).unwrap();
    repo.git(&[
        "-C",
        "new",
        "commit",
        "--allow-empty",
        "-m",
        "tracking advance",
    ]);
    assert_ne!(
        status_fingerprint(&repo.path).unwrap().worktree_sum,
        before.worktree_sum
    );
    assert_ne!(
        repository_refs(&existing)
            .unwrap()
            .worktrees
            .iter()
            .find(|row| row.branch.as_deref() == Some("tracking"))
            .unwrap()
            .head
            .as_deref(),
        Some(base.as_str())
    );

    add(&repo, "detached", "", AddMode::Detached, Some(&base));
    let before = status_fingerprint(&existing).unwrap();
    repo.git(&[
        "-C",
        "detached",
        "commit",
        "--allow-empty",
        "-m",
        "only on detached worktree",
    ]);
    repo.git(&["commit", "--allow-empty", "-m", "only on main"]);
    let detached = repository_refs(&existing)
        .unwrap()
        .worktrees
        .into_iter()
        .find(|row| row.detached)
        .unwrap();
    let tip = detached.head.unwrap();
    assert_ne!(
        status_fingerprint(&existing).unwrap().worktree_sum,
        before.worktree_sum
    );
    assert!(commit_log(&repo.path, 20)
        .unwrap()
        .iter()
        .any(|row| row.id == tip));
    assert!(commit_log(&existing, 20)
        .unwrap()
        .iter()
        .any(|row| row.id == tip));
    let comparison = worktree::compare(&existing, &tip, "refs/heads/main").unwrap();
    assert_eq!((comparison.ahead, comparison.behind), (1, 1));
    assert_eq!(comparison.merge_base.as_deref(), Some(base.as_str()));
    let scoped = commit_log_page(
        &existing,
        20,
        false,
        &[tip.clone(), format!("^{}", comparison.base)],
        false,
        0,
    )
    .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].id, tip);
    assert!(perform(
        &repo.path,
        Mutation::AddWorktree {
            path: "occupied".into(),
            branch: "existing".into(),
            mode: AddMode::Existing,
            start: None
        }
    )
    .is_err());
}

#[test]
fn worktree_details_preserves_checkout_and_submodule_indexes() {
    let repo = repository();
    let submodule_source = repository();
    let allow_local = "protocol.file.allow=always";
    repo.git(&[
        "-c",
        allow_local,
        "submodule",
        "add",
        submodule_source.path.to_str().unwrap(),
        "vendor",
    ]);
    repo.git(&["commit", "-m", "add submodule"]);
    add(&repo, "checkout", "", AddMode::Detached, None);
    repo.git(&[
        "-C", "checkout", "-c", allow_local, "submodule", "update", "--init",
    ]);
    assert!(!worktree::is_linked(&repo.path.join("checkout/vendor")).unwrap());

    let indexes = ["checkout", "checkout/vendor"].map(|dir| {
        // A changed stat entry normally makes Git refresh a clean index on disk.
        let file = fs::OpenOptions::new()
            .write(true)
            .open(repo.path.join(dir).join(".gitignore"))
            .unwrap();
        let mtime = file.metadata().unwrap().modified().unwrap();
        file.set_modified(mtime - Duration::from_secs(60)).unwrap();
        let path = repo.output(&[
            "-C", dir, "rev-parse", "--path-format=absolute", "--git-path", "index",
        ]);
        let path = PathBuf::from(path.trim());
        let bytes = fs::read(&path).unwrap();
        let mtime = fs::metadata(&path).unwrap().modified().unwrap();
        (path, bytes, mtime)
    });

    let details = worktree::details(&repo.path, "checkout").unwrap();
    assert!(details.has_submodules);
    assert_eq!(details.change_count, 0);
    for (path, bytes, mtime) in indexes {
        assert!(fs::read(&path).unwrap() == bytes, "rewrote {}", path.display());
        assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), mtime);
    }
}

#[test]
fn worktree_remove_requires_current_review_and_preserves_branch() {
    let repo = repository();
    add(&repo, "checkout", "topic", AddMode::New, None);
    let checkout = repo.path.join("checkout");
    fs::write(checkout.join(".gitignore"), "ignored/\nextra\n").unwrap();
    fs::create_dir(checkout.join("ignored")).unwrap();
    fs::write(checkout.join("ignored/settings"), "initial ignored content").unwrap();
    let preview = worktree::details(&repo.path, "checkout").unwrap();
    assert!(preview.changes.iter().any(|change| change.code == "!!"));
    assert!(preview.changes.iter().any(|change| change.code == " M"));
    let remove = |force, expected| {
        perform(
            &repo.path,
            Mutation::RemoveWorktree {
                path: "checkout".into(),
                force,
                expected,
                delete_branch: None,
            },
        )
    };
    assert!(remove(false, Some(preview.stamp.clone())).is_err());
    fs::write(checkout.join("ignored/settings"), "changed after review").unwrap();
    assert!(remove(true, Some(preview.stamp)).is_err());
    let preview = worktree::details(&repo.path, "checkout").unwrap();
    fs::write(checkout.join(".gitignore"), "ignored/\nother entry\n").unwrap();
    assert!(remove(true, Some(preview.stamp)).is_err());
    let preview = worktree::details(&repo.path, "checkout").unwrap();
    remove(true, Some(preview.stamp)).unwrap();
    assert!(!checkout.exists());
    repo.git(&["show-ref", "--verify", "refs/heads/topic"]);
}

#[test]
fn worktree_remove_deletes_only_the_reviewed_branch() {
    let repo = repository();
    add(&repo, "checkout", "topic/worktree", AddMode::New, None);
    repo.git(&["update-ref", "refs/remotes/origin/topic/worktree", "HEAD"]);
    let remove = |branch: &str, expected| {
        perform(
            &repo.path,
            Mutation::RemoveWorktree {
                path: "checkout".into(),
                force: false,
                expected,
                delete_branch: Some(branch.into()),
            },
        )
    };

    let preview = worktree::details(&repo.path, "checkout").unwrap();
    assert_eq!(preview.branch.as_deref(), Some("topic/worktree"));
    assert!(remove("topic/worktree", None).is_err());
    assert!(remove("main", Some(preview.stamp.clone())).is_err());
    repo.git(&["-C", "checkout", "commit", "--allow-empty", "-m", "new branch tip"]);
    assert!(remove("topic/worktree", Some(preview.stamp)).is_err());
    assert!(repo.path.join("checkout").is_dir());
    repo.git(&["show-ref", "--verify", "refs/heads/topic/worktree"]);

    let preview = worktree::details(&repo.path, "checkout").unwrap();
    assert!(remove("topic/worktree", Some(preview.stamp)).unwrap().is_empty());
    assert!(!repo.path.join("checkout").exists());
    assert_eq!(repo.code(&["show-ref", "--verify", "--quiet", "refs/heads/topic/worktree"]), 1);
    repo.git(&["show-ref", "--verify", "refs/remotes/origin/topic/worktree"]);
    repo.git(&["show-ref", "--verify", "refs/heads/main"]);
}

#[test]
fn worktree_remove_reports_a_branch_deletion_failure() {
    let repo = repository();
    add(&repo, "checkout", "topic", AddMode::New, None);
    let preview = worktree::details(&repo.path, "checkout").unwrap();
    fs::write(repo.path.join(".git/refs/heads/topic.lock"), "").unwrap();
    let output = perform(
        &repo.path,
        Mutation::RemoveWorktree {
            path: "checkout".into(),
            force: false,
            expected: Some(preview.stamp),
            delete_branch: Some("topic".into()),
        },
    )
    .unwrap();
    assert!(output.contains("工作树已删除，分支 topic 删除失败"), "{output}");
    assert!(!repo.path.join("checkout").exists());
    repo.git(&["show-ref", "--verify", "refs/heads/topic"]);
}

#[test]
fn worktree_lock_move_repair_and_protected_prune() {
    let repo = repository();
    add(&repo, "side", "side", AddMode::New, None);
    let lock = |locked| Mutation::SetWorktreeLock {
        path: "side".into(),
        locked,
        reason: Some("removable disk\nsecond line".into()),
    };
    perform(&repo.path, lock(true)).unwrap();
    assert_eq!(
        worktree::list(&repo.path).unwrap()[1].locked.as_deref(),
        Some("removable disk\nsecond line")
    );
    assert!(perform(
        &repo.path,
        Mutation::RemoveWorktree {
            path: "side".into(),
            force: false,
            expected: None,
            delete_branch: None,
        }
    )
    .is_err());
    perform(&repo.path, lock(false)).unwrap();
    fs::create_dir(repo.path.join("parent")).unwrap();
    let target = worktree::move_destination(&repo.path, "side", "parent").unwrap();
    perform(
        &repo.path,
        Mutation::MoveWorktree {
            path: "side".into(),
            to: "parent".into(),
        },
    )
    .unwrap();
    assert!(target.join(".git").is_file());
    fs::rename(&target, repo.path.join("relocated")).unwrap();
    assert!(repository_refs(&repo.path).unwrap().worktrees[1].missing);
    perform(
        &repo.path,
        Mutation::RepairWorktree {
            path: Some("relocated".into()),
        },
    )
    .unwrap();
    assert!(!repository_refs(&repo.path).unwrap().worktrees[1].missing);
    assert!(status(repo.path.join("relocated"))
        .unwrap()
        .unstaged
        .is_empty());

    add(&repo, "detached-one", "", AddMode::Detached, None);
    repo.git(&[
        "-C",
        "detached-one",
        "commit",
        "--allow-empty",
        "-m",
        "save before pruning",
    ]);
    let tip = repo
        .output(&["-C", "detached-one", "rev-parse", "HEAD"])
        .trim()
        .to_string();
    let details = worktree::details(&repo.path, "detached-one").unwrap();
    assert!(details.protects_commit);
    assert!(perform(
        &repo.path,
        Mutation::RemoveWorktree {
            path: "detached-one".into(),
            force: true,
            expected: Some(details.stamp),
            delete_branch: None,
        }
    )
    .is_err());
    add(&repo, "detached-two", "", AddMode::Detached, Some(&tip));
    for path in ["detached-one", "detached-two"] {
        fs::remove_dir_all(repo.path.join(path)).unwrap();
    }
    let preview = worktree::prune_preview(&repo.path).unwrap();
    assert_eq!(preview.protected_paths.len(), 2);
    let prune = |expected| perform(&repo.path, Mutation::PruneWorktrees { expected });
    assert!(prune(preview.stamp.clone()).is_err());
    repo.git(&["branch", "saved", &tip]);
    assert!(prune(preview.stamp).is_err());
    let preview = worktree::prune_preview(&repo.path).unwrap();
    assert!(preview.protected_paths.is_empty());
    prune(preview.stamp).unwrap();
    assert_eq!(worktree::list(&repo.path).unwrap().len(), 2);
    repo.git(&["show-ref", "--verify", "refs/heads/saved"]);
}

#[test]
fn worktree_can_be_created_from_bare_repository() {
    let source = repository();
    source.git(&["tag", "main"]);
    let bare = common::TempRepo::new();
    source.git(&["clone", "--bare", ".", bare.path.to_str().unwrap()]);
    assert!(!worktree::is_linked(&bare.path).unwrap());
    assert!(status(&bare.path).unwrap().unstaged.is_empty());
    let refs = repository_refs(&bare.path).unwrap();
    assert!(refs.worktrees[0].bare);
    assert_eq!(refs.branches[0].name, "main");
    assert!(!refs.branches[0].current);
    assert_eq!(commit_log(&bare.path, 10).unwrap().len(), 1);
    let holder = common::TempRepo::new();
    let checkout = holder.path.join("linked");
    add(
        &bare,
        checkout.to_str().unwrap(),
        "main",
        AddMode::Existing,
        None,
    );
    assert!(worktree::is_linked(&checkout).unwrap());
    assert_eq!(status(&checkout).unwrap().branch.as_deref(), Some("main"));
    assert!(repository_refs(&checkout).unwrap().branches[0].current);
}
