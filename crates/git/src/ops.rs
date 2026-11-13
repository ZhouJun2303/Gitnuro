//! Writes that must match the installed Git: network, history, and worktree changes.
//!
//! Nothing here writes the global gitconfig. Revisions and names are checked before
//! they are placed on the command line. Cancellation of a running command is the
//! caller's job; these functions wait for `git` to exit.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::autostash;
use crate::cli::{
    check_name, check_path, check_rev, commit_config, has_local_author, run, run_env,
    run_env_committer_only, run_paths, run_repo_author, run_stdin, with_operation_repo, AuthorMode,
};
use crate::model::in_progress;
use crate::{
    commit, discard_hunk, discard_line, discard_lines, resolve_conflict, resolve_conflicts,
    stage_hunk, stage_line, stage_lines, stage_paths, unstage_paths, CommitRequest, Error,
    InProgress, LineEdit,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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
        #[serde(default)]
        no_commit: bool,
        #[serde(default)]
        message: String,
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
    /// Resolve many unmerged paths to one side with one index write.
    ResolvePaths {
        files: Vec<String>,
        side: String,
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
    LfsLock {
        path: String,
    },
    LfsUnlock {
        path: String,
    },
    /// Replace the repository's hidden branch list. An empty list shows every branch.
    SetHidden {
        names: Vec<String>,
    },
    /// Open branch-group names for this repository. An empty list opens every group.
    SetExpanded {
        names: Vec<String>,
    },
    /// Write `signoff.enabled` and `signoff.format` in `.git/awegit`.
    SetSignOff {
        enabled: bool,
        format: String,
    },
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

/// A blocked write's retry uses the same durable autostash as ordinary flows.
/// Never leave the restoration identity in a webview's memory alone.
pub fn perform_stashed(repo: &Path, mut mutation: Mutation) -> Result<String, Error> {
    ensure_no_paused_flow(repo, &mutation)?;
    match &mut mutation {
        Mutation::Pull { autostash, .. } | Mutation::PullRef { autostash, .. } |
        Mutation::Merge { autostash, .. } | Mutation::Rebase { autostash, .. } |
        Mutation::RebaseInteractive { autostash, .. } => { *autostash = true; perform(repo, mutation) }
        Mutation::StashApply { name } | Mutation::StashPop { name } => {
            // Creating our stash renumbers stash@{n}.
            *name = crate::flow::commit(repo, if name.is_empty() { "refs/stash" } else { name })
                .ok_or_else(|| Error::Rev(name.clone()))?.id;
            let mut output = String::new();
            with_autostash(repo, "retry stash", || { output = perform(repo, mutation)?; Ok(()) })?;
            Ok(output)
        }
        Mutation::ApplyPatch { .. } => {
            let mut output = String::new();
            with_autostash(repo, "retry patch", || { output = perform(repo, mutation)?; Ok(()) })?;
            Ok(output)
        }
        _ => perform(repo, mutation),
    }
}

/// Full object ids make stash messages hard to scan; the UI already shows them.
fn short_rev(rev: &str) -> String {
    let rev = rev.trim();
    if matches!(rev.len(), 40 | 64) && rev.chars().all(|c| c.is_ascii_hexdigit()) {
        rev[..7].to_string()
    } else {
        rev.to_string()
    }
}

/// Operations that can leave unmerged paths behind and have no stash of their own.
/// A pull, merge, or rebase with autostash already keeps one visible entry, so it
/// must never get a second backup.
fn conflict_label(mutation: &Mutation) -> Option<String> {
    match mutation {
        Mutation::Pull { rebase, autostash } if !*autostash => {
            Some(if *rebase { "pull --rebase" } else { "pull" }.to_string())
        }
        Mutation::PullRef {
            remote,
            branch,
            autostash,
            ..
        } if !*autostash => Some(format!("pull {remote}/{branch}")),
        Mutation::Merge {
            name, autostash, ..
        } if !*autostash => Some(format!("merge {name}")),
        Mutation::Rebase { onto, autostash } if !*autostash => {
            Some(format!("rebase onto {}", short_rev(onto)))
        }
        Mutation::RebaseInteractive {
            onto, autostash, ..
        } if !*autostash => Some(format!("rebase onto {}", short_rev(onto))),
        Mutation::Revert { rev, no_commit: true, .. } => {
            Some(format!("revert {}", short_rev(rev)))
        }
        _ => None,
    }
}

/// Snapshot the tracked local changes before `action`. If it ends with unmerged paths,
/// keep the snapshot as a stash entry so the pre-conflict work is never lost. Without
/// a conflict the snapshot is dropped, and a failed snapshot never blocks the action.
fn with_conflict_backup(
    repo: &Path,
    label: &str,
    action: impl FnOnce() -> Result<(), Error>,
) -> Result<(), Error> {
    let snapshot = snapshot_local_changes(repo);
    let result = action();
    if let Some(hash) = snapshot {
        if autostash::has_unmerged(repo) {
            let message = autostash::stash_message(
                "AweGit conflict backup",
                label,
                autostash::stash_files(repo, &hash),
            );
            let _ = run(repo, &["stash", "store", "-m", &message, &hash]);
        }
    }
    result
}

/// Keep local work out of the way while `action` rewrites history or the work
/// tree, then put it back once the operation settles. A paused operation keeps its
/// one visible entry until a later continue, skip, or abort finishes it.
fn with_autostash(
    repo: &Path,
    label: &str,
    action: impl FnOnce() -> Result<(), Error>,
) -> Result<(), Error> {
    autostash::settle_before(repo)?;
    let pending = autostash::push(repo, label)?;
    let result = action();
    if pending.is_some() {
        match autostash::finish_if_settled(repo) {
            Ok(_) => {}
            Err(error) if result.is_ok() => return Err(error),
            Err(_) => {}
        }
    }
    result
}

/// Continue, skip, and abort only finish work another operation started. They never
/// create a stash; they put the one it kept back when the flow is over.
fn settle_after_flow(repo: &Path, action: impl FnOnce() -> Result<(), Error>) -> Result<(), Error> {
    let result = action();
    clear_settled_revert(repo)?;
    match (result, autostash::finish_if_settled(repo)) {
        (Ok(()), Ok(_)) => Ok(()),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), _) => Err(error),
    }
}

