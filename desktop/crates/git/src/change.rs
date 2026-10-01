//! Stage, unstage, and commit through the `git` command.

use std::path::Path;

use serde::Serialize;

use crate::cli::{check_path, commit_config, run, run_stdin};
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
    let mut args = commit_config(repo);
    args.push("commit".to_string());
    args.push("-m".to_string());
    args.push(summary.to_string());
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
    /// Index into the file that `stage_line` edits. Absent for context and headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage_at: Option<u32>,
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
    /// How many zero-context hunks can be staged one at a time.
    pub hunks: u32,
}

const MAX_DIFF_LINES: usize = 4_000;
const MAX_UNTRACKED_BYTES: u64 = 1_048_576;

/// Stage or unstage a single hunk. Hunks come from a zero-context diff, so adjacent edits stay separate.
pub fn stage_hunk(repo: &Path, path: &str, index: u32, unstage: bool) -> Result<(), Error> {
    let path = check_path(path)?;
    let mut args = vec!["diff", "--no-ext-diff", "--no-color", "--unified=0"];
    if unstage {
        args.push("--cached");
    }
    args.push("--");
    args.push(path);
    let output = run(repo, &args)?;
    let text = String::from_utf8_lossy(&output.stdout);
    let (preamble, hunks) = split_hunks(&text);
    if hunks.is_empty() {
        if !unstage && index == 0 && !in_index(repo, path)? {
            return stage_paths(repo, &[path.to_string()]);
        }
        return Err(Error::Git(format!("hunk {index} is not in {path}")));
    }
    let hunk = hunks.get(index as usize).ok_or_else(|| Error::Git(format!("hunk {index} is not in {path}")))?;
    let mut patch = preamble;
    patch.push_str(hunk);
    if !patch.ends_with('\n') {
        patch.push('\n');
    }
    let mut apply = vec!["apply", "--cached", "--unidiff-zero"];
    if unstage {
        apply.push("--reverse");
    }
    run_stdin(repo, &apply, patch.as_bytes()).map(|_| ())
}

/// Stage or unstage one added or removed line. `at` is the index carried on that diff line.
pub fn stage_line(repo: &Path, path: &str, text: &str, addition: bool, at: u32, unstage: bool) -> Result<(), Error> {
    let path = check_path(path)?;
    if text.contains(['\n', '\r', '\0']) {
        return Err(Error::Git("a staged line cannot contain a newline".into()));
    }
    let current = index_blob(repo, path)?;
    let (mut lines, trailing) = split_lines(&current);
    let remove = unstage == addition;
    if remove {
        let index = at as usize;
        if lines.get(index).map(String::as_str) != Some(text) {
            return Err(Error::Git(format!("line {at} is not in {path}")));
        }
        lines.remove(index);
    } else {
        let index = (at as usize).min(lines.len());
        lines.insert(index, text.to_string());
    }
    let mut body = lines.join("\n");
    if trailing || !body.is_empty() {
        body.push('\n');
    }
    write_index(repo, path, &body)
}

fn index_blob(repo: &Path, path: &str) -> Result<String, Error> {
    match run(repo, &["show", &format!(":{path}")]) {
        Ok(output) => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        Err(Error::Git(_)) => Ok(String::new()),
        Err(error) => Err(error),
    }
}

fn split_lines(text: &str) -> (Vec<String>, bool) {
    if text.is_empty() {
        return (Vec::new(), true);
    }
    let trailing = text.ends_with('\n');
    let mut lines: Vec<String> = text.split('\n').map(|line| line.trim_end_matches('\r').to_string()).collect();
    if trailing {
        lines.pop();
    }
    (lines, trailing)
}

fn write_index(repo: &Path, path: &str, body: &str) -> Result<(), Error> {
    let hashed = run_stdin(repo, &["hash-object", "-w", "--stdin"], body.as_bytes())?;
    let hash = String::from_utf8_lossy(&hashed.stdout).trim().to_string();
    if hash.len() < 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Git("git hash-object returned no id".into()));
    }
    let mode = index_mode(repo, path);
    let info = format!("{mode},{hash},{path}");
    run(repo, &["update-index", "--add", "--cacheinfo", &info]).map(|_| ())
}

