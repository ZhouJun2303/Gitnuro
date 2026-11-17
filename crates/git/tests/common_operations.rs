mod common;

use std::fs;

use awegit_git::{perform, stage_all, unstage_all, unstage_paths, Mutation};
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

fn save(repo: &TempRepo, file: &str, body: &str) {
    fs::write(repo.path.join(file), body).unwrap();
    repo.git(&["add", "--", file]);
    repo.git(&["commit", "-m", "test: save file"]);
}

#[test]
fn unstage_before_first_commit_preserves_modified_files_and_other_selections() {
    let repo = repo();
    fs::create_dir(repo.path.join("新目录")).unwrap();
    fs::write(repo.path.join("新目录/a[1].txt"), "staged\n").unwrap();
    fs::write(repo.path.join("keep.txt"), "keep\n").unwrap();
    repo.git(&["add", "."]);
    fs::write(repo.path.join("新目录/a[1].txt"), "working\n").unwrap();

    unstage_paths(&repo.path, &["新目录".into()]).unwrap();

    assert_eq!(repo.output(&["ls-files", "-z"]), "keep.txt\0");
    assert_eq!(
        fs::read_to_string(repo.path.join("新目录/a[1].txt")).unwrap(),
        "working\n"
    );
    assert_eq!(repo.code(&["rev-parse", "--verify", "HEAD"]), 128);
}

#[test]
fn unstage_all_before_first_commit_preserves_modified_and_deleted_files() {
    let repo = repo();
    fs::write(repo.path.join("new.txt"), "staged\n").unwrap();
    fs::write(repo.path.join("deleted.txt"), "staged\n").unwrap();
    repo.git(&["add", "."]);
    fs::write(repo.path.join("new.txt"), "working\n").unwrap();
    fs::remove_file(repo.path.join("deleted.txt")).unwrap();

    unstage_all(&repo.path).unwrap();
    unstage_all(&repo.path).unwrap();

    assert_eq!(repo.output(&["ls-files", "-z"]), "");
    assert_eq!(
        fs::read_to_string(repo.path.join("new.txt")).unwrap(),
        "working\n"
    );
    assert!(!repo.path.join("deleted.txt").exists());
}

#[test]
fn unstage_empty_index_is_repeatable_with_and_without_a_commit() {
    let repo = repo();
    unstage_all(&repo.path).unwrap();
    repo.git(&["commit", "--allow-empty", "-m", "test: empty repository"]);
    let head = repo.output(&["rev-parse", "HEAD"]);
    unstage_all(&repo.path).unwrap();
    unstage_paths(&repo.path, &["already-unstaged.txt".into()]).unwrap();
    assert_eq!(repo.output(&["rev-parse", "HEAD"]), head);
}

#[test]
fn unstage_large_selection_is_literal_and_keeps_head_and_worktree() {
    let repo = repo();
    save(&repo, "keep.txt", "base\n");
    let head = repo.output(&["rev-parse", "HEAD"]);
    let paths: Vec<String> = (0..64).map(|i| format!("new [{i}].txt")).collect();
    for path in &paths {
        fs::write(repo.path.join(path), "staged\n").unwrap();
    }
    fs::write(repo.path.join("keep.txt"), "staged\n").unwrap();
    repo.git(&["add", "."]);
    for path in &paths {
        fs::write(repo.path.join(path), "working\n").unwrap();
    }
    unstage_paths(&repo.path, &paths).unwrap();
    assert_eq!(
        repo.output(&["diff", "--cached", "--name-only"]),
        "keep.txt\n"
    );
    assert_eq!(repo.output(&["rev-parse", "HEAD"]), head);
    for path in &paths {
        assert_eq!(
            fs::read_to_string(repo.path.join(path)).unwrap(),
            "working\n"
        );
    }
}

#[test]
fn directory_stage_compacts_only_complete_selections_and_keeps_hidden_scratch_out() {
    let repo = repo();
    save(&repo, "keep.txt", "base\n");
    fs::create_dir(repo.path.join("new [files]")).unwrap();
    let files: Vec<String> = (0..64).map(|i| format!("new [files]/file-{i}.txt")).collect();
    for file in &files { fs::write(repo.path.join(file), "new\n").unwrap(); }
    let compact = || awegit_git::compact_directory_paths(&repo.path, "new [files]", files.clone()).unwrap();
    assert_eq!(compact(), vec!["new [files]"]);
    assert_eq!(awegit_git::compact_directory_paths(&repo.path, "new [files]", files[..63].to_vec()).unwrap(), files[..63]);
    fs::create_dir(repo.path.join("new [files]/.tmp_artifact2")).unwrap();
    fs::write(repo.path.join("new [files]/.tmp_artifact2/hidden.txt"), "scratch\n").unwrap();
    assert_eq!(compact(), files);
    awegit_git::stage_paths(&repo.path, &compact()).unwrap();
    assert_eq!(repo.output(&["diff", "--cached", "--name-only"]).lines().count(), 64);
    assert!(!repo.output(&["ls-files"]).contains("hidden.txt"));
    assert!(awegit_git::compact_directory_paths(&repo.path, "new [files]", Vec::new()).unwrap().is_empty());
}

