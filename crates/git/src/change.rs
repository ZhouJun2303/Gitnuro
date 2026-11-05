//! Stage, unstage, and commit through the `git` command.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cli::{check_path, commit_config, run, run_capped, run_paths, run_stdin};
use crate::{in_progress, Error, InProgress};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRequest {
    pub summary: String,
    pub description: String,
    pub amend: bool,
    pub sign_off: bool,
    /// Empty keeps Git's own `Signed-off-by` line. `%user` and `%email` are replaced otherwise.
    pub sign_off_format: String,
    /// When true, `git commit` gets `--no-verify` for this commit only.
    pub skip_hooks: bool,
    /// Editor commits check the subject against the built-in commit-msg rule and drop `#` commentary.
    pub convention: bool,
}

/// Stage the given work-tree paths, including new files.
pub fn stage_paths(repo: &Path, paths: &[String]) -> Result<(), Error> {
    if paths.is_empty() {
        return Ok(());
    }
    run_paths(repo, &["add"], paths).map(|_| ())
}

/// Stage every change in the work tree. Ignored files stay ignored, and unresolved conflicts are left alone.
pub fn stage_all(repo: &Path) -> Result<(), Error> {
    let unmerged = unmerged_paths(repo)?;
    if unmerged.is_empty() {
        return run(repo, &["add", "-A", "--", "."]).map(|_| ());
    }
    let blocked: std::collections::HashSet<String> = unmerged.into_iter().collect();
    let status = run(repo, &["status", "--porcelain", "-z"])?;
    let parts: Vec<&[u8]> = status.stdout.split(|byte| *byte == 0).collect();
    let mut paths = Vec::new();
    let mut index = 0;
    while index < parts.len() {
        let part = parts[index];
        index += 1;
        if part.len() < 4 {
            continue;
        }
        let code = &part[..2];
        let path = String::from_utf8_lossy(&part[3..]).into_owned();
        if !blocked.contains(&path) {
            paths.push(path);
        }
        if code.iter().any(|byte| *byte == b'R' || *byte == b'C') {
            if let Some(next) = parts.get(index) {
                let other = String::from_utf8_lossy(next).into_owned();
                if !other.is_empty() && !blocked.contains(&other) {
                    paths.push(other);
                }
                index += 1;
            }
        }
    }
    stage_path_chunks(repo, &paths)
}

fn stage_path_chunks(repo: &Path, paths: &[String]) -> Result<(), Error> {
    run_paths(repo, &["add", "-A"], paths).map(|_| ())
}

/// Paths that still have unmerged index entries.
fn unmerged_paths(repo: &Path) -> Result<Vec<String>, Error> {
    let output = run(repo, &["ls-files", "-u", "-z"])?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut paths = std::collections::BTreeSet::new();
    for entry in text.split('\0') {
        if let Some((_, path)) = entry.split_once('\t') {
            if !path.is_empty() {
                paths.insert(path.to_string());
            }
        }
    }
    Ok(paths.into_iter().collect())
}

