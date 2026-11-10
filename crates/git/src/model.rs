//! Commit graph and the sidebar's branches, remotes, tags, stashes, submodules, and worktrees.
//!
//! These reads go through `git` porcelain so decorations, ahead/behind, and submodule
//! state match the Git the user already has installed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread::ScopedJoinHandle;

use serde::Serialize;

use crate::change::{parse_diff, spec_size, FileDiff};
use crate::cli::{check_path, check_rev, run, run_capped, run_output, run_stdin_output, with_read};
use crate::{ChangeKind, Error, FileChange};

static GIT_DIRS: Mutex<BTreeMap<PathBuf, PathBuf>> = Mutex::new(BTreeMap::new());
static LFS_INSTALLED: OnceLock<bool> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitRow {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author: String,
    pub when: String,
    pub parents: Vec<String>,
    pub refs: Vec<String>,
    pub lane: u32,
    /// Unix seconds from `%at`. Zero when the log line has no timestamp.
    #[serde(default)]
    pub at: i64,
    /// Author email from `%ae`, used only for an optional avatar.
    #[serde(default)]
    pub email: String,
    /// Whether this commit has not been pushed to any remote.
    #[serde(default)]
    pub unpushed: bool,
    /// Whether this commit is on a remote but not yet reachable from a local branch.
    #[serde(default)]
    pub remote_only: bool,
}

