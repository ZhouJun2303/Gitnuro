//! Structured observations of app-owned writes and isolated, per-commit previews.
//! Observers are scoped to a write, so background reads never become flow steps.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::cli::{check_rev, run, run_output, run_stdin};
use crate::{Error, InProgress};

static IDS: AtomicU64 = AtomicU64::new(1);
pub type Observer = Arc<dyn Fn(Event) + Send + Sync>;
thread_local! { static OBSERVER: RefCell<Option<Observer>> = RefCell::new(None); }

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub id: String,
    pub summary: String,
    pub author: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub path: String,
    pub base: String,
    pub ours: String,
    pub theirs: String,
    pub resolved: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub key: String,
    pub kind: String,
    pub state: String,
    pub at: u64,
    pub command: String,
    pub detail: String,
    pub source: Option<Commit>,
    pub target: Option<Commit>,
    pub base: Option<Commit>,
    pub stash: String,
    pub files: Vec<File>,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Used after staging the last conflict of a stash retry, and on app recovery.
/// A paused sequencer or unmerged index always keeps the saved work untouched.
pub fn restore_pending(repo: &Path) -> Result<(), Error> {
    crate::autostash::finish_if_settled(repo).map(|_| ())
}

pub fn observe<T>(observer: Observer, action: impl FnOnce() -> T) -> T {
    struct Reset(Option<Observer>);
    impl Drop for Reset {
        fn drop(&mut self) {
            OBSERVER.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let _reset = Reset(OBSERVER.with(|slot| slot.borrow_mut().replace(observer)));
    action()
}

pub(crate) fn emit(mut event: Event) {
    event.at = now();
    let observer = OBSERVER.with(|slot| slot.borrow().clone());
    if let Some(observer) = observer {
        observer(event);
    }
}

/// A command's id is reused for its result. Paths and revisions are metadata;
/// stdin (patches, file bodies, and credentials) is never recorded.
pub(crate) struct CommandSpan {
    event: Option<Event>,
    trace: Option<Trace>,
}

impl CommandSpan {
    pub(crate) fn start(args: &[&str]) -> Self {
        let enabled = OBSERVER.with(|slot| slot.borrow().is_some());
        let mut position = 0;
        while position < args.len() && args[position].starts_with('-') {
            position += if matches!(args[position], "-c" | "-C" | "--git-dir" | "--work-tree") {
                2
            } else {
                1
            };
        }
        let verb = args
            .get(position)
            .filter(|arg| {
                matches!(
                    **arg,
                    "pull"
                        | "push"
                        | "fetch"
                        | "merge"
                        | "rebase"
                        | "cherry-pick"
                        | "revert"
                        | "reset"
                        | "checkout"
                        | "stash"
                        | "commit"
                        | "add"
                        | "rm"
                        | "apply"
                )
            })
            .map(|_| position);
        let Some(at) = verb.filter(|_| enabled) else {
            return Self {
                event: None,
                trace: None,
            };
        };
        let kind = args[at];
        if kind == "stash"
            && args
                .get(at + 1)
                .is_some_and(|arg| matches!(*arg, "list" | "show"))
        {
            return Self {
                event: None,
                trace: None,
            };
        }
        // Only the actual subcommand is stored: configuration overrides may contain secrets.
        let event = Event {
            key: format!("command-{}-{}", now(), IDS.fetch_add(1, Ordering::Relaxed)),
            kind: if kind == "stash" {
                format!("stash-{}", args.get(at + 1).unwrap_or(&"push"))
            } else {
                kind.into()
            },
            state: "running".into(),
            command: redact(&format!("git {}", args[at..].join(" "))),
            ..Event::default()
        };
        emit(event.clone());
        Self {
            event: Some(event),
            trace: None,
        }
    }

    pub(crate) fn configure(&mut self, command: &mut std::process::Command) {
        if self
            .event
            .as_ref()
            .is_some_and(|event| event.kind == "pull")
        {
            self.trace = Trace::start(command);
        }
    }

    pub(crate) fn finish(&mut self, success: bool, stderr: &[u8]) {
        self.trace.take();
        if let Some(mut event) = self.event.take() {
            event.state = if success { "completed" } else { "failed" }.into();
            event.detail = redact(
                &String::from_utf8_lossy(stderr)
                    .chars()
                    .take(4000)
                    .collect::<String>(),
            );
            emit(event);
        }
    }
}

impl Drop for CommandSpan {
    fn drop(&mut self) {
        self.trace.take();
        if let Some(mut event) = self.event.take() {
            event.state = "interrupted".into();
            emit(event);
        }
    }
}

/// Native pull owns its fetch and integration children. Observe those phases
/// without replacing Git's pull semantics (fork-point, configuration and refspecs).
struct Trace {
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    path: PathBuf,
}
impl Trace {
    fn start(command: &mut std::process::Command) -> Option<Self> {
        let observer = OBSERVER.with(|slot| slot.borrow().clone())?;
        let path = std::env::temp_dir().join(format!(
            "awegit-trace-{}-{}-{}.jsonl",
            std::process::id(),
            now(),
            IDS.fetch_add(1, Ordering::Relaxed)
        ));
        let file = std::fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .ok()?;
        command.env("GIT_TRACE2_EVENT", &path);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker = std::thread::spawn(crate::timing::inherit(move || {
            use std::io::Read;
            let mut file = file;
            let mut buffer = Vec::new();
            let mut phases = std::collections::HashMap::<String, Event>::new();
            loop {
                let final_read = stopped.load(Ordering::Acquire);
                let _ = file.read_to_end(&mut buffer);
                while let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
                    let line: Vec<_> = buffer.drain(..=end).collect();
                    let Ok(record) = serde_json::from_slice::<serde_json::Value>(&line) else {
                        continue;
                    };
                    let id = format!(
                        "{}-{}",
                        record["sid"].as_str().unwrap_or_default(),
                        record["child_id"]
                    );
                    if record["event"] == "child_start" {
                        let args: Vec<_> = record["argv"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|arg| arg.as_str())
                            .collect();
                        // Only Git-owned phase verbs are retained; hooks and transport argv
                        // can contain private data and are deliberately excluded.
                        let verb = args
                            .iter()
                            .skip(1)
                            .find(|arg| matches!(**arg, "fetch" | "merge" | "rebase"));
                        if let Some(verb) = verb {
                            let event = Event {
                                key: format!(
                                    "phase-{}-{}",
                                    now(),
                                    IDS.fetch_add(1, Ordering::Relaxed)
                                ),
                                kind: (*verb).into(),
                                state: "running".into(),
                                at: now(),
                                command: format!("git {verb}"),
                                detail: "git-child".into(),
                                ..Default::default()
                            };
                            observer(event.clone());
                            phases.insert(id, event);
                        }
                    } else if record["event"] == "child_exit" {
                        if let Some(mut event) = phases.remove(&id) {
                            event.at = now();
                            event.state = if record["code"].as_i64() == Some(0) {
                                "completed"
                            } else {
                                "failed"
                            }
                            .into();
                            observer(event);
                        }
                    }
                }
                if final_read {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
            for (_, mut event) in phases {
                event.state = "interrupted".into();
                observer(event);
            }
        }));
        Some(Self {
            stop,
            worker: Some(worker),
            path,
        })
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn redact(text: &str) -> String {
    text.split_inclusive(char::is_whitespace)
        .map(|part| {
            let Some(protocol) = part.find("://") else {
                return part.to_string();
            };
            let start = protocol + 3;
            let end = part[start..]
                .find('/')
                .map(|at| at + start)
                .unwrap_or(part.len());
            if let Some(at) = part[start..end].rfind('@') {
                format!("{}[redacted]@{}", &part[..start], &part[start + at + 1..])
            } else {
                part.to_string()
            }
        })
        .collect()
}

pub fn git_dir(repo: &Path) -> Result<PathBuf, Error> {
    crate::model::absolute_git_dir(repo)
}

fn text(repo: &Path, args: &[&str]) -> Result<String, Error> {
    Ok(String::from_utf8_lossy(&run(repo, args)?.stdout)
        .trim_end()
        .to_string())
}

pub fn commit(repo: &Path, revision: &str) -> Option<Commit> {
    let revision = check_rev(revision).ok()?;
    // `show -s` still computes combined merge diffs (notably stash commits).
    // A metadata read must never walk trees, including in partial clones.
    let value = text(repo, &["log", "-1", "--no-walk", "--no-patch", "--format=%H%x00%s%x00%an", revision]).ok()?;
    let mut parts = value.split('\0');
    Some(Commit {
        id: parts.next()?.into(),
        summary: parts.next().unwrap_or_default().into(),
        author: parts.next().unwrap_or_default().into(),
    })
}

/// One bounded log read, instead of spawning Git once per commit on long branches.
pub fn replay_commits(repo: &Path, onto: &str, head: &str) -> Result<Vec<Commit>, Error> {
    check_rev(onto)?;
    check_rev(head)?;
    let range = format!("{onto}..{head}");
    let output = run(
        repo,
        &[
            "log",
            "--reverse",
            "--topo-order",
            "--no-merges",
            "--max-count=2001",
            "--format=%H%x00%s%x00%an",
            "-z",
            &range,
        ],
    )?;
    let values: Vec<_> = output.stdout.split(|byte| *byte == 0).collect();
    Ok(values
        .chunks_exact(3)
        .map(|parts| Commit {
            id: String::from_utf8_lossy(parts[0]).into(),
            summary: String::from_utf8_lossy(parts[1]).into(),
            author: String::from_utf8_lossy(parts[2]).into(),
        })
        .collect())
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    pub head: Option<Commit>,
    pub branch: String,
    pub progress: Option<InProgress>,
    pub replay: Option<Commit>,
    pub incoming: Option<Commit>,
    pub base: Option<Commit>,
    pub pending_stash: String,
    pub files: Vec<File>,
    pub done: Vec<String>,
}

pub fn checkpoint(repo: &Path) -> Result<Checkpoint, Error> {
    let dir = git_dir(repo)?;
    let progress = crate::in_progress(repo)?;
    let replay = match progress {
        Some(InProgress::Rebase) => commit(repo, "REBASE_HEAD"),
        Some(InProgress::CherryPick) => commit(repo, "CHERRY_PICK_HEAD"),
        Some(InProgress::Revert) => commit(repo, "REVERT_HEAD"),
        _ => None,
    };
    let incoming = match progress {
        Some(InProgress::Merge) => commit(repo, "MERGE_HEAD"),
        _ => replay.clone(),
    };
    let head = commit(repo, "HEAD");
    let base = incoming.as_ref().and_then(|source| match progress {
        // Replay applies the original parent -> commit patch, whereas revert
        // applies commit -> parent. Neither uses the branches' merge-base.
        Some(InProgress::Rebase | InProgress::CherryPick) => {
            commit(repo, &format!("{}^1", source.id))
        }
        Some(InProgress::Revert) => Some(source.clone()),
        _ => {
            let id = text(repo, &["merge-base", "HEAD", &source.id]).ok()?;
            commit(repo, &id)
        }
    });
    let mut files: Vec<File> = Vec::new();
    let mut positions = HashMap::new();
    let output = run(repo, &["ls-files", "--unmerged", "-z"])?;
    for entry in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let Some(tab) = entry.iter().position(|byte| *byte == b'\t') else {
            continue;
        };
        let path = String::from_utf8_lossy(&entry[tab + 1..]).to_string();
        let fields = String::from_utf8_lossy(&entry[..tab]);
        let fields: Vec<_> = fields.split_whitespace().collect();
        if fields.len() != 3 {
            continue;
        }
        let index = *positions.entry(path.clone()).or_insert_with(|| {
                files.push(File {
                    path,
                    ..File::default()
                });
                files.len() - 1
            });
        match fields[2] {
            "1" => files[index].base = fields[1].into(),
            "2" => files[index].ours = fields[1].into(),
            "3" => files[index].theirs = fields[1].into(),
            _ => {}
        }
    }
    let done = std::fs::read_to_string(dir.join("rebase-merge/done"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1).map(str::to_string))
        .collect();
    Ok(Checkpoint {
        head,
        branch: text(repo, &["symbolic-ref", "--short", "-q", "HEAD"]).unwrap_or_default(),
        progress,
        replay,
        incoming,
        base,
        files,
        done,
        pending_stash: crate::model::pending_autostash(repo).unwrap_or_default(),
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewStep {
    pub commit: Commit,
    /// clean, conflict, conditional, dropped, or unknown; never claim certainty after a conflict.
    pub state: String,
    pub files: Vec<String>,
    pub verb: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub source: Option<Commit>,
    pub target: Option<Commit>,
    pub steps: Vec<PreviewStep>,
    pub detail: String,
}

static SANDBOX_ID: AtomicU64 = AtomicU64::new(1);

/// How many replay steps one `merge-tree --stdin` call may cover.
/// A conflict ends the preview, so the batch stays short enough that the first
/// conflict does not merge the rest of a long branch.
const PREVIEW_BATCH: usize = 32;

struct PlannedCommit {
    commit: Commit,
    verb: String,
    parents: Option<Vec<String>>,
    tree: String,
}

struct MergeOutcome {
    clean: bool,
    tree: String,
    files: Vec<String>,
}

struct BatchProgress {
    next: usize,
    current: String,
    conflict: bool,
}

fn copy_preview_config(repo: &Path, sandbox: &Path) -> Result<(), Error> {
    // Stored keys are lowercase. One regexp replaces a failing `config --get` per key.
    let output = run_output(
        repo,
        &[
            "config",
            "--get-regexp",
            r"^(merge\.(renames|renamelimit|directoryrenames|renormalize)|diff\.renames|core\.ignorecase)$",
        ],
    )?;
    if !output.status.success() {
        return Ok(());
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((key, value)) = line.split_once(' ') else {
            continue;
        };
        if key.is_empty() {
            continue;
        }
        run(sandbox, &["config", key, value])?;
    }
    Ok(())
}

fn replay_plan(repo: &Path, onto: &str, head: &str) -> Result<Vec<PlannedCommit>, Error> {
    check_rev(onto)?;
    check_rev(head)?;
    let range = format!("{onto}..{head}");
    let output = run(
        repo,
        &[
            "log",
            "--reverse",
            "--topo-order",
            "--no-merges",
            "--max-count=2001",
            "--format=%H%x00%s%x00%an%x00%P%x00%T",
            "-z",
            &range,
        ],
    )?;
    let values: Vec<_> = output.stdout.split(|byte| *byte == 0).collect();
    Ok(values
        .chunks_exact(5)
        .map(|parts| PlannedCommit {
            commit: Commit {
                id: String::from_utf8_lossy(parts[0]).into(),
                summary: String::from_utf8_lossy(parts[1]).into(),
                author: String::from_utf8_lossy(parts[2]).into(),
            },
            verb: "pick".into(),
            parents: Some(
                String::from_utf8_lossy(parts[3])
                    .split_whitespace()
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .collect(),
            ),
            tree: String::from_utf8_lossy(parts[4]).into(),
        })
        .collect())
}

fn lookup_parents(repo: &Path, id: &str) -> Result<Vec<String>, Error> {
    let parents = text(repo, &["rev-list", "--parents", "-n", "1", id])?;
    Ok(parents
        .split_whitespace()
        .skip(1)
        .map(str::to_string)
        .collect())
}

fn field_at(bytes: &[u8], index: &mut usize) -> Option<String> {
    let start = *index;
    let rest = bytes.get(start..)?;
    let end = rest.iter().position(|byte| *byte == 0)?;
    *index = start + end + 1;
    Some(String::from_utf8_lossy(&bytes[start..start + end]).into_owned())
}

fn skip_merge_messages(bytes: &[u8], index: &mut usize) -> Option<()> {
    while *index < bytes.len() {
        if bytes[*index] == 0 {
            *index += 1;
            return Some(());
        }
        let count: usize = field_at(bytes, index)?.parse().ok()?;
        for _ in 0..count {
            field_at(bytes, index)?;
        }
        field_at(bytes, index)?;
        field_at(bytes, index)?;
    }
    None
}

/// `merge-tree --stdin` records: status, tree, files, messages, then a trailing NUL.
fn parse_merge_tree_stdin(bytes: &[u8]) -> Option<Vec<MergeOutcome>> {
    if bytes.is_empty() {
        return Some(Vec::new());
    }
    let mut index = 0;
    let mut merges = Vec::new();
    while index < bytes.len() {
        let status = field_at(bytes, &mut index)?;
        let clean = match status.as_str() {
            "1" => true,
            "0" => false,
            _ => return None,
        };
        let tree = field_at(bytes, &mut index)?;
        if !matches!(tree.len(), 40 | 64) {
            return None;
        }
        let mut files = Vec::new();
        if clean {
            if !field_at(bytes, &mut index)?.is_empty() {
                return None;
            }
        } else {
            loop {
                let file = field_at(bytes, &mut index)?;
                if file.is_empty() {
                    break;
                }
                files.push(file);
            }
            skip_merge_messages(bytes, &mut index)?;
        }
        merges.push(MergeOutcome { clean, tree, files });
    }
    Some(merges)
}

fn outcome_from_merge(output: &std::process::Output) -> Option<MergeOutcome> {
    if !matches!(output.status.code(), Some(0 | 1)) {
        return None;
    }
    let parts: Vec<_> = output.stdout.split(|byte| *byte == 0).collect();
    let tree = parts
        .first()
        .map(|value| String::from_utf8_lossy(value).trim().to_string())
        .unwrap_or_default();
    if !matches!(tree.len(), 40 | 64) {
        return None;
    }
    let files = if output.status.success() {
        Vec::new()
    } else {
        parts
            .iter()
            .skip(1)
            .take_while(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part).into_owned())
            .collect()
    };
    Some(MergeOutcome {
        clean: output.status.success(),
        tree,
        files,
    })
}

fn batch_end(commits: &[PlannedCommit], start: usize) -> usize {
    let mut end = start;
    while end < commits.len() && end < 200 && end - start < PREVIEW_BATCH {
        let step = &commits[end];
        let Some(parents) = &step.parents else {
            break;
        };
        if parents.len() != 1
            || matches!(step.verb.as_str(), "drop" | "squash" | "fixup")
            || !matches!(step.tree.len(), 40 | 64)
        {
            break;
        }
        end += 1;
    }
    end
}

fn known_commit(commits: &[PlannedCommit], source: &str, current: &str) -> bool {
    current == source || commits.iter().any(|item| item.commit.id == current)
}

fn recorded_step(step: &PlannedCommit, state: &str, files: Vec<String>) -> PreviewStep {
    PreviewStep {
        commit: step.commit.clone(),
        state: state.into(),
        files,
        verb: step.verb.clone(),
    }
}

fn advance_current(step: &PlannedCommit, tree: &str) -> String {
    if !step.tree.is_empty() && tree == step.tree {
        step.commit.id.clone()
    } else {
        tree.to_string()
    }
}

/// Replay a prefix in one process. Later rows assume each previous result tree
/// is that commit's own tree; the first row that breaks the chain is kept, and
/// the rows after it are discarded.
fn apply_batch(
    sandbox: &Path,
    commits: &[PlannedCommit],
    start: usize,
    end: usize,
    current: &str,
    preview: &mut Preview,
) -> Result<Option<BatchProgress>, Error> {
    let mut input = String::new();
    let mut ours = current.to_string();
    for step in &commits[start..end] {
        let parents = step.parents.as_ref().expect("batch steps have one parent");
        input.push_str(&format!("{} -- {} {}\n", parents[0], ours, step.commit.id));
        ours = step.commit.id.clone();
    }
    let output = match run_stdin(
        sandbox,
        &["merge-tree", "--write-tree", "--name-only", "-z", "--stdin"],
        input.as_bytes(),
    ) {
        Ok(output) => output,
        Err(_) => return Ok(None),
    };
    let Some(merges) = parse_merge_tree_stdin(&output.stdout) else {
        return Ok(None);
    };
    if merges.len() != end - start {
        return Ok(None);
    }
    let mut current = current.to_string();
    let mut consumed = start;
    for (offset, merge) in merges.iter().enumerate() {
        let step_index = start + offset;
        if offset > 0 {
            let previous = &merges[offset - 1];
            let previous_step = &commits[step_index - 1];
            if !previous.clean || previous.tree != previous_step.tree {
                break;
            }
        }
        let step = &commits[step_index];
        if merge.clean {
            current = advance_current(step, &merge.tree);
            preview.steps.push(recorded_step(step, "clean", Vec::new()));
            consumed = step_index + 1;
            if merge.tree != step.tree {
                break;
            }
        } else {
            preview
                .steps
                .push(recorded_step(step, "conflict", merge.files.clone()));
            return Ok(Some(BatchProgress {
                next: step_index + 1,
                current,
                conflict: true,
            }));
        }
    }
    if consumed == start {
        return Ok(None);
    }
    Ok(Some(BatchProgress {
        next: consumed,
        current,
        conflict: false,
    }))
}

struct Sandbox(PathBuf);
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sandbox(repo: &Path) -> Result<Sandbox, Error> {
    let path = std::env::temp_dir().join(format!(
        "awegit-preview-{}-{}-{}",
        std::process::id(),
        now(),
        SANDBOX_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })?;
    let sandbox = Sandbox(path);
    run(&sandbox.0, &["init", "--quiet", "--template="])?;
    run(&sandbox.0, &["config", "core.hooksPath", ".git/no-hooks"])?;
    run(&sandbox.0, &["config", "core.fsmonitor", "false"])?;
    copy_preview_config(repo, &sandbox.0)?;
    let objects = text(
        repo,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "objects",
        ],
    )?;
    std::fs::write(
        sandbox.0.join(".git/objects/info/alternates"),
        format!("{}\n", objects.replace('\\', "/")),
    )
    .map_err(|source| Error::Read {
        path: sandbox.0.display().to_string(),
        source,
    })?;
    Ok(sandbox)
}

/// Replay against a frozen target in a separate object database. No checkout,
/// hooks, filters, stash, or refs in the user's repository are touched.
pub fn preview(
    repo: &Path,
    onto: &str,
    rebase: bool,
    instructions: &[crate::RebaseStep],
) -> Result<Preview, Error> {
    preview_at(repo, onto, "HEAD", rebase, instructions)
}

pub fn preview_at(
    repo: &Path,
    onto: &str,
    head: &str,
    rebase: bool,
    instructions: &[crate::RebaseStep],
) -> Result<Preview, Error> {
    preview_at_cancellable(repo, onto, head, rebase, instructions, &|| false)
}

pub fn preview_at_cancellable(
    repo: &Path,
    onto: &str,
    head: &str,
    rebase: bool,
    instructions: &[crate::RebaseStep],
    cancelled: &dyn Fn() -> bool,
) -> Result<Preview, Error> {
    if cancelled() { return Ok(Preview { detail: "preview-cancelled".into(), ..Default::default() }); }
    let onto = check_rev(onto)?;
    let source = commit(repo, onto).ok_or_else(|| Error::Rev(onto.into()))?;
    let target = commit(repo, head).ok_or_else(|| Error::Rev(head.into()))?;
    let mut preview = Preview {
        source: Some(source.clone()),
        target: Some(target.clone()),
        ..Preview::default()
    };
    if std::fs::read(git_dir(repo)?.join("info/attributes")).is_ok_and(|bytes| !bytes.is_empty()) {
        preview.detail = "attributes-overrides".into();
        return Ok(preview);
    }
    // A custom merge driver can have arbitrary side effects and cannot be predicted safely.
    let drivers = run_output(repo, &["config", "--get-regexp", "^merge\\..*\\.driver$"])?;
    if drivers.status.success() {
        preview.detail = "custom-merge-driver".into();
        return Ok(preview);
    }
    if cancelled() { preview.detail = "preview-cancelled".into(); return Ok(preview); }
    let sandbox = sandbox(repo)?;
    let started = Instant::now();
    let commits: Vec<PlannedCommit> = if !rebase {
        vec![PlannedCommit {
            commit: source.clone(),
            verb: "merge".into(),
            parents: None,
            tree: String::new(),
        }]
    } else if !instructions.is_empty() {
        instructions
            .iter()
            .filter_map(|step| {
                commit(repo, &step.rev).map(|commit| PlannedCommit {
                    commit,
                    verb: step.verb.clone(),
                    parents: None,
                    tree: String::new(),
                })
            })
            .collect()
    } else {
        replay_plan(repo, &source.id, &target.id)?
    };
    let mut current = if rebase {
        source.id.clone()
    } else {
        target.id.clone()
    };
    let mut conditional = false;
    let mut index = 0;
    while index < commits.len() {
        if cancelled() {
            preview.detail = "preview-cancelled".into();
            return Ok(preview);
        }
        if commits[index].verb == "drop" {
            preview.steps.push(recorded_step(&commits[index], "dropped", Vec::new()));
            index += 1;
            continue;
        }
        if conditional || index >= 200 || started.elapsed().as_secs() >= 5 {
            conditional = true;
            if preview.detail.is_empty() {
                preview.detail = "conditional-after-conflict-or-limit".into();
            }
            preview
                .steps
                .push(recorded_step(&commits[index], "conditional", Vec::new()));
            index += 1;
            continue;
        }
        if cancelled() {
            preview.detail = "preview-cancelled".into();
            return Ok(preview);
        }
        // merge-tree --write-tree does not read the index or work tree, so the
        // sandbox does not need read-tree. A result tree is a valid next "ours"
        // when --merge-base is set, so the replay does not need commit-tree either.
        if rebase {
            let end = batch_end(&commits, index);
            if end > index + 1 && known_commit(&commits, &source.id, &current) {
                if let Some(applied) = apply_batch(&sandbox.0, &commits, index, end, &current, &mut preview)? {
                    index = applied.next;
                    current = applied.current;
                    if applied.conflict {
                        conditional = true;
                    }
                    continue;
                }
            }
            let parents = match &commits[index].parents {
                Some(parents) => parents.clone(),
                None => lookup_parents(repo, &commits[index].commit.id)?,
            };
            if parents.len() != 1 || matches!(commits[index].verb.as_str(), "squash" | "fixup") {
                preview.detail = "preview-unavailable".into();
                conditional = true;
                preview
                    .steps
                    .push(recorded_step(&commits[index], "unknown", Vec::new()));
                index += 1;
                continue;
            }
            let output = run_output(
                &sandbox.0,
                &[
                    "merge-tree",
                    "--write-tree",
                    "--name-only",
                    "-z",
                    &format!("--merge-base={}", parents[0]),
                    &current,
                    &commits[index].commit.id,
                ],
            )?;
            match outcome_from_merge(&output) {
                Some(outcome) if outcome.clean => {
                    current = advance_current(&commits[index], &outcome.tree);
                    preview
                        .steps
                        .push(recorded_step(&commits[index], "clean", Vec::new()));
                }
                Some(outcome) => {
                    conditional = true;
                    preview.steps.push(recorded_step(
                        &commits[index],
                        "conflict",
                        outcome.files,
                    ));
                }
                None => {
                    preview.detail = "preview-unavailable".into();
                    conditional = true;
                    preview
                        .steps
                        .push(recorded_step(&commits[index], "unknown", Vec::new()));
                }
            }
            index += 1;
            continue;
        }
        let output = run_output(
            &sandbox.0,
            &[
                "merge-tree",
                "--write-tree",
                "--name-only",
                "-z",
                &current,
                &commits[index].commit.id,
            ],
        )?;
        match outcome_from_merge(&output) {
            Some(outcome) if outcome.clean => {
                preview
                    .steps
                    .push(recorded_step(&commits[index], "clean", Vec::new()));
            }
            Some(outcome) => {
                conditional = true;
                preview.steps.push(recorded_step(
                    &commits[index],
                    "conflict",
                    outcome.files,
                ));
            }
            None => {
                preview.detail = "preview-unavailable".into();
                conditional = true;
                preview
                    .steps
                    .push(recorded_step(&commits[index], "unknown", Vec::new()));
            }
        }
        index += 1;
    }
    Ok(preview)
}

#[cfg(test)]
mod stdin_parse_tests {
    use super::parse_merge_tree_stdin;

    #[test]
    fn reads_a_clean_record_and_a_conflict_record() {
        let mut bytes = Vec::new();
        bytes.extend(b"1\0");
        bytes.extend(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\0\0");
        bytes.extend(b"0\0");
        bytes.extend(b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\0");
        bytes.extend(b"n.txt\0\0");
        bytes.extend(b"1\0n.txt\0Auto-merging\0Auto-merging n.txt\n\0");
        bytes.extend(b"1\0n.txt\0CONFLICT (contents)\0CONFLICT (content): Merge conflict in n.txt\n\0\0");
        let merges = parse_merge_tree_stdin(&bytes).expect("records");
        assert_eq!(merges.len(), 2);
        assert!(merges[0].clean);
        assert!(merges[0].files.is_empty());
        assert!(!merges[1].clean);
        assert_eq!(merges[1].files, ["n.txt"]);
        assert_eq!(merges[1].tree, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    }
}
