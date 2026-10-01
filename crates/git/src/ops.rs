//! Writes that must match the installed Git: network, history, and worktree changes.
//!
//! Nothing here writes the global gitconfig. Revisions and names are checked before
//! they are placed on the command line. Cancellation of a running command is the
//! caller's job; these functions wait for `git` to exit.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cli::{check_name, check_path, check_rev, run, run_env, run_output, run_stdin};
use crate::model::in_progress;
use crate::{commit, discard_hunk, discard_line, resolve_conflict, stage_hunk, stage_line, stage_paths, unstage_paths, CommitRequest, Error, InProgress};

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
        #[serde(default)]
        autostash: bool,
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
        #[serde(default)]
        include_untracked: bool,
    },
    StashPop {
        #[serde(default)]
        name: String,
    },
    StashApply {
        name: String,
    },
    StashDrop {
        name: String,
    },
    /// Replace the message on one stash entry.
    StashRename {
        name: String,
        message: String,
    },
    /// `git stash branch`: a new branch starting from the stashed commit.
    StashBranch {
        name: String,
        branch: String,
    },
    /// `git stash show -p`. The patch text is returned to the caller.
    StashPatch {
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
        #[serde(default)]
        force: bool,
    },
    Merge {
        name: String,
        #[serde(default)]
        squash: bool,
        #[serde(default)]
        no_ff: bool,
        #[serde(default)]
        autostash: bool,
    },
    Rebase {
        onto: String,
        #[serde(default)]
        autostash: bool,
    },
    RebaseInteractive {
        onto: String,
        #[serde(default)]
        drop: Vec<String>,
        #[serde(default)]
        steps: Vec<RebaseStep>,
        #[serde(default)]
        autostash: bool,
        /// Pass `--update-refs` so branches that point at rewritten commits move with them.
        #[serde(default)]
        update_refs: bool,
    },
    Abort,
    Continue {
        #[serde(default)]
        message: String,
    },
    Skip,
    Reset {
        rev: String,
        mode: ResetMode,
    },
    CherryPick {
        rev: String,
        /// Pass `-x`, which records the source commit in the message.
        #[serde(default)]
        record: bool,
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
    DeleteRemoteTag {
        remote: String,
        name: String,
    },
    /// Check one file out of a revision into the index and work tree.
    RestoreFile {
        rev: String,
        file: String,
    },
    Bisect {
        verb: String,
        #[serde(default)]
        rev: String,
    },
    WriteGitignore {
        text: String,
    },
    StageHunk {
        file: String,
        index: u32,
        #[serde(default)]
        unstage: bool,
    },
    StageLine {
        file: String,
        text: String,
        addition: bool,
        at: u32,
        #[serde(default)]
        unstage: bool,
    },
    StagePaths {
        files: Vec<String>,
        #[serde(default)]
        unstage: bool,
    },
    DiscardHunk {
        file: String,
        index: u32,
    },
    DiscardLine {
        file: String,
        text: String,
        addition: bool,
        at: u32,
    },
    Resolve {
        file: String,
        side: String,
        /// Used when `side` is `text`: the file contents to write before `git add`.
        #[serde(default)]
        text: String,
    },
    RenameBranch {
        name: String,
        to: String,
    },
    SetUpstream {
        branch: String,
        #[serde(default)]
        upstream: String,
    },
    DeleteRemoteBranch {
        remote: String,
        branch: String,
    },
    CheckoutRemote {
        remote: String,
        branch: String,
    },
    PushRef {
        remote: String,
        branch: String,
        #[serde(default)]
        force_with_lease: bool,
    },
    PullRef {
        remote: String,
        branch: String,
        #[serde(default)]
        rebase: bool,
        #[serde(default)]
        autostash: bool,
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
    SubmoduleInit {
        path: String,
    },
    SubmoduleSync {
        path: String,
    },
    SubmoduleRemove {
        path: String,
    },
    GitFlowInit {
        #[serde(default)]
        master: String,
        #[serde(default)]
        develop: String,
        #[serde(default)]
        feature: String,
        #[serde(default)]
        release: String,
        #[serde(default)]
        hotfix: String,
        #[serde(default)]
        support: String,
    },
    GitFlowStart {
        name: String,
        /// `feature`, `release`, or `hotfix`. Empty means feature.
        #[serde(default)]
        kind: String,
    },
    GitFlowFinish {
        name: String,
        #[serde(default)]
        kind: String,
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
    LfsLock { path: String },
    LfsUnlock { path: String },
    /// Replace the repository's hidden branch list. An empty list shows every branch.
    SetHidden { names: Vec<String> },
    /// Open branch-group names for this repository. An empty list opens every group.
    SetExpanded { names: Vec<String> },
    /// Write `signoff.enabled` and `signoff.format` in `.git/awegit`.
    SetSignOff { enabled: bool, format: String },
    Custom {
        command: String,
        #[serde(default)]
        repo: String,
        #[serde(default)]
        sha: String,
        #[serde(default)]
        branch: String,
        #[serde(default)]
        file: String,
    },
}

/// One instruction in an interactive rebase. `verb` is pick, reword, squash, fixup, or drop.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebaseStep {
    pub verb: String,
    pub rev: String,
    #[serde(default)]
    pub message: String,
}

pub fn perform(repo: &Path, mutation: Mutation) -> Result<String, Error> {
    if let Mutation::Custom {
        command,
        repo: marked,
        sha,
        branch,
        file,
    } = &mutation
    {
        return custom(repo, command, marked, sha, branch, file);
    }
    if let Mutation::StashPatch { name } = &mutation {
        return stash_patch(repo, name);
    }
    apply(repo, mutation).map(|_| String::new())
}

fn apply(repo: &Path, mutation: Mutation) -> Result<(), Error> {
    match mutation {
        Mutation::Fetch { remote, prune, tags } => fetch(repo, remote, prune, tags),
        Mutation::Pull { rebase, autostash } => pull(repo, rebase, autostash),
        Mutation::Push {
            remote,
            set_upstream,
            tags,
            force_with_lease,
        } => push(repo, remote, set_upstream, tags, force_with_lease),
        Mutation::Stash { message, include_untracked } => stash(repo, &message, include_untracked),
        Mutation::StashPop { name } => stash_pop(repo, &name),
        Mutation::StashApply { name } => run(repo, &["stash", "apply", check_rev(&name)?]).map(|_| ()),
        Mutation::StashDrop { name } => run(repo, &["stash", "drop", check_rev(&name)?]).map(|_| ()),
        Mutation::StashRename { name, message } => stash_rename(repo, &name, &message),
        Mutation::StashBranch { name, branch } => {
            run(repo, &["stash", "branch", check_name(&branch)?, check_rev(&name)?]).map(|_| ())
        }
        Mutation::StashPatch { .. } => Ok(()),
        Mutation::Checkout { name } => run(repo, &["checkout", check_rev(&name)?]).map(|_| ()),
        Mutation::CreateBranch { name, start } => create_branch(repo, &name, start.as_deref()),
        Mutation::DeleteBranch { name, force } => {
            run(repo, &["branch", if force { "-D" } else { "-d" }, check_name(&name)?]).map(|_| ())
        }
        Mutation::Merge {
            name,
            squash,
            no_ff,
            autostash,
        } => merge(repo, &name, squash, no_ff, autostash),
        Mutation::Rebase { onto, autostash } => rebase_onto(repo, &onto, autostash),
        Mutation::RebaseInteractive { onto, drop, steps, autostash, update_refs } => {
            rebase_interactive(repo, &onto, &drop, &steps, autostash, update_refs)
        }
        Mutation::Abort => flow(repo, "abort"),
        Mutation::Continue { message } => continue_with(repo, &message),
        Mutation::Skip => flow(repo, "skip"),
        Mutation::Reset { rev, mode } => reset(repo, &rev, mode),
        Mutation::CherryPick { rev, record } => {
            let rev = check_rev(&rev)?;
            if record {
                with_editor(repo, &["cherry-pick", "-x", "--no-edit", rev])
            } else {
                with_editor(repo, &["cherry-pick", "--no-edit", rev])
            }
        }
        Mutation::Revert { rev } => with_editor(repo, &["revert", "--no-edit", check_rev(&rev)?]),
        Mutation::Reword { rev, summary } => reword(repo, &rev, &summary),
        Mutation::Tag { name, rev, message } => tag(repo, &name, &rev, &message),
        Mutation::DeleteTag { name } => run(repo, &["tag", "-d", check_name(&name)?]).map(|_| ()),
        Mutation::DeleteRemoteTag { remote, name } => {
            let spec = format!(":refs/tags/{}", check_name(&name)?);
            run(repo, &["push", check_name(&remote)?, &spec]).map(|_| ())
        }
        Mutation::RestoreFile { rev, file } => {
            run(repo, &["checkout", check_rev(&rev)?, "--", check_path(&file)?]).map(|_| ())
        }
        Mutation::Bisect { verb, rev } => bisect(repo, &verb, &rev),
        Mutation::WriteGitignore { text } => write_gitignore(repo, &text),
        Mutation::StageHunk { file, index, unstage } => stage_hunk(repo, &file, index, unstage),
        Mutation::StageLine {
            file,
            text,
            addition,
            at,
            unstage,
        } => stage_line(repo, &file, &text, addition, at, unstage),
        Mutation::StagePaths { files, unstage } => {
            if unstage {
                unstage_paths(repo, &files)
            } else {
                stage_paths(repo, &files)
            }
        }
        Mutation::DiscardHunk { file, index } => discard_hunk(repo, &file, index),
        Mutation::DiscardLine {
            file,
            text,
            addition,
            at,
        } => discard_line(repo, &file, &text, addition, at),
        Mutation::Resolve { file, side, text } => resolve_conflict(repo, &file, &side, &text),
        Mutation::RenameBranch { name, to } => {
            run(repo, &["branch", "-m", check_name(&name)?, check_name(&to)?]).map(|_| ())
        }
        Mutation::SetUpstream { branch, upstream } => set_upstream(repo, &branch, &upstream),
        Mutation::DeleteRemoteBranch { remote, branch } => {
            run(repo, &["push", check_name(&remote)?, "--delete", check_name(&branch)?]).map(|_| ())
        }
        Mutation::CheckoutRemote { remote, branch } => checkout_remote(repo, &remote, &branch),
        Mutation::PushRef {
            remote,
            branch,
            force_with_lease,
        } => push_ref(repo, &remote, &branch, force_with_lease),
        Mutation::PullRef {
            remote,
            branch,
            rebase,
            autostash,
        } => pull_ref(repo, &remote, &branch, rebase, autostash),
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
        Mutation::SubmoduleInit { path } => {
            run(repo, &["submodule", "update", "--init", "--", check_path(&path)?]).map(|_| ())
        }
        Mutation::SubmoduleSync { path } => {
            run(repo, &["submodule", "sync", "--", check_path(&path)?]).map(|_| ())
        }
        Mutation::SubmoduleRemove { path } => remove_submodule(repo, &path),
        Mutation::GitFlowInit {
            master,
            develop,
            feature,
            release,
            hotfix,
            support,
        } => git_flow_init(repo, &master, &develop, &feature, &release, &hotfix, &support),
        Mutation::GitFlowStart { name, kind } => git_flow_start(repo, &name, &kind),
        Mutation::GitFlowFinish { name, kind } => git_flow_finish(repo, &name, &kind),
        Mutation::Squash { from, to, summary } => squash(repo, &from, &to, &summary),
        Mutation::Clone { url, destination } => clone_repo(&url, &destination),
        Mutation::Init { destination } => init_repo(&destination),
        Mutation::LfsPull => lfs(repo, &["lfs", "pull"]),
        Mutation::LfsPush => lfs(repo, &["lfs", "push", "--all"]),
        Mutation::LfsLock { path } => lfs(repo, &["lfs", "lock", crate::cli::check_path(&path)?]),
        Mutation::LfsUnlock { path } => lfs(repo, &["lfs", "unlock", crate::cli::check_path(&path)?]),
        Mutation::SetHidden { names } => crate::model::write_hidden_refs(repo, &names),
        Mutation::SetExpanded { names } => crate::model::write_expanded_groups(repo, &names),
        Mutation::SetSignOff { enabled, format } => crate::model::write_sign_off(repo, enabled, &format),
        Mutation::Custom { .. } => Ok(()),
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
    surface(run(repo, &args).map(|_| ()))
}

fn pull(repo: &Path, rebase: bool, autostash: bool) -> Result<(), Error> {
    let mut args = vec!["pull"];
    if autostash {
        args.push("--autostash");
    }
    args.push(if rebase { "--rebase" } else { "--no-rebase" });
    surface(with_editor(repo, &args))
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
    surface(run(repo, &args).map(|_| ()))
}

fn stash_pop(repo: &Path, name: &str) -> Result<(), Error> {
    let name = name.trim();
    if name.is_empty() {
        run(repo, &["stash", "pop"]).map(|_| ())
    } else {
        run(repo, &["stash", "pop", check_rev(name)?]).map(|_| ())
    }
}

fn rebase_onto(repo: &Path, onto: &str, autostash: bool) -> Result<(), Error> {
    let onto = check_rev(onto)?;
    let mut args = vec!["rebase"];
    if autostash {
        args.push("--autostash");
    }
    args.push(onto);
    with_editor(repo, &args)
}

fn stash(repo: &Path, message: &str, include_untracked: bool) -> Result<(), Error> {
    let message = message.trim();
    if !message.is_empty() && (message.starts_with('-') || message.contains(['\n', '\r'])) {
        return Err(Error::Rev(message.to_string()));
    }
    let mut args = vec!["stash", "push"];
    if include_untracked {
        args.push("--include-untracked");
    }
    if !message.is_empty() {
        args.push("-m");
        args.push(message);
    }
    run(repo, &args).map(|_| ())
}

fn stash_patch(repo: &Path, name: &str) -> Result<String, Error> {
    let output = run(repo, &["stash", "show", "-p", check_rev(name)?])?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn stash_rename(repo: &Path, name: &str, message: &str) -> Result<(), Error> {
    let selector = check_rev(name)?;
    let message = message.trim();
    if message.is_empty() || message.starts_with('-') || message.contains(['\n', '\r', '\0']) {
        return Err(Error::Rev(message.to_string()));
    }
    let listed = run(repo, &["rev-parse", selector])?;
    let hash = String::from_utf8_lossy(&listed.stdout).trim().to_string();
    if hash.len() < 7 {
        return Err(Error::Git("stash has no commit".into()));
    }
    run(repo, &["stash", "store", "-m", message, &hash])?;
    let index = stash_index(selector).unwrap_or(0);
    let shifted = format!("stash@{{{}}}", index + 1);
    run(repo, &["stash", "drop", &shifted]).map(|_| ())
}

fn stash_index(selector: &str) -> Option<usize> {
    let start = selector.find("{")?;
    let end = selector.find('}')?;
    selector[start + 1..end].parse().ok()
}

fn bisect(repo: &Path, verb: &str, rev: &str) -> Result<(), Error> {
    match verb {
        "start" => run(repo, &["bisect", "start"]).map(|_| ()),
        "reset" => run(repo, &["bisect", "reset"]).map(|_| ()),
        "good" | "bad" | "skip" => {
            if rev.trim().is_empty() {
                run(repo, &["bisect", verb]).map(|_| ())
            } else {
                run(repo, &["bisect", verb, check_rev(rev)?]).map(|_| ())
            }
        }
        _ => Err(Error::Git(format!("unknown bisect action {verb}"))),
    }
}

fn write_gitignore(repo: &Path, text: &str) -> Result<(), Error> {
    if text.contains('\0') {
        return Err(Error::Git("gitignore contains a null".into()));
    }
    std::fs::write(repo.join(".gitignore"), text).map_err(|source| Error::Read {
        path: ".gitignore".into(),
        source,
    })
}

fn create_branch(repo: &Path, name: &str, start: Option<&str>) -> Result<(), Error> {
    let name = check_name(name)?;
    if let Some(start) = start {
        run(repo, &["branch", name, check_rev(start)?]).map(|_| ())
    } else {
        run(repo, &["branch", name]).map(|_| ())
    }
}

fn merge(repo: &Path, name: &str, squash: bool, no_ff: bool, autostash: bool) -> Result<(), Error> {
    let name = check_rev(name)?;
    let mut args = vec!["merge"];
    if autostash {
        args.push("--autostash");
    }
    if squash {
        args.push("--squash");
    } else if no_ff {
        args.push("--no-ff");
    }
    args.push("--no-edit");
    args.push(name);
    with_editor(repo, &args)
}

/// Store a credential in the user's installed helper. The password is not written to settings.
pub fn approve_credential(repo: &Path, protocol: &str, host: &str, username: &str, password: &str) -> Result<(), Error> {
    for part in [protocol, host, username, password] {
        if part.contains(['\n', '\r', '\0']) {
            return Err(Error::Rev("credential field".into()));
        }
    }
    if protocol.is_empty() || host.is_empty() || password.is_empty() {
        return Err(Error::Git("protocol, host, and password are required".into()));
    }
    let mut body = format!("protocol={protocol}\nhost={host}\n");
    if !username.is_empty() {
        body.push_str(&format!("username={username}\n"));
    }
    body.push_str(&format!("password={password}\n\n"));
    run_stdin(repo, &["credential", "approve"], body.as_bytes()).map(|_| ())
}

fn continue_with(repo: &Path, message: &str) -> Result<(), Error> {
    let message = message.replace("\r\n", "\n");
    if message.trim().is_empty() || !matches!(in_progress(repo)?, Some(InProgress::Rebase)) {
        return flow(repo, "continue");
    }
    if message.contains('\0') {
        return Err(Error::Git("rebase message contains a null".into()));
    }
    let editor = SequenceEditor::with_messages("", &[message])?;
    run_env(repo, &["rebase", "--continue"], &[("GIT_EDITOR", editor.editor.as_str())]).map(|_| ())
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
                sign_off_format: String::new(),
                skip_hooks: false,
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

fn git_flow_init(
    repo: &Path,
    master: &str,
    develop: &str,
    feature: &str,
    release: &str,
    hotfix: &str,
    support: &str,
) -> Result<(), Error> {
    let master = flow_value(master, "main");
    let develop = flow_value(develop, "develop");
    let feature = flow_value(feature, "feature/");
    let release = flow_value(release, "release/");
    let hotfix = flow_value(hotfix, "hotfix/");
    let support = flow_value(support, "support/");
    config_local(repo, "gitflow.branch.master", &master)?;
    config_local(repo, "gitflow.branch.develop", &develop)?;
    config_local(repo, "gitflow.prefix.feature", &feature)?;
    config_local(repo, "gitflow.prefix.release", &release)?;
    config_local(repo, "gitflow.prefix.hotfix", &hotfix)?;
    config_local(repo, "gitflow.prefix.support", &support)?;
    if run(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{develop}")]).is_err() {
        let base = if run(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{master}")]).is_ok() {
            master
        } else {
            integration_base(repo)?
        };
        run(repo, &["branch", &develop, &base])?;
    }
    Ok(())
}

fn flow_kind(kind: &str) -> &'static str {
    match kind {
        "release" => "release",
        "hotfix" => "hotfix",
        _ => "feature",
    }
}

fn git_flow_start(repo: &Path, name: &str, kind: &str) -> Result<(), Error> {
    let kind = flow_kind(kind);
    let (prefix_key, default_prefix) = match kind {
        "release" => ("gitflow.prefix.release", "release/"),
        "hotfix" => ("gitflow.prefix.hotfix", "hotfix/"),
        _ => ("gitflow.prefix.feature", "feature/"),
    };
    let prefix = config_get(repo, prefix_key, default_prefix);
    let branch = prefixed_branch(&prefix, name)?;
    let base_name = if kind == "hotfix" {
        config_get(repo, "gitflow.branch.master", "main")
    } else {
        config_get(repo, "gitflow.branch.develop", "develop")
    };
    let fallback = integration_base(repo)?;
    if run(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{base_name}")]).is_err() {
        run(repo, &["branch", &base_name, &fallback])?;
    }
    run(repo, &["checkout", &base_name])?;
    run(repo, &["checkout", "-b", &branch]).map(|_| ())
}

fn git_flow_finish(repo: &Path, name: &str, kind: &str) -> Result<(), Error> {
    let kind = flow_kind(kind);
    let (prefix_key, default_prefix) = match kind {
        "release" => ("gitflow.prefix.release", "release/"),
        "hotfix" => ("gitflow.prefix.hotfix", "hotfix/"),
        _ => ("gitflow.prefix.feature", "feature/"),
    };
    let prefix = config_get(repo, prefix_key, default_prefix);
    let branch = prefixed_branch(&prefix, name)?;
    if kind == "feature" {
        let develop = config_get(repo, "gitflow.branch.develop", "develop");
        run(repo, &["checkout", &develop])?;
        with_editor(repo, &["merge", "--no-edit", &branch])?;
        return run(repo, &["branch", "-d", &branch]).map(|_| ());
    }
    let master = config_get(repo, "gitflow.branch.master", "main");
    let develop = config_get(repo, "gitflow.branch.develop", "develop");
    run(repo, &["checkout", &master])?;
    with_editor(repo, &["merge", "--no-ff", "--no-edit", &branch])?;
    let short = name.trim().trim_start_matches(prefix.as_str()).to_string();
    if let Ok(tag) = check_name(&short) {
        let _ = run(repo, &["tag", tag]);
    }
    if run(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{develop}")]).is_ok() {
        run(repo, &["checkout", &develop])?;
        with_editor(repo, &["merge", "--no-ff", "--no-edit", &branch])?;
    }
    run(repo, &["branch", "-d", &branch]).map(|_| ())
}

fn prefixed_branch(prefix: &str, name: &str) -> Result<String, Error> {
    let name = name.trim().trim_start_matches(prefix);
    let name = check_name(name)?;
    Ok(format!("{prefix}{name}"))
}

fn flow_value(value: &str, default: &str) -> String {
    let value = value.trim();
    if value.is_empty() { default.to_string() } else { value.to_string() }
}

fn config_local(repo: &Path, key: &str, value: &str) -> Result<(), Error> {
    if value.contains(['\n', '\r', '\0']) || value.starts_with('-') {
        return Err(Error::Rev(value.to_string()));
    }
    run(repo, &["config", "--local", key, value]).map(|_| ())
}

fn config_get(repo: &Path, key: &str, default: &str) -> String {
    run(repo, &["config", "--local", "--get", key])
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| default.to_string())
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
    let dirty = run(repo, &["status", "--porcelain"])?;
    if !String::from_utf8_lossy(&dirty.stdout).trim().is_empty() {
        return Err(Error::Git("squash needs a clean worktree".into()));
    }
    let parent = format!("{from}^");
    let range = if run(repo, &["rev-parse", "--verify", "--quiet", &parent]).is_ok() {
        format!("{parent}..{to}")
    } else {
        format!("{from}..{to}")
    };
    let merges = run(repo, &["rev-list", "--merges", &range])?;
    if !String::from_utf8_lossy(&merges.stdout).trim().is_empty() {
        return Err(Error::Git("squash range contains a merge".into()));
    }
    if run(repo, &["rev-parse", "--verify", "--quiet", "@{upstream}"]).is_ok() {
        let all = run(repo, &["rev-list", &range])?;
        let novel = run(repo, &["rev-list", &range, "--not", "@{upstream}"])?;
        if String::from_utf8_lossy(&all.stdout).trim() != String::from_utf8_lossy(&novel.stdout).trim() {
            return Err(Error::Git("squash range is already in upstream".into()));
        }
    }
    run(repo, &["merge-base", "--is-ancestor", from, to])?;
    let head = rev_parse(repo, "HEAD")?;
    let target = rev_parse(repo, to)?;
    run(repo, &["merge-base", "--is-ancestor", to, "HEAD"])?;
    let later = if head == target {
        Vec::new()
    } else {
        let listed = run(repo, &["rev-list", "--reverse", &format!("{to}..HEAD")])?;
        String::from_utf8_lossy(&listed.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    };
    if run(repo, &["rev-parse", "--verify", "--quiet", &parent]).is_err() {
        return Err(Error::Git("the oldest commit has no parent".into()));
    }
    if head != target {
        run(repo, &["reset", "--hard", to])?;
    }
    run(repo, &["reset", "--soft", &parent])?;
    commit(
        repo,
        CommitRequest {
            summary: summary.to_string(),
            description: String::new(),
            amend: false,
            sign_off: false,
            sign_off_format: String::new(),
            skip_hooks: false,
        },
    )?;
    for id in later {
        with_editor(repo, &["cherry-pick", &id])?;
    }
    Ok(())
}

fn rebase_interactive(
    repo: &Path,
    onto: &str,
    drop: &[String],
    steps: &[RebaseStep],
    autostash: bool,
    update_refs: bool,
) -> Result<(), Error> {
    if !steps.is_empty() {
        return rebase_steps(repo, onto, steps, autostash, update_refs);
    }
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
    let mut args = vec!["rebase"];
    if autostash {
        args.push("--autostash");
    }
    if update_refs {
        args.push("--update-refs");
    }
    args.push("-i");
    args.push(onto);
    let result = run_env(
        repo,
        &args,
        &[
            ("GIT_SEQUENCE_EDITOR", editor.sequence.as_str()),
            ("GIT_EDITOR", editor.editor.as_str()),
        ],
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

fn rebase_steps(repo: &Path, onto: &str, steps: &[RebaseStep], autostash: bool, update_refs: bool) -> Result<(), Error> {
    let onto = check_rev(onto)?;
    let mut todo = String::new();
    let mut messages = Vec::new();
    for step in steps {
        let verb = match step.verb.as_str() {
            "pick" | "reword" | "squash" | "fixup" | "drop" | "edit" => step.verb.as_str(),
            "break" => {
                todo.push_str("break\n");
                continue;
            }
            "exec" => {
                let command = step.message.trim();
                if command.is_empty() || command.contains(['\n', '\r', '\0']) {
                    return Err(Error::Git("rebase exec needs one line".into()));
                }
                todo.push_str("exec ");
                todo.push_str(command);
                todo.push('\n');
                continue;
            }
            other => return Err(Error::Git(format!("unknown rebase action {other}"))),
        };
        let rev = check_rev(&step.rev)?;
        todo.push_str(verb);
        todo.push(' ');
        todo.push_str(rev);
        todo.push('\n');
        if verb == "reword" || verb == "squash" {
            let message = if step.message.trim().is_empty() {
                commit_subject(repo, rev)?
            } else {
                step.message.replace("\r\n", "\n")
            };
            if message.contains('\0') {
                return Err(Error::Git("rebase message contains a null".into()));
            }
            messages.push(message);
        }
    }
    if todo.is_empty() {
        return Err(Error::Git("nothing to rebase".into()));
    }
    let editor = SequenceEditor::with_messages(&todo, &messages)?;
    let mut args = vec!["rebase"];
    if autostash {
        args.push("--autostash");
    }
    if update_refs {
        args.push("--update-refs");
    }
    args.push("-i");
    args.push(onto);
    run_env(
        repo,
        &args,
        &[
            ("GIT_SEQUENCE_EDITOR", editor.sequence.as_str()),
            ("GIT_EDITOR", editor.editor.as_str()),
        ],
    )
    .map(|_| ())
}

fn commit_subject(repo: &Path, rev: &str) -> Result<String, Error> {
    let output = run(repo, &["log", "-1", "--format=%s", rev])?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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
    surface(run(&cwd, &["clone", url, destination]).map(|_| ()))
}

fn init_repo(destination: &str) -> Result<(), Error> {
    let destination = check_external(destination)?;
    std::fs::create_dir_all(destination).map_err(|source| Error::Read {
        path: destination.to_string(),
        source,
    })?;
    run(Path::new(destination), &["init", "-b", "main"]).map(|_| ())
}

fn custom(repo: &Path, command: &str, marked: &str, sha: &str, branch: &str, file: &str) -> Result<String, Error> {
    let command = command.trim();
    if command.is_empty() || command.contains('\0') {
        return Err(Error::Rev(command.to_string()));
    }
    let repo_text = if marked.trim().is_empty() { repo.display().to_string() } else { marked.to_string() };
    for value in [&repo_text, sha, branch, file] {
        if cfg!(windows) && (value.contains('"') || value.contains('%')) {
            return Err(Error::Git("refusing a command value that contains \" or %".into()));
        }
    }
    let expanded = command
        .replace("${repo}", if cfg!(windows) { "%AWEGIT_REPO%" } else { "\"$AWEGIT_REPO\"" })
        .replace("${sha}", if cfg!(windows) { "%AWEGIT_SHA%" } else { "\"$AWEGIT_SHA\"" })
        .replace("${branch}", if cfg!(windows) { "%AWEGIT_BRANCH%" } else { "\"$AWEGIT_BRANCH\"" })
        .replace("${file}", if cfg!(windows) { "%AWEGIT_FILE%" } else { "\"$AWEGIT_FILE\"" });
    let mut process = if cfg!(windows) {
        let mut process = std::process::Command::new("cmd");
        process.arg("/C").arg(expanded);
        process
    } else {
        let mut process = std::process::Command::new("sh");
        process.arg("-c").arg(expanded);
        process
    };
    let output = process
        .current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("AWEGIT_REPO", &repo_text)
        .env("AWEGIT_SHA", sha)
        .env("AWEGIT_BRANCH", branch)
        .env("AWEGIT_FILE", file)
        .output()
        .map_err(Error::GitMissing)?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(if stdout.is_empty() { stderr } else { stdout })
    } else {
        Err(Error::Git(if stderr.is_empty() {
            if stdout.is_empty() { "custom command failed".into() } else { stdout }
        } else {
            stderr
        }))
    }
}

fn lfs(repo: &Path, args: &[&str]) -> Result<(), Error> {
    surface(run(repo, args).map(|_| ()))
}

fn surface(result: Result<(), Error>) -> Result<(), Error> {
    match result {
        Err(Error::Git(message)) if needs_lfs(&message) => Err(Error::Git(
            "git lfs is not installed. Install Git LFS, then pull or push again.".into(),
        )),
        other => other,
    }
}

fn needs_lfs(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("git-lfs")
        || lower.contains("'lfs'")
        || (lower.contains("not a git command") && lower.contains("lfs"))
}

fn set_upstream(repo: &Path, branch: &str, upstream: &str) -> Result<(), Error> {
    let branch = check_name(branch)?;
    if upstream.trim().is_empty() {
        run(repo, &["branch", "--unset-upstream", branch]).map(|_| ())
    } else {
        let upstream = check_rev(upstream)?;
        run(repo, &["branch", &format!("--set-upstream-to={upstream}"), branch]).map(|_| ())
    }
}

fn checkout_remote(repo: &Path, remote: &str, branch: &str) -> Result<(), Error> {
    let remote = check_name(remote)?;
    let branch = check_name(branch)?;
    let local = run(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]);
    if local.is_ok() {
        run(repo, &["checkout", branch]).map(|_| ())
    } else {
        let tracked = format!("{remote}/{branch}");
        run(repo, &["checkout", "--track", &tracked]).map(|_| ())
    }
}

fn push_ref(repo: &Path, remote: &str, branch: &str, force_with_lease: bool) -> Result<(), Error> {
    let remote = check_name(remote)?;
    let branch = check_name(branch)?;
    let spec = format!("HEAD:{branch}");
    let mut args = vec!["push"];
    if force_with_lease {
        args.push("--force-with-lease");
    }
    args.push(remote);
    args.push(&spec);
    surface(run(repo, &args).map(|_| ()))
}

fn pull_ref(repo: &Path, remote: &str, branch: &str, rebase: bool, autostash: bool) -> Result<(), Error> {
    let remote = check_name(remote)?;
    let branch = check_name(branch)?;
    let mut args = vec!["pull"];
    if autostash {
        args.push("--autostash");
    }
    args.push(if rebase { "--rebase" } else { "--no-rebase" });
    args.push(remote);
    args.push(branch);
    surface(with_editor(repo, &args))
}

fn remove_submodule(repo: &Path, path: &str) -> Result<(), Error> {
    let path = check_path(path)?;
    let _ = run(repo, &["submodule", "deinit", "-f", "--", path]);
    run(repo, &["rm", "-f", "--", path]).map(|_| ())
}

pub fn set_repo_author(repo: &Path, name: &str, email: &str) -> Result<(), Error> {
    for value in [name, email] {
        if value.contains(['\n', '\r', '\0']) {
            return Err(Error::Rev("author".into()));
        }
    }
    if !name.trim().is_empty() {
        config_local(repo, "user.name", name.trim())?;
    }
    if !email.trim().is_empty() {
        config_local(repo, "user.email", email.trim())?;
    }
    Ok(())
}

fn with_editor(repo: &Path, args: &[&str]) -> Result<(), Error> {
    let editor = SequenceEditor::noop_only()?;
    run_env(repo, args, &[("GIT_EDITOR", editor.editor.as_str())]).map(|_| ())
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
    editor: String,
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
        Ok(Self {
            _dir: dir,
            sequence,
            editor: noop,
        })
    }

    fn with_messages(todo: &str, messages: &[String]) -> Result<Self, Error> {
        let mut editor = Self::new(todo)?;
        let unix = editor._dir.display().to_string().replace('\\', "/");
        std::fs::write(editor._dir.join("index"), "0").map_err(|source| Error::Read {
            path: editor._dir.display().to_string(),
            source,
        })?;
        for (index, message) in messages.iter().enumerate() {
            let path = editor._dir.join(format!("msg-{index}"));
            let mut body = message.clone();
            if !body.ends_with('\n') {
                body.push('\n');
            }
            std::fs::write(&path, body.replace("\r\n", "\n")).map_err(|source| Error::Read {
                path: path.display().to_string(),
                source,
            })?;
        }
        let script = format!(
            "#!/bin/sh\nn=$(cat \"{unix}/index\" 2>/dev/null || echo 0)\nprintf '%s\\n' \"$((n + 1))\" > \"{unix}/index\"\nif [ -f \"{unix}/msg-$n\" ]; then\n  cp \"{unix}/msg-$n\" \"$1\"\nfi\nexit 0\n"
        );
        editor.editor = write_script(&editor._dir, "message.sh", &script)?;
        Ok(editor)
    }
}

impl Drop for SequenceEditor {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self._dir);
    }
}

