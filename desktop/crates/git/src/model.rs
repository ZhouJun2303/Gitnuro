//! Commit graph and the sidebar's branches, remotes, tags, stashes, submodules, and worktrees.
//!
//! These reads go through `git` porcelain so decorations, ahead/behind, and submodule
//! state match the Git the user already has installed.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::change::{parse_diff, FileDiff};
use crate::cli::{check_path, check_rev, run};
use crate::{ChangeKind, Error, FileChange};

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
    pub branches: Vec<BranchRow>,
    pub remotes: Vec<RemoteRow>,
    pub tags: Vec<TagRow>,
    pub stashes: Vec<StashRow>,
    pub submodules: Vec<SubmoduleRow>,
    pub worktrees: Vec<WorktreeRow>,
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
    log_with(repo, limit, true, &[])
}

/// Commits reachable from `HEAD` only, without `--all`.
pub fn branch_commits(repo: &Path, limit: usize) -> Result<Vec<CommitRow>, Error> {
    log_with(repo, limit, false, &[])
}

pub fn file_history(repo: &Path, path: &str, limit: usize) -> Result<Vec<CommitRow>, Error> {
    let path = check_path(path)?;
    log_with(repo, limit, false, &[path])
}

fn log_with(repo: &Path, limit: usize, all: bool, path: &[&str]) -> Result<Vec<CommitRow>, Error> {
    if run(repo, &["rev-parse", "--verify", "HEAD"]).is_err() {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 2000).to_string();
    let mut args = vec![
        "log",
        "-n",
        &limit,
        "--date-order",
        "-z",
        "--pretty=format:%H%x1f%P%x1f%an%x1f%ar%x1f%s%x1f%D%x1f%at",
    ];
    if path.is_empty() && all {
        args.insert(1, "--all");
    } else if !path.is_empty() {
        args.push("--");
        args.extend_from_slice(path);
    }
    let output = run(repo, &args)?;
    let mut commits = parse_commits(&String::from_utf8_lossy(&output.stdout));
    assign_lanes(&mut commits);
    Ok(commits)
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
        });
    }
    commits
}

pub fn repository_refs(repo: &Path) -> Result<RefSnapshot, Error> {
    Ok(RefSnapshot {
        branches: local_branches(repo)?,
        remotes: remotes(repo)?,
        tags: tags(repo)?,
        stashes: stashes(repo)?,
        submodules: submodules(repo)?,
        worktrees: worktrees(repo)?,
    })
}

pub fn commit_files(repo: &Path, id: &str) -> Result<Vec<FileChange>, Error> {
    let id = check_rev(id)?;
    let output = run(repo, &["diff-tree", "--no-commit-id", "--name-status", "-r", "-M", "--root", id])?;
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
}

pub fn blame_file(repo: &Path, path: &str) -> Result<Vec<BlameLine>, Error> {
    let path = check_path(path)?;
    let output = run(repo, &["blame", "--porcelain", "--", path])?;
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
    Ok(lines)
}

pub fn show_commit_file(repo: &Path, id: &str, path: &str) -> Result<FileDiff, Error> {
    show_commit_file_with(repo, id, path, 3)
}

pub fn show_commit_file_with(repo: &Path, id: &str, path: &str, context: u32) -> Result<FileDiff, Error> {
    let id = check_rev(id)?;
    let path = check_path(path)?;
    let unified = format!("--unified={}", context.min(1_000_000));
    let output = run(
        repo,
        &[
            "show",
            "--no-ext-diff",
            "--no-color",
            &unified,
            "--format=",
            id,
            "--",
            path,
        ],
    )?;
    Ok(parse_diff(path, false, &String::from_utf8_lossy(&output.stdout)))
}

pub fn in_progress(repo: &Path) -> Result<Option<InProgress>, Error> {
    if git_path_exists(repo, "rebase-merge")? || git_path_exists(repo, "rebase-apply")? {
        return Ok(Some(InProgress::Rebase));
    }
    if git_path_exists(repo, "MERGE_HEAD")? {
        return Ok(Some(InProgress::Merge));
    }
    if git_path_exists(repo, "CHERRY_PICK_HEAD")? {
        return Ok(Some(InProgress::CherryPick));
    }
    if git_path_exists(repo, "REVERT_HEAD")? {
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
    let output = run(repo, &["stash", "list", "--pretty=format:%gd%x09%gs"])?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| {
            let (name, summary) = line.split_once('\t')?;
            Some(StashRow { name: name.to_string(), summary: summary.to_string() })
        })
        .collect())
}

fn submodules(repo: &Path) -> Result<Vec<SubmoduleRow>, Error> {
    let output = match run(repo, &["submodule", "status"]) {
        Ok(output) => output,
        Err(Error::Git(message)) if message.contains("no submodule") => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let ready = !line.starts_with('-');
        let line = line.trim_start_matches([' ', '-', '+', 'U']);
        let mut parts = line.split_whitespace();
        let id = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("").to_string();
        if path.is_empty() {
            continue;
        }
        rows.push(SubmoduleRow { path, id, ready });
    }
    Ok(rows)
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

fn git_path_exists(repo: &Path, name: &str) -> Result<bool, Error> {
    let output = run(repo, &["rev-parse", "--git-path", name])?;
    let text = String::from_utf8_lossy(&output.stdout);
    let relative = PathBuf::from(text.trim());
    let full = if relative.is_absolute() { relative } else { repo.join(relative) };
    Ok(full.exists())
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
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(clean_ref)
        .filter(|item| *item != "HEAD" && !item.is_empty())
        .map(str::to_string)
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
    let mut active: Vec<Option<String>> = Vec::new();
    for commit in commits.iter_mut() {
        let found = active.iter().position(|slot| slot.as_deref() == Some(commit.id.as_str()));
        let lane = if let Some(lane) = found {
            lane
        } else if let Some(free) = active.iter().position(|slot| slot.is_none()) {
            active[free] = Some(commit.id.clone());
            free
        } else {
            active.push(Some(commit.id.clone()));
            active.len() - 1
        };
        commit.lane = lane as u32;
        if let Some(parent) = commit.parents.first() {
            active[lane] = Some(parent.clone());
        } else {
            active[lane] = None;
        }
        for parent in commit.parents.iter().skip(1) {
            if let Some(free) = active.iter().position(|slot| slot.is_none()) {
                active[free] = Some(parent.clone());
            } else {
                active.push(Some(parent.clone()));
            }
        }
    }
}
