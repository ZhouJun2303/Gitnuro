//! Writes that must match the installed Git: network, history, and worktree changes.
//!
//! Nothing here writes the global gitconfig. Revisions and names are checked before
//! they are placed on the command line. Cancellation of a running command is the
//! caller's job; these functions wait for `git` to exit.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cli::{check_name, check_path, check_rev, run, run_env, run_stdin};
use crate::model::in_progress;
use crate::{commit, stage_hunk, CommitRequest, Error, InProgress};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum Mutation {
    Fetch {
        #[serde(default)]
        remote: Option<String>,
        #[serde(default)]
        prune: bool,
        #[serde(default)]
        tags: bool,
    },
    Pull {
        #[serde(default)]
        rebase: bool,
    },
    Push {
        #[serde(default)]
        remote: Option<String>,
        #[serde(default)]
        set_upstream: bool,
        #[serde(default)]
        tags: bool,
        #[serde(default)]
        force_with_lease: bool,
    },
    Stash {
        #[serde(default)]
        message: String,
    },
    StashPop,
    StashApply {
        name: String,
    },
    StashDrop {
        name: String,
    },
    Checkout {
        name: String,
    },
    CreateBranch {
        name: String,
        #[serde(default)]
        start: Option<String>,
    },
    DeleteBranch {
        name: String,
    },
    Merge {
        name: String,
        #[serde(default)]
        squash: bool,
    },
    Rebase {
        onto: String,
    },
    RebaseInteractive {
        onto: String,
        #[serde(default)]
        drop: Vec<String>,
    },
    Abort,
    Continue,
    Skip,
    Reset {
        rev: String,
        mode: ResetMode,
    },
    CherryPick {
        rev: String,
    },
    Revert {
        rev: String,
    },
    Reword {
        rev: String,
        summary: String,
    },
    Tag {
        name: String,
        #[serde(default)]
        rev: String,
        #[serde(default)]
        message: String,
    },
    DeleteTag {
        name: String,
    },
    StageHunk {
        file: String,
        index: u32,
        #[serde(default)]
        unstage: bool,
    },
    Discard {
        file: String,
    },
    Delete {
        file: String,
    },
    ApplyPatch {
        patch: String,
    },
    AddRemote {
        name: String,
        url: String,
    },
    RemoveRemote {
        name: String,
    },
    SetRemoteUrl {
        name: String,
        url: String,
    },
    AddWorktree {
        path: String,
        branch: String,
    },
    RemoveWorktree {
        path: String,
    },
    SubmoduleUpdate,
    SubmoduleAdd {
        url: String,
        path: String,
    },
    GitFlowStart {
        name: String,
    },
    GitFlowFinish {
        name: String,
    },
    Squash {
        from: String,
        to: String,
        summary: String,
    },
    Clone {
        url: String,
        destination: String,
    },
    Init {
        destination: String,
    },
    LfsPull,
    LfsPush,
    Custom {
        command: String,
    },
}