/// Give one stash commit a new reflog message.
///
/// `git stash store` of the commit that is already `refs/stash` writes nothing:
/// Git skips the reflog entry when the tip does not change, so an `autostash`
/// subject stays in place. Drop that entry, then store the same commit. The hold
/// ref keeps it reachable if the process stops between those two commands.
fn replace_stash_entry(repo: &Path, hash: &str, index: usize, message: &str) -> Result<(), Error> {
    const HOLD: &str = "refs/awegit/stash-rename";
    run(repo, &["update-ref", HOLD, hash])?;
    if let Err(error) = run(repo, &["stash", "drop", &format!("stash@{{{index}}}")]) {
        let _ = run(repo, &["update-ref", "-d", HOLD]);
        return Err(error);
    }
    let stored = run(repo, &["stash", "store", "-m", message, hash]);
    if stored.is_err() {
        let _ = run(
            repo,
            &[
                "update-ref",
                "-m",
                message,
                "--create-reflog",
                "refs/stash",
                hash,
            ],
        );
    }
    let _ = run(repo, &["update-ref", "-d", HOLD]);
    stored.map(|_| ())
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

fn apply(repo: &Path, mutation: Mutation) -> Result<(), Error> {
    ensure_no_paused_flow(repo, &mutation)?;
    match conflict_label(&mutation) {
        Some(label) => with_conflict_backup(repo, &label, || apply_unguarded(repo, mutation)),
        None => apply_unguarded(repo, mutation),
    }
}

/// A paused rebase, merge, cherry-pick, or revert owns the index and `HEAD`.
/// Git still lets a merge fast-forward, a checkout switch branches, or a
/// cherry-pick add a commit on top of the paused state, so starting one of
/// those writes here would rewrite the wrong base. Continue, skip, or abort
/// ends the flow first. Commits that conclude a merge do not come through here.
fn ensure_no_paused_flow(repo: &Path, mutation: &Mutation) -> Result<(), Error> {
    if !starts_flow(mutation) {
        return Ok(());
    }
    let Some(state) = in_progress(repo)? else {
        if autostash::has_unmerged(repo) {
            return Err(Error::Git("resolve the current conflicts before starting another operation".into()));
        }
        return Ok(());
    };
    let name = match state {
        InProgress::Merge => "merge",
        InProgress::Rebase => "rebase",
        InProgress::CherryPick => "cherry-pick",
        InProgress::Revert => "revert",
    };
    Err(Error::Git(format!(
        "a {name} is in progress; continue or abort it before starting another operation"
    )))
}

fn starts_flow(mutation: &Mutation) -> bool {
    matches!(
        mutation,
        Mutation::Pull { .. }
            | Mutation::PullRef { .. }
            | Mutation::Merge { .. }
            | Mutation::Rebase { .. }
            | Mutation::RebaseInteractive { .. }
            | Mutation::CherryPick { .. }
            | Mutation::Revert { .. }
            | Mutation::Checkout { .. }
            | Mutation::CheckoutRemote { .. }
            | Mutation::Reset { .. }
            | Mutation::Reword { .. }
            | Mutation::Squash { .. }
            | Mutation::StashBranch { .. }
            | Mutation::StashApply { .. }
            | Mutation::StashPop { .. }
            | Mutation::GitFlowStart { .. }
            | Mutation::GitFlowFinish { .. }
    )
}

fn apply_unguarded(repo: &Path, mutation: Mutation) -> Result<(), Error> {
    match mutation {
        Mutation::Fetch {
            remote,
            prune,
            tags,
        } => fetch(repo, remote, prune, tags),
        Mutation::Pull { rebase, autostash } => {
            let label = if rebase { "pull --rebase" } else { "pull" };
            if autostash {
                with_autostash(repo, label, || pull(repo, rebase))
            } else {
                pull(repo, rebase)
            }
        }
        Mutation::Push {
            remote,
            set_upstream,
            tags,
            force_with_lease,
            force,
        } => push(repo, remote, set_upstream, tags, force_with_lease, force),
        Mutation::Stash {
            message,
            include_untracked,
        } => stash(repo, &message, include_untracked),
        Mutation::StashPop { name } => stash_pop(repo, &name),
        Mutation::StashApply { name } => {
            run(repo, &["stash", "apply", &stash_selector(repo, &name)?]).map(|_| ())
        }
        Mutation::StashDrop { name } => {
            run(repo, &["stash", "drop", &stash_selector(repo, &name)?]).map(|_| ())
        }
        Mutation::StashRename { name, message } => stash_rename(repo, &name, &message),
        Mutation::StashBranch { name, branch } => run(
            repo,
            &[
                "stash",
                "branch",
                check_name(&branch)?,
                &stash_selector(repo, &name)?,
            ],
        )
        .map(|_| ()),
        Mutation::StashPatch { .. } => Ok(()),
        Mutation::StashPaths {
            files,
            message,
            include_untracked,
        } => stash_paths(repo, &files, &message, include_untracked),
        Mutation::Checkout { name } => with_autostash(repo, &format!("checkout {name}"), || {
            run(repo, &["checkout", check_rev(&name)?]).map(|_| ())
        }),
        Mutation::CreateBranch { name, start } => create_branch(repo, &name, start.as_deref()),
        Mutation::DeleteBranch { name, force } => {
            if force {
                run(repo, &["branch", "-D", check_name(&name)?]).map(|_| ())
            } else {
                delete_branch(repo, &name)
            }
        }
        Mutation::Merge {
            name,
            squash,
            no_ff,
            autostash,
            summary,
            description,
        } => {
            let label = if squash {
                format!("squash-merge {name}")
            } else {
                format!("merge {name}")
            };
            if autostash {
                with_autostash(repo, &label, || {
                    merge(repo, &name, squash, no_ff, &summary, &description)
                })
            } else {
                merge(repo, &name, squash, no_ff, &summary, &description)
            }
        }
        Mutation::Rebase { onto, autostash } => {
            let label = format!("rebase onto {}", short_rev(&onto));
            if autostash {
                with_autostash(repo, &label, || rebase_onto(repo, &onto))
            } else {
                rebase_onto(repo, &onto)
            }
        }
        Mutation::RebaseInteractive {
            onto,
            drop,
            steps,
            autostash,
            update_refs,
        } => {
            let label = format!("rebase onto {}", short_rev(&onto));
            if autostash {
                with_autostash(repo, &label, || {
                    rebase_interactive(repo, &onto, &drop, &steps, update_refs)
                })
            } else {
                rebase_interactive(repo, &onto, &drop, &steps, update_refs)
            }
        }
        Mutation::Abort => settle_after_flow(repo, || abort(repo)),
        Mutation::Continue { message } => settle_after_flow(repo, || continue_with(repo, &message)),
        Mutation::Skip => settle_after_flow(repo, || flow(repo, "skip")),
        Mutation::Reset { rev, mode } => reset(repo, &rev, mode),
        Mutation::CherryPick { rev, record } => {
            with_autostash(repo, &format!("cherry-pick {}", short_rev(&rev)), || {
                let rev = check_rev(&rev)?;
                if record {
                    with_editor_committer_only(repo, &["cherry-pick", "-x", "--no-edit", rev])
                } else {
                    with_editor_committer_only(repo, &["cherry-pick", "--no-edit", rev])
                }
            })
        }
        Mutation::Revert {
            rev,
            no_commit,
            message,
        } => {
            if no_commit {
                // Applying with --no-commit preserves unrelated local/index changes.
                // Restoring an autostash with --index would overwrite this staged result.
                autostash::settle_before(repo)?;
                revert(repo, &rev, true, &message)
            } else {
                with_autostash(repo, &format!("revert {}", short_rev(&rev)), || {
                    revert(repo, &rev, false, &message)
                })
            }
        }
        Mutation::Reword { rev, summary } => {
            with_autostash(repo, &format!("reword {}", short_rev(&rev)), || {
                reword(repo, &rev, &summary)
            })
        }
        Mutation::Tag { name, rev, message } => tag(repo, &name, &rev, &message),
        Mutation::DeleteTag { name } => run(repo, &["tag", "-d", check_name(&name)?]).map(|_| ()),
        Mutation::DeleteRemoteTag { remote, name } => {
            let spec = format!(":refs/tags/{}", check_name(&name)?);
            run(repo, &["push", check_name(&remote)?, &spec]).map(|_| ())
        }
        Mutation::RestoreFile { rev, file } => run(
            repo,
            &[
                "--literal-pathspecs",
                "checkout",
                check_rev(&rev)?,
                "--",
                check_path(&file)?,
            ],
        )
        .map(|_| ()),
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
        Mutation::StageHunk {
            file,
            index,
            unstage,
            ignore_space,
        } => stage_hunk(repo, &file, index, unstage, ignore_space),
        Mutation::StageLine {
            file,
            text,
            addition,
            at,
            unstage,
        } => stage_line(repo, &file, &text, addition, at, unstage),
        Mutation::StageLines {
            file,
            lines,
            unstage,
        } => stage_lines(repo, &file, &lines, unstage),
        Mutation::StagePaths { files, unstage } => {
            if unstage {
                unstage_paths(repo, &files)
            } else {
                stage_paths(repo, &files)
            }
        }
        Mutation::DiscardHunk {
            file,
            index,
            ignore_space,
        } => discard_hunk(repo, &file, index, ignore_space),
        Mutation::DiscardLine {
            file,
            text,
            addition,
            at,
        } => discard_line(repo, &file, &text, addition, at),
        Mutation::DiscardLines { file, lines } => discard_lines(repo, &file, &lines),
        Mutation::Resolve { file, side, text } => resolve_conflict(repo, &file, &side, &text),
        Mutation::ResolvePaths { files, side } => resolve_conflicts(repo, &files, &side),
        Mutation::RenameBranch { name, to } => run(
            repo,
            &["branch", "-m", check_name(&name)?, check_name(&to)?],
        )
        .map(|_| ()),
        Mutation::SetUpstream { branch, upstream } => set_upstream(repo, &branch, &upstream),
        Mutation::DeleteRemoteBranch { remote, branch } => run(
            repo,
            &[
                "push",
                check_name(&remote)?,
                "--delete",
                check_name(&branch)?,
            ],
        )
        .map(|_| ()),
        Mutation::CheckoutRemote { remote, branch } => {
            with_autostash(repo, &format!("checkout {remote}/{branch}"), || {
                checkout_remote(repo, &remote, &branch)
            })
        }
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
        } => {
            let label = format!("pull {remote}/{branch}");
            if autostash {
                with_autostash(repo, &label, || pull_ref(repo, &remote, &branch, rebase))
            } else {
                pull_ref(repo, &remote, &branch, rebase)
            }
        }
        Mutation::Discard { file } => discard(repo, &file),
        Mutation::DiscardPaths { files } => discard_paths(repo, &files),
        Mutation::Delete { file } => delete_path(repo, &file),
        Mutation::DeletePaths { files } => delete_paths(repo, &files),
        Mutation::ApplyPatch { patch } => {
            run_stdin(repo, &["apply", "--index"], patch.as_bytes()).map(|_| ())
        }
        Mutation::AddRemote { name, url } => run(
            repo,
            &["remote", "add", check_name(&name)?, check_url(&url)?],
        )
        .map(|_| ()),
        Mutation::RemoveRemote { name } => {
            run(repo, &["remote", "remove", check_name(&name)?]).map(|_| ())
        }
        Mutation::SetRemoteUrl { name, url } => run(
            repo,
            &["remote", "set-url", check_name(&name)?, check_url(&url)?],
        )
        .map(|_| ()),
        Mutation::AddWorktree { path, branch } => add_worktree(repo, &path, &branch),
        Mutation::RemoveWorktree { path } => {
            run(repo, &["worktree", "remove", check_external(&path)?]).map(|_| ())
        }
        Mutation::SubmoduleUpdate => {
            run(repo, &["submodule", "update", "--init", "--recursive"]).map(|_| ())
        }
        Mutation::SubmoduleAdd { url, path } => run(
            repo,
            &["submodule", "add", check_url(&url)?, check_path(&path)?],
        )
        .map(|_| ()),
        Mutation::SubmoduleInit { path } => run(
            repo,
            &["submodule", "update", "--init", "--", check_path(&path)?],
        )
        .map(|_| ()),
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
        } => git_flow_init(
            repo, &master, &develop, &feature, &release, &hotfix, &support,
        ),
        Mutation::GitFlowStart { name, kind } => {
            with_autostash(repo, &format!("git flow {kind} start {name}"), || {
                git_flow_start(repo, &name, &kind)
            })
        }
        Mutation::GitFlowFinish { name, kind } => {
            with_autostash(repo, &format!("git flow {kind} finish {name}"), || {
                git_flow_finish(repo, &name, &kind)
            })
        }
        Mutation::Squash { from, to, summary } => with_autostash(
            repo,
            &format!("squash {}..{}", short_rev(&from), short_rev(&to)),
            || squash(repo, &from, &to, &summary),
        ),
        Mutation::Clone { url, destination } => clone_repo(&url, &destination),
        Mutation::Init { destination } => init_repo(&destination),
        Mutation::LfsPull => lfs(repo, &["lfs", "pull"]),
        Mutation::LfsPush => lfs(repo, &["lfs", "push", "--all"]),
        Mutation::LfsLock { path } => lfs(repo, &["lfs", "lock", crate::cli::check_path(&path)?]),
        Mutation::LfsUnlock { path } => {
            lfs(repo, &["lfs", "unlock", crate::cli::check_path(&path)?])
        }
        Mutation::SetHidden { names } => crate::model::write_hidden_refs(repo, &names),
        Mutation::SetExpanded { names } => crate::model::write_expanded_groups(repo, &names),
        Mutation::SetSignOff { enabled, format } => {
            crate::model::write_sign_off(repo, enabled, &format)
        }
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

fn pull(repo: &Path, rebase: bool) -> Result<(), Error> {
    let mut args = vec!["pull", "--no-autostash"];
    args.push(if rebase { "--rebase" } else { "--no-rebase" });
    surface(with_editor_mode(
        repo,
        &args,
        if rebase {
            AuthorMode::CommitterOnly
        } else {
            AuthorMode::Both
        },
    ))
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
        run(repo, &["stash", "pop", &stash_selector(repo, name)?]).map(|_| ())
    }
}