/// Author, committer, and message body for the history detail pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub id: String,
    pub summary: String,
    pub body: String,
    pub author: String,
    pub author_email: String,
    pub author_at: i64,
    pub committer: String,
    pub committer_email: String,
    pub committer_at: i64,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchRow {
    pub name: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRow {
    pub name: String,
    pub url: Option<String>,
    pub head: Option<String>,
    pub branches: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagRow {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StashRow {
    pub name: String,
    pub summary: String,
    /// Commit id of the stash. Empty when this Git does not print it.
    #[serde(default)]
    pub id: String,
    /// First parent, the commit the stash was taken from.
    #[serde(default)]
    pub parent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleRow {
    pub path: String,
    pub id: String,
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRow {
    pub path: String,
    pub branch: Option<String>,
    pub detached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefSnapshot {
    /// Commit HEAD points at, so the UI can tell which history row is rewordable.
    #[serde(default)]
    pub head: Option<String>,
    pub branches: Vec<BranchRow>,
    pub remotes: Vec<RemoteRow>,
    pub tags: Vec<TagRow>,
    pub stashes: Vec<StashRow>,
    pub submodules: Vec<SubmoduleRow>,
    pub worktrees: Vec<WorktreeRow>,
    /// Branch names hidden in the sidebar. Stored in `.git/awegit` as `awegit.hiddenRef`,
    /// the same file the Kotlin client writes. The current branch is still shown.
    #[serde(default)]
    pub hidden_refs: Vec<String>,
    /// Open branch groups for this repository. An empty list means every group is open.
    #[serde(default)]
    pub expanded_groups: Vec<String>,
    /// True when `.git/awegit` has a `signoff.enabled` key.
    #[serde(default)]
    pub sign_off_set: bool,
    #[serde(default)]
    pub sign_off: bool,
    #[serde(default)]
    pub sign_off_format: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InProgress {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

pub fn commit_log(repo: &Path, limit: usize) -> Result<Vec<CommitRow>, Error> {
    log_with(repo, limit, true, &[], &[], false)
}

pub fn commit_log_sorted(repo: &Path, limit: usize, topo: bool) -> Result<Vec<CommitRow>, Error> {
    log_with(repo, limit, true, &[], &[], topo)
}

/// Commits reachable from `HEAD` only, without `--all`.
pub fn branch_commits(repo: &Path, limit: usize) -> Result<Vec<CommitRow>, Error> {
    log_with(repo, limit, false, &[], &[], false)
}

pub fn branch_commits_sorted(repo: &Path, limit: usize, topo: bool) -> Result<Vec<CommitRow>, Error> {
    log_with(repo, limit, false, &[], &[], topo)
}

/// Commits reachable from the named revisions, such as the current branch and its upstream.
pub fn commits_from(repo: &Path, limit: usize, revs: &[String], topo: bool) -> Result<Vec<CommitRow>, Error> {
    let checked: Vec<&str> = revs.iter().map(|rev| check_rev(rev)).collect::<Result<_, _>>()?;
    log_with(repo, limit, false, &[], &checked, topo)
}

/// Commits matching a sidebar search.
///
/// `field` is `message`, `author`, `path`, or `diff`. `scope` is `repository`
/// (`--branches --remotes`) or `branch` (`HEAD` only). The query is one argv
/// token, never a separate option.
pub fn search_log(repo: &Path, field: &str, scope: &str, query: &str, limit: usize, topo: bool) -> Result<Vec<CommitRow>, Error> {
    let query = query.trim();
    if query.is_empty() || query.starts_with('-') || query.contains(['\n', '\r', '\0']) {
        return Err(Error::Rev(query.to_string()));
    }
    if field == "path" && query.contains("..") {
        return Err(Error::Path(query.to_string()));
    }
    let head = with_read(|| run_output(repo, &["rev-parse", "--verify", "HEAD"]));
    if !matches!(head, Ok(output) if output.status.success()) {
        return Ok(Vec::new());
    }
    let limit_text = limit.clamp(1, 100_000).to_string();
    let order = if topo { "--topo-order" } else { "--date-order" };
    let mut owned = vec![
        "log".to_string(),
        "-n".to_string(),
        limit_text,
        order.to_string(),
        "-z".to_string(),
        "--pretty=format:%H%x1f%P%x1f%an%x1f%ar%x1f%s%x1f%D%x1f%at%x1f%ae".to_string(),
    ];
    if scope != "branch" {
        owned.insert(1, "--remotes".to_string());
        owned.insert(1, "--branches".to_string());
    }
    match field {
        "author" => {
            owned.push("--regexp-ignore-case".to_string());
            owned.push("--fixed-strings".to_string());
            owned.push(format!("--author={query}"));
        }
        "path" => {
            owned.push("--".to_string());
            owned.push(format!("*{query}*"));
        }
        "diff" => owned.push(format!("-S{query}")),
        _ => {
            owned.push("--regexp-ignore-case".to_string());
            owned.push("--fixed-strings".to_string());
            owned.push(format!("--grep={query}"));
        }
    }
    let args: Vec<&str> = owned.iter().map(String::as_str).collect();
    let (output, _) = with_read(|| run_capped(repo, &args, 16_000_000))?;
    let mut commits = parse_commits(&String::from_utf8_lossy(&output.stdout));
    assign_lanes(&mut commits);
    mark_remote_state(repo, &mut commits);
    Ok(commits)
}

/// The commit `name` points at, plus a history page large enough to contain it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocatedRef {
    pub id: String,
    pub commits: Vec<CommitRow>,
}

/// Resolve a branch or tag, then grow `--all` history until that commit is in the page.
///
/// Searching by decoration name fails when the commit is older than the first page,
/// and also when two clicks of a double-click race a checkout. The id is resolved
/// first so the caller can select it even if decorations are missing.
/// History pages stop growing here. A ref far below this still opens: the commit is
/// prepended so the UI can select it without materialising the whole repository.
const MAX_LOCATE_COMMITS: usize = 20_000;

pub fn locate_ref(repo: &Path, name: &str, topo: bool) -> Result<LocatedRef, Error> {
    locate_ref_from(repo, name, topo, 500)
}

pub(crate) fn locate_ref_from(repo: &Path, name: &str, topo: bool, start: usize) -> Result<LocatedRef, Error> {
    let name = check_rev(name)?;
    let output = with_read(|| run(repo, &["rev-parse", "--verify", name]))?;
    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if id.len() < 7 {
        return Err(Error::Rev(name.to_string()));
    }
    let mut limit = start.clamp(1, MAX_LOCATE_COMMITS);
    loop {
        let commits = log_with(repo, limit, true, &[], &[], topo)?;
        if commits.iter().any(|commit| commit.id == id) {
            return Ok(LocatedRef { id, commits });
        }
        if commits.len() < limit || limit >= MAX_LOCATE_COMMITS {
            let mut commits = commits;
            if let Ok(extra) = log_with(repo, 1, false, &[], &[name], topo) {
                if extra.iter().any(|commit| commit.id == id) && !commits.iter().any(|commit| commit.id == id) {
                    commits.splice(0..0, extra);
                }
            }
            return Ok(LocatedRef { id, commits });
        }
        limit = limit.saturating_mul(4).min(MAX_LOCATE_COMMITS);
    }
}

pub fn file_history(repo: &Path, path: &str, limit: usize) -> Result<Vec<CommitRow>, Error> {
    let path = check_path(path)?;
    log_with(repo, limit, false, &[path], &[], false)
}

fn log_with(repo: &Path, limit: usize, all: bool, path: &[&str], revs: &[&str], topo: bool) -> Result<Vec<CommitRow>, Error> {
    let head = with_read(|| run_output(repo, &["rev-parse", "--verify", "HEAD"]));
    if !matches!(head, Ok(output) if output.status.success()) {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 100_000).to_string();
    let order = if topo { "--topo-order" } else { "--date-order" };
    let mut args = vec![
        "--literal-pathspecs",
        "log",
        "-n",
        &limit,
        order,
        "-z",
        "--pretty=format:%H%x1f%P%x1f%an%x1f%ar%x1f%s%x1f%D%x1f%at%x1f%ae",
    ];
    if !revs.is_empty() {
        // Named revisions only. Extra remote tips would walk the whole remote set.
        args.extend_from_slice(revs);
    } else if path.is_empty() && all {
        // Branches and remotes, not `--all`: tags and notes would bury the tips.
        args.insert(2, "--branches");
        args.insert(3, "--remotes");
    } else if !path.is_empty() {
        args.push("--");
        args.extend_from_slice(path);
    }
    let (output, _) = with_read(|| run_capped(repo, &args, 16_000_000))?;
    let mut commits = parse_commits(&String::from_utf8_lossy(&output.stdout));
    assign_lanes(&mut commits);
    with_read(|| mark_remote_state(repo, &mut commits));
    Ok(commits)
}

fn mark_remote_state(repo: &Path, commits: &mut [CommitRow]) {
    if commits.is_empty() {
        return;
    }
    if !has_remotes(repo) {
        return;
    }
    if let Ok(output) = run(repo, &["rev-list", "--all", "--not", "--remotes", "--max-count=20000"]) {
        let text = String::from_utf8_lossy(&output.stdout);
        let local_only: std::collections::HashSet<&str> = text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if !local_only.is_empty() {
            for commit in commits.iter_mut() {
                if local_only.contains(commit.id.as_str()) {
                    commit.unpushed = true;
                }
            }
        }
    }
    if let Ok(output) = run(repo, &["rev-list", "--remotes", "--not", "--branches", "HEAD", "--max-count=20000"]) {
        let text = String::from_utf8_lossy(&output.stdout);
        let remote_only: std::collections::HashSet<&str> = text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if !remote_only.is_empty() {
            for commit in commits.iter_mut() {
                if remote_only.contains(commit.id.as_str()) {
                    commit.remote_only = true;
                }
            }
        }
    }
}

/// A remote adds a section to `.git/config`; reading that file avoids a `git remote`
/// process on every history refresh. Falls back to the command when the file is unreadable.
fn has_remotes(repo: &Path) -> bool {
    let Ok(dir) = absolute_git_dir(repo) else {
        return run(repo, &["remote"]).map(|out| !out.stdout.is_empty()).unwrap_or(false);
    };
    let Ok(text) = std::fs::read_to_string(dir.join("config")) else {
        return run(repo, &["remote"]).map(|out| !out.stdout.is_empty()).unwrap_or(false);
    };
    text.lines()
        .map(str::trim)
        .any(|line| line.starts_with("[remote ") || line.starts_with("[remote\""))
}

fn parse_commits(text: &str) -> Vec<CommitRow> {
    let mut commits = Vec::new();
    for record in text.split('\0') {
        if record.is_empty() {
            continue;
        }
        let mut fields = record.split('\u{1f}');
        let id = fields.next().unwrap_or("").trim().to_string();
        if id.len() < 7 {
            continue;
        }
        let parents = fields.next().unwrap_or("").split_whitespace().map(str::to_string).collect();
        let author = fields.next().unwrap_or("").to_string();
        let when = fields.next().unwrap_or("").to_string();
        let summary = fields.next().unwrap_or("").to_string();
        let refs = parse_decoration(fields.next().unwrap_or(""));
        let at = fields.next().unwrap_or("").trim().parse().unwrap_or(0);
        let email = fields.next().unwrap_or("").trim().to_string();
        let short_id = id.chars().take(7).collect();
        commits.push(CommitRow {
            id,
            short_id,
            summary,
            author,
            when,
            parents,
            refs,
            lane: 0,
            at,
            email,
            unpushed: false,
            remote_only: false,
        });
    }
    commits
}

pub fn commit_detail(repo: &Path, id: &str) -> Result<CommitDetail, Error> {
    let id = check_rev(id)?;
    let output = with_read(|| {
        run(
            repo,
            &[
                "show",
                "-s",
                "--format=%H%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%P%x1f%s%x1e%b",
                id,
            ],
        )
    })?;
    let text = String::from_utf8_lossy(&output.stdout);
    let (head, body) = text.split_once('\u{1e}').unwrap_or((text.as_ref(), ""));
    let mut fields = head.split('\u{1f}');
    let id = fields.next().unwrap_or("").trim().to_string();
    let author = fields.next().unwrap_or("").trim().to_string();
    let author_email = fields.next().unwrap_or("").trim().to_string();
    let author_at = fields.next().unwrap_or("").trim().parse().unwrap_or(0);
    let committer = fields.next().unwrap_or("").trim().to_string();
    let committer_email = fields.next().unwrap_or("").trim().to_string();
    let committer_at = fields.next().unwrap_or("").trim().parse().unwrap_or(0);
    let parents = fields
        .next()
        .unwrap_or("")
        .split_whitespace()
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect();
    let summary = fields.next().unwrap_or("").trim().to_string();
    Ok(CommitDetail {
        id,
        summary,
        body: body.trim().to_string(),
        author,
        author_email,
        author_at,
        committer,
        committer_email,
        committer_at,
        parents,
    })
}

pub fn repository_refs(repo: &Path) -> Result<RefSnapshot, Error> {
    let local = local_text(repo);
    let first = |text: &str, section: &str, key: &str| values_in(text, section, key).into_iter().next().unwrap_or_default();
    let sign_off = first(&local, "signoff", "enabled");
    let head = run(repo, &["rev-parse", "--verify", "--quiet", "HEAD"])
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|id| !id.is_empty());
    std::thread::scope(|scope| {
        let branches = scope.spawn(|| local_branches(repo));
        let remotes = scope.spawn(|| remotes(repo));
        let tags = scope.spawn(|| tags(repo));
        let stashes = scope.spawn(|| stashes(repo));
        let submodules = scope.spawn(|| submodules(repo));
        let worktrees = worktrees(repo);
        Ok(RefSnapshot {
            head,
            branches: joined(branches)?,
            remotes: joined(remotes)?,
            tags: joined(tags)?,
            stashes: joined(stashes)?,
            submodules: joined(submodules)?,
            worktrees: worktrees?,
            hidden_refs: hidden_refs(repo),
            expanded_groups: values_in(&local, "awegit", "expandedGroup"),
            sign_off_set: !sign_off.is_empty(),
            sign_off: sign_off.eq_ignore_ascii_case("true"),
            sign_off_format: first(&local, "signoff", "format"),
        })
    })
}

fn joined<T>(handle: ScopedJoinHandle<'_, Result<T, Error>>) -> Result<T, Error> {
    handle.join().unwrap_or_else(|_| Err(Error::Git("a git read stopped unexpectedly".into())))
}

/// Names listed under `[awegit] hiddenRef` in `.git/awegit`, or the legacy `.git/gitnuro` file.
pub fn hidden_refs(repo: &Path) -> Vec<String> {
    let Ok(dir) = absolute_git_dir(repo) else {
        return Vec::new();
    };
    if let Ok(text) = std::fs::read_to_string(dir.join("awegit")) {
        return parse_hidden_refs(&text);
    }
    std::fs::read_to_string(dir.join("gitnuro"))
        .map(|text| parse_hidden_refs(&text))
        .unwrap_or_default()
}

/// Replace the `hiddenRef` list in `.git/awegit` and leave every other key in that file alone.
pub fn write_hidden_refs(repo: &Path, names: &[String]) -> Result<(), Error> {
    for name in names {
        if name.is_empty() || name.contains(['\n', '\r', '\0']) {
            return Err(Error::Rev(name.clone()));
        }
    }
    let dir = absolute_git_dir(repo)?;
    let path = dir.join("awegit");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let next = rewrite_hidden_refs(&existing, names);
    std::fs::write(&path, next).map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })
}

/// The work tree's own git directory. Remembered per path; a directory that disappeared is looked up again.
fn absolute_git_dir(repo: &Path) -> Result<PathBuf, Error> {
    if let Some(dir) = GIT_DIRS.lock().ok().and_then(|dirs| dirs.get(repo).cloned()) {
        if dir.is_dir() {
            return Ok(dir);
        }
    }
    let output = run(repo, &["rev-parse", "--absolute-git-dir"])?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        return Err(Error::Git("could not find the git directory".into()));
    }
    let dir = PathBuf::from(text);
    if let Ok(mut dirs) = GIT_DIRS.lock() {
        dirs.insert(repo.to_path_buf(), dir.clone());
    }
    Ok(dir)
}

fn parse_hidden_refs(text: &str) -> Vec<String> {
    let mut section = String::new();
    let mut names = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = section_name(line) {
            section = name;
            continue;
        }
        if section != "awegit" {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "hiddenRef" {
            let value = unquote_config(value.trim());
            if !value.is_empty() {
                names.push(value);
            }
        }
    }
    names
}

fn rewrite_hidden_refs(text: &str, names: &[String]) -> String {
    let mut out = String::new();
    let mut section = String::new();
    let mut saw = false;
    let mut inserted = false;
    for raw in text.lines() {
        let trimmed = raw.trim();
        if let Some(name) = section_name(trimmed) {
            if section == "awegit" && !inserted {
                push_hidden(&mut out, names);
                inserted = true;
            }
            section = name;
            if section == "awegit" {
                saw = true;
            }
            out.push_str(raw);
            out.push('\n');
            continue;
        }
        if section == "awegit" {
            if let Some((key, _)) = trimmed.split_once('=') {
                if key.trim() == "hiddenRef" {
                    continue;
                }
            }
        }
        out.push_str(raw);
        out.push('\n');
    }
    if section == "awegit" && !inserted {
        push_hidden(&mut out, names);
        inserted = true;
    }
    if !saw {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("[awegit]\n");
        push_hidden(&mut out, names);
    }
    let _ = inserted;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn section_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix('[')?;
    let end = rest.find(']')?;
    Some(rest[..end].split_whitespace().next().unwrap_or("").to_string())
}

fn push_hidden(out: &mut String, names: &[String]) {
    for name in names {
        out.push_str("\thiddenRef = ");
        out.push_str(&quote_config(name));
        out.push('\n');
    }
}

fn quote_config(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.' | '@'))
    {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

fn unquote_config(value: &str) -> String {
    let Some(inner) = value.strip_prefix('"').and_then(|text| text.strip_suffix('"')) else {
        return value.to_string();
    };
    inner.replace("\\\"", "\"").replace("\\\\", "\\")
}

fn local_text(repo: &Path) -> String {
    let Ok(dir) = absolute_git_dir(repo) else {
        return String::new();
    };
    if let Ok(text) = std::fs::read_to_string(dir.join("awegit")) {
        return text;
    }
    std::fs::read_to_string(dir.join("gitnuro")).unwrap_or_default()
}

fn value_key(repo: &Path, section: &str, key: &str) -> String {
    values_in(&local_text(repo), section, key).into_iter().next().unwrap_or_default()
}

fn values_in(text: &str, want_section: &str, want_key: &str) -> Vec<String> {
    let mut section = String::new();
    let mut values = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = section_name(line) {
            section = name;
            continue;
        }
        if section != want_section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == want_key {
            let value = unquote_config(value.trim());
            if !value.is_empty() {
                values.push(value);
            }
        }
    }
    values
}

pub fn write_expanded_groups(repo: &Path, names: &[String]) -> Result<(), Error> {
    for name in names {
        if name.is_empty() || name.contains(['\n', '\r', '\0']) {
            return Err(Error::Rev(name.clone()));
        }
    }
    rewrite_local_key(repo, "awegit", "expandedGroup", names)
}

/// `signoff.format` from `.git/awegit`. Empty when this repository has not set one.
pub fn repo_sign_off_format(repo: &Path) -> String {
    value_key(repo, "signoff", "format")
}

pub fn write_sign_off(repo: &Path, enabled: bool, format: &str) -> Result<(), Error> {
    if format.contains(['\n', '\r', '\0']) {
        return Err(Error::Rev(format.to_string()));
    }
    let dir = absolute_git_dir(repo)?;
    let path = dir.join("awegit");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let next = rewrite_sign_off(&existing, enabled, format);
    std::fs::write(&path, next).map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })
}

fn rewrite_local_key(repo: &Path, section: &str, key: &str, names: &[String]) -> Result<(), Error> {
    let dir = absolute_git_dir(repo)?;
    let path = dir.join("awegit");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let next = rewrite_key(&existing, section, key, names);
    std::fs::write(&path, next).map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })
}

