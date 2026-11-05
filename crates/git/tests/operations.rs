mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use awegit_git::{
    blob_view, conflict_view, discard_lines, file_diff, file_preview, in_progress, launch_merge_tool, merge_preview, perform, repository_refs, search_commits,
    search_log, stage_hunk, stage_line, stage_lines, stage_paths, status, stop_process_tree, ChangeKind, InProgress, LineEdit, Mutation, RebaseStep, ResetMode,
};

fn repo() -> common::TempRepo {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    // A global `core.hooksPath` (for example a commit-msg format check) must not decide these tests.
    repo.git(&["config", "core.hooksPath", ".git/no-hooks"]);
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
            force: false,
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
            force: false,
        },
    )
    .unwrap();

    perform(&other, Mutation::Pull { rebase: false, autostash: false }).unwrap();
    let log = git_at(&other, &["log", "--format=%s"]);
    assert!(log.contains("second"), "{log}");
    assert!(log.contains("first"), "{log}");
}

#[test]
fn merge_conflict_rebase_abort_stash_and_tag() {
    let repo = repo();
    commit_file(&repo, "a.txt", "one\n", "base");
    fs::write(repo.path.join("a.txt"), "dirty\n").unwrap();
    perform(&repo.path, Mutation::Stash { include_untracked: false, message: "wip".into() }).unwrap();
    let clean = status(&repo.path).unwrap();
    assert!(clean.staged.is_empty() && clean.unstaged.is_empty(), "{clean:?}");
    let stashed = repository_refs(&repo.path).unwrap();
    assert!(stashed.stashes.iter().any(|stash| stash.summary.contains("wip")), "{:?}", stashed.stashes);
    perform(&repo.path, Mutation::StashPop { name: String::new() }).unwrap();
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
    let conflict = perform(
        &repo.path,
        Mutation::Merge {
            name: "topic".into(),
            squash: false,
            no_ff: false,
            autostash: false,
            summary: String::new(),
            description: String::new(),
        },
    );
    assert!(conflict.is_err(), "merge should conflict");
    assert_eq!(in_progress(&repo.path).unwrap(), Some(InProgress::Merge));
    perform(&repo.path, Mutation::Abort).unwrap();
    assert_eq!(in_progress(&repo.path).unwrap(), None);

    repo.git(&["checkout", "-b", "side", "HEAD~1"]);
    commit_file(&repo, "a.txt", "side\n", "side");
    let rebase = perform(&repo.path, Mutation::Rebase { onto: "main".into(), autostash: false });
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
    perform(&repo.path, Mutation::CherryPick { rev: "side".into(), record: false }).unwrap();
    assert!(repo.output(&["log", "--format=%s"]).contains("from-side"));
    assert!(repo.path.join("c.txt").exists());
    perform(&repo.path, Mutation::Revert { rev: "HEAD".into() }).unwrap();
    assert!(!repo.path.join("c.txt").exists());
    assert!(repo.output(&["log", "-1", "--format=%s"]).contains("Revert"));
}

#[test]
fn line_stage_keeps_the_other_line() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "1\n2\n3\n4\n5\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("a.txt"), "1\nTWO\nTHREE\n4\n5\n").unwrap();

    let diff = file_diff(&repo.path, "a.txt", false).unwrap();
    let line = diff.lines.iter().find(|line| line.text == "+TWO").expect("added line");
    stage_line(&repo.path, "a.txt", "TWO", true, line.stage_at.expect("stage index"), false).unwrap();

    let cached = repo.output(&["diff", "--cached"]);
    assert!(cached.contains("TWO"), "{cached}");
    assert!(!cached.contains("THREE"), "{cached}");
    let pending = repo.output(&["diff"]);
    assert!(pending.contains("THREE"), "{pending}");
}

#[test]
fn line_stage_handles_a_comma_in_the_path() {
    let repo = repo();
    fs::write(repo.path.join("a,b.txt"), "1\n2\n").unwrap();
    repo.git(&["add", "-A", "--", "."]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("a,b.txt"), "1\nTWO\n2\n").unwrap();

    let diff = file_diff(&repo.path, "a,b.txt", false).unwrap();
    let line = diff.lines.iter().find(|line| line.text == "+TWO").expect("added line");
    stage_line(&repo.path, "a,b.txt", "TWO", true, line.stage_at.expect("stage index"), false).unwrap();

    let cached = repo.output(&["diff", "--cached"]);
    assert!(cached.contains("TWO"), "{cached}");
}

#[test]
fn preview_reads_a_png() {
    let repo = repo();
    let png = [
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    ];
    fs::write(repo.path.join("dot.png"), png).unwrap();
    let preview = file_preview(&repo.path, "dot.png").unwrap().expect("png");
    assert_eq!(preview.mime, "image/png");
    assert!(preview.data_url.starts_with("data:image/png;base64,"));
    assert!(file_preview(&repo.path, "missing.png").unwrap().is_none());
}