pub fn perform(repo: &Path, mutation: Mutation) -> Result<(), Error> {
    match mutation {
        Mutation::Fetch { remote, prune, tags } => fetch(repo, remote, prune, tags),
        Mutation::Pull { rebase } => pull(repo, rebase),
        Mutation::Push {
            remote,
            set_upstream,
            tags,
            force_with_lease,
        } => push(repo, remote, set_upstream, tags, force_with_lease),
        Mutation::Stash { message } => stash(repo, &message),
        Mutation::StashPop => run(repo, &["stash", "pop"]).map(|_| ()),
        Mutation::StashApply { name } => run(repo, &["stash", "apply", check_rev(&name)?]).map(|_| ()),
        Mutation::StashDrop { name } => run(repo, &["stash", "drop", check_rev(&name)?]).map(|_| ()),
        Mutation::Checkout { name } => run(repo, &["checkout", check_rev(&name)?]).map(|_| ()),
        Mutation::CreateBranch { name, start } => create_branch(repo, &name, start.as_deref()),
        Mutation::DeleteBranch { name } => run(repo, &["branch", "-d", check_name(&name)?]).map(|_| ()),
        Mutation::Merge { name, squash } => merge(repo, &name, squash),
        Mutation::Rebase { onto } => with_editor(repo, &["rebase", check_rev(&onto)?]),
        Mutation::RebaseInteractive { onto, drop } => rebase_interactive(repo, &onto, &drop),
        Mutation::Abort => flow(repo, "abort"),
        Mutation::Continue => flow(repo, "continue"),
        Mutation::Skip => flow(repo, "skip"),
        Mutation::Reset { rev, mode } => reset(repo, &rev, mode),
        Mutation::CherryPick { rev } => with_editor(repo, &["cherry-pick", "--no-edit", check_rev(&rev)?]),
        Mutation::Revert { rev } => with_editor(repo, &["revert", "--no-edit", check_rev(&rev)?]),
        Mutation::Reword { rev, summary } => reword(repo, &rev, &summary),
        Mutation::Tag { name, rev, message } => tag(repo, &name, &rev, &message),
        Mutation::DeleteTag { name } => run(repo, &["tag", "-d", check_name(&name)?]).map(|_| ()),
        Mutation::StageHunk { file, index, unstage } => stage_hunk(repo, &file, index, unstage),
        Mutation::Discard { file } => discard(repo, &file),
        Mutation::Delete { file } => delete_path(repo, &file),
        Mutation::ApplyPatch { patch } => run_stdin(repo, &["apply", "--index"], patch.as_bytes()).map(|_| ()),
        Mutation::AddRemote { name, url } => {
            run(repo, &["remote", "add", check_name(&name)?, check_url(&url)?]).map(|_| ())
        }
        Mutation::RemoveRemote { name } => run(repo, &["remote", "remove", check_name(&name)?]).map(|_| ()),
        Mutation::SetRemoteUrl { name, url } => {
            run(repo, &["remote", "set-url", check_name(&name)?, check_url(&url)?]).map(|_| ())
        }
        Mutation::AddWorktree { path, branch } => add_worktree(repo, &path, &branch),
        Mutation::RemoveWorktree { path } => {
            run(repo, &["worktree", "remove", check_external(&path)?]).map(|_| ())
        }
        Mutation::SubmoduleUpdate => {
            run(repo, &["submodule", "update", "--init", "--recursive"]).map(|_| ())
        }
        Mutation::SubmoduleAdd { url, path } => {
            run(repo, &["submodule", "add", check_url(&url)?, check_path(&path)?]).map(|_| ())
        }
        Mutation::GitFlowStart { name } => git_flow_start(repo, &name),
        Mutation::GitFlowFinish { name } => git_flow_finish(repo, &name),
        Mutation::Squash { from, to, summary } => squash(repo, &from, &to, &summary),
        Mutation::Clone { url, destination } => clone_repo(&url, &destination),
        Mutation::Init { destination } => init_repo(&destination),
        Mutation::LfsPull => run(repo, &["lfs", "pull"]).map(|_| ()),
        Mutation::LfsPush => run(repo, &["lfs", "push", "--all"]).map(|_| ()),
        Mutation::Custom { command } => custom(repo, &command),
    }
}

fn fetch(repo: &Path, remote: Option<String>, prune: bool, tags: bool) -> Result<(), Error> {
    let remote = optional_name(remote)?;
    let mut args = vec!["fetch"];
    if prune {
        args.push("--prune");
    }
    if tags {
        args.push("--tags");
    }
    if let Some(remote) = remote.as_deref() {
        args.push(remote);
    } else {
        args.push("--all");
    }
    run(repo, &args).map(|_| ())
}

fn pull(repo: &Path, rebase: bool) -> Result<(), Error> {
    let flag = if rebase { "--rebase" } else { "--no-rebase" };
    with_editor(repo, &["pull", flag])
}

fn push(
    repo: &Path,
    remote: Option<String>,
    set_upstream: bool,
    tags: bool,
    force_with_lease: bool,
) -> Result<(), Error> {
    let remote = optional_name(remote)?;
    let mut args = vec!["push"];
    if set_upstream {
        args.push("--set-upstream");
    }
    if tags {
        args.push("--tags");
    }
    if force_with_lease {
        args.push("--force-with-lease");
    }
    if let Some(remote) = remote.as_deref() {
        args.push(remote);
        if set_upstream {
            args.push("HEAD");
        }
    }
    run(repo, &args).map(|_| ())
}

fn stash(repo: &Path, message: &str) -> Result<(), Error> {
    let message = message.trim();
    if message.is_empty() {
        return run(repo, &["stash", "push"]).map(|_| ());
    }
    if message.starts_with('-') || message.contains(['\n', '\r']) {
        return Err(Error::Rev(message.to_string()));
    }
    run(repo, &["stash", "push", "-m", message]).map(|_| ())
}