fn index_mode(repo: &Path, path: &str) -> String {
    let Ok(output) = run(repo, &["ls-files", "-s", "--", path]) else {
        return "100644".into();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace()
        .next()
        .filter(|mode| mode.len() == 6 && mode.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or("100644")
        .to_string()
}

fn split_hunks(text: &str) -> (String, Vec<String>) {
    let mut preamble = String::new();
    let mut hunks = Vec::new();
    let mut current = String::new();
    let mut in_hunk = false;
    for line in text.split_inclusive('\n') {
        if line.starts_with("@@") {
            if in_hunk {
                hunks.push(std::mem::take(&mut current));
            }
            in_hunk = true;
        }
        if in_hunk {
            current.push_str(line);
        } else {
            preamble.push_str(line);
        }
    }
    if in_hunk && !current.is_empty() {
        hunks.push(current);
    }
    (preamble, hunks)
}

/// Unified diff for one path. `staged` selects the index rather than the work tree.
pub fn file_diff(repo: &Path, path: &str, staged: bool) -> Result<FileDiff, Error> {
    file_diff_with(repo, path, staged, 3)
}

/// `context` is the number of unchanged lines around each hunk. A large value shows the whole file.
pub fn file_diff_with(repo: &Path, path: &str, staged: bool, context: u32) -> Result<FileDiff, Error> {
    let path = check_path(path)?;
    let unified = format!("--unified={}", context.min(1_000_000));
    let mut args = vec!["diff", "--no-ext-diff", "--no-color", unified.as_str()];
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
    let mut parsed = parse_diff(path, staged, &text);
    parsed.hunks = zero_hunks(repo, path, staged).unwrap_or(0);
    Ok(parsed)
}

fn zero_hunks(repo: &Path, path: &str, staged: bool) -> Result<u32, Error> {
    let mut args = vec!["diff", "--no-ext-diff", "--no-color", "--unified=0"];
    if staged {
        args.push("--cached");
    }
    args.push("--");
    args.push(path);
    let output = run(repo, &args)?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().filter(|line| line.starts_with("@@")).count() as u32)
}

/// Staged and unstaged diffs, capped, for a commit-message suggestion.
pub fn workspace_diff(repo: &Path) -> Result<String, Error> {
    let staged = run(repo, &["diff", "--cached", "--no-color", "--no-ext-diff"])?;
    let work = run(repo, &["diff", "--no-color", "--no-ext-diff"])?;
    let mut text = String::new();
    if !staged.stdout.is_empty() {
        text.push_str("STAGED\n");
        text.push_str(&String::from_utf8_lossy(&staged.stdout));
    }
    if !work.stdout.is_empty() {
        text.push_str("UNSTAGED\n");
        text.push_str(&String::from_utf8_lossy(&work.stdout));
    }
    const LIMIT: usize = 12_000;
    if text.len() > LIMIT {
        text.truncate(LIMIT);
    }
    Ok(text)
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
                hunks: 0,
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
            hunks: 0,
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
            hunks: 0,
        });
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut parts: Vec<&str> = text.split('\n').map(|line| line.trim_end_matches('\r')).collect();
    if text.ends_with('\n') {
        parts.pop();
    }
    let mut body: Vec<DiffLine> = parts
        .into_iter()
        .enumerate()
        .map(|(index, line)| DiffLine {
            kind: DiffLineKind::Add,
            text: format!("+{line}"),
            stage_at: Some(index as u32),
        })
        .collect();
    let truncated = body.len() > MAX_DIFF_LINES;
    body.truncate(MAX_DIFF_LINES);
    let mut lines = vec![DiffLine {
        kind: DiffLineKind::Hunk,
        text: format!("@@ -0,0 +1,{} @@", body.len()),
        stage_at: None,
    }];
    lines.append(&mut body);
    Ok(FileDiff {
        path: path.to_string(),
        staged: false,
        binary: false,
        truncated,
        lines,
        hunks: 1,
    })
}