#[test]
fn cancel_stops_a_child_process() {
    let mut child = Command::new("ping")
        .args(["-n", "30", "127.0.0.1"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    stop_process_tree(child.id());
    let status = child.wait().unwrap();
    assert!(!status.success(), "{status}");
}

#[test]
fn hunk_stage_keeps_only_that_hunk() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "1\n2\n3\n4\n5\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("a.txt"), "1\n2-changed\n3\n4\n5-changed\n").unwrap();

    stage_hunk(&repo.path, "a.txt", 0, false, false).unwrap();
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
            steps: Vec::new(),
            autostash: false,
            update_refs: false,
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
fn squash_replays_commits_after_the_range() {
    let repo = repo();
    commit_file(&repo, "a.txt", "a\n", "base");
    commit_file(&repo, "b.txt", "b\n", "second");
    let second = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    commit_file(&repo, "c.txt", "c\n", "third");
    let third = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    commit_file(&repo, "d.txt", "d\n", "fourth");
    fs::write(repo.path.join("dirty.txt"), "nope\n").unwrap();
    let dirty = perform(
        &repo.path,
        Mutation::Squash {
            from: second.clone(),
            to: third.clone(),
            summary: "squashed".into(),
        },
    )
    .unwrap_err();
    assert!(dirty.to_string().contains("clean"), "{dirty}");
    fs::remove_file(repo.path.join("dirty.txt")).unwrap();
    perform(
        &repo.path,
        Mutation::Squash {
            from: second,
            to: third,
            summary: "squashed".into(),
        },
    )
    .unwrap();
    let log = repo.output(&["log", "--format=%s"]);
    assert!(log.contains("base"), "{log}");
    assert!(log.contains("squashed"), "{log}");
    assert!(log.contains("fourth"), "{log}");
    assert!(!log.contains("second"), "{log}");
    assert!(!log.contains("third"), "{log}");
    assert_eq!(fs::read_to_string(repo.path.join("d.txt")).unwrap(), "d\n");
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
    perform(&repo.path, Mutation::GitFlowStart { name: "demo".into(), kind: String::new() }).unwrap();
    assert!(repo.output(&["rev-parse", "--abbrev-ref", "HEAD"]).contains("feature/demo"));
    commit_file(&repo, "b.txt", "b\n", "feature work");
    perform(&repo.path, Mutation::GitFlowFinish { name: "demo".into(), kind: String::new() }).unwrap();
    assert!(repo.output(&["rev-parse", "--abbrev-ref", "HEAD"]).contains("develop"));
    let log = repo.output(&["log", "--format=%s"]);
    assert!(log.contains("feature work"), "{log}");
    assert!(!repo.output(&["branch"]).contains("feature/demo"));
}

#[test]
fn discard_hunk_keeps_the_other_change() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "1\n2\n3\n4\n5\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("a.txt"), "1\n2-changed\n3\n4\n5-changed\n").unwrap();
    perform(
        &repo.path,
        Mutation::DiscardHunk {
            file: "a.txt".into(),
            index: 0,
            ignore_space: false,
        },
    )
    .unwrap();
    let text = fs::read_to_string(repo.path.join("a.txt")).unwrap();
    assert!(text.contains("\n2\n"), "{text}");
    assert!(text.contains("5-changed"), "{text}");
}

#[test]
fn resolve_conflict_keeps_ours() {
    let repo = repo();
    commit_file(&repo, "a.txt", "base\n", "base");
    repo.git(&["checkout", "-b", "side"]);
    commit_file(&repo, "a.txt", "side\n", "side");
    repo.git(&["checkout", "main"]);
    commit_file(&repo, "a.txt", "main\n", "main");
    let merge = perform(
        &repo.path,
        Mutation::Merge {
            name: "side".into(),
            squash: false,
            no_ff: false,
            autostash: false,
            summary: String::new(),
            description: String::new(),
        },
    );
    assert!(merge.is_err(), "expected a conflict");
    perform(
        &repo.path,
        Mutation::Resolve {
            file: "a.txt".into(),
            side: "ours".into(),
            text: String::new(),
        },
    )
    .unwrap();
    let text = fs::read_to_string(repo.path.join("a.txt")).unwrap();
    assert!(text.contains("main"), "{text}");
    assert!(!text.contains("<<<<<<<"), "{text}");
}

#[test]
fn rename_and_reword_during_rebase() {
    let repo = repo();
    commit_file(&repo, "a.txt", "a\n", "base");
    let onto = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    commit_file(&repo, "b.txt", "b\n", "old-subject");
    let rev = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    repo.git(&["branch", "side"]);
    perform(
        &repo.path,
        Mutation::RenameBranch {
            name: "side".into(),
            to: "renamed".into(),
        },
    )
    .unwrap();
    assert!(repo.output(&["branch"]).contains("renamed"));
    perform(
        &repo.path,
        Mutation::RebaseInteractive {
            onto,
            drop: Vec::new(),
            steps: vec![RebaseStep {
                verb: "reword".into(),
                rev,
                message: "new-subject".into(),
            }],
            autostash: false,
            update_refs: false,
        },
    )
    .unwrap();
    let log = repo.output(&["log", "--format=%s"]);
    assert!(log.contains("new-subject"), "{log}");
    assert!(!log.contains("old-subject"), "{log}");
}

#[test]
fn deletes_a_remote_branch() {
    let origin = bare();
    let local = repo();
    commit_file(&local, "a.txt", "one\n", "first");
    perform(
        &local.path,
        Mutation::AddRemote {
            name: "origin".into(),
            url: file_url(&origin.path),
        },
    )
    .unwrap();
    local.git(&["checkout", "-b", "topic"]);
    perform(
        &local.path,
        Mutation::Push {
            remote: Some("origin".into()),
            set_upstream: true,
            tags: false,
            force_with_lease: false,
            force: false,
        },
    )
    .unwrap();
    perform(
        &local.path,
        Mutation::DeleteRemoteBranch {
            remote: "origin".into(),
            branch: "topic".into(),
        },
    )
    .unwrap();
    let remote = local.output(&["ls-remote", "--heads", "origin"]);
    assert!(!remote.contains("topic"), "{remote}");
}

#[test]
fn git_flow_init_writes_local_config() {
    let repo = repo();
    commit_file(&repo, "a.txt", "a\n", "base");
    perform(
        &repo.path,
        Mutation::GitFlowInit {
            master: "main".into(),
            develop: "develop".into(),
            feature: "feature/".into(),
            release: "release/".into(),
            hotfix: "hotfix/".into(),
            support: "support/".into(),
        },
    )
    .unwrap();
    let prefix = repo.output(&["config", "--local", "--get", "gitflow.prefix.feature"]);
    assert!(prefix.contains("feature/"), "{prefix}");
    assert!(repo.output(&["branch"]).contains("develop"));
}

#[test]
fn hidden_refs_stay_in_the_repository_file() {
    let repo = repo();
    commit_file(&repo, "a.txt", "a\n", "base");
    fs::write(
        repo.path.join(".git").join("awegit"),
        "[signoff]\n\tenabled = false\n[awegit]\n\thiddenRef = old\n",
    )
    .unwrap();
    assert_eq!(repository_refs(&repo.path).unwrap().hidden_refs, vec!["old".to_string()]);
    perform(
        &repo.path,
        Mutation::SetHidden {
            names: vec!["feature/a".into(), "wip".into()],
        },
    )
    .unwrap();
    let refs = repository_refs(&repo.path).unwrap();
    assert_eq!(refs.hidden_refs, vec!["feature/a".to_string(), "wip".to_string()]);
    let text = fs::read_to_string(repo.path.join(".git").join("awegit")).unwrap();
    assert!(text.contains("enabled = false"), "{text}");
    assert!(text.contains("hiddenRef = feature/a"), "{text}");
    assert!(!text.contains("hiddenRef = old"), "{text}");
}

#[test]
fn custom_command_expands_placeholders() {
    let repo = repo();
    let output = perform(
        &repo.path,
        Mutation::Custom {
            command: "echo ${sha}".into(),
            repo: String::new(),
            sha: "abc123".into(),
            branch: String::new(),
            file: String::new(),
        },
    )
    .unwrap();
    assert!(output.contains("abc123"), "{output}");
    let error = perform(
        &repo.path,
        Mutation::Custom {
            command: "echo ${repo}".into(),
            repo: "bad\"name".into(),
            sha: String::new(),
            branch: String::new(),
            file: String::new(),
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("refusing"), "{error}");
}

#[test]
fn repository_file_keeps_hidden_refs_signoff_and_groups() {
    let repo = repo();
    commit_file(&repo, "a.txt", "a\n", "base");
    fs::write(
        repo.path.join(".git").join("awegit"),
        "[signoff]\n\tenabled = false\n[awegit]\n\thiddenRef = old\n",
    )
    .unwrap();
    perform(
        &repo.path,
        Mutation::SetHidden { names: vec!["feature/a".into()] },
    )
    .unwrap();
    perform(
        &repo.path,
        Mutation::SetSignOff {
            enabled: true,
            format: "Reviewed-by: %user <%email>".into(),
        },
    )
    .unwrap();
    perform(
        &repo.path,
        Mutation::SetExpanded { names: vec!["feature".into()] },
    )
    .unwrap();
    let refs = repository_refs(&repo.path).unwrap();
    assert_eq!(refs.hidden_refs, vec!["feature/a".to_string()]);
    assert!(refs.sign_off_set);
    assert!(refs.sign_off);
    assert_eq!(refs.sign_off_format, "Reviewed-by: %user <%email>");
    assert_eq!(refs.expanded_groups, vec!["feature".to_string()]);
    let text = fs::read_to_string(repo.path.join(".git").join("awegit")).unwrap();
    assert!(text.contains("hiddenRef = feature/a"), "{text}");
    assert!(!text.contains("hiddenRef = old"), "{text}");
    assert!(text.contains("enabled = true"), "{text}");
    assert!(text.contains("expandedGroup = feature"), "{text}");
}

#[test]
fn conflict_keeps_a_backup_of_local_changes() {
    let repo = repo();
    commit_file(&repo, "a.txt", "one\n", "base");
    commit_file(&repo, "b.txt", "keep\n", "second file");
    fs::write(repo.path.join("a.txt"), "stashed\n").unwrap();
    perform(&repo.path, Mutation::Stash { include_untracked: false, message: "wip".into() }).unwrap();
    commit_file(&repo, "a.txt", "main\n", "main-edit");
    fs::write(repo.path.join("b.txt"), "local work\n").unwrap();

    let popped = perform(&repo.path, Mutation::StashPop { name: String::new() });
    assert!(popped.is_err(), "stash pop should conflict");
    let stashes = repository_refs(&repo.path).unwrap().stashes;
    let backup = stashes
        .iter()
        .find(|stash| stash.summary.contains("conflict backup"))
        .unwrap_or_else(|| panic!("no backup in {stashes:?}"));
    let saved = repo.output(&["show", &format!("{}:b.txt", backup.name)]);
    assert_eq!(saved.replace("\r\n", "\n"), "local work\n");
    assert!(stashes.iter().any(|stash| stash.summary.contains("wip")), "{stashes:?}");
}

#[test]
fn clean_merge_leaves_no_backup() {
    let repo = repo();
    commit_file(&repo, "a.txt", "one\n", "base");
    repo.git(&["checkout", "-b", "topic"]);
    commit_file(&repo, "b.txt", "topic\n", "topic");
    repo.git(&["checkout", "main"]);
    fs::write(repo.path.join("a.txt"), "dirty\n").unwrap();
    perform(
        &repo.path,
        Mutation::Merge { name: "topic".into(), squash: false, no_ff: false, autostash: false, summary: String::new(), description: String::new() },
    )
    .unwrap();
    assert!(repository_refs(&repo.path).unwrap().stashes.is_empty());
}

#[test]
fn named_stash_pop_restores_that_stash() {
    let repo = repo();
    commit_file(&repo, "a.txt", "one\n", "base");
    fs::write(repo.path.join("a.txt"), "first\n").unwrap();
    perform(&repo.path, Mutation::Stash { include_untracked: false, message: "first".into() }).unwrap();
    fs::write(repo.path.join("a.txt"), "second\n").unwrap();
    perform(&repo.path, Mutation::Stash { include_untracked: false, message: "second".into() }).unwrap();
    let stashes = repository_refs(&repo.path).unwrap().stashes;
    let older = stashes.iter().find(|stash| stash.summary.contains("first")).expect("older stash");
    perform(&repo.path, Mutation::StashPop { name: older.name.clone() }).unwrap();
    assert_eq!(fs::read_to_string(repo.path.join("a.txt")).unwrap(), "first\n");
    let left = repository_refs(&repo.path).unwrap().stashes;
    assert!(left.iter().any(|stash| stash.summary.contains("second")), "{left:?}");
    assert!(!left.iter().any(|stash| stash.summary.contains("first")), "{left:?}");
}

#[test]
fn reflog_and_tree_list_the_commit() {
    let repo = repo();
    commit_file(&repo, "note.txt", "hello\n", "note");
    let rows = awegit_git::reflog(&repo.path, 20).unwrap();
    assert!(rows.iter().any(|row| row.summary.contains("note")), "{rows:?}");
    let tree = awegit_git::commit_tree(&repo.path, "HEAD").unwrap();
    assert_eq!(tree, vec!["note.txt".to_string()]);
}

#[test]
fn merge_tree_names_the_conflict() {
    let repo = repo();
    commit_file(&repo, "same.txt", "base\n", "base");
    repo.git(&["checkout", "-b", "other"]);
    fs::write(repo.path.join("same.txt"), "other\n").unwrap();
    repo.git(&["commit", "-am", "other"]);
    repo.git(&["checkout", "main"]);
    fs::write(repo.path.join("same.txt"), "main\n").unwrap();
    repo.git(&["commit", "-am", "main"]);
    let files = awegit_git::rebase_conflicts(&repo.path, "other").unwrap();
    assert!(files.iter().any(|file| file.contains("same.txt")), "{files:?}");
}

#[test]
fn skip_hooks_commits_past_a_failing_hook() {
    let repo = repo();
    commit_file(&repo, "a.txt", "one\n", "base");
    repo.git(&["config", "core.hooksPath", ".git/hooks"]);
    let hook = repo.path.join(".git").join("hooks").join("pre-commit");
    fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&hook, permissions).unwrap();
    }
    fs::write(repo.path.join("a.txt"), "two\n").unwrap();
    repo.git(&["add", "--", "a.txt"]);
    let blocked = awegit_git::commit(
        &repo.path,
        awegit_git::CommitRequest {
            summary: "blocked".into(),
            description: String::new(),
            amend: false,
            sign_off: false,
            sign_off_format: String::new(),
            skip_hooks: false,
            convention: false,
        },
    );
    assert!(blocked.is_err(), "hook should reject the commit");
    awegit_git::commit(
        &repo.path,
        awegit_git::CommitRequest {
            summary: "skipped".into(),
            description: String::new(),
            amend: false,
            sign_off: false,
            sign_off_format: String::new(),
            skip_hooks: true,
            convention: false,
        },
    )
    .unwrap();
    assert!(repo.output(&["log", "-1", "--format=%s"]).contains("skipped"));
}