fn create_branch(repo: &Path, name: &str, start: Option<&str>) -> Result<(), Error> {
    let name = check_name(name)?;
    if let Some(start) = start {
        run(repo, &["branch", name, check_rev(start)?]).map(|_| ())
    } else {
        run(repo, &["branch", name]).map(|_| ())
    }
}

fn merge(repo: &Path, name: &str, squash: bool) -> Result<(), Error> {
    let name = check_rev(name)?;
    if squash {
        with_editor(repo, &["merge", "--squash", "--no-edit", name])
    } else {
        with_editor(repo, &["merge", "--no-edit", name])
    }
}

fn flow(repo: &Path, verb: &str) -> Result<(), Error> {
    if verb == "skip" && matches!(in_progress(repo)?, Some(InProgress::Merge) | None) {
        return Err(Error::Git("skip is only available during rebase, cherry-pick, or revert".into()));
    }
    let command = match in_progress(repo)? {
        Some(InProgress::Merge) => "merge",
        Some(InProgress::Rebase) => "rebase",
        Some(InProgress::CherryPick) => "cherry-pick",
        Some(InProgress::Revert) => "revert",
        None => return Err(Error::Git("nothing in progress".into())),
    };
    let flag = format!("--{verb}");
    with_editor(repo, &[command, &flag])
}

fn reset(repo: &Path, rev: &str, mode: ResetMode) -> Result<(), Error> {
    let flag = match mode {
        ResetMode::Soft => "--soft",
        ResetMode::Mixed => "--mixed",
        ResetMode::Hard => "--hard",
    };
    run(repo, &["reset", flag, check_rev(rev)?]).map(|_| ())
}

fn reword(repo: &Path, rev: &str, summary: &str) -> Result<(), Error> {
    let summary = summary.trim();
    if summary.is_empty() {
        return Err(Error::EmptySummary);
    }
    let rev = check_rev(rev)?;
    let head = rev_parse(repo, "HEAD")?;
    let target = rev_parse(repo, rev)?;
    if head != target {
        return Err(Error::Git("reword currently changes HEAD only".into()));
    }
    let staged = run(repo, &["diff", "--cached", "--name-only"])?;
    if !staged.stdout.iter().any(|byte| !byte.is_ascii_whitespace()) {
        commit(
            repo,
            CommitRequest {
                summary: summary.to_string(),
                description: String::new(),
                amend: true,
                sign_off: false,
            },
        )
    } else {
        Err(Error::Git("unstage changes before rewording HEAD".into()))
    }
}

fn tag(repo: &Path, name: &str, rev: &str, message: &str) -> Result<(), Error> {
    let name = check_name(name)?;
    let rev = if rev.trim().is_empty() { "HEAD" } else { check_rev(rev)? };
    if message.trim().is_empty() {
        run(repo, &["tag", name, rev]).map(|_| ())
    } else {
        run(repo, &["tag", "-a", name, rev, "-m", message.trim()]).map(|_| ())
    }
}

fn discard(repo: &Path, file: &str) -> Result<(), Error> {
    let file = check_path(file)?;
    if in_index(repo, file)? {
        run(repo, &["restore", "--worktree", "--source=HEAD", "--", file]).map(|_| ())
    } else {
        remove_worktree_file(repo, file)
    }
}

fn delete_path(repo: &Path, file: &str) -> Result<(), Error> {
    let file = check_path(file)?;
    if in_index(repo, file)? {
        run(repo, &["rm", "-f", "--", file]).map(|_| ())
    } else {
        remove_worktree_file(repo, file)
    }
}

fn remove_worktree_file(repo: &Path, file: &str) -> Result<(), Error> {
    let full = repo.join(file);
    if !full.starts_with(repo) {
        return Err(Error::Path(file.to_string()));
    }
    if full.is_dir() {
        std::fs::remove_dir_all(&full)
    } else {
        std::fs::remove_file(&full)
    }
    .map_err(|source| Error::Read {
        path: file.to_string(),
        source,
    })
}