pub(crate) fn parse_diff(path: &str, staged: bool, text: &str) -> FileDiff {
    let mut lines = Vec::new();
    let mut binary = false;
    let mut truncated = false;
    let mut in_hunk = false;
    let mut old_i = 0u32;
    let mut new_i = 0u32;
    let mut insert_at = 0u32;
    let mut last_keep: i32 = -1;
    for raw in text.lines() {
        if raw.starts_with("Binary files ") || raw.starts_with("GIT binary patch") {
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
        if raw.starts_with("@@") {
            if let Some((old_start, old_count, new_start, _)) = hunk_header(raw) {
                in_hunk = true;
                old_i = old_start.saturating_sub(1);
                new_i = new_start.saturating_sub(1);
                insert_at = if old_count == 0 { old_start } else { old_i };
                last_keep = new_start as i32 - 1;
            }
            lines.push(DiffLine {
                kind: DiffLineKind::Hunk,
                text: raw.to_string(),
                stage_at: None,
            });
            continue;
        }
        let kind = if raw.starts_with('+') && !raw.starts_with("+++") {
            DiffLineKind::Add
        } else if raw.starts_with('-') && !raw.starts_with("---") {
            DiffLineKind::Delete
        } else if raw.starts_with(' ') || raw == "\\ No newline at end of file" {
            DiffLineKind::Context
        } else {
            DiffLineKind::Meta
        };
        let mut stage_at = None;
        if in_hunk {
            match kind {
                DiffLineKind::Context => {
                    insert_at = old_i + 1;
                    last_keep = new_i as i32;
                    old_i += 1;
                    new_i += 1;
                }
                DiffLineKind::Delete => {
                    let unstage_at = if last_keep < 0 { 0 } else { last_keep as u32 + 1 };
                    stage_at = Some(if staged { unstage_at } else { old_i });
                    old_i += 1;
                }
                DiffLineKind::Add => {
                    stage_at = Some(if staged { new_i } else { insert_at });
                    last_keep = new_i as i32;
                    new_i += 1;
                }
                _ => {}
            }
        }
        lines.push(DiffLine {
            kind,
            text: raw.to_string(),
            stage_at,
        });
    }
    FileDiff {
        path: path.to_string(),
        staged,
        binary,
        truncated,
        lines,
        hunks: 0,
    }
}

fn hunk_header(line: &str) -> Option<(u32, u32, u32, u32)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let new = rest.split_once(' ')?.0;
    let (old_start, old_count) = split_count(old)?;
    let (new_start, new_count) = split_count(new)?;
    Some((old_start, old_count, new_start, new_count))
}

fn split_count(text: &str) -> Option<(u32, u32)> {
    if let Some((count, size)) = text.split_once(',') {
        Some((count.parse().ok()?, size.parse().ok()?))
    } else {
        Some((text.parse().ok()?, 1))
    }
}

/// Image bytes from the work tree, when the path is a png, jpeg, gif, webp, or bmp.
pub fn file_preview(repo: &Path, path: &str) -> Result<Option<FilePreview>, Error> {
    let path = check_path(path)?;
    let full = repo.join(path);
    let bytes = match std::fs::read(&full) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(Error::Read {
                path: path.to_string(),
                source,
            })
        }
    };
    if bytes.len() > 8_000_000 {
        return Ok(None);
    }
    let Some((mime, animated)) = image_kind(&bytes) else {
        return Ok(None);
    };
    Ok(Some(FilePreview {
        mime: mime.into(),
        data_url: format!("data:{mime};base64,{}", base64_encode(&bytes)),
        animated,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub mime: String,
    pub data_url: String,
    pub animated: bool,
}

fn image_kind(bytes: &[u8]) -> Option<(&'static str, bool)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(("image/png", false));
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(("image/jpeg", false));
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        let frames = bytes.windows(8).filter(|window| *window == b"\x21\xF9\x04").count();
        return Some(("image/gif", frames > 1));
    }
    if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        let animated = bytes.windows(4).any(|window| window == b"ANIM");
        return Some(("image/webp", animated));
    }
    if bytes.starts_with(b"BM") {
        return Some(("image/bmp", false));
    }
    None
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0] as u32;
        let second = chunk.get(1).copied().unwrap_or(0) as u32;
        let third = chunk.get(2).copied().unwrap_or(0) as u32;
        let value = (first << 16) | (second << 8) | third;
        out.push(TABLE[((value >> 18) & 63) as usize] as char);
        out.push(TABLE[((value >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((value >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(value & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