#[test]
fn stash_records_its_parent() {
    let repo = repo();
    commit_file(&repo, "a.txt", "one\n", "base");
    let parent = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    fs::write(repo.path.join("a.txt"), "dirty\n").unwrap();
    perform(&repo.path, Mutation::Stash { include_untracked: false, message: "keep".into() }).unwrap();
    let stash = repository_refs(&repo.path).unwrap().stashes.into_iter().next().expect("stash");
    assert!(stash.summary.contains("keep"), "{}", stash.summary);
    assert_eq!(stash.parent, parent);
}

#[test]
fn rejects_an_option_shaped_revision() {
    let repo = repo();
    let error = perform(&repo.path, Mutation::Checkout { name: "--force".into() }).unwrap_err();
    assert!(error.to_string().contains("--force"), "{error}");
}

#[test]
fn restore_file_checks_out_one_path() {
    let repo = repo();
    commit_file(&repo, "a.txt", "base\n", "base");
    commit_file(&repo, "a.txt", "next\n", "next");
    perform(
        &repo.path,
        Mutation::RestoreFile { rev: "HEAD~1".into(), file: "a.txt".into() },
    )
    .unwrap();
    let text = fs::read_to_string(repo.path.join("a.txt")).unwrap();
    assert!(text.contains("base"), "{text}");
}