/// Facts the repository manager, bisect, and commit box read together.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoFacts {
    pub ignored: Vec<String>,
    pub bisect_active: bool,
    pub bisect_log: String,
    pub template: String,
    pub head_message: String,
    pub weekday: Vec<u32>,
    pub hour: Vec<u32>,
    pub lfs: String,
}

pub fn repo_facts(repo: &Path) -> Result<RepoFacts, Error> {
    let ignored = lines_of(repo, &["ls-files", "-o", "-i", "--exclude-standard"], 500);
    let bisect_log = text_of(repo, &["bisect", "log"]);
    let bisect_active = bisect_log.contains("git bisect");
    let template = commit_template(repo);
    let head_message = text_of(repo, &["log", "-1", "--format=%B"]);
    let (weekday, hour) = activity(repo);
    let lfs = text_of(repo, &["lfs", "ls-files"]);
    Ok(RepoFacts {
        ignored,
        bisect_active,
        bisect_log,
        template,
        head_message,
        weekday,
        hour,
        lfs,
    })
}

fn lines_of(repo: &Path, args: &[&str], limit: usize) -> Vec<String> {
    text_of(repo, args).lines().map(str::trim).filter(|line| !line.is_empty()).take(limit).map(str::to_string).collect()
}

fn text_of(repo: &Path, args: &[&str]) -> String {
    run_output(repo, args)
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

fn commit_template(repo: &Path) -> String {
    let configured = text_of(repo, &["config", "--get", "commit.template"]);
    let path = configured.trim();
    if path.is_empty() {
        return String::new();
    }
    let full = if Path::new(path).is_absolute() { PathBuf::from(path) } else { repo.join(path) };
    std::fs::read_to_string(full).unwrap_or_default()
}

fn activity(repo: &Path) -> (Vec<u32>, Vec<u32>) {
    let mut weekday = vec![0; 7];
    let mut hour = vec![0; 24];
    for line in text_of(repo, &["log", "-n", "4000", "--format=%at"]).lines() {
        let Ok(unix) = line.trim().parse::<i64>() else { continue };
        let days = unix.div_euclid(86_400);
        let hour_index = (unix.rem_euclid(86_400) / 3_600) as usize;
        let day_index = ((days + 4).rem_euclid(7)) as usize;
        if day_index < 7 {
            weekday[day_index] += 1;
        }
        if hour_index < 24 {
            hour[hour_index] += 1;
        }
    }
    (weekday, hour)
}

/// Find git work trees under `root`, without entering a repository or common build folders.
pub fn scan_repositories(root: &Path) -> Result<Vec<String>, Error> {
    if !root.is_dir() {
        return Err(Error::Git(format!("{} is not a directory", root.display())));
    }
    let mut found = Vec::new();
    scan_dir(root, 0, &mut found);
    Ok(found)
}

fn scan_dir(dir: &Path, depth: usize, found: &mut Vec<String>) {
    if found.len() >= 300 || depth > 5 {
        return;
    }
    if dir.join(".git").exists() {
        found.push(dir.display().to_string());
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if !kind.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if matches!(name.as_ref(), "node_modules" | "target" | "dist" | "vendor" | ".git" | ".next" | "out") {
            continue;
        }
        scan_dir(&entry.path(), depth + 1, found);
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