fn add_worktree(repo: &Path, path: &str, branch: &str) -> Result<(), Error> {
    let path = check_external(path)?;
    let branch = check_name(branch)?;
    let exists = run(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_ok();
    if exists {
        run(repo, &["worktree", "add", path, branch]).map(|_| ())
    } else {
        run(repo, &["worktree", "add", "-b", branch, path]).map(|_| ())
    }
}

fn git_flow_start(repo: &Path, name: &str) -> Result<(), Error> {
    let feature = feature_branch(name)?;
    let base = integration_base(repo)?;
    if run(repo, &["rev-parse", "--verify", "--quiet", "refs/heads/develop"]).is_err() {
        run(repo, &["branch", "develop", &base])?;
    }
    run(repo, &["checkout", "develop"])?;
    run(repo, &["checkout", "-b", &feature]).map(|_| ())
}

fn git_flow_finish(repo: &Path, name: &str) -> Result<(), Error> {
    let feature = feature_branch(name)?;
    run(repo, &["checkout", "develop"])?;
    with_editor(repo, &["merge", "--no-edit", &feature])?;
    run(repo, &["branch", "-d", &feature]).map(|_| ())
}

fn feature_branch(name: &str) -> Result<String, Error> {
    let name = name.trim().trim_start_matches("feature/");
    let name = check_name(name)?;
    Ok(format!("feature/{name}"))
}

fn integration_base(repo: &Path) -> Result<String, Error> {
    if run(repo, &["rev-parse", "--verify", "--quiet", "refs/heads/main"]).is_ok() {
        Ok("main".into())
    } else if run(repo, &["rev-parse", "--verify", "--quiet", "refs/heads/master"]).is_ok() {
        Ok("master".into())
    } else {
        Err(Error::Git("git flow needs a main or master branch".into()))
    }
}

fn squash(repo: &Path, from: &str, to: &str, summary: &str) -> Result<(), Error> {
    let from = check_rev(from)?;
    let to = check_rev(to)?;
    let range = format!("{from}..{to}");
    let merges = run(repo, &["rev-list", "--merges", &range])?;
    if !String::from_utf8_lossy(&merges.stdout).trim().is_empty() {
        return Err(Error::Git("squash range contains a merge".into()));
    }
    if run(repo, &["rev-parse", "--verify", "--quiet", "@{upstream}"]).is_ok() {
        let novel = run(repo, &["rev-list", &range, "--not", "@{upstream}"])?;
        if String::from_utf8_lossy(&novel.stdout).trim().is_empty() {
            return Err(Error::Git("squash range is already in upstream".into()));
        }
    }
    let head = rev_parse(repo, "HEAD")?;
    let target = rev_parse(repo, to)?;
    if head != target {
        return Err(Error::Git("squash target must be HEAD".into()));
    }
    run(repo, &["merge-base", "--is-ancestor", from, to])?;
    let parent = format!("{from}^");
    run(repo, &["reset", "--soft", &parent])?;
    commit(
        repo,
        CommitRequest {
            summary: summary.to_string(),
            description: String::new(),
            amend: false,
            sign_off: false,
        },
    )
}

fn rebase_interactive(repo: &Path, onto: &str, drop: &[String]) -> Result<(), Error> {
    let onto = check_rev(onto)?;
    for id in drop {
        check_rev(id)?;
    }
    let listed = run(repo, &["rev-list", "--reverse", &format!("{onto}..HEAD")])?;
    let ids: Vec<String> = String::from_utf8_lossy(&listed.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    if ids.is_empty() {
        return Err(Error::Git("nothing to rebase".into()));
    }
    let mut todo = String::new();
    for id in &ids {
        let verb = if dropped(id, drop) { "drop" } else { "pick" };
        todo.push_str(verb);
        todo.push(' ');
        todo.push_str(id);
        todo.push('\n');
    }
    let editor = SequenceEditor::new(&todo)?;
    let noop = editor.noop.clone();
    let sequence = editor.sequence.clone();
    let result = run_env(
        repo,
        &["rebase", "-i", onto],
        &[("GIT_SEQUENCE_EDITOR", sequence.as_str()), ("GIT_EDITOR", noop.as_str())],
    );
    match result {
        Ok(_) => Ok(()),
        Err(error) => {
            if error.to_string().contains("editor") {
                if in_progress(repo)?.is_some() {
                    let _ = run(repo, &["rebase", "--abort"]);
                }
                replay_drop(repo, onto, &ids, drop)
            } else {
                Err(error)
            }
        }
    }
}

fn dropped(id: &str, drop: &[String]) -> bool {
    drop.iter().any(|item| item == id || id.starts_with(item.as_str()) || item.starts_with(id))
}

fn replay_drop(repo: &Path, onto: &str, ids: &[String], drop: &[String]) -> Result<(), Error> {
    run(repo, &["reset", "--hard", onto])?;
    for id in ids {
        if dropped(id, drop) {
            continue;
        }
        with_editor(repo, &["cherry-pick", id])?;
    }
    Ok(())
}

fn clone_repo(url: &str, destination: &str) -> Result<(), Error> {
    let url = check_url(url)?;
    let destination = check_external(destination)?;
    let parent = Path::new(destination).parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|source| Error::Read {
        path: parent.display().to_string(),
        source,
    })?;
    let cwd = if parent.as_os_str().is_empty() {
        std::env::current_dir().map_err(Error::GitMissing)?
    } else {
        parent.to_path_buf()
    };
    run(&cwd, &["clone", url, destination]).map(|_| ())
}

fn init_repo(destination: &str) -> Result<(), Error> {
    let destination = check_external(destination)?;
    std::fs::create_dir_all(destination).map_err(|source| Error::Read {
        path: destination.to_string(),
        source,
    })?;
    run(Path::new(destination), &["init", "-b", "main"]).map(|_| ())
}

fn custom(repo: &Path, command: &str) -> Result<(), Error> {
    let command = command.trim();
    if command.is_empty() || command.contains('\0') {
        return Err(Error::Rev(command.to_string()));
    }
    let mut process = if cfg!(windows) {
        let mut process = std::process::Command::new("cmd");
        process.arg("/C").arg(command);
        process
    } else {
        let mut process = std::process::Command::new("sh");
        process.arg("-c").arg(command);
        process
    };
    let output = process
        .current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(Error::GitMissing)?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(Error::Git(if stderr.is_empty() {
            "custom command failed".into()
        } else {
            stderr
        }))
    }
}

