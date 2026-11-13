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
    for key in [
        "merge.renames",
        "merge.renameLimit",
        "merge.directoryRenames",
        "merge.renormalize",
        "diff.renames",
        "core.ignoreCase",
    ] {
        if let Ok(value) = text(repo, &["config", "--get", key]) {
            run(&sandbox.0, &["config", key, &value])?;
        }
    }
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
    let commits = if !rebase {
        vec![(source.clone(), "merge".to_string())]
    } else if !instructions.is_empty() {
        instructions
            .iter()
            .filter_map(|step| commit(repo, &step.rev).map(|commit| (commit, step.verb.clone())))
            .collect()
    } else {
        replay_commits(repo, &source.id, &target.id)?
            .into_iter()
            .map(|commit| (commit, "pick".to_string()))
            .collect()
    };
    let mut current = if rebase {
        source.id.clone()
    } else {
        target.id.clone()
    };
    let mut conditional = false;
    for (index, (original, verb)) in commits.into_iter().enumerate() {
        if cancelled() { preview.detail = "preview-cancelled".into(); return Ok(preview); }
        let mut step = PreviewStep {
            commit: original.clone(),
            state: "conditional".into(),
            files: Vec::new(),
            verb: verb.clone(),
        };
        if verb == "drop" {
            step.state = "dropped".into();
            preview.steps.push(step);
            continue;
        }
        if conditional || index >= 200 || started.elapsed().as_secs() >= 5 {
            conditional = true;
            if preview.detail.is_empty() {
                preview.detail = "conditional-after-conflict-or-limit".into();
            }
            preview.steps.push(step);
            continue;
        }
        let mut args = vec![
            "merge-tree".to_string(),
            "--write-tree".into(),
            "--name-only".into(),
            "-z".into(),
        ];
        // Make committed attributes visible without checking out files or running filters.
        std::fs::write(sandbox.0.join(".git/HEAD"), format!("{current}\n")).map_err(|source| {
            Error::Read {
                path: sandbox.0.display().to_string(),
                source,
            }
        })?;
        run(&sandbox.0, &["read-tree", &current])?;
        if cancelled() { preview.detail = "preview-cancelled".into(); return Ok(preview); }
        if rebase {
            // A replay is a three-way application of this commit against its original parent,
            // not a merge of the two final branch trees.
            let parents = text(repo, &["rev-list", "--parents", "-n", "1", &original.id])?;
            let parents: Vec<_> = parents.split_whitespace().skip(1).collect();
            if parents.len() != 1 || matches!(verb.as_str(), "squash" | "fixup") {
                step.state = "unknown".into();
                preview.detail = "preview-unavailable".into();
                conditional = true;
                preview.steps.push(step);
                continue;
            }
            let parent = parents[0];
            args.push(format!("--merge-base={parent}"));
        }
        args.extend([current.clone(), original.id.clone()]);
        let borrowed: Vec<_> = args.iter().map(String::as_str).collect();
        let output = run_output(&sandbox.0, &borrowed)?;
        let parts: Vec<_> = output.stdout.split(|byte| *byte == 0).collect();
        let tree = parts
            .first()
            .map(|value| String::from_utf8_lossy(value).trim().to_string())
            .unwrap_or_default();
        if output.status.success() && matches!(tree.len(), 40 | 64) {
            step.state = "clean".into();
            if rebase {
                let output = run_stdin(
                    &sandbox.0,
                    &[
                        "-c",
                        "commit.gpgsign=false",
                        "-c",
                        "user.name=AweGit preview",
                        "-c",
                        "user.email=preview@localhost",
                        "commit-tree",
                        &tree,
                        "-p",
                        &current,
                    ],
                    b"AweGit isolated preview\n",
                )?;
                current = String::from_utf8_lossy(&output.stdout).trim().into();
            }
        } else if output.status.code() == Some(1) && matches!(tree.len(), 40 | 64) {
            step.state = "conflict".into();
            step.files = parts
                .iter()
                .skip(1)
                .take_while(|part| !part.is_empty())
                .map(|part| String::from_utf8_lossy(part).into_owned())
                .collect();
            conditional = true;
        } else {
            step.state = "unknown".into();
            preview.detail = "preview-unavailable".into();
            conditional = true;
        }
        preview.steps.push(step);
    }
    Ok(preview)
}
