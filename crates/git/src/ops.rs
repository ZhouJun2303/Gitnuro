//! Writes that must match the installed Git: network, history, and worktree changes.
//!
//! Nothing here writes the global gitconfig. Revisions and names are checked before
//! they are placed on the command line. Cancellation of a running command is the
//! caller's job; these functions wait for `git` to exit.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cli::{check_name, check_path, check_rev, commit_config, has_local_author, run, run_env, run_paths, run_repo_author, run_stdin, with_operation_repo};
use crate::model::in_progress;
use crate::{commit, discard_hunk, discard_line, discard_lines, resolve_conflict, stage_hunk, stage_line, stage_lines, stage_paths, unstage_paths, CommitRequest, Error, InProgress, LineEdit};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", rename_all_fields = "camelCase")]
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
        /// Bare `--force`. Lease stays the default when this is false.
        #[serde(default)]
        force: bool,
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
    /// Stash only the listed paths.
    StashPaths {
        files: Vec<String>,
        #[serde(default)]
        message: String,
        #[serde(default)]
        include_untracked: bool,
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
        /// First line of the merge commit. Empty keeps Git's own message.
        #[serde(default)]
        summary: String,
        /// Extra paragraphs, usually the files this merge brings in.
        #[serde(default)]
        description: String,
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
    /// Append paths to `.gitignore` or `.fakeignore` without touching the other lines.
    AppendIgnore {
        #[serde(default)]
        file: String,
        #[serde(default)]
        files: Vec<String>,
        #[serde(default)]
        fake: bool,
        #[serde(default)]
        directory: bool,
    },
    WriteMailmap {
        text: String,
    },
    StageHunk {
        file: String,
        index: u32,
        #[serde(default)]
        unstage: bool,
        /// The displayed diff ignored whitespace, so the hunk index must use the same view.
        #[serde(default)]
        ignore_space: bool,
    },
    StageLine {
        file: String,
        text: String,
        addition: bool,
        at: u32,
        #[serde(default)]
        unstage: bool,
    },
    /// Apply many line edits with one index write and one status refresh.
    StageLines {
        file: String,
        lines: Vec<LineEdit>,
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
        /// The displayed diff ignored whitespace, so the hunk index must use the same view.
        #[serde(default)]
        ignore_space: bool,
    },
    DiscardLine {
        file: String,
        text: String,
        addition: bool,
        at: u32,
    },
    /// Apply many discards with one file read and one write.
    DiscardLines {
        file: String,
        lines: Vec<LineEdit>,
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
        #[serde(default)]
        force: bool,
        /// When set, push `branch` to this remote branch name.
        #[serde(default)]
        remote_branch: String,
        #[serde(default)]
        set_upstream: bool,
        #[serde(default)]
        tags: bool,
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
    /// Drop worktree changes for many paths in one pass.
    DiscardPaths {
        files: Vec<String>,
    },
    Delete {
        file: String,
    },
    /// Remove many paths from the index and the work tree in one pass.
    DeletePaths {
        files: Vec<String>,
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

/// Operations that can leave unmerged paths behind. Each one gets a conflict backup.
fn conflict_label(mutation: &Mutation) -> Option<&'static str> {
    Some(match mutation {
        Mutation::Pull { .. } => "pull",
        Mutation::PullRef { .. } => "pull",
        Mutation::Merge { .. } => "merge",
        Mutation::Rebase { .. } => "rebase",
        Mutation::RebaseInteractive { .. } => "rebase",
        Mutation::Continue { .. } => "continue",
        Mutation::CherryPick { .. } => "cherry-pick",
        Mutation::Revert { .. } => "revert",
        Mutation::StashPop { .. } => "stash pop",
        Mutation::StashApply { .. } => "stash apply",
        Mutation::StashBranch { .. } => "stash branch",
        Mutation::Squash { .. } => "squash",
        Mutation::GitFlowFinish { .. } => "git flow finish",
        _ => return None,
    })
}

/// Snapshot the tracked local changes before `action`. If it ends with unmerged paths,
/// keep the snapshot as a stash entry so the pre-conflict work is never lost. Without
/// a conflict the snapshot is dropped, and a failed snapshot never blocks the action.
fn with_conflict_backup(repo: &Path, label: &str, action: impl FnOnce() -> Result<(), Error>) -> Result<(), Error> {
    let snapshot = snapshot_local_changes(repo);
    let result = action();
    if let Some(hash) = snapshot {
        if has_unmerged(repo) {
            let message = format!("AweGit conflict backup: {label}");
            let _ = run(repo, &["stash", "store", "-m", &message, &hash]);
        }
    }
    result
}

/// Keep local work out of the way while `action` rewrites history, then put it
/// back. If the action fails the stash is still restored; if restoring it fails,
/// the stash entry stays in the list and its error is returned.
fn with_autostash(repo: &Path, label: &str, action: impl FnOnce() -> Result<(), Error>) -> Result<(), Error> {
    let dirty = run(repo, &["status", "--porcelain"])?;
    if String::from_utf8_lossy(&dirty.stdout).trim().is_empty() {
        return action();
    }
    let message = format!("AweGit autostash: {label}");
    run(repo, &["stash", "push", "--include-untracked", "-m", &message])?;
    let result = action();
    let restored = run(repo, &["stash", "pop"]);
    match (result, restored) {
        (Ok(()), Ok(_)) => Ok(()),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), _) => Err(error),
    }
}