fn rewrite_key(text: &str, want_section: &str, want_key: &str, names: &[String]) -> String {
    let mut out = String::new();
    let mut section = String::new();
    let mut saw = false;
    let mut inserted = false;
    for raw in text.lines() {
        let trimmed = raw.trim();
        if let Some(name) = section_name(trimmed) {
            if section == want_section && !inserted {
                push_key(&mut out, want_key, names);
                inserted = true;
            }
            section = name;
            if section == want_section {
                saw = true;
            }
            out.push_str(raw);
            out.push('\n');
            continue;
        }
        if section == want_section {
            if let Some((key, _)) = trimmed.split_once('=') {
                if key.trim() == want_key {
                    continue;
                }
            }
        }
        out.push_str(raw);
        out.push('\n');
    }
    if section == want_section && !inserted {
        push_key(&mut out, want_key, names);
        inserted = true;
    }
    if !saw {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('[');
        out.push_str(want_section);
        out.push_str("]\n");
        push_key(&mut out, want_key, names);
    }
    let _ = inserted;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn push_key(out: &mut String, key: &str, names: &[String]) {
    for name in names {
        out.push('\t');
        out.push_str(key);
        out.push_str(" = ");
        out.push_str(&quote_config(name));
        out.push('\n');
    }
}

fn rewrite_sign_off(text: &str, enabled: bool, format: &str) -> String {
    let mut out = String::new();
    let mut section = String::new();
    let mut saw = false;
    let mut inserted = false;
    for raw in text.lines() {
        let trimmed = raw.trim();
        if let Some(name) = section_name(trimmed) {
            if section == "signoff" && !inserted {
                push_sign_off(&mut out, enabled, format);
                inserted = true;
            }
            section = name;
            if section == "signoff" {
                saw = true;
            }
            out.push_str(raw);
            out.push('\n');
            continue;
        }
        if section == "signoff" {
            if let Some((key, _)) = trimmed.split_once('=') {
                let key = key.trim();
                if key == "enabled" || key == "format" {
                    continue;
                }
            }
        }
        out.push_str(raw);
        out.push('\n');
    }
    if section == "signoff" && !inserted {
        push_sign_off(&mut out, enabled, format);
        inserted = true;
    }
    if !saw {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("[signoff]\n");
        push_sign_off(&mut out, enabled, format);
    }
    let _ = inserted;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn push_sign_off(out: &mut String, enabled: bool, format: &str) {
    out.push_str(if enabled { "\tenabled = true\n" } else { "\tenabled = false\n" });
    if !format.is_empty() {
        out.push_str("\tformat = ");
        out.push_str(&quote_config(format));
        out.push('\n');
    }
}

/// For a merge commit (a stash is one too), the first parent to diff against.
/// Plain `diff-tree` and `show` print nothing for such commits.
fn merge_base_of(repo: &Path, id: &str) -> Option<String> {
    let second = format!("{id}^2");
    let output = run_output(repo, &["rev-parse", "--verify", "-q", &second]).ok()?;
    if !output.status.success() {
        return None;
    }
    Some(format!("{id}^1"))
}

pub fn commit_files(repo: &Path, id: &str) -> Result<Vec<FileChange>, Error> {
    let id = check_rev(id)?;
    let base = merge_base_of(repo, id);
    let (output, _) = with_read(|| match base {
        Some(ref base) => run_capped(repo, &["diff", "--name-status", "-r", "-M", base, id], 8_000_000),
        None => run_capped(repo, &["diff-tree", "--no-commit-id", "--name-status", "-r", "-M", "--root", id], 8_000_000),
    })?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();
    for line in text.lines() {
        let mut parts = line.split('\t');
        let status = parts.next().unwrap_or("");
        let first = parts.next().unwrap_or("");
        if first.is_empty() {
            continue;
        }
        let (kind, path, previous) = if status.starts_with('R') {
            (ChangeKind::Renamed, parts.next().unwrap_or(first), Some(first.to_string()))
        } else if status.starts_with('C') {
            (ChangeKind::Added, parts.next().unwrap_or(first), None)
        } else {
            let kind = match status.chars().next().unwrap_or('M') {
                'A' => ChangeKind::Added,
                'D' => ChangeKind::Deleted,
                'U' => ChangeKind::Conflict,
                _ => ChangeKind::Modified,
            };
            (kind, first, None)
        };
        files.push(FileChange {
            path: path.to_string(),
            kind,
            previous_path: previous.filter(|item| !item.is_empty()),
        });
    }
    Ok(files)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author: String,
    pub line: u32,
    pub text: String,
    #[serde(default)]
    pub truncated: bool,
}

pub fn blame_file(repo: &Path, path: &str) -> Result<Vec<BlameLine>, Error> {
    let path = check_path(path)?;
    let (output, capped) = with_read(|| run_capped(repo, &["--literal-pathspecs", "blame", "--porcelain", "--", path], 8_000_000))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = Vec::new();
    let mut id = String::new();
    let mut summary = String::new();
    let mut author = String::new();
    let mut line_no = 0_u32;
    for raw in text.lines() {
        if raw.starts_with('\t') {
            let short_id = id.chars().take(7).collect();
            lines.push(BlameLine {
                id: id.clone(),
                short_id,
                summary: summary.clone(),
                author: author.clone(),
                line: line_no,
                text: raw[1..].to_string(),
                truncated: false,
            });
            continue;
        }
        if raw.len() >= 40 && raw.as_bytes()[..40].iter().all(|byte| byte.is_ascii_hexdigit()) {
            let mut parts = raw.split_whitespace();
            id = parts.next().unwrap_or("").to_string();
            let _original = parts.next();
            line_no = parts.next().unwrap_or("0").parse().unwrap_or(line_no);
            continue;
        }
        if let Some(value) = raw.strip_prefix("summary ") {
            summary = value.to_string();
        } else if let Some(value) = raw.strip_prefix("author ") {
            author = value.to_string();
        }
    }
    if capped {
        lines.push(BlameLine {
            id: String::new(),
            short_id: String::new(),
            summary: String::new(),
            author: String::new(),
            line: 0,
            text: "…".into(),
            truncated: true,
        });
    }
    Ok(lines)
}

pub fn show_commit_file(repo: &Path, id: &str, path: &str) -> Result<FileDiff, Error> {
    show_commit_file_with(repo, id, path, 3)
}

pub fn show_commit_file_with(repo: &Path, id: &str, path: &str, context: u32) -> Result<FileDiff, Error> {
    let id = check_rev(id)?;
    let path = check_path(path)?;
    let unified = format!("--unified={}", context.min(1_000_000));
    let base = merge_base_of(repo, id);
    let (output, capped) = with_read(|| match base {
        Some(ref base) => run_capped(
            repo,
            &["--literal-pathspecs", "diff", "--no-ext-diff", "--no-color", &unified, base, id, "--", path],
            8_000_000,
        ),
        None => run_capped(
            repo,
            &[
                "--literal-pathspecs",
                "show",
                "--no-ext-diff",
                "--no-color",
                &unified,
                "--format=",
                id,
                "--",
                path,
            ],
            8_000_000,
        ),
    })?;
    let mut parsed = parse_diff(path, false, &String::from_utf8_lossy(&output.stdout));
    parsed.truncated |= capped;
    if parsed.binary {
        let old_rev = base.unwrap_or_else(|| format!("{id}^"));
        parsed.old_bytes = spec_size(repo, &format!("{old_rev}:{path}"));
        parsed.new_bytes = spec_size(repo, &format!("{id}:{path}"));
    }
    Ok(parsed)
}

/// Files that differ between two revisions.
pub fn compare_files(repo: &Path, from: &str, to: &str) -> Result<Vec<FileChange>, Error> {
    let from = check_rev(from)?;
    let to = check_rev(to)?;
    let (output, _) = with_read(|| run_capped(repo, &["diff", "--name-status", "-r", "-M", from, to], 8_000_000))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();
    for line in text.lines() {
        let mut parts = line.split('\t');
        let status = parts.next().unwrap_or("");
        let first = parts.next().unwrap_or("");
        if first.is_empty() {
            continue;
        }
        let (kind, path) = if status.starts_with('R') || status.starts_with('C') {
            (ChangeKind::Renamed, parts.next().unwrap_or(first))
        } else {
            let kind = match status.chars().next().unwrap_or('M') {
                'A' => ChangeKind::Added,
                'D' => ChangeKind::Deleted,
                _ => ChangeKind::Modified,
            };
            (kind, first)
        };
        files.push(FileChange {
            path: path.to_string(),
            kind,
            previous_path: None,
        });
    }
    Ok(files)
}

/// Diff of one path between two revisions.
/// Staged file names, the current branch, and the last five subjects. Used to fill a prompt.
pub fn prompt_facts(repo: &Path) -> (String, String, String) {
    let files = run(repo, &["diff", "--cached", "--name-only"])
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();
    let branch = run(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();
    let branch = if branch.is_empty() { "HEAD".to_string() } else { branch };
    let recent = run(repo, &["log", "-5", "--format=%s"])
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();
    (files, branch, recent)
}

pub fn compare_file(repo: &Path, from: &str, to: &str, path: &str, context: u32) -> Result<FileDiff, Error> {
    let from = check_rev(from)?;
    let to = check_rev(to)?;
    let path = check_path(path)?;
    let unified = format!("--unified={}", context.min(1_000_000));
    let (output, capped) = with_read(|| {
        run_capped(
            repo,
            &["--literal-pathspecs", "diff", "--no-ext-diff", "--no-color", &unified, from, to, "--", path],
            8_000_000,
        )
    })?;
    let mut parsed = parse_diff(path, false, &String::from_utf8_lossy(&output.stdout));
    parsed.truncated |= capped;
    if parsed.binary {
        parsed.old_bytes = spec_size(repo, &format!("{from}:{path}"));
        parsed.new_bytes = spec_size(repo, &format!("{to}:{path}"));
    }
    Ok(parsed)
}

pub fn in_progress(repo: &Path) -> Result<Option<InProgress>, Error> {
    let dir = absolute_git_dir(repo)?;
    let exists = |name: &str| dir.join(name).exists();
    if exists("rebase-merge") || exists("rebase-apply") {
        return Ok(Some(InProgress::Rebase));
    }
    if exists("MERGE_HEAD") {
        return Ok(Some(InProgress::Merge));
    }
    if exists("CHERRY_PICK_HEAD") {
        return Ok(Some(InProgress::CherryPick));
    }
    if exists("REVERT_HEAD") {
        return Ok(Some(InProgress::Revert));
    }
    Ok(None)
}

fn local_branches(repo: &Path) -> Result<Vec<BranchRow>, Error> {
    let output = run(
        repo,
        &["for-each-ref", "--format=%(refname:short)%09%(upstream:short)%09%(upstream:track)%09%(HEAD)", "refs/heads"],
    )?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut branches = Vec::new();
    for line in text.lines() {
        let mut fields = line.split('\t');
        let name = fields.next().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let upstream = fields.next().filter(|item| !item.is_empty()).map(str::to_string);
        let track = fields.next().unwrap_or("");
        let current = fields.next().unwrap_or("") == "*";
        let (ahead, behind) = parse_track(track);
        branches.push(BranchRow { name, upstream, ahead, behind, current });
    }
    Ok(branches)
}

fn remotes(repo: &Path) -> Result<Vec<RemoteRow>, Error> {
    let output = run(repo, &["remote", "-v"])?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut rows: Vec<RemoteRow> = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let name = parts.next().unwrap_or("");
        let url = parts.next().unwrap_or("");
        let kind = parts.next().unwrap_or("");
        if name.is_empty() || kind != "(fetch)" {
            continue;
        }
        rows.push(RemoteRow {
            name: name.to_string(),
            url: Some(url.to_string()),
            head: None,
            branches: Vec::new(),
        });
    }
    let refs = run(repo, &["for-each-ref", "--format=%(refname:short)%09%(symref:short)", "refs/remotes"])?;
    let refs = String::from_utf8_lossy(&refs.stdout);
    for line in refs.lines() {
        let mut fields = line.split('\t');
        let name = fields.next().unwrap_or("");
        let symref = fields.next().unwrap_or("");
        let Some((remote, branch)) = name.split_once('/') else { continue };
        let Some(row) = rows.iter_mut().find(|row| row.name == remote) else { continue };
        if branch == "HEAD" {
            row.head = symref.split_once('/').map(|(_, branch)| branch.to_string()).or_else(|| {
                if symref.is_empty() { None } else { Some(symref.to_string()) }
            });
        } else {
            row.branches.push(branch.to_string());
        }
    }
    for row in &mut rows {
        row.branches.sort();
    }
    Ok(rows)
}

fn tags(repo: &Path) -> Result<Vec<TagRow>, Error> {
    let output = run(repo, &["for-each-ref", "--sort=-creatordate", "--format=%(refname:short)%09%(objectname:short)", "refs/tags"])?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| {
            let (name, id) = line.split_once('\t')?;
            if name.is_empty() {
                None
            } else {
                Some(TagRow { name: name.to_string(), id: id.to_string() })
            }
        })
        .collect())
}

