mod common;

use std::fs;

use awegit_git::worktree::{self, AddMode};
use awegit_git::{
    commit_log, commit_log_page, perform, repository_refs, status, status_fingerprint, Mutation,
};

fn repository() -> common::TempRepo {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.hooksPath", ".git/no-hooks"]);
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

#[test]
fn worktree_modes_history_and_shared_registry() {
    let repo = repository();
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
            expected: None
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
            expected: Some(details.stamp)
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
    let bare = common::TempRepo::new();
    source.git(&["clone", "--bare", ".", bare.path.to_str().unwrap()]);
    assert!(status(&bare.path).unwrap().unstaged.is_empty());
    assert!(repository_refs(&bare.path).unwrap().worktrees[0].bare);
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
    assert_eq!(status(&checkout).unwrap().branch.as_deref(), Some("main"));
}