/// `git stash create`: a commit of staged and unstaged changes that touches no ref
/// and no file. `None` when the tree is clean or the snapshot cannot be made.
fn snapshot_local_changes(repo: &Path) -> Option<String> {
    let output = run(repo, &["stash", "create"]).ok()?;
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if hash.len() >= 7 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(hash)
    } else {
        None
    }
}

fn has_unmerged(repo: &Path) -> bool {
    run(repo, &["ls-files", "--unmerged"])
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(false)
}

fn apply(repo: &Path, mutation: Mutation) -> Result<(), Error> {
    match conflict_label(&mutation) {
        Some(label) => with_conflict_backup(repo, label, || apply_unguarded(repo, mutation)),
        None => apply_unguarded(repo, mutation),
    }
}

fn apply_unguarded(repo: &Path, mutation: Mutation) -> Result<(), Error> {
    match mutation {
        Mutation::Fetch { remote, prune, tags } => fetch(repo, remote, prune, tags),
        Mutation::Pull { rebase, autostash } => pull(repo, rebase, autostash),
        Mutation::Push {
            remote,
            set_upstream,
            tags,
            force_with_lease,
            force,
        } => push(repo, remote, set_upstream, tags, force_with_lease, force),
        Mutation::Stash { message, include_untracked } => stash(repo, &message, include_untracked),
        Mutation::StashPop { name } => stash_pop(repo, &name),
        Mutation::StashApply { name } => run(repo, &["stash", "apply", check_rev(&name)?]).map(|_| ()),
        Mutation::StashDrop { name } => run(repo, &["stash", "drop", check_rev(&name)?]).map(|_| ()),
        Mutation::StashRename { name, message } => stash_rename(repo, &name, &message),
        Mutation::StashBranch { name, branch } => {
            run(repo, &["stash", "branch", check_name(&branch)?, check_rev(&name)?]).map(|_| ())
        }
        Mutation::StashPatch { .. } => Ok(()),
        Mutation::StashPaths { files, message, include_untracked } => stash_paths(repo, &files, &message, include_untracked),
        Mutation::Checkout { name } => with_autostash(repo, "checkout", || {
            run(repo, &["checkout", check_rev(&name)?]).map(|_| ())
        }),
        Mutation::CreateBranch { name, start } => create_branch(repo, &name, start.as_deref()),
        Mutation::DeleteBranch { name, force } => {
            run(repo, &["branch", if force { "-D" } else { "-d" }, check_name(&name)?]).map(|_| ())
        }
        Mutation::Merge {
            name,
            squash,
            no_ff,
            autostash,
            summary,
            description,
        } => merge(repo, &name, squash, no_ff, autostash, &summary, &description),
        Mutation::Rebase { onto, autostash } => rebase_onto(repo, &onto, autostash),
        Mutation::RebaseInteractive { onto, drop, steps, autostash, update_refs } => {
            rebase_interactive(repo, &onto, &drop, &steps, autostash, update_refs)
        }
        Mutation::Abort => abort(repo),
        Mutation::Continue { message } => continue_with(repo, &message),
        Mutation::Skip => flow(repo, "skip"),
        Mutation::Reset { rev, mode } => reset(repo, &rev, mode),
        Mutation::CherryPick { rev, record } => with_autostash(repo, "cherry-pick", || {
            let rev = check_rev(&rev)?;
            if record {
                with_editor(repo, &["cherry-pick", "-x", "--no-edit", rev])
            } else {
                with_editor(repo, &["cherry-pick", "--no-edit", rev])
            }
        }),
        Mutation::Revert { rev } => {
            with_autostash(repo, "revert", || with_editor(repo, &["revert", "--no-edit", check_rev(&rev)?]))
        }
        Mutation::Reword { rev, summary } => with_autostash(repo, "reword", || reword(repo, &rev, &summary)),
        Mutation::Tag { name, rev, message } => tag(repo, &name, &rev, &message),
        Mutation::DeleteTag { name } => run(repo, &["tag", "-d", check_name(&name)?]).map(|_| ()),
        Mutation::DeleteRemoteTag { remote, name } => {
            let spec = format!(":refs/tags/{}", check_name(&name)?);
            run(repo, &["push", check_name(&remote)?, &spec]).map(|_| ())
        }
        Mutation::RestoreFile { rev, file } => {
            run(repo, &["--literal-pathspecs", "checkout", check_rev(&rev)?, "--", check_path(&file)?]).map(|_| ())
        }
        Mutation::Bisect { verb, rev } => bisect(repo, &verb, &rev),
        Mutation::WriteGitignore { text } => write_gitignore(repo, &text),
        Mutation::AppendIgnore {
            file,
            files,
            fake,
            directory,
        } => {
            let mut paths = files;
            if paths.is_empty() && !file.is_empty() {
                paths.push(file);
            }
            append_ignore(repo, &paths, fake, directory)
        }
        Mutation::WriteMailmap { text } => write_mailmap(repo, &text),
        Mutation::StageHunk { file, index, unstage, ignore_space } => stage_hunk(repo, &file, index, unstage, ignore_space),
        Mutation::StageLine {
            file,
            text,
            addition,
            at,
            unstage,
        } => stage_line(repo, &file, &text, addition, at, unstage),
        Mutation::StageLines { file, lines, unstage } => stage_lines(repo, &file, &lines, unstage),
        Mutation::StagePaths { files, unstage } => {
            if unstage {
                unstage_paths(repo, &files)
            } else {
                stage_paths(repo, &files)
            }
        }
        Mutation::DiscardHunk { file, index, ignore_space } => discard_hunk(repo, &file, index, ignore_space),
        Mutation::DiscardLine {
            file,
            text,
            addition,
            at,
        } => discard_line(repo, &file, &text, addition, at),
        Mutation::DiscardLines { file, lines } => discard_lines(repo, &file, &lines),
        Mutation::Resolve { file, side, text } => resolve_conflict(repo, &file, &side, &text),
        Mutation::RenameBranch { name, to } => {
            run(repo, &["branch", "-m", check_name(&name)?, check_name(&to)?]).map(|_| ())
        }
        Mutation::SetUpstream { branch, upstream } => set_upstream(repo, &branch, &upstream),
        Mutation::DeleteRemoteBranch { remote, branch } => {
            run(repo, &["push", check_name(&remote)?, "--delete", check_name(&branch)?]).map(|_| ())
        }
        Mutation::CheckoutRemote { remote, branch } => with_autostash(repo, "checkout", || checkout_remote(repo, &remote, &branch)),
        Mutation::PushRef {
            remote,
            branch,
            force_with_lease,
            force,
            remote_branch,
            set_upstream,
            tags,
        } => push_ref(
            repo,
            PushRefOptions {
                remote: &remote,
                branch: &branch,
                remote_branch: &remote_branch,
                force_with_lease,
                force,
                set_upstream,
                tags,
            },
        ),
        Mutation::PullRef {
            remote,
            branch,
            rebase,
            autostash,
        } => pull_ref(repo, &remote, &branch, rebase, autostash),
        Mutation::Discard { file } => discard(repo, &file),
        Mutation::DiscardPaths { files } => discard_paths(repo, &files),
        Mutation::Delete { file } => delete_path(repo, &file),
        Mutation::DeletePaths { files } => delete_paths(repo, &files),
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
        Mutation::GitFlowStart { name, kind } => with_autostash(repo, "git flow", || git_flow_start(repo, &name, &kind)),
        Mutation::GitFlowFinish { name, kind } => with_autostash(repo, "git flow", || git_flow_finish(repo, &name, &kind)),
        Mutation::Squash { from, to, summary } => with_autostash(repo, "squash", || squash(repo, &from, &to, &summary)),
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
    force: bool,
) -> Result<(), Error> {
    let remote = optional_name(remote)?;
    let mut args = vec!["push"];
    if set_upstream {
        args.push("--set-upstream");
    }
    if tags {
        args.push("--tags");
    }
    if force {
        args.push("--force");
    } else if force_with_lease {
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
    if end <= start + 1 {
        return None;
    }
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

fn append_ignore(repo: &Path, files: &[String], fake: bool, directory: bool) -> Result<(), Error> {
    if files.is_empty() {
        return Err(Error::Git("ignore needs a path".into()));
    }
    let name = if fake { ".fakeignore" } else { ".gitignore" };
    let mut patterns = Vec::with_capacity(files.len());
    for file in files {
        let pattern = ignore_pattern(file, directory)?;
        if !patterns.iter().any(|have| have == &pattern) {
            patterns.push(pattern);
        }
    }
    let path = repo.join(name);
    let mut text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(source) => {
            return Err(Error::Read {
                path: name.into(),
                source,
            })
        }
    };
    if text.contains('\0') {
        return Err(Error::Git(format!("{name} contains a null")));
    }
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut changed = false;
    for pattern in patterns {
        if ignore_has_pattern(&text, &pattern) {
            continue;
        }
        if !text.is_empty() && !text.ends_with('\n') {
            text.push_str(newline);
        }
        text.push_str(&pattern);
        text.push_str(newline);
        changed = true;
    }
    if !changed {
        return Ok(());
    }
    std::fs::write(&path, text).map_err(|source| Error::Read {
        path: name.into(),
        source,
    })
}

fn ignore_pattern(file: &str, directory: bool) -> Result<String, Error> {
    let normalized = file.replace('\\', "/");
    let file = check_path(&normalized)?;
    let mut pattern = String::from("/");
    for ch in file.chars() {
        if matches!(ch, '*' | '?' | '[' | ']' | '\\') {
            pattern.push('\\');
        }
        pattern.push(ch);
    }
    if directory {
        pattern.push('/');
    }
    Ok(pattern)
}

fn ignore_has_pattern(text: &str, pattern: &str) -> bool {
    let bare = pattern.trim_start_matches('/');
    text.lines().any(|line| {
        let line = line.trim();
        !line.is_empty() && !line.starts_with('#') && (line == pattern || line == bare)
    })
}

fn write_mailmap(repo: &Path, text: &str) -> Result<(), Error> {
    if text.contains('\0') {
        return Err(Error::Git("mailmap contains a null".into()));
    }
    std::fs::write(repo.join(".mailmap"), text).map_err(|source| Error::Read {
        path: ".mailmap".into(),
        source,
    })
}

fn stash_paths(repo: &Path, files: &[String], message: &str, include_untracked: bool) -> Result<(), Error> {
    if files.is_empty() {
        return Err(Error::Git("stash needs a path".into()));
    }
    let message = message.trim();
    if !message.is_empty() && (message.starts_with('-') || message.contains(['\n', '\r'])) {
        return Err(Error::Rev(message.to_string()));
    }
    for file in files {
        check_path(file)?;
    }
    // `git stash push -- <untracked path>` fails with "did not match any file(s) known
    // to git" unless --include-untracked is given, so a selection that holds an
    // untracked path always asks for it.
    let include_untracked = include_untracked || has_untracked(repo, files);
    let mut prefix: Vec<&str> = vec!["stash", "push"];
    if include_untracked {
        prefix.push("--include-untracked");
    }
    if !message.is_empty() {
        prefix.push("-m");
        prefix.push(message);
    }
    run_paths(repo, &prefix, files).map(|_| ())
}

/// True when any of `files` is not tracked yet (and not ignored).
fn has_untracked(repo: &Path, files: &[String]) -> bool {
    if files.is_empty() {
        return false;
    }
    if files.len() > 40 {
        // `ls-files` has no --pathspec-from-file, so ask for the untracked set once.
        let output = run(repo, &["--literal-pathspecs", "ls-files", "--others", "--exclude-standard", "-z"]);
        let Ok(output) = output else { return false };
        let wanted: HashSet<String> = files.iter().map(|path| slash(path)).collect();
        return output
            .stdout
            .split(|byte| *byte == 0)
            .any(|path| !path.is_empty() && wanted.contains(&String::from_utf8_lossy(path).into_owned()));
    }
    let mut args = vec!["--literal-pathspecs", "ls-files", "--others", "--exclude-standard", "--"];
    args.extend(files.iter().map(String::as_str));
    run(repo, &args)
        .map(|output| output.stdout.iter().any(|byte| !byte.is_ascii_whitespace()))
        .unwrap_or(false)
}

fn create_branch(repo: &Path, name: &str, start: Option<&str>) -> Result<(), Error> {
    let name = check_name(name)?;
    if let Some(start) = start {
        run(repo, &["branch", name, check_rev(start)?]).map(|_| ())
    } else {
        run(repo, &["branch", name]).map(|_| ())
    }
}

fn merge(
    repo: &Path,
    name: &str,
    squash: bool,
    no_ff: bool,
    autostash: bool,
    summary: &str,
    description: &str,
) -> Result<(), Error> {
    let name = check_rev(name)?;
    let summary = summary.trim();
    let description = description.trim();
    let mut message = String::new();
    if !summary.is_empty() || !description.is_empty() {
        if summary.is_empty() {
            message.push_str("Merge ");
            message.push_str(name);
        } else {
            message.push_str(summary);
        }
        if !description.is_empty() {
            message.push_str("\n\n");
            message.push_str(description);
        }
    }
    let stored = if message.is_empty() { None } else { Some(message_file(&message)?) };
    let mut args = vec!["merge".to_string()];
    if autostash {
        args.push("--autostash".into());
    }
    if squash {
        args.push("--squash".into());
    } else if no_ff {
        args.push("--no-ff".into());
    }
    if let Some(file) = stored.as_ref().filter(|_| !squash) {
        args.push("-F".into());
        args.push(file.0.display().to_string());
    } else if !squash {
        args.push("--no-edit".into());
    }
    args.push(name.to_string());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    with_editor(repo, &refs)?;
    if squash && !has_unmerged(repo) {
        let staged = run(repo, &["diff", "--cached", "--name-only"])?;
        if staged.stdout.iter().any(|byte| !byte.is_ascii_whitespace()) {
            let fallback = if message.is_empty() { Some(message_file(&format!("Squash {name}"))?) } else { None };
            let path = fallback.as_ref().or(stored.as_ref()).map(|file| file.0.as_path());
            if let Some(path) = path {
                commit_from_file(repo, path)?;
            }
        }
    }
    Ok(())
}

struct MessageFile(PathBuf);

impl Drop for MessageFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn message_file(text: &str) -> Result<MessageFile, Error> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let path = std::env::temp_dir().join(format!("awegit-merge-{}-{nanos}.txt", std::process::id()));
    std::fs::write(&path, text).map_err(|source| Error::Read { path: path.display().to_string(), source })?;
    Ok(MessageFile(path))
}

fn commit_from_file(repo: &Path, path: &Path) -> Result<(), Error> {
    let mut args = commit_config(repo);
    args.push("commit".into());
    args.push("-F".into());
    args.push(path.display().to_string());
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    if has_local_author(repo) {
        run_repo_author(repo, &borrowed).map(|_| ())
    } else {
        run(repo, &borrowed).map(|_| ())
    }
}

/// Files the merge would bring in, plus the branch that would receive them.
pub fn merge_preview(repo: &Path, name: &str) -> Result<MergePreview, Error> {
    let name = check_rev(name)?;
    Ok(MergePreview {
        into: current_branch_name(repo),
        files: changed_names(repo, name)?,
    })
}

fn current_branch_name(repo: &Path) -> String {
    match run(repo, &["branch", "--show-current"]) {
        Ok(output) => {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if name.is_empty() { "HEAD".into() } else { name }
        }
        Err(_) => "HEAD".into(),
    }
}

fn changed_names(repo: &Path, name: &str) -> Result<Vec<MergeFile>, Error> {
    let triple = format!("HEAD...{name}");
    let output = match run(repo, &["diff", "--name-status", &triple]) {
        Ok(output) => output,
        Err(_) => run(repo, &["diff", "--name-status", &format!("HEAD..{name}")])?,
    };
    Ok(parse_name_status(&output.stdout))
}

fn parse_name_status(stdout: &[u8]) -> Vec<MergeFile> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let mut parts = line.split('\t');
            let status = parts.next()?.trim();
            let path = parts.last()?.trim();
            if status.is_empty() || path.is_empty() {
                return None;
            }
            Some(MergeFile {
                status: status.chars().next()?.to_string(),
                path: path.to_string(),
            })
        })
        .collect()
}