#[test]
fn write_mailmap_replaces_the_root_file() {
    let repo = repo();
    commit_file(&repo, "a.txt", "a\n", "base");
    perform(&repo.path, Mutation::WriteMailmap { text: "Alice <a@ex> <b@ex>\n".into() }).unwrap();
    let text = fs::read_to_string(repo.path.join(".mailmap")).unwrap();
    assert_eq!(text, "Alice <a@ex> <b@ex>\n");
}

#[test]
fn stash_paths_keeps_the_other_file() {
    let repo = repo();
    commit_file(&repo, "a.txt", "a\n", "base");
    commit_file(&repo, "b.txt", "b\n", "base b");
    fs::write(repo.path.join("a.txt"), "changed\n").unwrap();
    fs::write(repo.path.join("b.txt"), "changed b\n").unwrap();
    perform(
        &repo.path,
        Mutation::StashPaths {
            files: vec!["a.txt".into()],
            message: "only a".into(),
            include_untracked: false,
        },
    )
    .unwrap();
    let a = fs::read_to_string(repo.path.join("a.txt")).unwrap();
    let b = fs::read_to_string(repo.path.join("b.txt")).unwrap();
    assert!(a.contains("a\n") || a == "a\n", "{a}");
    assert!(b.contains("changed"), "{b}");
}

