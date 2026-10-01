mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use awegit_git::{
    file_diff, in_progress, perform, repository_refs, stage_hunk, status, InProgress, Mutation, ResetMode,
};

fn repo() -> common::TempRepo {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.git(&["config", "user.name", "Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    repo
}

fn bare() -> common::TempRepo {
    let repo = common::TempRepo::new();
    repo.git(&["init", "--bare", "-b", "main"]);
    repo
}

fn file_url(path: &Path) -> String {
    let text = path.display().to_string().replace('\\', "/");
    if let Some(rest) = text.strip_prefix("//") {
        format!("file://{rest}")
    } else {
        format!("file:///{text}")
    }
}

fn commit_file(repo: &common::TempRepo, name: &str, body: &str, message: &str) {
    fs::write(repo.path.join(name), body).unwrap();
    repo.git(&["add", "--", name]);
    repo.git(&["commit", "-m", message]);
}

fn git_at(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap_or_else(|error| panic!("git is not on PATH: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

struct RemoveWorktree {
    repo: PathBuf,
    path: PathBuf,
}

impl Drop for RemoveWorktree {
    fn drop(&mut self) {
        let _ = Command::new("git")
            .current_dir(&self.repo)
            .args(["worktree", "remove", "--force"])
            .arg(&self.path)
            .env("GIT_TERMINAL_PROMPT", "0")
            .status();
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn fetch_push_and_fast_forward() {
    let origin = bare();
    let local = repo();
    commit_file(&local, "a.txt", "one\n", "first");
    let url = file_url(&origin.path);
    perform(
        &local.path,
        Mutation::AddRemote {
            name: "origin".into(),
            url: url.clone(),
        },
    )
    .unwrap();
    perform(
        &local.path,
        Mutation::Push {
            remote: Some("origin".into()),
            set_upstream: true,
            tags: false,
            force_with_lease: false,
        },
    )
    .unwrap();

    let other = std::env::temp_dir().join(format!(
        "awegit-clone-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    git_at(&std::env::temp_dir(), &["clone", &url, other.to_str().unwrap()]);
    let _cleanup = RemoveDir(other.clone());
    git_at(&other, &["config", "commit.gpgsign", "false"]);
    git_at(&other, &["config", "user.name", "Test"]);
    git_at(&other, &["config", "user.email", "test@example.com"]);

    commit_file(&local, "a.txt", "two\n", "second");
    let ahead = repository_refs(&local.path).unwrap();
    let main = ahead.branches.iter().find(|branch| branch.name == "main").unwrap();
    assert_eq!(main.ahead, 1, "{main:?}");
    assert_eq!(main.behind, 0, "{main:?}");

    perform(
        &local.path,
        Mutation::Push {
            remote: Some("origin".into()),
            set_upstream: false,
            tags: false,
            force_with_lease: false,
        },
    )
    .unwrap();

    perform(&other, Mutation::Pull { rebase: false }).unwrap();
    let log = git_at(&other, &["log", "--format=%s"]);
    assert!(log.contains("second"), "{log}");
    assert!(log.contains("first"), "{log}");
}

#[test]
fn merge_conflict_rebase_abort_stash_and_tag() {
    let repo = repo();
    commit_file(&repo, "a.txt", "one\n", "base");
    fs::write(repo.path.join("a.txt"), "dirty\n").unwrap();
    perform(&repo.path, Mutation::Stash { message: "wip".into() }).unwrap();
    let clean = status(&repo.path).unwrap();
    assert!(clean.staged.is_empty() && clean.unstaged.is_empty(), "{clean:?}");
    let stashed = repository_refs(&repo.path).unwrap();
    assert!(stashed.stashes.iter().any(|stash| stash.summary.contains("wip")), "{:?}", stashed.stashes);
    perform(&repo.path, Mutation::StashPop).unwrap();
    assert_eq!(fs::read_to_string(repo.path.join("a.txt")).unwrap(), "dirty\n");
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();

    perform(
        &repo.path,
        Mutation::Tag {
            name: "v1".into(),
            rev: String::new(),
            message: String::new(),
        },
    )
    .unwrap();
    assert!(repository_refs(&repo.path).unwrap().tags.iter().any(|tag| tag.name == "v1"));

    repo.git(&["checkout", "-b", "topic"]);
    commit_file(&repo, "a.txt", "topic\n", "topic");
    repo.git(&["checkout", "main"]);
    commit_file(&repo, "a.txt", "main\n", "main-edit");
    let conflict = perform(&repo.path, Mutation::Merge { name: "topic".into(), squash: false });
    assert!(conflict.is_err(), "merge should conflict");
    assert_eq!(in_progress(&repo.path).unwrap(), Some(InProgress::Merge));
    perform(&repo.path, Mutation::Abort).unwrap();
    assert_eq!(in_progress(&repo.path).unwrap(), None);

    repo.git(&["checkout", "-b", "side", "HEAD~1"]);
    commit_file(&repo, "a.txt", "side\n", "side");
    let rebase = perform(&repo.path, Mutation::Rebase { onto: "main".into() });
    assert!(rebase.is_err(), "rebase should conflict");
    assert_eq!(in_progress(&repo.path).unwrap(), Some(InProgress::Rebase));
    perform(&repo.path, Mutation::Abort).unwrap();
    assert_eq!(in_progress(&repo.path).unwrap(), None);
}

#[test]
fn resets_cherry_pick_and_revert() {
    let repo = repo();
    commit_file(&repo, "a.txt", "one\n", "first");
    fs::write(repo.path.join("a.txt"), "two\n").unwrap();
    fs::write(repo.path.join("b.txt"), "extra\n").unwrap();
    repo.git(&["add", "a.txt", "b.txt"]);
    repo.git(&["commit", "-m", "second"]);

    perform(
        &repo.path,
        Mutation::Reset {
            rev: "HEAD~1".into(),
            mode: ResetMode::Soft,
        },
    )
    .unwrap();
    assert!(repo.output(&["log", "-1", "--format=%s"]).contains("first"));
    let soft = status(&repo.path).unwrap();
    assert!(soft.unstaged.is_empty(), "{soft:?}");
    assert!(soft.staged.iter().any(|file| file.path == "a.txt"));
    assert!(soft.staged.iter().any(|file| file.path == "b.txt"));

    repo.git(&["commit", "-m", "second-again"]);
    perform(
        &repo.path,
        Mutation::Reset {
            rev: "HEAD~1".into(),
            mode: ResetMode::Mixed,
        },
    )
    .unwrap();
    let mixed = status(&repo.path).unwrap();
    assert!(mixed.staged.is_empty(), "{mixed:?}");
    assert!(mixed.unstaged.iter().any(|file| file.path == "b.txt"), "{mixed:?}");

    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-m", "second-3"]);
    perform(
        &repo.path,
        Mutation::Reset {
            rev: "HEAD~1".into(),
            mode: ResetMode::Hard,
        },
    )
    .unwrap();
    assert_eq!(fs::read_to_string(repo.path.join("a.txt")).unwrap(), "one\n");
    assert!(!repo.path.join("b.txt").exists());
    let hard = status(&repo.path).unwrap();
    assert!(hard.staged.is_empty() && hard.unstaged.is_empty(), "{hard:?}");

    repo.git(&["checkout", "-b", "side"]);
    commit_file(&repo, "c.txt", "c\n", "from-side");
    repo.git(&["checkout", "main"]);
    perform(&repo.path, Mutation::CherryPick { rev: "side".into() }).unwrap();
    assert!(repo.output(&["log", "--format=%s"]).contains("from-side"));
    assert!(repo.path.join("c.txt").exists());
    perform(&repo.path, Mutation::Revert { rev: "HEAD".into() }).unwrap();
    assert!(!repo.path.join("c.txt").exists());
    assert!(repo.output(&["log", "-1", "--format=%s"]).contains("Revert"));
}

#[test]
fn hunk_stage_keeps_only_that_hunk() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "1\n2\n3\n4\n5\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("a.txt"), "1\n2-changed\n3\n4\n5-changed\n").unwrap();

    stage_hunk(&repo.path, "a.txt", 0, false).unwrap();
    let cached = repo.output(&["diff", "--cached"]);
    assert!(cached.contains("2-changed"), "{cached}");
    assert!(!cached.contains("5-changed"), "{cached}");
    let unstaged = file_diff(&repo.path, "a.txt", false).unwrap();
    assert!(unstaged.lines.iter().any(|line| line.text.contains("5-changed")), "{unstaged:?}");
}

#[test]
fn interactive_rebase_drops_a_commit() {
    let repo = repo();
    commit_file(&repo, "base.txt", "base\n", "base");
    let onto = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    commit_file(&repo, "drop.txt", "gone\n", "drop-me");
    let drop = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    commit_file(&repo, "keep.txt", "stay\n", "keep-me");

    perform(
        &repo.path,
        Mutation::RebaseInteractive {
            onto: onto.clone(),
            drop: vec![drop],
        },
    )
    .unwrap();
    let log = repo.output(&["log", "--format=%s"]);
    assert!(log.contains("keep-me"), "{log}");
    assert!(log.contains("base"), "{log}");
    assert!(!log.contains("drop-me"), "{log}");
    assert!(!repo.path.join("drop.txt").exists());
    assert!(repo.path.join("keep.txt").exists());
}

#[test]
fn squash_rejects_a_merge() {
    let repo = repo();
    commit_file(&repo, "a.txt", "a\n", "base");
    let base = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    repo.git(&["checkout", "-b", "topic"]);
    commit_file(&repo, "b.txt", "b\n", "topic");
    repo.git(&["checkout", "main"]);
    repo.git(&["merge", "--no-ff", "topic", "-m", "merge topic"]);

    let error = perform(
        &repo.path,
        Mutation::Squash {
            from: base,
            to: "HEAD".into(),
            summary: "squashed".into(),
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("merge"), "{error}");
    assert!(repo.output(&["log", "-1", "--format=%s"]).contains("merge topic"));
}

#[test]
fn submodule_and_worktree() {
    let lib = repo();
    commit_file(&lib, "lib.txt", "lib\n", "lib");
    let app = repo();
    commit_file(&app, "app.txt", "app\n", "app");
    let url = file_url(&lib.path);
    app.git(&["-c", "protocol.file.allow=always", "submodule", "add", &url, "vendor"]);
    perform(&app.path, Mutation::SubmoduleUpdate).unwrap();
    let refs = repository_refs(&app.path).unwrap();
    assert!(refs.submodules.iter().any(|row| row.path == "vendor" && row.ready), "{:?}", refs.submodules);

    let wt = app.path.with_file_name(format!(
        "{}-wt",
        app.path.file_name().unwrap().to_string_lossy()
    ));
    let _guard = RemoveWorktree { repo: app.path.clone(), path: wt.clone() };
    perform(
        &app.path,
        Mutation::AddWorktree {
            path: wt.display().to_string(),
            branch: "extra".into(),
        },
    )
    .unwrap();
    let refs = repository_refs(&app.path).unwrap();
    assert!(refs.worktrees.len() >= 2, "{:?}", refs.worktrees);
    assert!(refs.worktrees.iter().any(|row| row.branch.as_deref() == Some("extra")), "{:?}", refs.worktrees);
}

#[test]
fn git_flow_start_and_finish() {
    let repo = repo();
    commit_file(&repo, "a.txt", "a\n", "base");
    perform(&repo.path, Mutation::GitFlowStart { name: "demo".into() }).unwrap();
    assert!(repo.output(&["rev-parse", "--abbrev-ref", "HEAD"]).contains("feature/demo"));
    commit_file(&repo, "b.txt", "b\n", "feature work");
    perform(&repo.path, Mutation::GitFlowFinish { name: "demo".into() }).unwrap();
    assert!(repo.output(&["rev-parse", "--abbrev-ref", "HEAD"]).contains("develop"));
    let log = repo.output(&["log", "--format=%s"]);
    assert!(log.contains("feature work"), "{log}");
    assert!(!repo.output(&["branch"]).contains("feature/demo"));
}

#[test]
fn rejects_an_option_shaped_revision() {
    let repo = repo();
    let error = perform(&repo.path, Mutation::Checkout { name: "--force".into() }).unwrap_err();
    assert!(error.to_string().contains("--force"), "{error}");
}

struct RemoveDir(PathBuf);

impl Drop for RemoveDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