/// One changed path in a merge preview.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeFile {
    pub status: String,
    pub path: String,
}

/// Where a merge lands, and the paths it would change.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePreview {
    pub into: String,
    pub files: Vec<MergeFile>,
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

fn abort(repo: &Path) -> Result<(), Error> {
    if in_progress(repo)?.is_some() {
        return flow(repo, "abort");
    }
    // A stash apply leaves unmerged paths without MERGE_HEAD. Reset the conflict
    // back to HEAD and keep the stash entry.
    if has_unmerged(repo) {
        return run(repo, &["reset", "--merge"]).map(|_| ());
    }
    Err(Error::Git("nothing in progress".into()))
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
                convention: false,
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
        run(repo, &["--literal-pathspecs", "restore", "--worktree", "--source=HEAD", "--", file]).map(|_| ())
    } else {
        remove_worktree_file(repo, file)
    }
}

/// Restore tracked paths from `HEAD`, drop index-only additions, and delete the rest.
/// One `ls-files` and one `cat-file` cover the whole list. Git itself is called per chunk.
fn discard_paths(repo: &Path, files: &[String]) -> Result<(), Error> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    for file in files {
        let path = check_path(file)?.to_string();
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        return Ok(());
    }
    let indexed = paths_in_index(repo, &paths)?;
    let headed = paths_in_head(repo, &paths)?;
    let mut restore = Vec::new();
    let mut remove_index = Vec::new();
    let mut remove_disk = Vec::new();
    for path in &paths {
        if headed.contains(path) {
            restore.push(path.clone());
        } else if indexed.contains(&slash(path)) {
            remove_index.push(path.clone());
        } else {
            remove_disk.push(path.clone());
        }
    }
    git_arg_paths(repo, &["restore", "--worktree", "--source=HEAD", "--"], &restore)?;
    git_arg_paths(repo, &["rm", "-f", "--"], &remove_index)?;
    for path in remove_disk {
        match remove_worktree_file(repo, &path) {
            Ok(()) => {}
            Err(Error::Read { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn slash(path: &str) -> String {
    path.replace('\\', "/")
}

fn paths_in_index(repo: &Path, paths: &[String]) -> Result<HashSet<String>, Error> {
    let wanted: HashSet<String> = paths.iter().map(|path| slash(path)).collect();
    let output = if paths.len() > 200 {
        // One index listing beats hundreds of `ls-files` processes on a large selection.
        run(repo, &["--literal-pathspecs", "ls-files", "-z"])?
    } else {
        let mut owned: Vec<String> = vec!["--literal-pathspecs".into(), "ls-files".into(), "-z".into(), "--".into()];
        for path in paths {
            owned.push(check_path(path)?.to_string());
        }
        let args: Vec<&str> = owned.iter().map(String::as_str).collect();
        run(repo, &args)?
    };
    let mut found = HashSet::new();
    for path in output.stdout.split(|byte| *byte == 0) {
        if path.is_empty() {
            continue;
        }
        let path = slash(&String::from_utf8_lossy(path));
        if wanted.contains(&path) {
            found.insert(path);
        }
    }
    Ok(found)
}

fn paths_in_head(repo: &Path, paths: &[String]) -> Result<HashSet<String>, Error> {
    // Paths cannot contain a newline here (check_path), so one batch process is safe.
    let mut body = Vec::new();
    for path in paths {
        body.extend_from_slice(b"HEAD:");
        body.extend_from_slice(path.as_bytes());
        body.push(b'\n');
    }
    let output = match run_stdin(repo, &["cat-file", "--batch-check"], &body) {
        Ok(output) => output,
        Err(_) => return Ok(HashSet::new()),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut found = HashSet::new();
    for (path, line) in paths.iter().zip(text.lines()) {
        let kind = line.split_whitespace().nth(1);
        if kind == Some("blob") || kind == Some("commit") {
            found.insert(path.clone());
        }
    }
    Ok(found)
}

fn git_arg_paths(repo: &Path, prefix: &[&str], paths: &[String]) -> Result<(), Error> {
    if paths.is_empty() {
        return Ok(());
    }
    run_paths(repo, prefix, paths).map(|_| ())
}

fn delete_paths(repo: &Path, files: &[String]) -> Result<(), Error> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    for file in files {
        let path = check_path(file)?.to_string();
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        return Ok(());
    }
    let indexed = paths_in_index(repo, &paths)?;
    let mut tracked = Vec::new();
    let mut untracked = Vec::new();
    for path in &paths {
        if indexed.contains(&slash(path)) {
            tracked.push(path.clone());
        } else {
            untracked.push(path.clone());
        }
    }
    git_arg_paths(repo, &["rm", "-f"], &tracked)?;
    for path in untracked {
        match remove_worktree_file(repo, &path) {
            Ok(()) => {}
            Err(Error::Read { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn delete_path(repo: &Path, file: &str) -> Result<(), Error> {
    delete_paths(repo, &[file.to_string()])
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
        "support" => "support",
        _ => "feature",
    }
}

fn git_flow_start(repo: &Path, name: &str, kind: &str) -> Result<(), Error> {
    let kind = flow_kind(kind);
    let (prefix_key, default_prefix) = flow_prefix(kind);
    let prefix = config_get(repo, prefix_key, default_prefix);
    let branch = prefixed_branch(&prefix, name)?;
    let base_name = if kind == "hotfix" || kind == "support" {
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
    let (prefix_key, default_prefix) = flow_prefix(kind);
    let prefix = config_get(repo, prefix_key, default_prefix);
    let branch = prefixed_branch(&prefix, name)?;
    if kind == "support" {
        let master = config_get(repo, "gitflow.branch.master", "main");
        run(repo, &["checkout", &master])?;
        with_editor(repo, &["merge", "--no-ff", "--no-edit", &branch])?;
        return run(repo, &["branch", "-d", &branch]).map(|_| ());
    }
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

fn flow_prefix(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "release" => ("gitflow.prefix.release", "release/"),
        "hotfix" => ("gitflow.prefix.hotfix", "hotfix/"),
        "support" => ("gitflow.prefix.support", "support/"),
        _ => ("gitflow.prefix.feature", "feature/"),
    }
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
            convention: false,
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

fn clone_repo(url: &str, destination: &str) -> Result<(), Error> {
    let url = check_url(url)?;
    let destination = check_external(destination)?;
    let destination_path = resolve_clone_destination(url, destination)?;
    let parent = destination_path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|source| Error::Read {
        path: parent.display().to_string(),
        source,
    })?;
    let cwd = if parent.as_os_str().is_empty() {
        std::env::current_dir().map_err(Error::GitMissing)?
    } else {
        parent.to_path_buf()
    };
    let dest = destination_path.display().to_string();
    let reported = PathBuf::from(destination);
    surface(with_operation_repo(&reported, || run(&cwd, &["clone", url, &dest]).map(|_| ())))
}

/// If `destination` is an existing folder that already has files, clone into
/// `<destination>/<repo name>` instead of asking Git to reuse that folder.
fn resolve_clone_destination(url: &str, destination: &str) -> Result<PathBuf, Error> {
    let dest = PathBuf::from(destination);
    if !dest.exists() {
        return Ok(dest);
    }
    if dest.is_file() {
        return Err(Error::Git(format!("destination path '{}' is not a directory", dest.display())));
    }
    let nonempty = std::fs::read_dir(&dest)
        .map_err(|source| Error::Read {
            path: dest.display().to_string(),
            source,
        })?
        .next()
        .is_some();
    if !nonempty {
        return Ok(dest);
    }
    let name = repo_folder_name(url);
    let current = dest.file_name().and_then(|item| item.to_str()).unwrap_or("");
    if name.is_empty() || current == name {
        return Err(Error::Git(format!(
            "destination path '{}' already exists and is not an empty directory",
            dest.display()
        )));
    }
    let nested = dest.join(&name);
    if nested.is_file()
        || (nested.is_dir()
            && std::fs::read_dir(&nested)
                .map_err(|source| Error::Read {
                    path: nested.display().to_string(),
                    source,
                })?
                .next()
                .is_some())
    {
        return Err(Error::Git(format!(
            "destination path '{}' already exists and is not an empty directory",
            nested.display()
        )));
    }
    Ok(nested)
}

fn repo_folder_name(url: &str) -> String {
    let text = url.trim();
    if text.is_empty() {
        return String::new();
    }
    if let Some(name) = http_repo_name(text) {
        return sanitize_folder(&name);
    }
    let value = text.trim_end_matches(['/', '\\']);
    let value = value.split(['?', '#']).next().unwrap_or(value);
    let value = value.strip_suffix(".git").unwrap_or(value);
    let name = value.rsplit(['/', '\\', ':']).next().unwrap_or(value);
    sanitize_folder(name)
}

fn http_repo_name(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let path = rest.split(['?', '#']).next().unwrap_or(rest);
    let path = path.split_once('/').map(|(_, path)| path).unwrap_or("");
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if let Some(index) = parts.iter().rposition(|part| *part == "_git") {
        return parts.get(index + 1).map(|name| (*name).trim_end_matches(".git").to_string());
    }
    let noise = ["-", "tree", "blob", "commit", "commits", "pull", "pulls", "issues", "compare", "src"];
    let end = parts
        .iter()
        .enumerate()
        .find(|(index, part)| *index > 0 && noise.contains(part))
        .map(|(index, _)| index)
        .unwrap_or(parts.len());
    parts.get(..end).and_then(|kept| kept.last()).map(|name| (*name).trim_end_matches(".git").to_string())
}

fn sanitize_folder(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\' | '/') || ch.is_control() {
            continue;
        }
        out.push(ch);
    }
    let trimmed = out.trim().trim_end_matches(['.', ' ']);
    let lower = trimmed.to_ascii_lowercase();
    let reserved = matches!(lower.as_str(), "con" | "prn" | "aux" | "nul")
        || (lower.len() == 4 && (lower.starts_with("com") || lower.starts_with("lpt")) && lower.as_bytes().get(3).is_some_and(|byte| byte.is_ascii_digit()));
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." || reserved {
        String::new()
    } else {
        trimmed.to_string()
    }
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
        crate::cli::hide_console(&mut process);
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

struct PushRefOptions<'a> {
    remote: &'a str,
    branch: &'a str,
    remote_branch: &'a str,
    force_with_lease: bool,
    force: bool,
    set_upstream: bool,
    tags: bool,
}

fn push_ref(repo: &Path, options: PushRefOptions<'_>) -> Result<(), Error> {
    let remote = check_name(options.remote)?;
    let branch = check_name(options.branch)?;
    let remote_branch = options.remote_branch.trim();
    let spec = if remote_branch.is_empty() {
        format!("HEAD:{branch}")
    } else {
        format!("{branch}:{}", check_name(remote_branch)?)
    };
    let mut args = vec!["push"];
    if options.set_upstream {
        args.push("--set-upstream");
    }
    if options.tags {
        args.push("--tags");
    }
    let (force, force_with_lease) = (options.force, options.force_with_lease);
    if force {
        args.push("--force");
    } else if force_with_lease {
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
    pub hot: Vec<HotPath>,
}

/// A path that shows up often in recent history. Used by the statistics treemap.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotPath {
    pub path: String,
    pub count: u32,
}

pub fn repo_facts(repo: &Path) -> Result<RepoFacts, Error> {
    std::thread::scope(|scope| {
        let ignored = scope.spawn(|| lines_of(repo, &["ls-files", "-o", "-i", "--exclude-standard"], 500));
        let activity = scope.spawn(|| activity(repo));
        let lfs = scope.spawn(|| text_of(repo, &["lfs", "ls-files"]));
        let hot = scope.spawn(|| hot_paths(repo));
        let brief = repo_facts_brief(repo)?;
        let (weekday, hour) = activity.join().unwrap_or_default();
        Ok(RepoFacts {
            ignored: ignored.join().unwrap_or_default(),
            weekday,
            hour,
            lfs: lfs.join().unwrap_or_default(),
            hot: hot.join().unwrap_or_default(),
            ..brief
        })
    })
}

/// Bisect state, the commit template, and the `HEAD` message. The lists and statistics stay empty.
pub fn repo_facts_brief(repo: &Path) -> Result<RepoFacts, Error> {
    let bisect_log = text_of(repo, &["bisect", "log"]);
    Ok(RepoFacts {
        ignored: Vec::new(),
        bisect_active: bisect_log.contains("git bisect"),
        bisect_log,
        template: commit_template(repo),
        head_message: text_of(repo, &["log", "-1", "--format=%B"]),
        weekday: Vec::new(),
        hour: Vec::new(),
        lfs: String::new(),
        hot: Vec::new(),
    })
}

fn hot_paths(repo: &Path) -> Vec<HotPath> {
    let mut counts = std::collections::BTreeMap::<String, u32>::new();
    for line in text_of(repo, &["log", "-n", "400", "--name-only", "--pretty=format:"]).lines() {
        let path = line.trim();
        if path.is_empty() {
            continue;
        }
        *counts.entry(path.to_string()).or_default() += 1;
    }
    let mut hot: Vec<HotPath> = counts.into_iter().map(|(path, count)| HotPath { path, count }).collect();
    hot.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.path.cmp(&b.path)));
    hot.truncate(24);
    hot
}

/// `-S` or `-G` search. The needle is one argv token, not a shell string.
pub fn search_commits(repo: &Path, kind: &str, needle: &str) -> Result<Vec<SearchHit>, Error> {
    let flag = if kind == "G" || kind == "g" { "-G" } else { "-S" };
    let needle = needle.trim();
    if needle.is_empty() || needle.starts_with('-') || needle.contains(['\n', '\r', '\0']) {
        return Err(Error::Rev(needle.to_string()));
    }
    let (output, _) = crate::cli::with_read(|| crate::cli::run_capped(repo, &["log", "-n", "80", "--format=%H%x09%h%x09%s", flag, needle], 1_000_000))?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            Some(SearchHit {
                id: parts.next()?.to_string(),
                short_id: parts.next()?.to_string(),
                summary: parts.next().unwrap_or("").to_string(),
            })
        })
        .collect())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub id: String,
    pub short_id: String,
    pub summary: String,
}

/// `git merge-tree` for a cherry-pick. A conflict still returns the name list.
pub fn cherry_preview(repo: &Path, rev: &str) -> Result<String, Error> {
    let rev = check_rev(rev)?;
    let (output, _) = crate::cli::with_read(|| {
        crate::cli::run_capped(repo, &["merge-tree", "--write-tree", "--name-only", "HEAD", rev], 1_000_000)
    })?;
    let mut text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !err.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&err);
        }
    }
    Ok(text)
}