#[test]
fn stash_paths_takes_untracked_files_without_the_flag() {
    let repo = repo();
    commit_file(&repo, "a.txt", "a\n", "base");
    fs::write(repo.path.join("new.txt"), "new\n").unwrap();
    fs::write(repo.path.join("a.txt"), "changed\n").unwrap();
    perform(
        &repo.path,
        Mutation::StashPaths {
            files: vec!["new.txt".into(), "a.txt".into()],
            message: "mixed".into(),
            include_untracked: false,
        },
    )
    .unwrap();
    assert!(!repo.path.join("new.txt").exists());
    assert_eq!(fs::read_to_string(repo.path.join("a.txt")).unwrap(), "a\n");
    assert_eq!(repository_refs(&repo.path).unwrap().stashes.len(), 1);
}

#[test]
fn branch_and_tag_names_may_use_any_language() {
    let repo = repo();
    commit_file(&repo, "a.txt", "a\n", "base");
    perform(&repo.path, Mutation::CreateBranch { name: "测试分支".into(), start: None }).unwrap();
    perform(&repo.path, Mutation::Checkout { name: "测试分支".into() }).unwrap();
    perform(&repo.path, Mutation::Tag { name: "v1.0+build5".into(), rev: String::new(), message: "说明".into() }).unwrap();
    perform(&repo.path, Mutation::DeleteTag { name: "v1.0+build5".into() }).unwrap();
    perform(&repo.path, Mutation::Checkout { name: "main".into() }).unwrap();
    perform(&repo.path, Mutation::DeleteBranch { name: "测试分支".into(), force: false }).unwrap();
    assert!(perform(&repo.path, Mutation::CreateBranch { name: "--force".into(), start: None }).is_err());
    assert!(perform(&repo.path, Mutation::CreateBranch { name: "a b".into(), start: None }).is_err());
}

#[test]
fn search_commits_finds_added_text() {
    let repo = repo();
    commit_file(&repo, "a.txt", "needle-token\n", "base");
    let hits = search_commits(&repo.path, "S", "needle-token").unwrap();
    assert!(hits.iter().any(|hit| hit.summary == "base"), "{hits:?}");
}

#[test]
fn search_log_finds_message_and_diff() {
    let repo = repo();
    commit_file(&repo, "a.txt", "one\n", "base");
    commit_file(&repo, "a.txt", "needle-token\n", "feat: add the needle");
    repo.git(&["checkout", "-b", "side"]);
    commit_file(&repo, "b.txt", "side\n", "only-on-side");
    repo.git(&["checkout", "main"]);
    let messages = search_log(&repo.path, "message", "repository", "needle", 20, false).unwrap();
    assert!(messages.iter().any(|row| row.summary.contains("needle")), "{messages:?}");
    let diffs = search_log(&repo.path, "diff", "repository", "needle-token", 20, false).unwrap();
    assert!(diffs.iter().any(|row| row.summary.contains("needle")), "{diffs:?}");
    let here = search_log(&repo.path, "message", "branch", "only-on-side", 20, false).unwrap();
    assert!(here.is_empty(), "{here:?}");
    let everywhere = search_log(&repo.path, "message", "repository", "only-on-side", 20, false).unwrap();
    assert!(everywhere.iter().any(|row| row.summary == "only-on-side"), "{everywhere:?}");
    assert!(search_log(&repo.path, "message", "repository", "-needle", 20, false).is_err());
}