/// `git stash drop` and `pop` only accept stash references, not commit ids. The UI
/// pins an entry by id when a new stash renumbers `stash@{n}`, so map it back to the
/// slot it occupies now.
fn stash_selector(repo: &Path, name: &str) -> Result<String, Error> {
    let name = name.trim();
    if name.len() >= 7 && name.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(listed) = run(repo, &["stash", "list", "--format=%H"]) {
            let text = String::from_utf8_lossy(&listed.stdout);
            let hashes: Vec<&str> = text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect();
            let index = hashes
                .iter()
                .position(|hash| *hash == name)
                .or_else(|| hashes.iter().position(|hash| hash.starts_with(name)));
            if let Some(index) = index {
                return Ok(format!("stash@{{{index}}}"));
            }
        }
    }
    Ok(check_rev(name)?.to_string())
}

fn rebase_onto(repo: &Path, onto: &str) -> Result<(), Error> {
    let onto = check_rev(onto)?;
    with_editor_committer_only(repo, &["rebase", "--no-autostash", onto])
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
    let list = run(repo, &["stash", "list", "--format=%H"])?;
    let index = String::from_utf8_lossy(&list.stdout)
        .lines()
        .position(|line| line.trim() == hash)
        .ok_or_else(|| Error::Git("stash has no commit".into()))?;
    replace_stash_entry(repo, &hash, index, message)
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

fn stash_paths(
    repo: &Path,
    files: &[String],
    message: &str,
    include_untracked: bool,
) -> Result<(), Error> {
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
        let output = run(
            repo,
            &[
                "--literal-pathspecs",
                "ls-files",
                "--others",
                "--exclude-standard",
                "-z",
            ],
        );
        let Ok(output) = output else { return false };
        let wanted: HashSet<String> = files.iter().map(|path| slash(path)).collect();
        return output.stdout.split(|byte| *byte == 0).any(|path| {
            !path.is_empty() && wanted.contains(&String::from_utf8_lossy(path).into_owned())
        });
    }
    let mut args = vec![
        "--literal-pathspecs",
        "ls-files",
        "--others",
        "--exclude-standard",
        "--",
    ];
    args.extend(files.iter().map(String::as_str));
    run(repo, &args)
        .map(|output| output.stdout.iter().any(|byte| !byte.is_ascii_whitespace()))
        .unwrap_or(false)
}