/// Remove the given paths from the index and leave the work tree as it is.
pub fn unstage_paths(repo: &Path, paths: &[String]) -> Result<(), Error> {
    if paths.is_empty() {
        return Ok(());
    }
    match run_paths(repo, &["restore", "--staged"], paths) {
        Ok(_) => Ok(()),
        // A repository with no commits yet has nothing to restore from.
        Err(Error::Git(message)) if message.contains("HEAD") => {
            run_paths(repo, &["rm", "--cached", "--ignore-unmatch"], paths).map(|_| ())
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
    if !unmerged_paths(repo)?.is_empty() {
        return Err(Error::Git("resolve all conflicts before committing".into()));
    }
    if request.convention && !commit_subject_allowed(summary) {
        return Err(Error::Git(CONVENTION_ERROR.into()));
    }
    let mut description = if request.convention {
        strip_commentary(&request.description)
    } else {
        request.description.trim().to_string()
    };
    let mut git_signoff = false;
    if request.sign_off {
        let format = request.sign_off_format.trim();
        if format.is_empty() || format == "Signed-off-by: %user <%email>" {
            git_signoff = true;
        } else {
            let line = sign_off_line(repo, format);
            if !description.contains(&line) {
                if !description.is_empty() {
                    description.push_str("\n\n");
                }
                description.push_str(&line);
            }
        }
    }
    let mut args = commit_config(repo);
    args.push("commit".to_string());
    // The message goes through stdin: a merge body can list hundreds of files and
    // Windows rejects a command line longer than 32K characters.
    args.push("-F".to_string());
    args.push("-".to_string());
    if request.amend {
        args.push("--amend".to_string());
    }
    if git_signoff {
        args.push("--signoff".to_string());
    }
    if request.skip_hooks {
        args.push("--no-verify".to_string());
    }
    let mut message = summary.to_string();
    if !description.is_empty() {
        message.push_str("\n\n");
        message.push_str(&description);
    }
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    if crate::cli::has_local_author(repo) {
        crate::cli::run_stdin_repo_author(repo, &borrowed, message.as_bytes()).map(|_| ())
    } else {
        run_stdin(repo, &borrowed, message.as_bytes()).map(|_| ())
    }
}

const COMMIT_TYPES: [&str; 10] = [
    "feat", "fix", "docs", "art", "style", "refactor", "test", "chore", "perf", "revert",
];

const CONVENTION_ERROR: &str = "\
ERROR: 提交信息不符合规范。
允许格式: type:描述 或 type: 描述（冒号后空格可选）
允许类型: feat|fix|docs|art|style|refactor|test|chore|perf|revert
示例: feat: 新增挂机奖励结算逻辑";

/// Same rule as the commit-msg hook: `type: description`, with an optional space after the colon.
fn commit_subject_allowed(summary: &str) -> bool {
    let Some((kind, rest)) = summary.split_once(':') else {
        return false;
    };
    if !COMMIT_TYPES.contains(&kind) {
        return false;
    }
    let rest = rest.trim_start_matches([' ', '\t', '\u{000B}', '\u{000C}', '\r']);
    rest.chars().next().is_some_and(|ch| !ch.is_whitespace())
}

/// Drop git commentary (`#` lines) and the blank lines that only separated them.
fn strip_commentary(text: &str) -> String {
    let mut kept = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches([' ', '\t']);
        if line.trim_start().starts_with('#') {
            continue;
        }
        kept.push(line);
    }
    while kept.first().is_some_and(|line| line.trim().is_empty()) {
        kept.remove(0);
    }
    while kept.last().is_some_and(|line| line.trim().is_empty()) {
        kept.pop();
    }
    let mut out = String::new();
    let mut blank = false;
    for line in kept {
        if line.trim().is_empty() {
            if blank {
                continue;
            }
            blank = true;
        } else {
            blank = false;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

fn sign_off_line(repo: &Path, format: &str) -> String {
    let (session, _) = crate::cli::session_snapshot();
    let user = if session.author_name.is_empty() {
        config_value(repo, "user.name").unwrap_or_else(|| "AweGit".into())
    } else {
        session.author_name
    };
    let email = if session.author_email.is_empty() {
        config_value(repo, "user.email").unwrap_or_else(|| "awegit@localhost".into())
    } else {
        session.author_email
    };
    format.replace("%user", &user).replace("%email", &email)
}

fn config_value(repo: &Path, key: &str) -> Option<String> {
    let output = run(repo, &["config", "--get", key]).ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

/// First bytes of a file, for format sniffing without reading a huge payload.
fn file_prefix(path: PathBuf, count: usize) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let mut bytes = vec![0u8; count];
    let read = file.read(&mut bytes).ok()?;
    bytes.truncate(read);
    Some(bytes)
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
    /// Index into the worktree file that `discard_line` edits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_at: Option<u32>,
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
const MAX_DIFF_BYTES: usize = 8_000_000;
const MAX_LARGE_FILE: u64 = 2_000_000;
const MAX_PREVIEW_BYTES: u64 = 8_000_000;
const MAX_BLOB_READ_BYTES: u64 = 8_000_000;
const MAX_BLOB_TEXT_BYTES: usize = 200_000;

/// Stage or unstage a single hunk. Hunks come from a zero-context diff, so adjacent edits stay separate.
pub fn stage_hunk(repo: &Path, path: &str, index: u32, unstage: bool, ignore_space: bool) -> Result<(), Error> {
    let path = check_path(path)?;
    let mut args = vec!["--literal-pathspecs", "diff", "--no-ext-diff", "--no-color", "--unified=0"];
    if ignore_space {
        args.push("--ignore-all-space");
    }
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
    let current = decode_text(&current, path, "stage the whole file instead")?;
    let (mut lines, eol) = split_keep_eol(&current);
    let remove = unstage == addition;
    if remove {
        if !remove_line(&mut lines, at as usize, text) {
            return Err(Error::Git(format!("line {at} is not in {path}")));
        }
    } else {
        insert_line(&mut lines, at as usize, text, &eol);
    }
    write_index(repo, path, lines.concat().as_bytes())
}

/// Drop one unstaged hunk from the work tree. The index is left alone.
pub fn discard_hunk(repo: &Path, path: &str, index: u32, ignore_space: bool) -> Result<(), Error> {
    let path = check_path(path)?;
    let mut args = vec!["--literal-pathspecs", "diff", "--no-ext-diff", "--no-color", "--unified=0"];
    if ignore_space {
        args.push("--ignore-all-space");
    }
    args.push("--");
    args.push(path);
    let output = run(repo, &args)?;
    let text = String::from_utf8_lossy(&output.stdout);
    let (preamble, hunks) = split_hunks(&text);
    let hunk = hunks.get(index as usize).ok_or_else(|| Error::Git(format!("hunk {index} is not in {path}")))?;
    let mut patch = preamble;
    patch.push_str(hunk);
    if !patch.ends_with('\n') {
        patch.push('\n');
    }
    run_stdin(repo, &["apply", "--reverse", "--unidiff-zero"], patch.as_bytes()).map(|_| ())
}

/// Drop or restore one unstaged line in the work tree.
pub fn discard_line(repo: &Path, path: &str, text: &str, addition: bool, at: u32) -> Result<(), Error> {
    let path = check_path(path)?;
    if text.contains(['\n', '\r', '\0']) {
        return Err(Error::Git("a discarded line cannot contain a newline".into()));
    }
    let full = repo.join(path);
    if !full.starts_with(repo) {
        return Err(Error::Path(path.to_string()));
    }
    let current = match std::fs::read(&full) {
        Ok(bytes) => decode_text(&bytes, path, "discard the whole file instead")?,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(source) => {
            return Err(Error::Read {
                path: path.to_string(),
                source,
            })
        }
    };
    let (mut lines, eol) = split_keep_eol(&current);
    if addition {
        if !remove_line(&mut lines, at as usize, text) {
            return Err(Error::Git(format!("line {at} is not in {path}")));
        }
    } else {
        insert_line(&mut lines, at as usize, text, &eol);
    }
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Read {
            path: path.to_string(),
            source,
        })?;
    }
    std::fs::write(&full, lines.concat().as_bytes()).map_err(|source| Error::Read {
        path: path.to_string(),
        source,
    })
}

fn decode_text(bytes: &[u8], path: &str, hint: &str) -> Result<String, Error> {
    String::from_utf8(bytes.to_vec()).map_err(|_| Error::Git(format!("{path} is not valid UTF-8; {hint}")))
}

/// Split text into lines that keep their CRLF/LF terminator.
fn split_keep_eol(text: &str) -> (Vec<String>, String) {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    (text.split_inclusive('\n').map(str::to_string).collect(), eol.to_string())
}

fn line_text(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

fn remove_line(lines: &mut Vec<String>, index: usize, text: &str) -> bool {
    if lines.get(index).map(|line| line_text(line)) != Some(text) {
        return false;
    }
    let removed = lines.remove(index);
    // The removed line was the unterminated last line: the new last line must not
    // gain a trailing newline just because the old separator became the end.
    if !removed.ends_with('\n') {
        if let Some(last) = lines.last_mut() {
            if last.ends_with('\n') {
                last.pop();
                if last.ends_with('\r') {
                    last.pop();
                }
            }
        }
    }
    true
}

/// Insert a line using the file's line ending and keep a missing trailing newline missing.
fn insert_line(lines: &mut Vec<String>, index: usize, text: &str, eol: &str) {
    if lines.is_empty() {
        lines.push(format!("{text}{eol}"));
        return;
    }
    let index = index.min(lines.len());
    if index == lines.len() && !lines.last().is_some_and(|line| line.ends_with('\n')) {
        if let Some(last) = lines.last_mut() {
            last.push_str(eol);
        }
        lines.push(text.to_string());
        return;
    }
    lines.insert(index, format!("{text}{eol}"));
}

/// One line to stage, unstage, or discard. `at` uses the index carried on the diff line.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineEdit {
    pub text: String,
    pub addition: bool,
    pub at: u32,
}

/// Apply several line edits to the index blob in one write.
pub fn stage_lines(repo: &Path, path: &str, lines: &[LineEdit], unstage: bool) -> Result<(), Error> {
    let path = check_path(path)?;
    for line in lines {
        check_line_text(line, "staged")?;
    }
    let current = index_blob(repo, path)?;
    if current.len() > 16_000_000 {
        return Err(Error::Git(format!("{path} is too large for line staging; stage the whole file instead")));
    }
    let current = decode_text(&current, path, "stage the whole file instead")?;
    let (mut body, eol) = split_keep_eol(&current);
    apply_line_edits(&mut body, lines, &eol, |edit| unstage == edit.addition)?;
    write_index(repo, path, body.concat().as_bytes())
}

/// Drop several unstaged lines from the work tree in one read and one write.
pub fn discard_lines(repo: &Path, path: &str, lines: &[LineEdit]) -> Result<(), Error> {
    let path = check_path(path)?;
    for line in lines {
        check_line_text(line, "discarded")?;
    }
    let full = repo.join(path);
    if !full.starts_with(repo) {
        return Err(Error::Path(path.to_string()));
    }
    let current = match std::fs::read(&full) {
        Ok(bytes) => decode_text(&bytes, path, "discard the whole file instead")?,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(source) => {
            return Err(Error::Read {
                path: path.to_string(),
                source,
            })
        }
    };
    let (mut body, eol) = split_keep_eol(&current);
    apply_line_edits(&mut body, lines, &eol, |edit| edit.addition)?;
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Read {
            path: path.to_string(),
            source,
        })?;
    }
    std::fs::write(&full, body.concat().as_bytes()).map_err(|source| Error::Read {
        path: path.to_string(),
        source,
    })
}

fn check_line_text(line: &LineEdit, verb: &str) -> Result<(), Error> {
    if line.text.contains(['\n', '\r', '\0']) {
        return Err(Error::Git(format!("a {verb} line cannot contain a newline")));
    }
    Ok(())
}

fn apply_line_edits(
    body: &mut Vec<String>,
    edits: &[LineEdit],
    eol: &str,
    remove: impl Fn(&LineEdit) -> bool,
) -> Result<(), Error> {
    let mut ordered: Vec<&LineEdit> = edits.iter().collect();
    ordered.sort_by(|left, right| {
        right.at.cmp(&left.at).then_with(|| remove(right).cmp(&remove(left)))
    });
    for edit in ordered {
        let at = edit.at as usize;
        if remove(edit) {
            if !remove_line(body, at, &edit.text) {
                return Err(Error::Git(format!("line {} is not in the file", edit.at)));
            }
        } else {
            insert_line(body, at, &edit.text, eol);
        }
    }
    Ok(())
}

/// The two conflict stages plus the working tree text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictSides {
    pub ours: String,
    pub theirs: String,
    pub working: String,
}