/// Local branches already contained in `HEAD`, excluding the current one.
pub fn merged_branches(repo: &Path) -> Result<Vec<String>, Error> {
    let (output, _) = crate::cli::with_read(|| crate::cli::run_capped(repo, &["branch", "--merged", "HEAD"], 1_000_000))?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('*'))
        .map(|line| line.trim_start_matches("+ ").to_string())
        .collect())
}

/// Commits whose patch introduces or removes `text` in one file.
pub fn pick_history(repo: &Path, file: &str, text: &str) -> Result<String, Error> {
    let file = check_path(file)?;
    let text = text.trim();
    if text.is_empty() || text.starts_with('-') || text.contains(['\n', '\r', '\0']) {
        return Err(Error::Git("pick a single line".into()));
    }
    let (output, _) = crate::cli::with_read(|| {
        crate::cli::run_capped(repo, &["--literal-pathspecs", "log", "-n", "40", "--format=%h %s", "-S", text, "--", file], 512_000)
    })?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn lines_of(repo: &Path, args: &[&str], limit: usize) -> Vec<String> {
    text_of(repo, args).lines().map(str::trim).filter(|line| !line.is_empty()).take(limit).map(str::to_string).collect()
}

fn text_of(repo: &Path, args: &[&str]) -> String {
    crate::cli::with_read(|| crate::cli::run_capped(repo, args, 4_000_000))
        .ok()
        .map(|(output, _)| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

/// Same text as `commit-template.txt`. Used when git has no `commit.template`.
const BUILTIN_COMMIT_TEMPLATE: &str = "\
feat: \n\
\n\
# 可用类型：\n\
# feat     新增功能\n\
# fix      修复 bug\n\
# docs     仅修改文档/注释/配置说明\n\
# art      美术资源新增或修改\n\
# style    代码格式调整（不影响逻辑）\n\
# refactor 重构（非新增功能、非修复 bug）\n\
# test     测试相关改动\n\
# chore    构建、工具、配置、流程调整\n\
# perf     性能优化\n\
# revert   回滚之前的提交\n";

fn commit_template(repo: &Path) -> String {
    let configured = text_of(repo, &["config", "--get", "commit.template"]);
    let path = configured.trim();
    if !path.is_empty() {
        let full = if Path::new(path).is_absolute() { PathBuf::from(path) } else { repo.join(path) };
        if let Ok(text) = std::fs::read_to_string(full) {
            if !text.trim().is_empty() {
                return text;
            }
        }
    }
    BUILTIN_COMMIT_TEMPLATE.to_string()
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
pub fn scan_repositories(root: &Path, depth_limit: usize) -> Result<Vec<String>, Error> {
    if !root.is_dir() {
        return Err(Error::Git(format!("{} is not a directory", root.display())));
    }
    let limit = if depth_limit == 0 { 5 } else { depth_limit };
    let mut found = Vec::new();
    scan_dir(root, 0, limit, &mut found);
    Ok(found)
}

fn scan_dir(dir: &Path, depth: usize, limit: usize, found: &mut Vec<String>) {
    if found.len() >= 300 || depth > limit {
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
        scan_dir(&entry.path(), depth + 1, limit, found);
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

#[cfg(test)]
mod clone_destination_tests {
    use super::{repo_folder_name, resolve_clone_destination};

    #[test]
    fn names_come_from_the_remote() {
        assert_eq!(repo_folder_name("https://github.com/org/Demo.git"), "Demo");
        assert_eq!(repo_folder_name("git@github.com:org/Demo.git"), "Demo");
        assert_eq!(repo_folder_name("https://github.com/org/Demo/tree/main"), "Demo");
        assert_eq!(repo_folder_name("https://dev.azure.com/org/project/_git/Demo"), "Demo");
        assert_eq!(repo_folder_name("ssh://git@github.com/org/Demo.git"), "Demo");
    }

    #[test]
    fn existing_root_gets_a_repo_subdirectory() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("awegit-clone-{nanos}"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("keep.txt"), "x").unwrap();
        let dest = resolve_clone_destination("https://github.com/org/Demo.git", &root.display().to_string()).unwrap();
        assert_eq!(dest, root.join("Demo"));
        std::fs::create_dir_all(root.join("Demo")).unwrap();
        std::fs::write(root.join("Demo").join("a.txt"), "x").unwrap();
        let err = resolve_clone_destination("https://github.com/org/Demo.git", &root.join("Demo").display().to_string()).unwrap_err();
        assert!(err.to_string().contains("already exists"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
