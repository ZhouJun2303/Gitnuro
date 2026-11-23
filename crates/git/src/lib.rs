//! Repository status, diffs, and the writes that change the index or `HEAD`.
//!
//! Status is read with gitoxide. Staging, committing, and diff text go through `git`.

mod autostash;
mod change;
mod cli;
mod fakeignore;
mod fingerprint;
pub mod flow;
mod locks;
mod model;
mod ops;
mod proc;
mod pull;
mod timing;

use std::path::{Path, PathBuf};

pub use timing::{enter_timing, set_timing_trace, timing_context, timing_phase, timing_repository, TimingContext, TimingPhase, TimingScope};
pub use pull::{refresh_state as pull_refresh_state, RefreshState as PullRefreshState};

pub use change::{
    blob_preview, blob_view, commit, conflict_sides, conflict_view, discard_hunk, discard_line,
    discard_lines, file_diff, file_diff_with, file_preview, index_preview, launch_diff_pair, launch_diff_tool,
    launch_editor, launch_merge_tool, porcelain_z_paths, range_diff, resolve_conflict,
    compact_directory_paths, resolve_conflicts, revision_bytes, stage_all, stage_hunk, stage_line, stage_lines, stage_paths,
    staged_diff, unstage_all, unstage_paths, workspace_diff, BlobView, CommitRequest,
    ConflictSides, ConflictView, DiffLine, DiffLineKind, FileDiff, FilePreview, LineEdit,
};
pub use cli::{
    cancel_reads, cancel_repo, cancel_running, capture_output, configure, git_program, git_runtime,
    hide_console, ls_remote_head, origin_url, parse_git_version, remote_list, repo_key, set_http_proxy,
    set_https_credentials, set_passphrase, set_progress, set_trace, stop_process_tree, use_bundled_git,
    common_repo_key, check_cancelled, cancel_token, check_path, with_cancellation, without_cancellation, with_operation_repo, GitRuntime, HttpsCredential, Session, Trace,
};
pub use fingerprint::{status_fingerprint, StatusFingerprint};
pub use model::{
    blame_file, blame_source, blame_file_range, branch_commits, branch_commits_sorted, commit_detail, commit_files, commit_log,
    commit_log_sorted, commit_tree, commits_from, compare_file, compare_files, file_history,
    ignored_paths, in_progress, index_lock_path, lfs_available, lfs_locks, locate_ref,
    prompt_facts, rebase_conflicts, reflog, remove_stale_index_lock, repo_sign_off_format,
    repository_refs, repository_summary, search_log, show_commit_file, show_commit_file_with,
    remote_account_overrides, stale_index_lock, stale_index_lock_for_error,
    write_expanded_groups, write_hidden_refs, write_remote_account, write_sign_off, BlameLine,
    BlameSource, BranchRow, CommitDetail, CommitRow, InProgress, LfsLockRow, LocatedRef, RefSnapshot,
    commit_log_page, commit_log_page_with_revision, file_history_page, file_history_page_with_revision,
    search_log_page, search_log_page_with_revision, reflog_page,
    ReflogRow, RemoteRow, RepoSummary, StaleIndexLock, StashRow, SubmoduleRow, TagRow, WorktreeRow,
};
pub use ops::{
    approve_credential, cherry_preview, merge_preview, merge_state, merged_branches, perform, perform_stashed,
    pick_history, push_commit_target, push_is_up_to_date, repo_facts, repo_facts_brief, scan_repositories, search_commits, set_repo_author,
    tracked_directories, HotPath, MergeFile, MergePreview, MergeState, Mutation, RebaseStep,
    RepoFacts, RepositoryScan, ResetMode, ScanResult, SearchHit, TrackedDirs,
};

use gix::bstr::BStr;
use serde::{Deserialize, Serialize};

/// One changed path, either staged or unstaged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
    pub previous_path: Option<String>,
}