pub fn conflict_sides(repo: &Path, path: &str) -> Result<ConflictSides, Error> {
    let path = check_path(path)?;
    Ok(ConflictSides {
        ours: show_stage(repo, path, 2),
        theirs: show_stage(repo, path, 3),
        working: std::fs::read_to_string(repo.join(path)).unwrap_or_default(),
    })
}

fn show_stage(repo: &Path, path: &str, stage: u8) -> String {
    match run(repo, &["show", &format!(":{stage}:{path}")]) {
        Ok(output) => String::from_utf8_lossy(&output.stdout).into_owned(),
        Err(_) => String::new(),
    }
}

/// The commit a conflict side comes from, so the merge panel can name it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictCommit {
    pub id: String,
    pub short: String,
    pub subject: String,
    pub author: String,
    pub email: String,
    /// Unix seconds from `%at`.
    pub at: i64,
}

/// One side of an unmerged path: the name Git would show, and how it differs from the base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictParty {
    pub label: String,
    pub state: String,
    pub bytes: u64,
    pub commit: Option<ConflictCommit>,
}

/// Labels and change kinds for the two sides of an unmerged file.
///
/// Ours is the current branch (the right side). Theirs is the incoming side:
/// the other branch, the commit being replayed, or `stash/patch` when the blob
/// belongs to a stash's working tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictView {
    pub ours: ConflictParty,
    pub theirs: ConflictParty,
    pub binary: bool,
}

pub fn conflict_view(repo: &Path, path: &str) -> Result<ConflictView, Error> {
    let path = check_path(path)?;
    let stages = stage_oids(repo, path)?;
    if stages[2].is_none() && stages[3].is_none() {
        return Err(Error::Git(format!("{path} is not unmerged")));
    }
    let base = stages[1].as_deref();
    let (their_label, their_rev) = theirs_party(repo, path, stages[3].as_deref());
    Ok(ConflictView {
        ours: ConflictParty {
            label: ours_label(repo),
            state: side_state(base, stages[2].as_deref()).into(),
            bytes: oid_size(repo, stages[2].as_deref()),
            commit: commit_info(repo, "HEAD"),
        },
        theirs: ConflictParty {
            label: their_label,
            state: side_state(base, stages[3].as_deref()).into(),
            bytes: oid_size(repo, stages[3].as_deref()),
            commit: their_rev.as_deref().and_then(|rev| commit_info(repo, rev)),
        },
        binary: conflict_is_binary(repo, path, &stages),
    })
}

fn commit_info(repo: &Path, rev: &str) -> Option<ConflictCommit> {
    let text = git_text(repo, &["log", "-1", "--format=%H%x1f%h%x1f%s%x1f%an%x1f%ae%x1f%at", rev])?;
    let mut fields = text.split('\x1f');
    let id = fields.next()?.trim().to_string();
    let short = fields.next()?.trim().to_string();
    let subject = fields.next()?.trim().to_string();
    let author = fields.next()?.trim().to_string();
    let email = fields.next().unwrap_or("").trim().to_string();
    let at = fields.next()?.trim().parse().ok()?;
    if id.is_empty() {
        return None;
    }
    Some(ConflictCommit { id, short, subject, author, email, at })
}

fn stage_oids(repo: &Path, path: &str) -> Result<[Option<String>; 4], Error> {
    let output = run(repo, &["--literal-pathspecs", "ls-files", "-u", "--stage", "--", path])?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut stages = [None, None, None, None];
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let _mode = fields.next();
        let oid = fields.next().unwrap_or("");
        let stage: usize = fields.next().unwrap_or("0").parse().unwrap_or(0);
        if (1..=3).contains(&stage) && oid.len() >= 7 {
            stages[stage] = Some(oid.to_string());
        }
    }
    Ok(stages)
}

fn oid_size(repo: &Path, oid: Option<&str>) -> u64 {
    let Some(oid) = oid else { return 0 };
    git_text(repo, &["cat-file", "-s", oid]).and_then(|text| text.parse().ok()).unwrap_or(0)
}