fn stashes(repo: &Path) -> Result<Vec<StashRow>, Error> {
    let output = run(repo, &["stash", "list", "--pretty=format:%H%x09%P%x09%gd%x09%gs"])?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\t');
            let id = parts.next()?.to_string();
            let parents = parts.next()?.to_string();
            let name = parts.next()?.to_string();
            let summary = parts.next()?.to_string();
            let parent = parents.split_whitespace().next().unwrap_or("").to_string();
            Some(StashRow { name, summary, id, parent })
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflogRow {
    pub id: String,
    pub short_id: String,
    pub selector: String,
    pub summary: String,
}

/// Recent HEAD movements. `selector` is a name such as `HEAD@{0}`.
pub fn reflog(repo: &Path, limit: usize) -> Result<Vec<ReflogRow>, Error> {
    let limit = limit.clamp(1, 2000);
    let output = run(repo, &["reflog", &format!("-n{limit}"), "--format=%H%x09%h%x09%gd%x09%gs"])?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\t');
            Some(ReflogRow {
                id: parts.next()?.to_string(),
                short_id: parts.next()?.to_string(),
                selector: parts.next()?.to_string(),
                summary: parts.next().unwrap_or("").to_string(),
            })
        })
        .collect())
}

/// Paths in the tree of `rev`.
pub fn commit_tree(repo: &Path, rev: &str) -> Result<Vec<String>, Error> {
    let rev = check_rev(rev)?;
    let (output, _) = with_read(|| run_capped(repo, &["ls-tree", "-r", "--name-only", rev], 8_000_000))?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().filter(|line| !line.is_empty()).map(str::to_string).collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSummary {
    pub path: String,
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub commits: u32,
    pub branches: u32,
    pub last_summary: String,
    pub last_when: String,
}

/// Lightweight facts for the repository list. Missing upstream leaves ahead and behind at zero.
pub fn repository_summary(repo: &Path) -> Result<RepoSummary, Error> {
    let branch = String::from_utf8_lossy(&run(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?.stdout).trim().to_string();
    let commits = parse_count(&run(repo, &["rev-list", "--count", "HEAD"])?.stdout);
    let branches = run(repo, &["branch", "--list"])
        .map(|output| String::from_utf8_lossy(&output.stdout).lines().filter(|line| !line.trim().is_empty()).count() as u32)
        .unwrap_or(0);
    let (ahead, behind) = run_output(repo, &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            let text = String::from_utf8_lossy(&output.stdout);
            let mut parts = text.split_whitespace();
            (parse_count(parts.next().unwrap_or("0").as_bytes()), parse_count(parts.next().unwrap_or("0").as_bytes()))
        })
        .unwrap_or((0, 0));
    let last = run(repo, &["log", "-1", "--format=%s%x09%ar"]).ok();
    let last_text = last.map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string()).unwrap_or_default();
    let (last_summary, last_when) = last_text.split_once('\t').unwrap_or((last_text.as_str(), ""));
    Ok(RepoSummary {
        path: repo.display().to_string(),
        branch,
        ahead,
        behind,
        commits,
        branches,
        last_summary: last_summary.to_string(),
        last_when: last_when.to_string(),
    })
}