fn with_editor(repo: &Path, args: &[&str]) -> Result<(), Error> {
    let editor = SequenceEditor::noop_only()?;
    run_env(repo, args, &[("GIT_EDITOR", editor.noop.as_str())]).map(|_| ())
}

fn optional_name(value: Option<String>) -> Result<Option<String>, Error> {
    match value {
        Some(name) if !name.trim().is_empty() => {
            check_name(&name)?;
            Ok(Some(name))
        }
        _ => Ok(None),
    }
}

fn check_url(url: &str) -> Result<&str, Error> {
    if url.is_empty() || url.starts_with('-') || url.contains(['\n', '\r', '\0']) {
        Err(Error::Rev(url.to_string()))
    } else {
        Ok(url)
    }
}

fn check_external(path: &str) -> Result<&str, Error> {
    if path.is_empty() || path.starts_with('-') || path.contains(['\n', '\r', '\0']) {
        Err(Error::Path(path.to_string()))
    } else {
        Ok(path)
    }
}

fn in_index(repo: &Path, path: &str) -> Result<bool, Error> {
    let output = run(repo, &["ls-files", "--", path])?;
    Ok(!output.stdout.is_empty())
}

fn rev_parse(repo: &Path, rev: &str) -> Result<String, Error> {
    let output = run(repo, &["rev-parse", check_rev(rev)?])?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

struct SequenceEditor {
    _dir: PathBuf,
    sequence: String,
    noop: String,
}

impl SequenceEditor {
    fn noop_only() -> Result<Self, Error> {
        Self::new("")
    }

    fn new(todo: &str) -> Result<Self, Error> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("awegit-editor-{nanos}"));
        std::fs::create_dir_all(&dir).map_err(|source| Error::Read {
            path: dir.display().to_string(),
            source,
        })?;
        let noop = write_script(&dir, "noop.sh", "#!/bin/sh\nexit 0\n")?;
        let sequence = if todo.is_empty() {
            noop.clone()
        } else {
            let todo_path = dir.join("todo");
            std::fs::write(&todo_path, todo).map_err(|source| Error::Read {
                path: todo_path.display().to_string(),
                source,
            })?;
            let unix = todo_path.display().to_string().replace('\\', "/");
            write_script(&dir, "edit.sh", &format!("#!/bin/sh\ncp \"{unix}\" \"$1\"\n"))?
        };
        Ok(Self { _dir: dir, sequence, noop })
    }
}

impl Drop for SequenceEditor {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self._dir);
    }
}

fn write_script(dir: &Path, name: &str, body: &str) -> Result<String, Error> {
    let path = dir.join(name);
    std::fs::write(&path, body).map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })?;
    Ok(path.display().to_string().replace('\\', "/"))
}