#[test]
fn append_ignore_adds_one_pattern_without_replacing_the_file() {
    let repo = repo();
    fs::write(repo.path.join(".gitignore"), "target/\n").unwrap();
    let add = Mutation::AppendIgnore {
        file: String::new(),
        files: vec!["src/A.kt".into(), "src/B.kt".into(), "src/A.kt".into()],
        fake: false,
        directory: false,
    };
    perform(&repo.path, add.clone()).unwrap();
    perform(&repo.path, add).unwrap();
    let text = fs::read_to_string(repo.path.join(".gitignore")).unwrap();
    assert_eq!(text, "target/\n/src/A.kt\n/src/B.kt\n");

    perform(
        &repo.path,
        Mutation::AppendIgnore {
            file: String::new(),
            files: vec!["notes".into()],
            fake: true,
            directory: true,
        },
    )
    .unwrap();
    let fake = fs::read_to_string(repo.path.join(".fakeignore")).unwrap();
    assert_eq!(fake, "/notes/\n");
    assert!(perform(
        &repo.path,
        Mutation::AppendIgnore {
            file: "../outside".into(),
            files: Vec::new(),
            fake: true,
            directory: false,
        },
    )
    .is_err());
}

#[test]
fn write_gitignore_replaces_the_root_file() {
    let repo = repo();
    commit_file(&repo, "a.txt", "a\n", "base");
    perform(&repo.path, Mutation::WriteGitignore { text: "target/\n".into() }).unwrap();
    let text = fs::read_to_string(repo.path.join(".gitignore")).unwrap();
    assert_eq!(text, "target/\n");
}

#[test]
fn resolve_text_writes_the_editor_result() {
    let repo = repo();
    commit_file(&repo, "a.txt", "base\n", "base");
    repo.git(&["checkout", "-b", "side"]);
    commit_file(&repo, "a.txt", "side\n", "side");
    repo.git(&["checkout", "main"]);
    commit_file(&repo, "a.txt", "main\n", "main");
    let _ = perform(
        &repo.path,
        Mutation::Merge { name: "side".into(), squash: false, no_ff: false, autostash: false, summary: String::new(), description: String::new() },
    );
    perform(
        &repo.path,
        Mutation::Resolve { file: "a.txt".into(), side: "text".into(), text: "merged\n".into() },
    )
    .unwrap();
    let text = fs::read_to_string(repo.path.join("a.txt")).unwrap();
    assert_eq!(text, "merged\n");
}

#[test]
fn stash_conflict_names_the_patch_and_abort_clears_it() {
    let repo = repo();
    commit_file(&repo, "a.txt", "base\n", "base");
    fs::write(repo.path.join("a.txt"), "stashed\n").unwrap();
    perform(
        &repo.path,
        Mutation::Stash { message: "wip".into(), include_untracked: false },
    )
    .unwrap();
    commit_file(&repo, "a.txt", "main\n", "main-edit");
    assert!(perform(&repo.path, Mutation::StashApply { name: "stash@{0}".into() }).is_err());
    assert!(in_progress(&repo.path).unwrap().is_none());
    let snap = status(&repo.path).unwrap();
    assert!(
        snap.unstaged.iter().any(|file| file.path == "a.txt" && file.kind == ChangeKind::Conflict),
        "{snap:?}"
    );
    let view = conflict_view(&repo.path, "a.txt").unwrap();
    assert_eq!(view.ours.label, "main");
    assert_eq!(view.ours.state, "modified");
    assert_eq!(view.theirs.label, "stash/patch");
    assert_eq!(view.theirs.state, "modified");
    assert!(!view.binary);
    let ours = view.ours.commit.as_ref().expect("ours commit");
    assert_eq!(ours.subject, "main-edit");
    assert_eq!(ours.author, "Test");
    assert!(ours.at > 0 && !ours.short.is_empty());
    let theirs = view.theirs.commit.as_ref().expect("stash commit");
    assert!(theirs.subject.contains("wip"), "{}", theirs.subject);
    perform(&repo.path, Mutation::Abort).unwrap();
    let after = status(&repo.path).unwrap();
    assert!(after.unstaged.is_empty() && after.staged.is_empty(), "{after:?}");
    assert_eq!(fs::read_to_string(repo.path.join("a.txt")).unwrap(), "main\n");
    assert!(repository_refs(&repo.path).unwrap().stashes.iter().any(|stash| stash.summary.contains("wip")));
}

