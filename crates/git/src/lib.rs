//! Repository status, diffs, and the writes that change the index or `HEAD`.
//!
//! Status is read with gitoxide. Staging, committing, and diff text go through `git`.

mod autostash;
mod change;
mod cli;
mod fakeignore;
pub mod flow;
mod locks;
mod model;
mod ops;
mod proc;

use std::path::{Path, PathBuf};

pub use change::{
    blob_preview, blob_view, commit, conflict_sides, conflict_view, discard_hunk, discard_line,
    discard_lines, file_diff, file_diff_with, file_preview, launch_diff_pair, launch_diff_tool,
    launch_editor, launch_merge_tool, porcelain_z_paths, range_diff, resolve_conflict,
    resolve_conflicts, revision_bytes, stage_all, stage_hunk, stage_line, stage_lines, stage_paths,
    staged_diff, unstage_all, unstage_paths, workspace_diff, BlobView, CommitRequest,
    ConflictSides, ConflictView, DiffLine, DiffLineKind, FileDiff, FilePreview, LineEdit,
};
pub use cli::{
    cancel_reads, cancel_repo, cancel_running, capture_output, configure, git_program, git_runtime,
    hide_console, origin_url, parse_git_version, repo_key, set_http_proxy, set_passphrase,
    set_progress, set_trace, stop_process_tree, use_bundled_git, with_operation_repo, GitRuntime,
    Session, Trace,
};
pub use model::{
    blame_file, branch_commits, branch_commits_sorted, commit_detail, commit_files, commit_log,
    commit_log_sorted, commit_tree, commits_from, compare_file, compare_file_options, compare_files, file_history,
    ignored_paths, in_progress, index_lock_path, lfs_available, lfs_locks, locate_ref,
    prompt_facts, rebase_conflicts, reflog, repo_sign_off_format, repository_refs,
    repository_summary, search_log, show_commit_file, show_commit_file_with, show_commit_file_options, write_expanded_groups,
    write_hidden_refs, write_sign_off, BlameLine, BranchRow, CommitDetail, CommitRow, InProgress,
    LfsLockRow, LocatedRef, RefSnapshot, ReflogRow, RemoteRow, RepoSummary, StashRow, SubmoduleRow,
    TagRow, WorktreeRow,
};
pub use ops::{
    approve_credential, cherry_preview, merge_preview, merge_state, merged_branches, perform, perform_stashed,
    pick_history, repo_facts, repo_facts_brief, scan_repositories, search_commits, set_repo_author,
    tracked_directories, HotPath, MergeFile, MergePreview, MergeState, Mutation, RebaseStep,
    RepoFacts, ResetMode, ScanResult, SearchHit, TrackedDirs,
};

use gix::bstr::BStr;
use serde::Serialize;

/// One changed path, either staged or unstaged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
    pub previous_path: Option<String>,
}

/// How a path changed, matching what `git status` would show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Conflict,
    Untracked,
}