/// `git branch -d` also refuses a branch that is not merged into its upstream,
/// even when HEAD already contains every commit. Deleting the local branch is
/// safe whenever all its commits are in HEAD or in its upstream; only a branch
/// with commits nowhere else stays an error and asks the UI for confirmation.
fn delete_branch(repo: &Path, name: &str) -> Result<(), Error> {
    let name = check_name(name)?;
    let error = match run(repo, &["branch", "-d", name]) {
        Ok(_) => return Ok(()),
        Err(error) => error,
    };
    let Some(tip) = revision(repo, &format!("refs/heads/{name}")) else {
        return Err(error);
    };
    let upstream = revision(repo, &format!("{name}@{{upstream}}"));
    let safe = is_ancestor(repo, &tip, "HEAD")
        || upstream
            .as_deref()
            .is_some_and(|upstream| is_ancestor(repo, &tip, upstream));
    if safe {
        return run(repo, &["branch", "-D", name]).map(|_| ());
    }
    Err(Error::Git(format!("branch is not fully merged\n{error}")))
}

fn revision(repo: &Path, rev: &str) -> Option<String> {
    let output = run(repo, &["rev-parse", "--verify", "--quiet", rev]).ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// A non-zero exit means "not an ancestor"; any failure here is treated as unsafe.
fn is_ancestor(repo: &Path, ancestor: &str, descendant: &str) -> bool {
    run(repo, &["merge-base", "--is-ancestor", ancestor, descendant]).is_ok()
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
    let stored = if message.is_empty() {
        None
    } else {
        Some(message_file(&message)?)
    };
    let mut args = vec!["merge".to_string(), "--no-autostash".to_string()];
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
    if squash && !autostash::has_unmerged(repo) {
        let staged = run(repo, &["diff", "--cached", "--name-only"])?;
        if staged.stdout.iter().any(|byte| !byte.is_ascii_whitespace()) {
            let fallback = if message.is_empty() {
                Some(message_file(&format!("Squash {name}"))?)
            } else {
                None
            };
            let path = fallback
                .as_ref()
                .or(stored.as_ref())
                .map(|file| file.0.as_path());
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
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path =
        std::env::temp_dir().join(format!("awegit-merge-{}-{nanos}.txt", std::process::id()));
    std::fs::write(&path, text).map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })?;
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

/// Branch labels and files for a merge paused with conflicts.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeState {
    pub source: String,
    pub into: String,
    pub files: Vec<MergeFile>,
}

pub fn merge_state(repo: &Path) -> Result<Option<MergeState>, Error> {
    let head = match run(repo, &["rev-parse", "--verify", "--quiet", "MERGE_HEAD"]) {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(_) => return Ok(None),
    };
    if head.is_empty() {
        return Ok(None);
    }
    Ok(Some(MergeState {
        source: merge_source(repo, &head),
        into: current_branch_name(repo),
        files: changed_names(repo, "MERGE_HEAD")?,
    }))
}

/// The branch pointing at `MERGE_HEAD`, or a short id when no ref does.
fn merge_source(repo: &Path, head: &str) -> String {
    if let Ok(output) = run(
        repo,
        &[
            "for-each-ref",
            "--points-at",
            head,
            "--format=%(refname)%09%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ],
    ) {
        let text = String::from_utf8_lossy(&output.stdout);
        let mut remote = String::new();
        for line in text.lines() {
            let mut fields = line.split('\t');
            let full = fields.next().unwrap_or("");
            let short = fields.next().unwrap_or("").trim();
            if short.is_empty() {
                continue;
            }
            if full.starts_with("refs/heads/") {
                return short.to_string();
            }
            if remote.is_empty() && full.starts_with("refs/remotes/") {
                remote = short.to_string();
            }
        }
        if !remote.is_empty() {
            return remote;
        }
    }
    head.chars().take(8).collect()
}

fn current_branch_name(repo: &Path) -> String {
    match run(repo, &["branch", "--show-current"]) {
        Ok(output) => {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if name.is_empty() {
                "HEAD".into()
            } else {
                name
            }
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
pub fn approve_credential(
    repo: &Path,
    protocol: &str,
    host: &str,
    username: &str,
    password: &str,
) -> Result<(), Error> {
    for part in [protocol, host, username, password] {
        if part.contains(['\n', '\r', '\0']) {
            return Err(Error::Rev("credential field".into()));
        }
    }
    if protocol.is_empty() || host.is_empty() || password.is_empty() {
        return Err(Error::Git(
            "protocol, host, and password are required".into(),
        ));
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
    let rebase = matches!(in_progress(repo)?, Some(InProgress::Rebase));
    if rebase {
        restore_rebase_message(repo);
    }
    if message.trim().is_empty() || !rebase {
        return flow(repo, "continue");
    }
    if message.contains('\0') {
        return Err(Error::Git("rebase message contains a null".into()));
    }
    let editor = SequenceEditor::with_messages("", &[message])?;
    run_env_committer_only(
        repo,
        &["rebase", "--continue"],
        &[("GIT_EDITOR", editor.editor.as_str())],
    )
    .map(|_| ())
}

/// A killed rebase can leave the pick applied but drop `.git/rebase-merge/message`.
/// Rebuild it from the commit `done` names so `--continue` commits the same text.
fn restore_rebase_message(repo: &Path) {
    let Ok(dir) = crate::flow::git_dir(repo) else {
        return;
    };
    let dir = dir.join("rebase-merge");
    let message = dir.join("message");
    if message.exists() {
        return;
    }
    let Ok(done) = std::fs::read_to_string(dir.join("done")) else {
        return;
    };
    let commit = done.lines().rev().find_map(|line| {
        let mut parts = line.split_whitespace();
        if !matches!(parts.next()?, "pick" | "p" | "reword" | "r" | "edit" | "e") {
            return None;
        }
        parts.next().map(str::to_string)
    });
    let Some(commit) = commit else {
        return;
    };
    let Ok(output) = run(repo, &["log", "-1", "--format=%B", &commit]) else {
        return;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        return;
    }
    let _ = std::fs::write(message, text.as_bytes());
}

fn abort(repo: &Path) -> Result<(), Error> {
    if in_progress(repo)?.is_some() {
        return flow(repo, "abort");
    }
    // A stash apply leaves unmerged paths without MERGE_HEAD. Reset the conflict
    // back to HEAD and keep the stash entry.
    if autostash::has_unmerged(repo) {
        return run(repo, &["reset", "--merge"]).map(|_| ());
    }
    Err(Error::Git("nothing in progress".into()))
}

fn flow(repo: &Path, verb: &str) -> Result<(), Error> {
    if verb == "continue" && in_progress(repo)? == Some(InProgress::Revert) {
        let source = rev_parse(repo, "REVERT_HEAD")?;
        if crate::model::revert_no_commit(repo).as_deref() == Some(source.as_str()) {
            if autostash::has_unmerged(repo) {
                return Err(Error::Git(
                    "resolve and stage all conflicts before continuing the revert".into(),
                ));
            }
            // Git's single-commit --continue commits even after --no-commit.
            // Quit only the operation metadata and retain the resolved index.
            return run(repo, &["revert", "--quit"]).map(|_| ());
        }
    }
    if verb == "skip" && matches!(in_progress(repo)?, Some(InProgress::Merge) | None) {
        return Err(Error::Git(
            "skip is only available during rebase, cherry-pick, or revert".into(),
        ));
    }
    let (command, mode) = match in_progress(repo)? {
        Some(InProgress::Merge) => ("merge", AuthorMode::Both),
        Some(InProgress::Rebase) => ("rebase", AuthorMode::CommitterOnly),
        Some(InProgress::CherryPick) => ("cherry-pick", AuthorMode::CommitterOnly),
        Some(InProgress::Revert) => ("revert", AuthorMode::CommitterOnly),
        None => return Err(Error::Git("nothing in progress".into())),
    };
    let flag = format!("--{verb}");
    with_editor_mode(repo, &[command, &flag], mode)
}

fn revert(repo: &Path, rev: &str, no_commit: bool, message: &str) -> Result<(), Error> {
    let rev = check_rev(rev)?;
    let output = run(repo, &["rev-parse", "--verify", &format!("{rev}^{{commit}}")])?;
    let rev = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let message = if message.trim().is_empty() {
        format!(
            "revert: {}\n\nThis reverts commit {rev}.\n",
            commit_subject(repo, &rev)?
        )
    } else {
        message.replace("\r\n", "\n").trim().to_string()
    };
    if message.contains('\0') {
        return Err(Error::Git("revert message contains a null".into()));
    }
    crate::model::set_revert_no_commit(repo, no_commit.then_some(rev.as_str()))?;
    // Stage the inverse first so the intended message reaches hooks on the first
    // commit attempt. Never create Git's default "Revert" commit and amend it.
    let applied = run(repo, &["revert", "--no-commit", &rev]).map(|_| ());
    if in_progress(repo)? != Some(InProgress::Revert) {
        clear_settled_revert(repo)?;
        return applied;
    }
    let path = crate::flow::git_dir(repo)?.join("MERGE_MSG");
    std::fs::write(&path, &message).map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })?;
    let result = applied.and_then(|_| {
        if no_commit {
            run(repo, &["revert", "--quit"]).map(|_| ())
        } else {
            commit_from_file(repo, &path)
        }
    });
    clear_settled_revert(repo)?;
    result
}

fn clear_settled_revert(repo: &Path) -> Result<(), Error> {
    if crate::model::revert_no_commit(repo).is_some() && in_progress(repo)? != Some(InProgress::Revert) {
        crate::model::set_revert_no_commit(repo, None)?;
    }
    Ok(())
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
                types: Vec::new(),
            },
        )
    } else {
        Err(Error::Git("unstage changes before rewording HEAD".into()))
    }
}

fn tag(repo: &Path, name: &str, rev: &str, message: &str) -> Result<(), Error> {
    let name = check_name(name)?;
    let rev = if rev.trim().is_empty() {
        "HEAD"
    } else {
        check_rev(rev)?
    };
    if message.trim().is_empty() {
        run(repo, &["tag", name, rev]).map(|_| ())
    } else {
        run(repo, &["tag", "-a", name, rev, "-m", message.trim()]).map(|_| ())
    }
}

fn discard(repo: &Path, file: &str) -> Result<(), Error> {
    let file = check_path(file)?;
    if in_index(repo, file)? {
        run(
            repo,
            &[
                "--literal-pathspecs",
                "restore",
                "--worktree",
                "--source=HEAD",
                "--",
                file,
            ],
        )
        .map(|_| ())
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
    git_arg_paths(
        repo,
        &["restore", "--worktree", "--source=HEAD", "--"],
        &restore,
    )?;
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
        let mut owned: Vec<String> = vec![
            "--literal-pathspecs".into(),
            "ls-files".into(),
            "-z".into(),
            "--".into(),
        ];
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
    let exists = run(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok();
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
    if run(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{develop}"),
        ],
    )
    .is_err()
    {
        let base = if run(
            repo,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/heads/{master}"),
            ],
        )
        .is_ok()
        {
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
    if run(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{base_name}"),
        ],
    )
    .is_err()
    {
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
    if run(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{develop}"),
        ],
    )
    .is_ok()
    {
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
    if value.is_empty() {
        default.to_string()
    } else {
        value.to_string()
    }
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
    if run(
        repo,
        &["rev-parse", "--verify", "--quiet", "refs/heads/main"],
    )
    .is_ok()
    {
        Ok("main".into())
    } else if run(
        repo,
        &["rev-parse", "--verify", "--quiet", "refs/heads/master"],
    )
    .is_ok()
    {
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
        if String::from_utf8_lossy(&all.stdout).trim()
            != String::from_utf8_lossy(&novel.stdout).trim()
        {
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
            types: Vec::new(),
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
    update_refs: bool,
) -> Result<(), Error> {
    if !steps.is_empty() {
        return rebase_steps(repo, onto, steps, update_refs);
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
    let mut args = vec!["rebase", "--no-autostash"];
    if update_refs {
        args.push("--update-refs");
    }
    args.push("-i");
    args.push(onto);
    run_env_committer_only(
        repo,
        &args,
        &[
            ("GIT_SEQUENCE_EDITOR", editor.sequence.as_str()),
            ("GIT_EDITOR", editor.editor.as_str()),
        ],
    )
    .map(|_| ())
}

fn rebase_steps(
    repo: &Path,
    onto: &str,
    steps: &[RebaseStep],
    update_refs: bool,
) -> Result<(), Error> {
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
    let mut args = vec!["rebase", "--no-autostash"];
    if update_refs {
        args.push("--update-refs");
    }
    args.push("-i");
    args.push(onto);
    run_env_committer_only(
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
    drop.iter()
        .any(|item| item == id || id.starts_with(item.as_str()) || item.starts_with(id))
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
        std::env::current_dir().map_err(|source| {
            Error::Git(format!("could not read the current directory: {source}"))
        })?
    } else {
        parent.to_path_buf()
    };
    let dest = destination_path.display().to_string();
    let reported = PathBuf::from(destination);
    surface(with_operation_repo(&reported, || {
        run(&cwd, &["clone", url, &dest]).map(|_| ())
    }))
}

/// If `destination` is an existing folder that already has files, clone into
/// `<destination>/<repo name>` instead of asking Git to reuse that folder.
fn resolve_clone_destination(url: &str, destination: &str) -> Result<PathBuf, Error> {
    let dest = PathBuf::from(destination);
    if !dest.exists() {
        return Ok(dest);
    }
    if dest.is_file() {
        return Err(Error::Git(format!(
            "destination path '{}' is not a directory",
            dest.display()
        )));
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
    let current = dest
        .file_name()
        .and_then(|item| item.to_str())
        .unwrap_or("");
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
        return parts
            .get(index + 1)
            .map(|name| (*name).trim_end_matches(".git").to_string());
    }
    let noise = [
        "-", "tree", "blob", "commit", "commits", "pull", "pulls", "issues", "compare", "src",
    ];
    let end = parts
        .iter()
        .enumerate()
        .find(|(index, part)| *index > 0 && noise.contains(part))
        .map(|(index, _)| index)
        .unwrap_or(parts.len());
    parts
        .get(..end)
        .and_then(|kept| kept.last())
        .map(|name| (*name).trim_end_matches(".git").to_string())
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
        || (lower.len() == 4
            && (lower.starts_with("com") || lower.starts_with("lpt"))
            && lower
                .as_bytes()
                .get(3)
                .is_some_and(|byte| byte.is_ascii_digit()));
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

fn custom(
    repo: &Path,
    command: &str,
    marked: &str,
    sha: &str,
    branch: &str,
    file: &str,
) -> Result<String, Error> {
    let command = command.trim();
    if command.is_empty() || command.contains('\0') {
        return Err(Error::Rev(command.to_string()));
    }
    let repo_text = if marked.trim().is_empty() {
        repo.display().to_string()
    } else {
        marked.to_string()
    };
    for value in [&repo_text, sha, branch, file] {
        if cfg!(windows)
            && value.chars().any(|ch| {
                matches!(
                    ch,
                    '"' | '%' | '&' | '|' | '<' | '>' | '^' | '(' | ')' | '\r' | '\n'
                )
            })
        {
            return Err(Error::Git(
                "refusing a command value that contains a cmd.exe metacharacter".into(),
            ));
        }
    }
    let expanded = command
        .replace(
            "${repo}",
            if cfg!(windows) {
                "%AWEGIT_REPO%"
            } else {
                "\"$AWEGIT_REPO\""
            },
        )
        .replace(
            "${sha}",
            if cfg!(windows) {
                "%AWEGIT_SHA%"
            } else {
                "\"$AWEGIT_SHA\""
            },
        )
        .replace(
            "${branch}",
            if cfg!(windows) {
                "%AWEGIT_BRANCH%"
            } else {
                "\"$AWEGIT_BRANCH\""
            },
        )
        .replace(
            "${file}",
            if cfg!(windows) {
                "%AWEGIT_FILE%"
            } else {
                "\"$AWEGIT_FILE\""
            },
        );
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
        .map_err(|source| Error::Git(format!("could not start the custom command: {source}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(if stdout.is_empty() { stderr } else { stdout })
    } else {
        Err(Error::Git(if stderr.is_empty() {
            if stdout.is_empty() {
                "custom command failed".into()
            } else {
                stdout
            }
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
        run(
            repo,
            &["branch", &format!("--set-upstream-to={upstream}"), branch],
        )
        .map(|_| ())
    }
}

fn checkout_remote(repo: &Path, remote: &str, branch: &str) -> Result<(), Error> {
    let remote = check_name(remote)?;
    let branch = check_name(branch)?;
    let local = run(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    );
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
    let target = if remote_branch.is_empty() {
        branch
    } else {
        check_name(remote_branch)?
    };
    let spec = format!("{branch}:{target}");
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

fn pull_ref(repo: &Path, remote: &str, branch: &str, rebase: bool) -> Result<(), Error> {
    let remote = check_name(remote)?;
    let branch = check_name(branch)?;
    let mut args = vec!["pull", "--no-autostash"];
    args.push(if rebase { "--rebase" } else { "--no-rebase" });
    args.push(remote);
    args.push(branch);
    surface(with_editor_mode(
        repo,
        &args,
        if rebase {
            AuthorMode::CommitterOnly
        } else {
            AuthorMode::Both
        },
    ))
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
    with_editor_mode(repo, args, AuthorMode::Both)
}

/// Rewrites keep the author recorded in the commit. Only the committer may follow
/// the identity picked in settings.
fn with_editor_committer_only(repo: &Path, args: &[&str]) -> Result<(), Error> {
    with_editor_mode(repo, args, AuthorMode::CommitterOnly)
}

fn with_editor_mode(repo: &Path, args: &[&str], mode: AuthorMode) -> Result<(), Error> {
    let editor = SequenceEditor::noop_only()?;
    let env = [("GIT_EDITOR", editor.editor.as_str())];
    match mode {
        AuthorMode::CommitterOnly => run_env_committer_only(repo, args, &env).map(|_| ()),
        _ => run_env(repo, args, &env).map(|_| ()),
    }
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
            write_script(
                &dir,
                "edit.sh",
                &format!("#!/bin/sh\ncp \"{unix}\" \"$1\"\n"),
            )?
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
        let ignored =
            scope.spawn(|| lines_of(repo, &["ls-files", "-o", "-i", "--exclude-standard"], 500));
        let activity = scope.spawn(|| activity(repo));
        let lfs = scope.spawn(|| {
            if crate::lfs_available(repo) {
                text_of(repo, &["lfs", "ls-files"])
            } else {
                String::new()
            }
        });
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
    for line in text_of(
        repo,
        &["log", "-n", "400", "--name-only", "--pretty=format:"],
    )
    .lines()
    {
        let path = line.trim();
        if path.is_empty() {
            continue;
        }
        *counts.entry(path.to_string()).or_default() += 1;
    }
    let mut hot: Vec<HotPath> = counts
        .into_iter()
        .map(|(path, count)| HotPath { path, count })
        .collect();
    hot.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.path.cmp(&b.path)));
    hot.truncate(24);
    hot
}

/// `-S` or `-G` search. The needle is one argv token, not a shell string.
pub fn search_commits(repo: &Path, kind: &str, needle: &str) -> Result<Vec<SearchHit>, Error> {
    let flag = if kind == "G" || kind == "g" {
        "-G"
    } else {
        "-S"
    };
    let needle = needle.trim();
    if needle.is_empty() || needle.starts_with('-') || needle.contains(['\n', '\r', '\0']) {
        return Err(Error::Rev(needle.to_string()));
    }
    let (output, _) = crate::cli::with_read(|| {
        crate::cli::run_capped(
            repo,
            &["log", "-n", "80", "--format=%H%x09%h%x09%s", flag, needle],
            1_000_000,
        )
    })?;
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
        crate::cli::run_capped(
            repo,
            &["merge-tree", "--write-tree", "--name-only", "HEAD", rev],
            1_000_000,
        )
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
    let (output, _) = crate::cli::with_read(|| {
        crate::cli::run_capped(repo, &["branch", "--merged", "HEAD"], 1_000_000)
    })?;
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
        crate::cli::run_capped(
            repo,
            &[
                "--literal-pathspecs",
                "log",
                "-n",
                "40",
                "--format=%h %s",
                "-S",
                text,
                "--",
                file,
            ],
            512_000,
        )
    })?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn lines_of(repo: &Path, args: &[&str], limit: usize) -> Vec<String> {
    text_of(repo, args)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(limit)
        .map(str::to_string)
        .collect()
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
        let full = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            repo.join(path)
        };
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
        let Ok(unix) = line.trim().parse::<i64>() else {
            continue;
        };
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

/// Parent directories of tracked files, plus the work tree root. Stops at `limit`.
#[derive(Debug, Clone)]
pub struct TrackedDirs {
    pub paths: Vec<PathBuf>,
    pub capped: bool,
}

pub fn tracked_directories(repo: &Path, limit: usize) -> TrackedDirs {
    if let Ok(repository) = crate::open_repo(repo) {
        if let Ok(index) = repository.index_or_empty() {
            return directories_from_paths(repo, limit, index.entries().iter().filter_map(|entry| std::str::from_utf8(entry.path(&index)).ok()));
        }
    }
    let Ok(output) = crate::cli::run(repo, &["ls-files", "-z"]) else {
        return TrackedDirs { paths: vec![repo.to_path_buf()], capped: false };
    };
    directories_from_paths(repo, limit, output.stdout.split(|byte| *byte == 0).filter_map(|path| std::str::from_utf8(path).ok()))
}

fn directories_from_paths<'a>(repo: &Path, limit: usize, entries: impl Iterator<Item = &'a str>) -> TrackedDirs {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    let root = repo.to_path_buf();
    seen.insert(root.clone());
    paths.push(root);
    let mut capped = false;
    for text in entries {
        if text.is_empty() {
            continue;
        }
        let Some(parent) = repo.join(text).parent().map(Path::to_path_buf) else {
            continue;
        };
        if !seen.insert(parent.clone()) {
            continue;
        }
        if paths.len() >= limit {
            capped = true;
            break;
        }
        paths.push(parent);
    }
    TrackedDirs { paths, capped }
}

/// Repositories found under `root`. `truncated` is set only when the 300 cap hid another repository.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub paths: Vec<String>,
    pub truncated: bool,
}

const SCAN_CAP: usize = 300;

/// Find git work trees under `root`, without entering a repository or common build folders.
pub fn scan_repositories(root: &Path, depth_limit: usize) -> Result<ScanResult, Error> {
    if !root.is_dir() {
        return Err(Error::Git(format!("{} is not a directory", root.display())));
    }
    let limit = if depth_limit == 0 { 5 } else { depth_limit };
    let mut found = Vec::new();
    let truncated = scan_dir(root, 0, limit, &mut found);
    Ok(ScanResult {
        paths: found,
        truncated,
    })
}

fn scan_dir(dir: &Path, depth: usize, limit: usize, found: &mut Vec<String>) -> bool {
    if depth > limit {
        return false;
    }
    if found.len() >= SCAN_CAP {
        return true;
    }
    if dir.join(".git").exists() {
        found.push(dir.display().to_string());
        return false;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut truncated = false;
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !kind.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if matches!(
            name.as_ref(),
            "node_modules" | "target" | "dist" | "vendor" | ".git" | ".next" | "out"
        ) {
            continue;
        }
        if found.len() >= SCAN_CAP {
            truncated = true;
            break;
        }
        if scan_dir(&entry.path(), depth + 1, limit, found) {
            truncated = true;
        }
    }
    truncated
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
        assert_eq!(
            repo_folder_name("https://github.com/org/Demo/tree/main"),
            "Demo"
        );
        assert_eq!(
            repo_folder_name("https://dev.azure.com/org/project/_git/Demo"),
            "Demo"
        );
        assert_eq!(
            repo_folder_name("ssh://git@github.com/org/Demo.git"),
            "Demo"
        );
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
        let dest = resolve_clone_destination(
            "https://github.com/org/Demo.git",
            &root.display().to_string(),
        )
        .unwrap();
        assert_eq!(dest, root.join("Demo"));
        std::fs::create_dir_all(root.join("Demo")).unwrap();
        std::fs::write(root.join("Demo").join("a.txt"), "x").unwrap();
        let err = resolve_clone_destination(
            "https://github.com/org/Demo.git",
            &root.join("Demo").display().to_string(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("already exists"));
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod scan_tests {
    use super::{scan_repositories, tracked_directories};
    use std::fs;
    use std::process::Command;

    fn scratch(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("awegit-{name}-{nanos}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn scan_marks_truncation_only_when_more_remain() {
        let root = scratch("scan");
        for index in 0..301 {
            fs::create_dir_all(root.join(format!("repo-{index:04}")).join(".git")).unwrap();
        }
        let full = scan_repositories(&root, 2).unwrap();
        assert_eq!(full.paths.len(), 300);
        assert!(full.truncated);
        let exact = scratch("scan-exact");
        fs::create_dir_all(exact.join("one").join(".git")).unwrap();
        fs::create_dir_all(exact.join("two").join(".git")).unwrap();
        let found = scan_repositories(&exact, 2).unwrap();
        assert_eq!(found.paths.len(), 2);
        assert!(!found.truncated);
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&exact);
    }

    #[test]
    fn tracked_directories_follow_ls_files() {
        let root = scratch("tracked");
        let init = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            init.status.success(),
            "{}",
            String::from_utf8_lossy(&init.stderr)
        );
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src").join("main.txt"), "one\n").unwrap();
        fs::write(root.join("top.txt"), "two\n").unwrap();
        let add = Command::new("git")
            .args(["add", "--", "src/main.txt", "top.txt"])
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            add.status.success(),
            "{}",
            String::from_utf8_lossy(&add.stderr)
        );
        let tracked = tracked_directories(&root, 10);
        assert!(!tracked.capped);
        assert!(tracked.paths.iter().any(|path| path == &root));
        assert!(tracked.paths.iter().any(|path| path.ends_with("src")));
        let capped = tracked_directories(&root, 1);
        assert!(capped.capped);
        assert_eq!(capped.paths.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }
}