/// How a path changed, matching what `git status` would show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    #[serde(default)]
    pub revision: u64,
    pub path: String,
    pub branch: Option<String>,
    pub commit_intent: CommitIntent,
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
    /// Set after a full status read. Path-limited reads leave it empty so a partial list cannot vouch for the index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<StatusFingerprint>,
}

/// Ignore cache-only index updates. A commit depends on the staged paths, modes and object IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitIntent {
    pub head: Option<String>,
    pub index: String,
}

pub fn commit_intent(path: &Path) -> Result<CommitIntent, Error> {
    let repo = open_repo(path)?;
    let head = repo.head()?.id().map(|id| id.to_string());
    index_intent(&repo, head, None)
}

pub fn worktree_intent(path: &Path, paths: &[String]) -> Result<CommitIntent, Error> {
    let repo = open_repo(path)?;
    let head = repo.head()?.id().map(|id| id.to_string());
    let paths = paths.iter().map(|path| path.as_str()).collect();
    index_intent(&repo, head, Some(&paths))
}

fn index_intent(repo: &gix::Repository, head: Option<String>, paths: Option<&std::collections::HashSet<&str>>) -> Result<CommitIntent, Error> {
    use std::hash::{Hash, Hasher};
    let index = repo.index_or_empty().map_err(|error| Error::Git(error.to_string()))?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for entry in index.entries() {
        if let Some(paths) = paths {
            let Ok(mut path) = std::str::from_utf8(entry.path(&index)) else { continue; };
            loop {
                if paths.contains(path) { break; }
                path = path.rsplit_once('/').map_or("", |(parent, _)| parent);
                if path.is_empty() { break; }
            }
            if path.is_empty() { continue; }
        }
        entry.path(&index).hash(&mut hash);
        entry.id.hash(&mut hash);
        entry.mode.bits().hash(&mut hash);
        entry.stage().hash(&mut hash);
        entry.flags.contains(gix::index::entry::Flags::INTENT_TO_ADD).hash(&mut hash);
    }
    Ok(CommitIntent { head, index: format!("{:016x}", hash.finish()) })
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
    status_patterns(path.as_ref(), fakeignore, Vec::new())
}

/// Read only the given literal paths and their descendants. An empty selection
/// returns no changes, rather than requesting the entire work tree.
pub fn status_paths_with(path: impl AsRef<Path>, fakeignore: bool, paths: &[String]) -> Result<StatusSnapshot, Error> {
    let mut patterns = Vec::with_capacity(paths.len().max(1));
    for path in paths {
        cli::check_path(path)?;
        let path = if cfg!(windows) { path.replace('\\', "/") } else { path.clone() };
        patterns.push(gix::bstr::BString::from(format!(":(top,literal){path}")));
    }
    if patterns.is_empty() { patterns.push(gix::bstr::BString::from(":(top,literal).git/awegit-empty-selection")); }
    status_patterns(path.as_ref(), fakeignore, patterns)
}

pub fn current_branch(path: &Path) -> Result<Option<String>, Error> {
    Ok(open_repo(path)?.head()?.referent_name().map(|name| name.shorten().to_string()))
}

fn status_patterns(path: &Path, fakeignore: bool, patterns: Vec<gix::bstr::BString>) -> Result<StatusSnapshot, Error> {
    let path = path.as_ref();
    let repo = open_repo(path)?;
    let worktree = repo.workdir().unwrap_or(path).to_path_buf();
    let head = repo.head()?;
    let branch = head
        .referent_name()
        .map(|name| name.shorten().to_string());
    let commit_intent = index_intent(&repo, head.id().map(|id| id.to_string()), None)?;

    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let iter = repo
        .status(gix::progress::Discard)?
        .untracked_files(gix::status::UntrackedFiles::Files)
        .into_iter(patterns)?;

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
        revision: 0,
        path: worktree.display().to_string(),
        branch,
        commit_intent,
        staged,
        unstaged,
        fingerprint: None,
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