/// The branch and the two change lists for one work tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    pub path: String,
    pub branch: Option<String>,
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not open repository at {path}")]
    Open {
        path: String,
        #[source]
        source: Box<gix::open::Error>,
    },
    /// The path is gone, or it is not a Git repository. Callers drop saved config for this path.
    #[error("could not open repository at {path}: missing")]
    Missing { path: String },
    #[error(transparent)]
    Status(#[from] gix::status::Error),
    #[error(transparent)]
    Iterate(#[from] gix::status::into_iter::Error),
    #[error(transparent)]
    Item(#[from] gix::status::iter::Error),
    #[error(transparent)]
    Head(#[from] gix::reference::find::existing::Error),
    #[error("could not start git: {0}")]
    GitMissing(#[source] std::io::Error),
    #[error("{0}")]
    Git(String),
    #[error("commit summary is empty")]
    EmptySummary,
    #[error("path is not inside the repository: {0}")]
    Path(String),
    #[error("revision is not allowed: {0}")]
    Rev(String),
    #[error("could not read {path}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Read status for the repository at `path` (the work tree, or any directory inside it).
pub fn status(path: impl AsRef<Path>) -> Result<StatusSnapshot, Error> {
    status_with(path, false)
}

/// Like [`status`]. When `fakeignore` is set, paths matched by `.fakeignore` are left out of both lists.
///
/// Conflicts stay visible. Passing `false` is the same as [`status`].
pub fn status_with(path: impl AsRef<Path>, fakeignore: bool) -> Result<StatusSnapshot, Error> {
    let path = path.as_ref();
    let repo = open_repo(path)?;
    let worktree = repo.workdir().unwrap_or(path).to_path_buf();
    let branch = repo
        .head()?
        .referent_name()
        .map(|name| name.shorten().to_string());

    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let iter = repo
        .status(gix::progress::Discard)?
        .untracked_files(gix::status::UntrackedFiles::Files)
        .into_iter(Vec::<gix::bstr::BString>::new())?;

    for item in iter {
        match item? {
            gix::status::Item::TreeIndex(change) => push_staged(&mut staged, change),
            gix::status::Item::IndexWorktree(change) => push_unstaged(&mut unstaged, change),
        }
    }

    sort_changes(&mut staged);
    sort_changes(&mut unstaged);
    hide_scratch(&mut staged);
    hide_scratch(&mut unstaged);
    if fakeignore {
        fakeignore::conceal(worktree.as_path(), &repo, &mut staged, &mut unstaged);
    }
    Ok(StatusSnapshot {
        path: worktree.display().to_string(),
        branch,
        staged,
        unstaged,
    })
}

/// Codex artifact directories (`.tmp_artifact`, `.tmp_artifact2`, …). They are not repository changes.
pub fn codex_scratch_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(".tmp_artifact") else {
        return false;
    };
    rest.is_empty() || rest.chars().all(|ch| ch.is_ascii_digit())
}

/// True when any path segment is a Codex artifact directory.
pub fn codex_scratch(path: &str) -> bool {
    path.split(['/', '\\']).any(codex_scratch_name)
}

fn hide_scratch(changes: &mut Vec<FileChange>) {
    changes.retain(|change| !scratch_change(change));
}

fn scratch_change(change: &FileChange) -> bool {
    if codex_scratch(&change.path) {
        return true;
    }
    change.previous_path.as_deref().is_some_and(codex_scratch)
}

/// Unstage index entries that `.fakeignore` matches. Used so stage-all and commit skip local test files.
pub fn unstage_fakeignored(repo: &Path) -> Result<(), Error> {
    fakeignore::unstage(repo)
}

fn open_repo(path: &Path) -> Result<gix::Repository, Error> {
    gix::open(path).map_err(|source| classify_open(path, source))
}

fn classify_open(path: &Path, source: gix::open::Error) -> Error {
    let missing = match &source {
        gix::open::Error::NotARepository { .. } => true,
        gix::open::Error::Io(err) => err.kind() == std::io::ErrorKind::NotFound || !path.exists(),
        _ => !path.exists(),
    };
    if missing {
        Error::Missing {
            path: path.display().to_string(),
        }
    } else {
        Error::Open {
            path: path.display().to_string(),
            source: Box::new(source),
        }
    }
}

/// Walk upward from `start` until a repository work tree is found.
pub fn discover(start: impl AsRef<Path>) -> Option<PathBuf> {
    let mut dir = start.as_ref().to_path_buf();
    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn push_staged(out: &mut Vec<FileChange>, change: gix::diff::index::Change) {
    use gix::diff::index::ChangeRef;
    match change {
        ChangeRef::Addition { location, .. } => {
            push(out, location.as_ref(), ChangeKind::Added, None)
        }
        ChangeRef::Deletion { location, .. } => {
            push(out, location.as_ref(), ChangeKind::Deleted, None)
        }
        ChangeRef::Modification { location, .. } => {
            push(out, location.as_ref(), ChangeKind::Modified, None)
        }
        ChangeRef::Rewrite {
            source_location,
            location,
            copy,
            ..
        } => {
            if copy {
                push(out, location.as_ref(), ChangeKind::Added, None);
            } else {
                push(
                    out,
                    location.as_ref(),
                    ChangeKind::Renamed,
                    Some(source_location.as_ref()),
                );
            }
        }
    }
}

fn push_unstaged(out: &mut Vec<FileChange>, change: gix::status::index_worktree::Item) {
    use gix::status::index_worktree::Item;
    use gix::status::plumbing::index_as_worktree::{Change, EntryStatus};

    match change {
        Item::Modification {
            rela_path, status, ..
        } => match status {
            EntryStatus::Conflict(_) => push(out, rela_path.as_ref(), ChangeKind::Conflict, None),
            EntryStatus::Change(Change::Removed) => {
                push(out, rela_path.as_ref(), ChangeKind::Deleted, None)
            }
            EntryStatus::Change(
                Change::Modification { .. }
                | Change::Type { .. }
                | Change::SubmoduleModification(_),
            ) => push(out, rela_path.as_ref(), ChangeKind::Modified, None),
            EntryStatus::IntentToAdd => push(out, rela_path.as_ref(), ChangeKind::Added, None),
            EntryStatus::NeedsUpdate(_) => {}
        },
        Item::DirectoryContents { entry, .. } => {
            if entry.status == gix::dir::entry::Status::Untracked {
                push(out, entry.rela_path.as_ref(), ChangeKind::Untracked, None);
            }
        }
        Item::Rewrite {
            source,
            dirwalk_entry,
            copy,
            ..
        } => {
            if copy {
                push(
                    out,
                    dirwalk_entry.rela_path.as_ref(),
                    ChangeKind::Untracked,
                    None,
                );
            } else {
                push(
                    out,
                    dirwalk_entry.rela_path.as_ref(),
                    ChangeKind::Renamed,
                    Some(source.rela_path()),
                );
            }
        }
    }
}

fn push(out: &mut Vec<FileChange>, path: &BStr, kind: ChangeKind, previous: Option<&BStr>) {
    out.push(FileChange {
        path: path.to_string(),
        kind,
        previous_path: previous.map(ToString::to_string),
    });
}

fn sort_changes(changes: &mut [FileChange]) {
    changes.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(kind_order(left.kind).cmp(&kind_order(right.kind)))
    });
}

fn kind_order(kind: ChangeKind) -> u8 {
    match kind {
        ChangeKind::Conflict => 0,
        ChangeKind::Added => 1,
        ChangeKind::Renamed => 2,
        ChangeKind::Modified => 3,
        ChangeKind::Deleted => 4,
        ChangeKind::Untracked => 5,
    }
}