fn parse_count(bytes: &[u8]) -> u32 {
    String::from_utf8_lossy(bytes).trim().parse().unwrap_or(0)
}

/// Files that would conflict if HEAD were rebased onto `onto`. An empty list means a clean replay.
pub fn rebase_conflicts(repo: &Path, onto: &str) -> Result<Vec<String>, Error> {
    let onto = check_rev(onto)?;
    let output = run_output(repo, &["merge-tree", "--write-tree", "--name-only", "HEAD", onto])?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("unknown option") || stderr.contains("usage:") {
        return Err(Error::Git(stderr.trim().to_string()));
    }
    if output.status.success() {
        return Ok(Vec::new());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("CONFLICT") && !line.starts_with("Auto-merging") && !is_object_id(line))
        .map(str::to_string)
        .collect())
}

fn is_object_id(line: &str) -> bool {
    line.len() == 40 && line.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LfsLockRow {
    pub path: String,
    pub owner: String,
}

/// True when `git lfs` runs. Checked once per process.
pub fn lfs_available(repo: &Path) -> bool {
    *LFS_INSTALLED.get_or_init(|| {
        run_output(repo, &["lfs", "version"])
            .map(|output| output.status.success())
            .unwrap_or(false)
    })
}

/// Locked LFS paths. An unavailable `git lfs` is reported as not available rather than a failure.
pub fn lfs_locks(repo: &Path) -> Result<(bool, Vec<LfsLockRow>), Error> {
    if !lfs_available(repo) {
        return Ok((false, Vec::new()));
    }
    let output = run_output(repo, &["lfs", "locks"])?;
    if !output.status.success() {
        return Ok((true, Vec::new()));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let rows = text
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.to_ascii_lowercase().starts_with("path"))
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let path = parts.next()?.to_string();
            let owner = parts.next().unwrap_or("").to_string();
            Some(LfsLockRow { path, owner })
        })
        .collect();
    Ok((true, rows))
}