fn side_state(base: Option<&str>, side: Option<&str>) -> &'static str {
    match (base, side) {
        (None, Some(_)) => "added",
        (_, None) => "deleted",
        (Some(base), Some(side)) if base == side => "unchanged",
        (Some(_), Some(_)) => "modified",
    }
}

fn ours_label(repo: &Path) -> String {
    if matches!(in_progress(repo).ok().flatten(), Some(InProgress::Rebase)) {
        if let Some(name) = rebase_onto_label(repo) {
            return name;
        }
    }
    head_label(repo)
}

/// Label for the incoming side plus the revision its commit details come from.
fn theirs_party(repo: &Path, path: &str, theirs: Option<&str>) -> (String, Option<String>) {
    match in_progress(repo).ok().flatten() {
        Some(InProgress::Merge) => (
            named_ref(repo, "MERGE_HEAD")
                .or_else(|| merge_message_name(repo))
                .or_else(|| subject_of(repo, "MERGE_HEAD"))
                .unwrap_or_else(|| "theirs".into()),
            Some("MERGE_HEAD".into()),
        ),
        Some(InProgress::Rebase) => (
            rebase_head_label(repo)
                .or_else(|| subject_of(repo, "REBASE_HEAD"))
                .unwrap_or_else(|| "theirs".into()),
            Some("REBASE_HEAD".into()),
        ),
        Some(InProgress::CherryPick) => (
            subject_of(repo, "CHERRY_PICK_HEAD").unwrap_or_else(|| "cherry-pick".into()),
            Some("CHERRY_PICK_HEAD".into()),
        ),
        Some(InProgress::Revert) => (
            subject_of(repo, "REVERT_HEAD").unwrap_or_else(|| "revert".into()),
            Some("REVERT_HEAD".into()),
        ),
        None => match theirs.and_then(|oid| stash_party(repo, path, oid)) {
            Some((label, rev)) => (label, Some(rev)),
            None => ("theirs".into(), None),
        },
    }
}

fn head_label(repo: &Path) -> String {
    match git_text(repo, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        Some(name) if name != "HEAD" => name,
        _ => "HEAD".into(),
    }
}

fn named_ref(repo: &Path, rev: &str) -> Option<String> {
    let text = git_text(repo, &[
        "for-each-ref",
        "--format=%(refname:short)",
        "--points-at",
        rev,
        "refs/heads",
        "refs/remotes",
    ])?;
    let names: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    names
        .iter()
        .copied()
        .find(|name| !name.contains('/'))
        .or_else(|| names.first().copied())
        .map(str::to_string)
}

fn subject_of(repo: &Path, rev: &str) -> Option<String> {
    let text = git_text(repo, &["log", "-1", "--format=%s", rev])?;
    let line = text.lines().next()?.trim();
    if line.is_empty() { None } else { Some(line.to_string()) }
}

fn merge_message_name(repo: &Path) -> Option<String> {
    let text = git_file(repo, "MERGE_MSG")?;
    let line = text.lines().next()?.trim();
    let start = line.find('\'')?;
    let rest = &line[start + 1..];
    let end = rest.find('\'')?;
    let name = rest[..end].trim();
    if name.is_empty() { None } else { Some(name.to_string()) }
}

fn rebase_head_label(repo: &Path) -> Option<String> {
    for name in ["rebase-merge/head-name", "rebase-apply/head-name"] {
        if let Some(text) = git_file(repo, name) {
            return Some(short_ref(&text));
        }
    }
    None
}

fn rebase_onto_label(repo: &Path) -> Option<String> {
    for name in ["rebase-merge/onto", "rebase-apply/onto"] {
        if let Some(sha) = git_file(repo, name) {
            if let Some(label) = named_ref(repo, sha.trim()) {
                return Some(label);
            }
        }
    }
    None
}

fn short_ref(name: &str) -> String {
    name.trim()
        .trim_start_matches("refs/heads/")
        .trim_start_matches("refs/remotes/")
        .to_string()
}

/// `stash/patch` is the stash commit's work tree. `stash/index` is its index parent.
/// The returned revision is the stash entry itself, which is what the stash list shows.
fn stash_party(repo: &Path, path: &str, oid: &str) -> Option<(String, String)> {
    let list = git_text(repo, &["stash", "list", "--format=%H"])?;
    // A stash conflict is rare; checking the first few entries is enough and avoids
    // dozens of `rev-parse` processes when a repository keeps a long stash list.
    for hash in list.lines().map(str::trim).filter(|line| !line.is_empty()).take(10) {
        let spec = format!("{hash}:{path}");
        if git_text(repo, &["rev-parse", "--verify", "--quiet", &spec]).as_deref() == Some(oid) {
            return Some(("stash/patch".into(), hash.to_string()));
        }
        let index = format!("{hash}^2:{path}");
        if git_text(repo, &["rev-parse", "--verify", "--quiet", &index]).as_deref() == Some(oid) {
            return Some(("stash/index".into(), hash.to_string()));
        }
    }
    None
}