#[test]
fn stage_all_during_conflict_keeps_staged_renames_and_deletions() {
    let repo = repo();
    save(&repo, "conflict.txt", "base\n");
    save(&repo, "old.txt", "old\n");
    save(&repo, "delete.txt", "delete\n");
    repo.git(&["checkout", "-b", "topic"]);
    save(&repo, "conflict.txt", "topic\n");
    repo.git(&["checkout", "main"]);
    save(&repo, "conflict.txt", "main\n");
    assert_eq!(repo.code(&["merge", "topic"]), 1);
    let conflicts = repo.output(&["ls-files", "-u", "-z"]);
    repo.git(&["mv", "old.txt", "renamed.txt"]);
    repo.git(&["rm", "delete.txt"]);
    fs::write(repo.path.join("renamed.txt"), "edited after rename\n").unwrap();
    fs::write(repo.path.join("loose.txt"), "new\n").unwrap();

    stage_all(&repo.path).unwrap();
    stage_all(&repo.path).unwrap();

    assert_eq!(repo.output(&["ls-files", "-u", "-z"]), conflicts);
    assert_eq!(
        repo.output(&["show", ":renamed.txt"]),
        "edited after rename\n"
    );
    assert_eq!(repo.output(&["show", ":loose.txt"]), "new\n");
    assert_eq!(
        repo.code(&["ls-files", "--error-unmatch", "old.txt", "delete.txt"]),
        1
    );
}

#[test]
fn checkout_does_not_report_success_for_a_file_named_like_a_missing_branch() {
    let repo = repo();
    save(&repo, "missing-branch", "base\n");
    fs::write(repo.path.join("missing-branch"), "local\n").unwrap();
    let before = repo.output(&["status", "--porcelain=v1"]);
    let error = perform(
        &repo.path,
        Mutation::Checkout {
            name: "missing-branch".into(),
        },
    );
    assert!(
        error.is_err(),
        "a file checkout must not count as switching branches"
    );
    assert_eq!(repo.output(&["branch", "--show-current"]), "main\n");
    assert_eq!(repo.output(&["status", "--porcelain=v1"]), before);
    assert_eq!(
        fs::read_to_string(repo.path.join("missing-branch")).unwrap(),
        "local\n"
    );
    assert_eq!(repo.output(&["stash", "list"]), "");
}

#[test]
fn push_ref_disambiguates_branch_and_tag_names_on_both_sides() {
    let origin = TempRepo::new();
    origin.git(&["init", "--bare", "-b", "main"]);
    let repo = repo();
    save(&repo, "note.txt", "base\n");
    repo.git(&["tag", "main"]);
    repo.git(&["tag", "published"]);
    repo.git(&["remote", "add", "origin", &origin.path.to_string_lossy()]);
    repo.git(&["push", "origin", "refs/tags/published"]);
    let tag = origin.output(&["rev-parse", "refs/tags/published"]);
    save(&repo, "note.txt", "next\n");

    perform(
        &repo.path,
        Mutation::PushRef {
            remote: "origin".into(),
            branch: "main".into(),
            remote_branch: "published".into(),
            set_upstream: true,
            tags: false,
            force: false,
            force_with_lease: false,
        },
    )
    .unwrap();

    assert_eq!(
        origin.output(&["rev-parse", "refs/heads/published"]),
        repo.output(&["rev-parse", "HEAD"])
    );
    assert_eq!(origin.output(&["rev-parse", "refs/tags/published"]), tag);
    assert_eq!(
        repo.output(&["config", "branch.main.merge"]),
        "refs/heads/published\n"
    );
}

#[test]
fn checkout_remote_tracks_the_selected_remote_despite_a_same_named_tag() {
    let repo = repo();
    save(&repo, "note.txt", "base\n");
    repo.git(&["remote", "add", "origin", "."]);
    repo.git(&["branch", "topic"]);
    repo.git(&["fetch", "origin"]);
    repo.git(&["tag", "origin/topic"]);
    repo.git(&["branch", "-D", "topic"]);
    fs::write(repo.path.join("note.txt"), "local\n").unwrap();

    perform(
        &repo.path,
        Mutation::CheckoutRemote {
            remote: "origin".into(),
            branch: "topic".into(),
        },
    )
    .unwrap();

    assert_eq!(repo.output(&["branch", "--show-current"]), "topic\n");
    assert_eq!(repo.output(&["config", "branch.topic.remote"]), "origin\n");
    assert_eq!(
        repo.output(&["config", "branch.topic.merge"]),
        "refs/heads/topic\n"
    );
    assert_eq!(
        fs::read_to_string(repo.path.join("note.txt")).unwrap(),
        "local\n"
    );
    assert_eq!(repo.output(&["stash", "list"]), "");
}

#[test]
fn checkout_still_guesses_remote_branches_and_supports_detached_revisions() {
    let repo = repo();
    save(&repo, "note.txt", "base\n");
    repo.git(&["remote", "add", "origin", "."]);
    repo.git(&["branch", "topic"]);
    repo.git(&["fetch", "origin"]);
    repo.git(&["branch", "-D", "topic"]);
    perform(
        &repo.path,
        Mutation::Checkout {
            name: "topic".into(),
        },
    )
    .unwrap();
    assert_eq!(repo.output(&["branch", "--show-current"]), "topic\n");
    let id = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    perform(&repo.path, Mutation::Checkout { name: id.clone() }).unwrap();
    assert_eq!(repo.output(&["branch", "--show-current"]), "");
    assert_eq!(repo.output(&["rev-parse", "HEAD"]).trim(), id);
}