/// Paths from `paths` that ignore rules exclude. Tracked files are never reported. Paths are relative to the work tree.
pub fn ignored_paths(repo: &Path, paths: &[String]) -> Vec<String> {
    if paths.is_empty() {
        return Vec::new();
    }
    let mut input = Vec::new();
    for path in paths {
        input.extend_from_slice(path.as_bytes());
        input.push(0);
    }
    let Ok(output) = run_stdin_output(repo, &["check-ignore", "-z", "--stdin"], &input) else {
        return Vec::new();
    };
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect()
}

/// Gitlinks named in `.gitmodules`. `git submodule status` also describes each commit, which costs
/// about half a second on Windows, so the index and each checkout are read directly instead.
fn submodules(repo: &Path) -> Result<Vec<SubmoduleRow>, Error> {
    if !repo.join(".gitmodules").is_file() {
        return Ok(Vec::new());
    }
    let config = run_output(repo, &["config", "-z", "--file", ".gitmodules", "--get-regexp", r"^submodule\..*\.path$"])?;
    let paths: Vec<String> = config
        .stdout
        .split(|byte| *byte == 0)
        .filter_map(|record| {
            let record = String::from_utf8_lossy(record);
            let (_, path) = record.split_once('\n')?;
            let path = path.trim();
            (!path.is_empty()).then(|| path.to_string())
        })
        .collect();
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = vec!["--literal-pathspecs", "ls-files", "--stage", "-z", "--"];
    args.extend(paths.iter().map(String::as_str));
    let output = run(repo, &args)?;
    let mut rows: Vec<SubmoduleRow> = Vec::new();
    for record in output.stdout.split(|byte| *byte == 0) {
        let record = String::from_utf8_lossy(record);
        let Some((meta, path)) = record.split_once('\t') else { continue };
        let mut fields = meta.split_whitespace();
        if fields.next() != Some("160000") || rows.iter().any(|row| row.path == path) {
            continue;
        }
        let staged = fields.next().unwrap_or("").to_string();
        let checkout = repo.join(path);
        let ready = checkout.join(".git").exists();
        let id = if ready { checked_out(&checkout).unwrap_or(staged) } else { staged };
        rows.push(SubmoduleRow { path: path.to_string(), id, ready });
    }
    rows.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(rows)
}