#[test]
fn merge_conflict_names_both_branches() {
    let repo = repo();
    commit_file(&repo, "a.txt", "base\n", "base");
    commit_file(&repo, "gone.txt", "keep\n", "keep");
    repo.git(&["checkout", "-b", "topic"]);
    commit_file(&repo, "a.txt", "topic\n", "topic");
    repo.git(&["rm", "gone.txt"]);
    repo.git(&["commit", "-m", "drop"]);
    repo.git(&["checkout", "main"]);
    commit_file(&repo, "a.txt", "main\n", "main-edit");
    commit_file(&repo, "gone.txt", "edited\n", "edit-gone");
    assert!(perform(
        &repo.path,
        Mutation::Merge { name: "topic".into(), squash: false, no_ff: false, autostash: false, summary: String::new(), description: String::new() },
    )
    .is_err());
    let edited = conflict_view(&repo.path, "a.txt").unwrap();
    assert_eq!(edited.ours.label, "main");
    assert_eq!(edited.ours.state, "modified");
    assert_eq!(edited.theirs.label, "topic");
    assert_eq!(edited.theirs.state, "modified");
    assert!(edited.ours.bytes > 0 && edited.theirs.bytes > 0);
    assert_eq!(edited.ours.commit.as_ref().unwrap().subject, "edit-gone");
    assert_eq!(edited.theirs.commit.as_ref().unwrap().subject, "drop");
    assert_eq!(edited.ours.commit.as_ref().unwrap().author, "Test");
    let removed = conflict_view(&repo.path, "gone.txt").unwrap();
    assert_eq!(removed.ours.label, "main");
    assert_eq!(removed.ours.state, "modified");
    assert_eq!(removed.theirs.label, "topic");
    assert_eq!(removed.theirs.state, "deleted");
    assert_eq!(removed.theirs.bytes, 0);
    assert!(removed.ours.bytes > 0);
}

fn binary_merge_conflict() -> common::TempRepo {
    let repo = repo();
    fs::write(repo.path.join("sheet.xlsx"), b"PK\x03\x04base").unwrap();
    repo.git(&["add", "--", "sheet.xlsx"]);
    repo.git(&["commit", "-m", "base"]);
    repo.git(&["checkout", "-b", "topic"]);
    fs::write(repo.path.join("sheet.xlsx"), b"PK\x03\x04topic").unwrap();
    repo.git(&["commit", "-am", "topic"]);
    repo.git(&["checkout", "main"]);
    fs::write(repo.path.join("sheet.xlsx"), b"PK\x03\x04main").unwrap();
    repo.git(&["commit", "-am", "main"]);
    assert!(perform(
        &repo.path,
        Mutation::Merge { name: "topic".into(), squash: false, no_ff: false, autostash: false, summary: String::new(), description: String::new() },
    )
    .is_err());
    repo
}

#[test]
fn merge_tool_without_program_explains_bad_exe() {
    let repo = binary_merge_conflict();
    let error = launch_merge_tool(&repo.path, "{ours} {theirs} {working} {file}", "sheet.xlsx").unwrap_err();
    let text = error.to_string();
    assert!(text.contains("os error 193"), "{text}");
    assert!(text.contains("sheet.xlsx"), "{text}");
    assert!(text.contains("not a Win32 application"), "{text}");
    let program = text.lines().find_map(|line| line.strip_prefix("program: ")).unwrap_or_else(|| panic!("{text}"));
    assert!(program.ends_with("sheet.xlsx"), "{program}");
    assert_eq!(fs::read(program).unwrap(), b"PK\x03\x04main");
    let command = text.lines().find_map(|line| line.strip_prefix("command: ")).unwrap_or_else(|| panic!("{text}"));
    assert!(command.contains(&repo.path.join("sheet.xlsx").display().to_string()), "{command}");
}

#[cfg(windows)]
#[test]
fn merge_tool_starts_cmd_without_bad_exe() {
    let repo = binary_merge_conflict();
    let tool_dir = repo.path.join("tool dir");
    fs::create_dir(&tool_dir).unwrap();
    let script = tool_dir.join("merge.cmd");
    fs::write(&script, "@echo off\r\n> \"%~dp0out.txt\" echo(%~1\r\n").unwrap();
    let template = format!("\"{}\" {{ours}}", script.display());
    launch_merge_tool(&repo.path, &template, "sheet.xlsx").unwrap();
    let out = tool_dir.join("out.txt");
    let started = (0..50).any(|_| {
        if out.is_file() {
            true
        } else {
            std::thread::sleep(std::time::Duration::from_millis(50));
            false
        }
    });
    assert!(started, "cmd.exe did not run {}", script.display());
    let launched = fs::read_to_string(&out).unwrap();
    let launched = launched.trim().trim_matches('"');
    assert!(launched.ends_with("sheet.xlsx"), "{launched}");
    assert_eq!(fs::read(launched).unwrap(), b"PK\x03\x04main");
}

#[test]
fn literal_pathspecs_only_stage_the_named_file() {
    let repo = repo();
    fs::write(repo.path.join("foo1.txt"), "one\n").unwrap();
    fs::write(repo.path.join("foo[1].txt"), "one\n").unwrap();
    repo.git(&["add", "-A", "--", "."]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("foo1.txt"), "one\nother\n").unwrap();
    fs::write(repo.path.join("foo[1].txt"), "one\nglob\n").unwrap();

    stage_paths(&repo.path, &["foo[1].txt".into()]).unwrap();
    let cached = repo.output(&["diff", "--cached", "--name-only"]);
    assert!(cached.contains("foo[1].txt"), "{cached}");
    assert!(!cached.contains("foo1.txt"), "{cached}");
}