fn git_text(repo: &Path, args: &[&str]) -> Option<String> {
    let output = run(repo, args).ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn git_file(repo: &Path, name: &str) -> Option<String> {
    let rel = git_text(repo, &["rev-parse", "--git-path", name])?;
    let path = PathBuf::from(&rel);
    let path = if path.is_absolute() { path } else { repo.join(path) };
    let text = std::fs::read_to_string(path).ok()?;
    let text = text.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn conflict_is_binary(repo: &Path, path: &str, stages: &[Option<String>; 4]) -> bool {
    if file_has_nul(repo, path) {
        return true;
    }
    stages.iter().flatten().any(|oid| blob_has_nul(repo, oid))
}

fn file_has_nul(repo: &Path, path: &str) -> bool {
    let mut file = match File::open(repo.join(path)) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut buf = [0u8; 8192];
    let read = file.read(&mut buf).unwrap_or(0);
    buf[..read].contains(&0)
}

fn blob_has_nul(repo: &Path, oid: &str) -> bool {
    run(repo, &["cat-file", "blob", oid])
        .map(|output| output.stdout.iter().take(8192).any(|byte| *byte == 0))
        .unwrap_or(false)
}

pub fn resolve_conflict(repo: &Path, path: &str, side: &str, text: &str) -> Result<(), Error> {
    let path = check_path(path)?;
    if side == "text" {
        if text.contains('\0') {
            return Err(Error::Git("conflict text contains a null".into()));
        }
        let full = repo.join(path);
        std::fs::write(&full, text).map_err(|source| Error::Read {
            path: path.to_string(),
            source,
        })?;
        return run(repo, &["--literal-pathspecs", "add", "--", path]).map(|_| ());
    }
    if side == "ours" || side == "theirs" {
        return resolve_conflicts(repo, &[path.to_string()], side);
    }
    match side {
        "both" => {
            let ours = show_stage(repo, path, 2);
            let theirs = show_stage(repo, path, 3);
            let mut body = ours;
            if !body.is_empty() && !body.ends_with('\n') {
                body.push('\n');
            }
            body.push_str(&theirs);
            if !body.is_empty() && !body.ends_with('\n') {
                body.push('\n');
            }
            let full = repo.join(path);
            std::fs::write(&full, body).map_err(|source| Error::Read {
                path: path.to_string(),
                source,
            })?;
            run(repo, &["--literal-pathspecs", "add", "--", path]).map(|_| ())
        }
        "mark" => run(repo, &["--literal-pathspecs", "add", "--", path]).map(|_| ()),
        other => Err(Error::Git(format!("unknown conflict side {other}"))),
    }
}

/// Keep `ours` or `theirs` for many unmerged paths with one index write.
///
/// A side that deleted the file has no stage entry, so those paths are removed
/// instead of checked out. Taking that side through `checkout` would fail.
pub fn resolve_conflicts(repo: &Path, paths: &[String], side: &str) -> Result<(), Error> {
    if paths.is_empty() {
        return Ok(());
    }
    let (flag, stage) = match side {
        "ours" => ("--ours", 2),
        "theirs" => ("--theirs", 3),
        other => return Err(Error::Git(format!("unknown conflict side {other}"))),
    };
    let stages = unmerged_stages(repo)?;
    let mut kept = Vec::new();
    let mut removed = Vec::new();
    for path in paths {
        check_path(path)?;
        if stages.get(path).map_or(true, |entry| entry[stage]) {
            kept.push(path.clone());
        } else {
            removed.push(path.clone());
        }
    }
    if !kept.is_empty() {
        run_paths(repo, &["checkout", flag], &kept)?;
        run_paths(repo, &["add"], &kept)?;
    }
    if !removed.is_empty() {
        run_paths(repo, &["rm", "-q", "-f"], &removed)?;
    }
    Ok(())
}

/// Index stages present for each unmerged path, indexed by stage number.
fn unmerged_stages(repo: &Path) -> Result<std::collections::BTreeMap<String, [bool; 4]>, Error> {
    let output = run(repo, &["ls-files", "-u", "-z"])?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut stages: std::collections::BTreeMap<String, [bool; 4]> = std::collections::BTreeMap::new();
    for entry in text.split('\0') {
        let Some((meta, path)) = entry.split_once('\t') else { continue };
        if path.is_empty() {
            continue;
        }
        let Some(stage) = meta.split_whitespace().nth(2).and_then(|value| value.parse::<usize>().ok()) else {
            continue;
        };
        if stage < 4 {
            stages.entry(path.to_string()).or_insert([false; 4])[stage] = true;
        }
    }
    Ok(stages)
}

fn index_blob(repo: &Path, path: &str) -> Result<Vec<u8>, Error> {
    match run(repo, &["show", &format!(":{path}")]) {
        Ok(output) => Ok(output.stdout),
        Err(Error::Git(_)) if !in_index(repo, path)? => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn write_index(repo: &Path, path: &str, body: &[u8]) -> Result<(), Error> {
    let hashed = run_stdin(repo, &["hash-object", "-w", "--stdin"], body)?;
    let hash = String::from_utf8_lossy(&hashed.stdout).trim().to_string();
    if hash.len() < 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Git("git hash-object returned no id".into()));
    }
    let mode = index_mode(repo, path);
    // The three-argument form keeps commas in a path out of the `mode,hash,path` split.
    run(repo, &["update-index", "--add", "--cacheinfo", &mode, &hash, path]).map(|_| ())
}

fn index_mode(repo: &Path, path: &str) -> String {
    let Ok(output) = run(repo, &["--literal-pathspecs", "ls-files", "-s", "--", path]) else {
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
    file_diff_with(repo, path, staged, 3, false)
}

/// `context` is the number of unchanged lines around each hunk. A large value shows the whole file.
pub fn file_diff_with(repo: &Path, path: &str, staged: bool, context: u32, ignore_space: bool) -> Result<FileDiff, Error> {
    let path = check_path(path)?;
    if let Ok(meta) = std::fs::metadata(repo.join(path)) {
        if meta.is_file() && meta.len() > MAX_LARGE_FILE {
            let previewable = file_prefix(repo.join(path), 32).is_some_and(|bytes| {
                image_kind(&bytes).is_some()
                    || (bytes.len() >= 18 && bytes[2] == 2 && matches!(bytes[16], 24 | 32))
            });
            return Ok(FileDiff {
                path: path.to_string(),
                staged,
                binary: previewable,
                truncated: true,
                lines: if previewable {
                    Vec::new()
                } else {
                    vec![DiffLine {
                        kind: DiffLineKind::Meta,
                        text: "large file skipped".into(),
                        stage_at: None,
                        work_at: None,
                    }]
                },
                hunks: 0,
            });
        }
    }
    let unified = format!("--unified={}", context.min(1_000_000));
    let mut args = vec!["--literal-pathspecs", "diff", "--no-ext-diff", "--no-color", unified.as_str()];
    if ignore_space {
        args.push("--ignore-all-space");
    }
    if staged {
        args.push("--cached");
    }
    args.push("--");
    args.push(path);
    // The displayed diff and the zero-context hunk count are independent; running them
    // together removes one process round-trip from every file open.
    let (output, capped, hunks) = std::thread::scope(|scope| {
      let hunks = scope.spawn(|| crate::cli::with_read(|| zero_hunks(repo, path, staged, ignore_space)));
      let (output, capped) = crate::cli::with_read(|| run_capped(repo, &args, MAX_DIFF_BYTES))?;
      let hunks = hunks.join().unwrap_or(Ok(0));
      Ok::<_, Error>((output, capped, hunks))
    })?;
    let text = String::from_utf8_lossy(&output.stdout);
    if !staged && text.trim().is_empty() && !in_index(repo, path)? {
        return diff_untracked(repo, path);
    }
    let mut parsed = parse_diff(path, staged, &text);
    parsed.truncated |= capped;
    parsed.hunks = hunks.unwrap_or(0);
    if text.contains("git-lfs.github.com/spec") {
        parsed.lines.insert(
            0,
            DiffLine {
                kind: DiffLineKind::Meta,
                text: "Git LFS pointer".into(),
                stage_at: None,
                work_at: None,
            },
        );
    }
    if parsed.binary {
        if let Some(lines) = lfs_pointer_diff(repo, path, staged) {
            parsed.binary = false;
            parsed.lines = lines;
        }
    }
    Ok(parsed)
}

fn lfs_pointer_diff(repo: &Path, path: &str, staged: bool) -> Option<Vec<DiffLine>> {
    let left_spec = if staged { format!("HEAD:{path}") } else { format!(":{path}") };
    let right_bytes = if staged {
        run(repo, &["show", &format!(":{path}")]).ok().map(|output| output.stdout)
    } else {
        std::fs::read(repo.join(path)).ok()
    };
    let left_bytes = run(repo, &["show", &left_spec]).ok().map(|output| output.stdout).unwrap_or_default();
    let right_bytes = right_bytes.unwrap_or_default();
    let left = lfs_pointer(&left_bytes);
    let right = lfs_pointer(&right_bytes);
    if left.is_none() && right.is_none() {
        return None;
    }
    let mut lines = vec![DiffLine {
        kind: DiffLineKind::Meta,
        text: "Git LFS pointer".into(),
        stage_at: None,
        work_at: None,
    }];
    if left == right {
        for row in right.unwrap_or_default().lines() {
            lines.push(DiffLine { kind: DiffLineKind::Context, text: format!(" {row}"), stage_at: None, work_at: None });
        }
        return Some(lines);
    }
    for row in left.unwrap_or_default().lines() {
        lines.push(DiffLine { kind: DiffLineKind::Delete, text: format!("-{row}"), stage_at: None, work_at: None });
    }
    for row in right.unwrap_or_default().lines() {
        lines.push(DiffLine { kind: DiffLineKind::Add, text: format!("+{row}"), stage_at: None, work_at: None });
    }
    Some(lines)
}

fn lfs_pointer(bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?.trim();
    if text.starts_with("version https://git-lfs.github.com/spec") {
        Some(text.to_string())
    } else {
        None
    }
}

fn zero_hunks(repo: &Path, path: &str, staged: bool, ignore_space: bool) -> Result<u32, Error> {
    let mut args = vec!["--literal-pathspecs", "diff", "--no-ext-diff", "--no-color", "--unified=0"];
    if ignore_space {
        args.push("--ignore-all-space");
    }
    if staged {
        args.push("--cached");
    }
    args.push("--");
    args.push(path);
    let (output, _) = run_capped(repo, &args, 1_000_000)?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().filter(|line| line.starts_with("@@")).count() as u32)
}

/// Diff from `base` to `HEAD` using the three-dot form.
pub fn range_diff(repo: &Path, base: &str) -> Result<String, Error> {
    let base = base.trim();
    if base.is_empty() || base.starts_with('-') || base.contains(['\n', '\r', '\0', ' ']) {
        return Err(Error::Rev(base.to_string()));
    }
    let spec = format!("{base}...HEAD");
    let (output, _) = run_capped(repo, &["diff", "--no-color", "--no-ext-diff", &spec], 4_000_000)?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Staged diff only. Commit suggestions must not describe unstaged work.
pub fn staged_diff(repo: &Path) -> Result<String, Error> {
    let (staged, _) = run_capped(repo, &["diff", "--cached", "--no-color", "--no-ext-diff"], 1_000_000)?;
    Ok(String::from_utf8_lossy(&staged.stdout).into_owned())
}

/// Staged and unstaged diffs, capped, for a commit-message suggestion.
pub fn workspace_diff(repo: &Path) -> Result<String, Error> {
    let (staged, _) = run_capped(repo, &["diff", "--cached", "--no-color", "--no-ext-diff"], 200_000)?;
    let (work, _) = run_capped(repo, &["diff", "--no-color", "--no-ext-diff"], 200_000)?;
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
        let mut at = LIMIT;
        while at > 0 && !text.is_char_boundary(at) {
            at -= 1;
        }
        text.truncate(at);
    }
    Ok(text)
}

fn in_index(repo: &Path, path: &str) -> Result<bool, Error> {
    let output = run(repo, &["--literal-pathspecs", "ls-files", "--", path])?;
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
            work_at: Some(index as u32),
        })
        .collect();
    let truncated = body.len() > MAX_DIFF_LINES;
    body.truncate(MAX_DIFF_LINES);
    let mut lines = vec![DiffLine {
        kind: DiffLineKind::Hunk,
        text: format!("@@ -0,0 +1,{} @@", body.len()),
        stage_at: None,
        work_at: None,
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
                work_at: None,
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
        let mut work_at = None;
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
                    work_at = Some(new_i);
                    old_i += 1;
                }
                DiffLineKind::Add => {
                    stage_at = Some(if staged { new_i } else { insert_at });
                    work_at = Some(new_i);
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
            work_at,
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
    let metadata = match std::fs::metadata(&full) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(Error::Read {
                path: path.to_string(),
                source,
            })
        }
    };
    if !metadata.is_file() || metadata.len() > MAX_PREVIEW_BYTES {
        return Ok(None);
    }
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

/// Image stored at `rev:path`, using the same formats as [`file_preview`].
pub fn blob_preview(repo: &Path, rev: &str, path: &str) -> Result<Option<FilePreview>, Error> {
    if blob_size(repo, rev, path)? > MAX_PREVIEW_BYTES {
        return Ok(None);
    }
    let bytes = blob_bytes(repo, rev, path)?;
    Ok(preview_from_bytes(&bytes))
}

/// Text or image at `rev:path`. Text is capped so a huge blob does not fill the window.
pub fn blob_view(repo: &Path, rev: &str, path: &str) -> Result<BlobView, Error> {
    let size = blob_size(repo, rev, path)?;
    if size > MAX_BLOB_READ_BYTES {
        return Ok(BlobView {
            binary: false,
            text: String::new(),
            preview: None,
            truncated: true,
        });
    }
    let bytes = blob_bytes(repo, rev, path)?;
    if let Some(preview) = preview_from_bytes(&bytes) {
        return Ok(BlobView { binary: true, text: String::new(), preview: Some(preview), truncated: false });
    }
    if bytes.iter().take(800).any(|byte| *byte == 0) {
        return Ok(BlobView { binary: true, text: String::new(), preview: None, truncated: false });
    }
    let end = bytes.len().min(MAX_BLOB_TEXT_BYTES);
    Ok(BlobView {
        binary: false,
        text: String::from_utf8_lossy(&bytes[..end]).into_owned(),
        preview: None,
        truncated: bytes.len() > end,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobView {
    pub binary: bool,
    pub text: String,
    pub preview: Option<FilePreview>,
    pub truncated: bool,
}

fn blob_size(repo: &Path, rev: &str, path: &str) -> Result<u64, Error> {
    let rev = crate::cli::check_rev(rev)?;
    let path = check_path(path)?;
    let spec = format!("{rev}:{path}");
    let output = crate::cli::run(repo, &["cat-file", "-s", &spec])?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(0))
}

fn blob_bytes(repo: &Path, rev: &str, path: &str) -> Result<Vec<u8>, Error> {
    let rev = crate::cli::check_rev(rev)?;
    let path = check_path(path)?;
    let spec = format!("{rev}:{path}");
    let output = crate::cli::run(repo, &["cat-file", "-p", &spec])?;
    Ok(output.stdout)
}

/// Bytes of `rev:path`, for saving a historical copy.
pub fn revision_bytes(repo: &Path, rev: &str, path: &str) -> Result<Vec<u8>, Error> {
    blob_bytes(repo, rev, path)
}

fn preview_from_bytes(bytes: &[u8]) -> Option<FilePreview> {
    if bytes.len() > 8_000_000 {
        return None;
    }
    if let Some(bmp) = tga_to_bmp(bytes) {
        return Some(FilePreview {
            mime: "image/bmp".into(),
            data_url: format!("data:image/bmp;base64,{}", base64_encode(&bmp)),
            animated: false,
        });
    }
    let (mime, animated) = image_kind(bytes)?;
    Some(FilePreview {
        mime: mime.into(),
        data_url: format!("data:{mime};base64,{}", base64_encode(bytes)),
        animated,
    })
}

/// Write the two sides of a file into temporary files and expand `{left}` `{right}` `{file}`.
pub fn launch_diff_tool(repo: &Path, template: &str, file: &str, staged: bool) -> Result<(), Error> {
    let path = check_path(file)?;
    let left_spec = if staged { format!("HEAD:{path}") } else { format!(":{path}") };
    let left_bytes = crate::cli::run(repo, &["cat-file", "-p", &left_spec]).map(|output| output.stdout).unwrap_or_default();
    let right_bytes = if staged {
        crate::cli::run(repo, &["cat-file", "-p", &format!(":{path}")]).map(|output| output.stdout).unwrap_or_default()
    } else {
        std::fs::read(repo.join(path)).unwrap_or_default()
    };
    let dir = tool_scratch();
    let left = write_copy(&dir, "left", path, &left_bytes)?;
    let right = write_copy(&dir, "right", path, &right_bytes)?;
    let absolute = repo.join(path).display().to_string();
    launch_template(repo, template, path, &[
        ("{left}", &left),
        ("{right}", &right),
        ("{file}", &absolute),
    ])
}

/// Diff two revisions of one path. A missing side is an empty file.
pub fn launch_diff_pair(repo: &Path, template: &str, file: &str, left_rev: &str, right_rev: &str) -> Result<(), Error> {
    let path = check_path(file)?;
    let left_bytes = blob_bytes(repo, left_rev, path).unwrap_or_default();
    let right_bytes = blob_bytes(repo, right_rev, path).unwrap_or_default();
    let dir = tool_scratch();
    let left = write_copy(&dir, "left", path, &left_bytes)?;
    let right = write_copy(&dir, "right", path, &right_bytes)?;
    let absolute = repo.join(path).display().to_string();
    launch_template(repo, template, path, &[
        ("{left}", &left),
        ("{right}", &right),
        ("{file}", &absolute),
    ])
}

/// Write ours, theirs, and the base, then expand `{ours}` `{theirs}` `{base}` `{working}` `{file}`.
///
/// `{working}` and `{file}` are the work-tree file, so the tool saves the merge back into the repository.
pub fn launch_merge_tool(repo: &Path, template: &str, file: &str) -> Result<(), Error> {
    let path = check_path(file)?;
    let dir = tool_scratch();
    let ours = write_copy(&dir, "ours", path, &stage_bytes(repo, path, 2))?;
    let theirs = write_copy(&dir, "theirs", path, &stage_bytes(repo, path, 3))?;
    let base = write_copy(&dir, "base", path, &stage_bytes(repo, path, 1))?;
    let working = repo.join(path).display().to_string();
    launch_template(repo, template, path, &[
        ("{ours}", &ours),
        ("{theirs}", &theirs),
        ("{base}", &base),
        ("{working}", &working),
        ("{file}", &working),
    ])
}

fn stage_bytes(repo: &Path, path: &str, stage: u8) -> Vec<u8> {
    match run(repo, &["show", &format!(":{stage}:{path}")]) {
        Ok(output) => output.stdout,
        Err(_) => Vec::new(),
    }
}

fn tool_scratch() -> PathBuf {
    let tick = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join("awegit-tool").join(format!("{tick}-{}", std::process::id()))
}

fn write_copy(dir: &Path, role: &str, rel: &str, bytes: &[u8]) -> Result<String, Error> {
    let folder = dir.join(role);
    std::fs::create_dir_all(&folder).map_err(|source| Error::Read { path: folder.display().to_string(), source })?;
    let dest = folder.join(leaf_name(rel));
    std::fs::write(&dest, bytes).map_err(|source| Error::Read { path: dest.display().to_string(), source })?;
    Ok(dest.display().to_string())
}

fn leaf_name(rel: &str) -> String {
    let leaf = Path::new(rel).file_name().and_then(|name| name.to_str()).unwrap_or("file");
    let cleaned: String = leaf
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            other => other,
        })
        .collect();
    if cleaned.is_empty() { "file".into() } else { cleaned }
}

fn launch_template(cwd: &Path, template: &str, file: &str, vars: &[(&str, &str)]) -> Result<(), Error> {
    let raw = template.trim();
    if raw.is_empty() {
        return Err(tool_failure("empty", "", raw, "", file, None));
    }
    let raw_program = split_command(raw).into_iter().next().unwrap_or_default();
    let command = expand_template(raw, vars);
    let args = split_command(&command);
    let Some((program, rest)) = args.split_first() else {
        return Err(tool_failure("empty", "", raw, &command, file, None));
    };
    if raw_program.contains('{') {
        return Err(tool_failure("no-program", program, raw, &command, file, None));
    }
    match classify_program(program) {
        ToolLaunch::Missing => Err(tool_failure("missing", program, raw, &command, file, None)),
        ToolLaunch::Directory => Err(tool_failure("directory", program, raw, &command, file, None)),
        ToolLaunch::NotProgram => Err(tool_failure("not-program", program, raw, &command, file, None)),
        ToolLaunch::Direct | ToolLaunch::Batch => spawn_tool(program, rest, cwd)
            .map(|_| ())
            .map_err(|source| tool_failure("spawn", program, raw, &command, file, Some(&source))),
    }
}

enum ToolLaunch {
    Direct,
    Batch,
    Missing,
    Directory,
    NotProgram,
}

fn classify_program(program: &str) -> ToolLaunch {
    let path = Path::new(program);
    if is_batch(program) {
        return if path.is_file() { ToolLaunch::Batch } else { ToolLaunch::Missing };
    }
    if path.is_dir() {
        return ToolLaunch::Directory;
    }
    if path.is_file() {
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_ascii_lowercase();
        if ext == "exe" || ext == "com" || looks_like_pe(path) {
            return ToolLaunch::Direct;
        }
        return ToolLaunch::NotProgram;
    }
    let rooted = program.contains(':') || program.contains('\\') || program.contains('/');
    if rooted { ToolLaunch::Missing } else { ToolLaunch::Direct }
}

fn looks_like_pe(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut magic = [0u8; 2];
    Read::read_exact(&mut file, &mut magic).is_ok() && magic == *b"MZ"
}

fn is_batch(program: &str) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let lower = program.to_ascii_lowercase();
    lower.ends_with(".cmd") || lower.ends_with(".bat")
}