fn checked_out(dir: &Path) -> Option<String> {
    if let Some(id) = head_from_files(dir) {
        return Some(id);
    }
    let repo = gix::open(dir).ok()?;
    let id = repo.head_id().ok()?;
    Some(id.to_string())
}

/// Read `HEAD` without opening the whole repository. Submodule refs are small, and
/// doing this for every gitlink on each refs refresh was the dominant cost.
fn head_from_files(dir: &Path) -> Option<String> {
    let dot = dir.join(".git");
    let git_dir = if dot.is_dir() {
        dot
    } else {
        let text = std::fs::read_to_string(&dot).ok()?;
        let line = text.lines().find(|line| !line.trim().is_empty())?;
        let rest = line.trim().strip_prefix("gitdir:")?.trim();
        let path = PathBuf::from(rest);
        if path.is_absolute() { path } else { dir.join(path) }
    };
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    let candidate = match head.strip_prefix("ref:") {
        Some(name) => read_ref(&git_dir, name.trim())?,
        None => head.to_string(),
    };
    let id = candidate.split_whitespace().next().unwrap_or("");
    (id.len() >= 7 && id.bytes().all(|byte| byte.is_ascii_hexdigit())).then(|| id.to_string())
}

fn read_ref(git_dir: &Path, name: &str) -> Option<String> {
    if let Ok(text) = std::fs::read_to_string(git_dir.join(name)) {
        return Some(text);
    }
    let common = std::fs::read_to_string(git_dir.join("commondir"))
        .ok()
        .map(|text| {
            let path = PathBuf::from(text.trim());
            if path.is_absolute() { path } else { git_dir.join(path) }
        })
        .unwrap_or_else(|| git_dir.to_path_buf());
    if let Ok(text) = std::fs::read_to_string(common.join(name)) {
        return Some(text);
    }
    let packed = std::fs::read_to_string(common.join("packed-refs")).ok()?;
    packed
        .lines()
        .filter(|line| !line.starts_with(['#', '^']))
        .find_map(|line| {
            let (id, path) = line.split_once(' ')?;
            (path.trim() == name).then(|| id.to_string())
        })
}