#[test]
fn stage_lines_applies_every_selected_line_in_one_pass() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "1\n2\n3\n4\n5\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("a.txt"), "1\nTWO\nTHREE\nFOUR\n5\n").unwrap();

    let diff = file_diff(&repo.path, "a.txt", false).unwrap();
    let mut lines = Vec::new();
    for (text, addition) in [("+TWO", true), ("+FOUR", true)] {
        let line = diff.lines.iter().find(|line| line.text == text).expect(text);
        lines.push(LineEdit {
            text: text[1..].to_string(),
            addition,
            at: line.stage_at.expect("stage index"),
        });
    }
    stage_lines(&repo.path, "a.txt", &lines, false).unwrap();

    let cached = repo.output(&["diff", "--cached"]);
    assert!(cached.contains("TWO"), "{cached}");
    assert!(cached.contains("FOUR"), "{cached}");
    assert!(!cached.contains("THREE"), "{cached}");
}

#[test]
fn discard_lines_removes_only_the_selected_lines() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "1\n2\n3\n4\n5\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("a.txt"), "1\nTWO\nTHREE\nFOUR\n5\n").unwrap();

    let diff = file_diff(&repo.path, "a.txt", false).unwrap();
    let mut lines = Vec::new();
    for text in ["+TWO", "+FOUR"] {
        let line = diff.lines.iter().find(|line| line.text == text).expect(text);
        lines.push(LineEdit {
            text: text[1..].to_string(),
            addition: true,
            at: line.work_at.expect("work index"),
        });
    }
    discard_lines(&repo.path, "a.txt", &lines).unwrap();
    assert_eq!(fs::read_to_string(repo.path.join("a.txt")).unwrap(), "1\nTHREE\n5\n");
}

#[test]
fn blob_view_reports_a_truncated_large_text() {
    let repo = repo();
    fs::write(repo.path.join("big.txt"), "x".repeat(300_000)).unwrap();
    repo.git(&["add", "big.txt"]);
    repo.git(&["commit", "-m", "base"]);

    let view = blob_view(&repo.path, "HEAD", "big.txt").unwrap();
    assert!(!view.binary);
    assert!(view.truncated);
    assert_eq!(view.text.len(), 200_000);
}

#[test]
fn discard_paths_restores_tracked_and_deletes_the_rest() {
    let repo = repo();
    commit_file(&repo, "a.txt", "one\n", "base");
    commit_file(&repo, "b.txt", "two\n", "base-b");
    fs::write(repo.path.join("a.txt"), "changed\n").unwrap();
    fs::write(repo.path.join("b.txt"), "changed\n").unwrap();
    fs::write(repo.path.join("c.txt"), "new\n").unwrap();
    fs::write(repo.path.join("e.txt"), "added\n").unwrap();
    repo.git(&["add", "--", "e.txt"]);
    perform(
        &repo.path,
        Mutation::DiscardPaths {
            files: vec!["a.txt".into(), "b.txt".into(), "c.txt".into(), "e.txt".into(), "missing.txt".into()],
        },
    )
    .unwrap();
    assert_eq!(fs::read_to_string(repo.path.join("a.txt")).unwrap(), "one\n");
    assert_eq!(fs::read_to_string(repo.path.join("b.txt")).unwrap(), "two\n");
    assert!(!repo.path.join("c.txt").exists());
    assert!(!repo.path.join("e.txt").exists());
    let after = status(&repo.path).unwrap();
    assert!(after.staged.is_empty() && after.unstaged.is_empty(), "{after:?}");
}

#[test]
fn delete_paths_removes_tracked_and_untracked_files() {
    let repo = repo();
    commit_file(&repo, "a.txt", "one\n", "base");
    fs::write(repo.path.join("added.txt"), "new\n").unwrap();
    repo.git(&["add", "--", "added.txt"]);
    fs::write(repo.path.join("loose.txt"), "loose\n").unwrap();

    perform(
        &repo.path,
        Mutation::DeletePaths {
            files: vec!["a.txt".into(), "added.txt".into(), "loose.txt".into()],
        },
    )
    .unwrap();
    assert!(!repo.path.join("a.txt").exists());
    assert!(!repo.path.join("added.txt").exists());
    assert!(!repo.path.join("loose.txt").exists());
    let after = status(&repo.path).unwrap();
    assert!(after.unstaged.is_empty(), "{after:?}");
    assert_eq!(after.staged.len(), 1, "{after:?}");
    assert_eq!(after.staged[0].path, "a.txt");
    assert_eq!(after.staged[0].kind, ChangeKind::Deleted);
}

#[test]
fn merge_preview_fills_message() {
    let repo = repo();
    commit_file(&repo, "a.txt", "base\n", "base");
    repo.git(&["checkout", "-b", "topic"]);
    commit_file(&repo, "b.txt", "topic\n", "topic");
    repo.git(&["checkout", "main"]);
    let preview = merge_preview(&repo.path, "topic").unwrap();
    assert_eq!(preview.into, "main");
    assert!(preview.files.iter().any(|file| file.path == "b.txt"), "{:?}", preview.files);
    perform(
        &repo.path,
        Mutation::Merge {
            name: "topic".into(),
            squash: false,
            no_ff: true,
            autostash: false,
            summary: "Merge topic into main".into(),
            description: "A\tb.txt".into(),
        },
    )
    .unwrap();
    let log = repo.output(&["log", "-1", "--format=%B"]);
    assert!(log.contains("Merge topic into main"), "{log}");
    assert!(log.contains("b.txt"), "{log}");
}

struct RemoveDir(PathBuf);

impl Drop for RemoveDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