fn spawn_tool(program: &str, args: &[String], cwd: &Path) -> std::io::Result<std::process::Child> {
    if is_batch(program) {
        // CreateProcess cannot run a batch file (os error 193). cmd.exe can.
        // Arguments stay separate so Rust quotes paths that contain spaces.
        let mut command = std::process::Command::new("cmd.exe");
        command.current_dir(cwd).arg("/d").arg("/c").arg(program).args(args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        return command.spawn();
    }
    std::process::Command::new(program).args(args).current_dir(cwd).spawn()
}

fn expand_template(template: &str, vars: &[(&str, &str)]) -> String {
    let mut text = template.to_string();
    for (key, value) in vars {
        text = replace_var(&text, key, value);
    }
    text
}

fn replace_var(text: &str, key: &str, value: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(index) = rest.find(key) {
        let quoted = index > 0 && matches!(rest.as_bytes()[index - 1], b'"' | b'\'');
        out.push_str(&rest[..index]);
        if !quoted && value.chars().any(|ch| ch.is_whitespace()) {
            out.push('"');
            out.push_str(value);
            out.push('"');
        } else {
            out.push_str(value);
        }
        rest = &rest[index + key.len()..];
    }
    out.push_str(rest);
    out
}

fn tool_failure(kind: &str, program: &str, template: &str, command: &str, file: &str, os: Option<&std::io::Error>) -> Error {
    let mut lines = Vec::new();
    lines.push("Could not start the external tool.".to_string());
    lines.push(format!("file: {file}"));
    if let Some(os) = os {
        lines.push(format!("system: {os}"));
    }
    let bad_exe = kind == "not-program" || kind == "no-program" || os.and_then(|err| err.raw_os_error()) == Some(193);
    if bad_exe {
        lines.push("os error 193 (ERROR_BAD_EXE_FORMAT): \"%1 不是有效的 Win32 应用程序\". Windows leaves %1 blank; %1 is the program path below, and that file is not a Win32 application.".into());
    }
    match kind {
        "empty" => lines.push("The tool command is empty. Set the program path in Preferences.".into()),
        "no-program" => {
            lines.push("The command does not start with a program. The first item is a placeholder, so Windows is asked to execute a temporary copy of the conflicted file.".into());
            lines.push("Set the tool's .exe path in Preferences and keep the placeholders as arguments. Example: \"C:\\Path\\CodeBuddy.exe\" {ours} {theirs} {working} {file}".into());
        }
        "not-program" => lines.push("The program path exists, but it is a document or another non-executable. Point it at the real .exe. A .cmd or .bat file is started through cmd.exe.".into()),
        "missing" => {
            lines.push("The program path does not exist.".into());
            lines.push("If the path contains spaces, wrap the program in double quotes.".into());
        }
        "directory" => lines.push("The program path is a directory, not an executable.".into()),
        _ => {}
    }
    if !template.is_empty() {
        lines.push(String::new());
        lines.push(format!("template: {template}"));
    }
    if !program.is_empty() {
        lines.push(format!("program: {program}"));
    }
    if !command.is_empty() {
        lines.push(format!("command: {command}"));
    }
    Error::Git(lines.join("\n"))
}

fn split_command(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in text.chars() {
        match (quote, ch) {
            (None, '"' | '\'') => quote = Some(ch),
            (Some(mark), ch) if ch == mark => quote = None,
            (None, ' ' | '\t') => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
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
    if bytes.starts_with(b"%PDF") {
        return Some(("application/pdf", false));
    }
    None
}

/// Uncompressed true-color TGA (type 2, 24 or 32 bits) as a bottom-up BMP.
fn tga_to_bmp(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 18 || bytes[2] != 2 {
        return None;
    }
    let id_len = bytes[0] as usize;
    let bpp = bytes[16];
    if bpp != 24 && bpp != 32 {
        return None;
    }
    let width = u16::from_le_bytes(bytes.get(12..14)?.try_into().ok()?) as usize;
    let height = u16::from_le_bytes(bytes.get(14..16)?.try_into().ok()?) as usize;
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return None;
    }
    let pixel = (bpp / 8) as usize;
    let start = 18 + id_len;
    let need = width.checked_mul(height)?.checked_mul(pixel)?;
    let pixels = bytes.get(start..start + need)?;
    let top_origin = bytes[17] & 0x20 != 0;
    let row_stride = (width * 3 + 3) & !3;
    let pixel_bytes = row_stride * height;
    let mut bmp = vec![0u8; 54 + pixel_bytes];
    bmp[0] = b'B';
    bmp[1] = b'M';
    let size = bmp.len() as u32;
    bmp[2..6].copy_from_slice(&size.to_le_bytes());
    bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&(width as u32).to_le_bytes());
    bmp[22..26].copy_from_slice(&(height as u32).to_le_bytes());
    bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
    bmp[34..38].copy_from_slice(&(pixel_bytes as u32).to_le_bytes());
    for y in 0..height {
        let src_y = if top_origin { height - 1 - y } else { y };
        for x in 0..width {
            let src = (src_y * width + x) * pixel;
            let dst = 54 + y * row_stride + x * 3;
            bmp[dst] = pixels[src];
            bmp[dst + 1] = pixels.get(src + 1).copied()?;
            bmp[dst + 2] = pixels.get(src + 2).copied()?;
        }
    }
    Some(bmp)
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