fn worktrees(repo: &Path) -> Result<Vec<WorktreeRow>, Error> {
    let output = run(repo, &["worktree", "list", "--porcelain"])?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut rows = Vec::new();
    let mut current: Option<WorktreeRow> = None;
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(row) = current.take() {
                rows.push(row);
            }
            current = Some(WorktreeRow { path: path.to_string(), branch: None, detached: false });
        } else if let Some(branch) = line.strip_prefix("branch ") {
            if let Some(row) = current.as_mut() {
                row.branch = branch.strip_prefix("refs/heads/").map(str::to_string);
            }
        } else if line == "detached" {
            if let Some(row) = current.as_mut() {
                row.detached = true;
            }
        }
    }
    if let Some(row) = current {
        rows.push(row);
    }
    Ok(rows)
}

fn parse_track(text: &str) -> (u32, u32) {
    let mut ahead = 0;
    let mut behind = 0;
    for part in text.trim().trim_matches(['[', ']']).split(',') {
        let part = part.trim();
        if let Some(number) = part.strip_prefix("ahead ") {
            ahead = number.parse().unwrap_or(0);
        } else if let Some(number) = part.strip_prefix("behind ") {
            behind = number.parse().unwrap_or(0);
        }
    }
    (ahead, behind)
}

fn parse_decoration(text: &str) -> Vec<String> {
    const MAX_REFS: usize = 32;
    const MAX_REF_CHARS: usize = 120;
    let mut names: Vec<String> = Vec::new();
    for item in text.split(',').map(str::trim).filter(|item| !item.is_empty()) {
        // `origin/HEAD -> origin/main` and bare `origin/HEAD` alias a remote
        // branch that is already decorated on the same commit.
        let source = item
            .split_once("->")
            .or_else(|| item.split_once('→'))
            .map(|(source, _)| source.trim())
            .unwrap_or(item);
        if source != "HEAD" && source.ends_with("/HEAD") {
            continue;
        }
        let name = clean_ref(item);
        if name.is_empty() || name == "HEAD" || name.ends_with("/HEAD") {
            continue;
        }
        if !names.iter().any(|seen| seen == name) {
            names.push(name.to_string());
        }
        if names.len() >= MAX_REFS {
            break;
        }
    }
    names
        .into_iter()
        .map(|item| {
            if item.chars().count() <= MAX_REF_CHARS {
                item
            } else {
                let mut short: String = item.chars().take(MAX_REF_CHARS).collect();
                short.push('…');
                short
            }
        })
        .collect()
}

fn clean_ref(item: &str) -> &str {
    let item = item.strip_prefix("tag:").map(str::trim).unwrap_or(item);
    if let Some((_, name)) = item.split_once("->") {
        name.trim()
    } else if let Some((_, name)) = item.split_once('→') {
        name.trim()
    } else {
        item
    }
}

fn assign_lanes(commits: &mut [CommitRow]) {
    use std::collections::{BTreeSet, HashMap};

    /// Lane slots with an index by commit id and the set of free slots, so each
    /// lookup stays cheap on histories with tens of thousands of commits.
    #[derive(Default)]
    struct Lanes {
        slots: Vec<Option<String>>,
        by_id: HashMap<String, BTreeSet<usize>>,
        free: BTreeSet<usize>,
    }

    impl Lanes {
        fn set(&mut self, lane: usize, value: Option<String>) {
            if let Some(old) = self.slots[lane].take() {
                if let Some(lanes) = self.by_id.get_mut(&old) {
                    lanes.remove(&lane);
                    if lanes.is_empty() {
                        self.by_id.remove(&old);
                    }
                }
            }
            match &value {
                Some(id) => {
                    self.free.remove(&lane);
                    self.by_id.entry(id.clone()).or_default().insert(lane);
                }
                None => {
                    self.free.insert(lane);
                }
            }
            self.slots[lane] = value;
        }

        /// Put `id` in the lowest free slot, or in a new one.
        fn place(&mut self, id: &str) -> usize {
            let lane = match self.free.iter().next().copied() {
                Some(free) => free,
                None => {
                    self.slots.push(None);
                    self.slots.len() - 1
                }
            };
            self.set(lane, Some(id.to_string()));
            lane
        }
    }

    let mut lanes = Lanes::default();
    for commit in commits.iter_mut() {
        let found = lanes.by_id.get(commit.id.as_str()).and_then(|set| set.iter().next().copied());
        let lane = match found {
            Some(lane) => lane,
            None => lanes.place(&commit.id),
        };
        commit.lane = lane as u32;
        match commit.parents.first() {
            Some(parent) => lanes.set(lane, Some(parent.clone())),
            None => lanes.set(lane, None),
        }
        for parent in commit.parents.iter().skip(1) {
            lanes.place(parent);
        }
    }
}

#[cfg(test)]
mod locate_tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn git(path: &Path, args: &[&str]) {
        let output = Command::new("git").current_dir(path).args(args).output().unwrap();
        assert!(output.status.success(), "git {args:?} {}", String::from_utf8_lossy(&output.stderr));
    }

    #[test]
    fn grows_the_page_until_the_old_branch_is_in_it() {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("awegit-locate-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        git(&path, &["init", "-b", "main"]);
        git(&path, &["config", "user.name", "Test"]);
        git(&path, &["config", "user.email", "t@e.com"]);
        git(&path, &["config", "commit.gpgsign", "false"]);
        git(&path, &["config", "core.hooksPath", ".git/no-hooks"]);
        git(&path, &["commit", "--allow-empty", "-m", "base"]);
        git(&path, &["branch", "buried"]);
        let buried = String::from_utf8_lossy(&Command::new("git").current_dir(&path).args(["rev-parse", "buried"]).output().unwrap().stdout)
            .trim()
            .to_string();
        for i in 0..24 {
            git(&path, &["commit", "--allow-empty", "-m", &format!("feat: later {i}")]);
        }
        let found = locate_ref_from(&path, "buried", false, 8).unwrap();
        let _ = fs::remove_dir_all(&path);
        assert_eq!(found.id, buried);
        assert!(found.commits.len() > 8, "{}", found.commits.len());
        assert!(found.commits.iter().any(|commit| commit.id == buried));
    }
}
