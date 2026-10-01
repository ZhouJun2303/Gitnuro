//! Stage, unstage, and commit through the `git` command.

use std::path::Path;

use serde::Serialize;

use crate::cli::{check_path, run};
use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRequest {
    pub summary: String,
    pub description: String,
    pub amend: bool,
    pub sign_off: bool,
}

/// Stage the given work-tree paths, including new files.
pub fn stage_paths(repo: &Path, paths: &[String]) -> Result<(), Error> {
    git_paths(repo, &["add", "--"], paths)
}

/// Stage every change in the work tree. Ignored files stay ignored.
pub fn stage_all(repo: &Path) -> Result<(), Error> {
    run(repo, &["add", "-A", "--", "."]).map(|_| ())
}

/// Remove the given paths from the index and leave the work tree as it is.
pub fn unstage_paths(repo: &Path, paths: &[String]) -> Result<(), Error> {
    match git_paths(repo, &["restore", "--staged", "--"], paths) {
        Ok(()) => Ok(()),
        // A repository with no commits yet has nothing to restore from.
        Err(Error::Git(message)) if message.contains("HEAD") => {
            git_paths(repo, &["rm", "--cached", "--ignore-unmatch", "--"], paths)
        }
        Err(error) => Err(error),
    }
}

/// Unstage everything that is currently in the index relative to `HEAD`.
pub fn unstage_all(repo: &Path) -> Result<(), Error> {
    match run(repo, &["restore", "--staged", "--", "."]) {
        Ok(_) => Ok(()),
        Err(Error::Git(message)) if message.contains("HEAD") => {
            run(repo, &["rm", "-r", "--cached", "--ignore-unmatch", "--", "."]).map(|_| ())
        }
        Err(error) => Err(error),
    }
}

/// Create a commit from the index. An empty summary is rejected before `git` runs.
pub fn commit(repo: &Path, request: CommitRequest) -> Result<(), Error> {
    let summary = request.summary.trim();
    if summary.is_empty() {
        return Err(Error::EmptySummary);
    }
    let description = request.description.trim();
    let mut args = vec!["commit".to_string(), "-m".to_string(), summary.to_string()];
    if !description.is_empty() {
        args.push("-m".to_string());
        args.push(description.to_string());
    }
    if request.amend {
        args.push("--amend".to_string());
    }
    if request.sign_off {
        args.push("--signoff".to_string());
    }
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run(repo, &borrowed).map(|_| ())
}

fn git_paths(repo: &Path, prefix: &[&str], paths: &[String]) -> Result<(), Error> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut owned: Vec<String> = prefix.iter().map(|part| (*part).to_string()).collect();
    for path in paths {
        owned.push(check_path(path)?.to_string());
    }
    let borrowed: Vec<&str> = owned.iter().map(String::as_str).collect();
    run(repo, &borrowed).map(|_| ())
}

/// One line of a unified diff, ready for the file view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffLineKind {
    Add,
    Delete,
    Hunk,
    Context,
    Meta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub staged: bool,
    pub binary: bool,
    pub truncated: bool,
    pub lines: Vec<DiffLine>,
}

const MAX_DIFF_LINES: usize = 4_000;
const MAX_UNTRACKED_BYTES: u64 = 1_048_576;

/// Unified diff for one path. `staged` selects the index rather than the work tree.
pub fn file_diff(repo: &Path, path: &str, staged: bool) -> Result<FileDiff, Error> {
    let path = check_path(path)?;
    let mut args = vec!["diff", "--no-ext-diff", "--no-color", "--unified=3"];
    if staged {
        args.push("--cached");
    }
    args.push("--");
    args.push(path);
    let output = run(repo, &args)?;
    let text = String::from_utf8_lossy(&output.stdout);
    if !staged && text.trim().is_empty() && !in_index(repo, path)? {
        return diff_untracked(repo, path);
    }
    Ok(parse_diff(path, staged, &text))
}

fn in_index(repo: &Path, path: &str) -> Result<bool, Error> {
    let output = run(repo, &["ls-files", "--", path])?;
    Ok(!output.stdout.is_empty())
}

fn diff_untracked(repo: &Path, path: &str) -> Result<FileDiff, Error> {
    let full = repo.join(path);
    let metadata = match std::fs::metadata(&full) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FileDiff {
                path: path.to_string(),
                staged: false,
                binary: false,
                truncated: false,
                lines: Vec::new(),
            });
        }
        Err(source) => {
            return Err(Error::Read {
                path: path.to_string(),
                source,
            });
        }
    };
    if metadata.len() > MAX_UNTRACKED_BYTES || !metadata.is_file() {
        return Ok(FileDiff {
            path: path.to_string(),
            staged: false,
            binary: true,
            truncated: metadata.len() > MAX_UNTRACKED_BYTES,
            lines: Vec::new(),
        });
    }
    let bytes = std::fs::read(&full).map_err(|source| Error::Read {
        path: path.to_string(),
        source,
    })?;
    if bytes.contains(&0) {
        return Ok(FileDiff {
            path: path.to_string(),
            staged: false,
            binary: true,
            truncated: false,
            lines: Vec::new(),
        });
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut parts: Vec<&str> = text.split('\n').map(|line| line.trim_end_matches('\r')).collect();
    if text.ends_with('\n') {
        parts.pop();
    }
    let mut body: Vec<DiffLine> = parts
        .into_iter()
        .map(|line| DiffLine {
            kind: DiffLineKind::Add,
            text: format!("+{line}"),
        })
        .collect();
    let truncated = body.len() > MAX_DIFF_LINES;
    body.truncate(MAX_DIFF_LINES);
    let mut lines = vec![DiffLine {
        kind: DiffLineKind::Hunk,
        text: format!("@@ -0,0 +1,{} @@", body.len()),
    }];
    lines.append(&mut body);
    Ok(FileDiff {
        path: path.to_string(),
        staged: false,
        binary: false,
        truncated,
        lines,
    })
}

fn parse_diff(path: &str, staged: bool, text: &str) -> FileDiff {
    let mut lines = Vec::new();
    let mut binary = false;
    let mut truncated = false;
    for raw in text.lines() {
        if raw.starts_with("Binary files ") {
            binary = true;
            break;
        }
        if raw.starts_with("diff --git ") || raw.starts_with("index ") {
            continue;
        }
        if lines.len() == MAX_DIFF_LINES {
            truncated = true;
            break;
        }
        let kind = if raw.starts_with("@@") {
            DiffLineKind::Hunk
        } else if raw.starts_with('+') && !raw.starts_with("+++") {
            DiffLineKind::Add
        } else if raw.starts_with('-') && !raw.starts_with("---") {
            DiffLineKind::Delete
        } else if raw.starts_with(' ') || raw == "\\ No newline at end of file" {
            DiffLineKind::Context
        } else {
            DiffLineKind::Meta
        };
        lines.push(DiffLine {
            kind,
            text: raw.to_string(),
        });
    }
    FileDiff {
        path: path.to_string(),
        staged,
        binary,
        truncated,
        lines,
    }
}
