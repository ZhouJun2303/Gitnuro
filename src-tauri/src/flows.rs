//! Durable app-owned operation journals. A delta contains one changed node,
//! never a copy of the whole graph. Git remains the authority for resolution.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant, SystemTime};

use awegit_git::flow::{self, Checkpoint, Commit, Event, File, Observer};
use awegit_git::{InProgress, Mutation};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static SESSIONS: OnceLock<Mutex<HashMap<PathBuf, Arc<Session>>>> = OnceLock::new();
static RECOVERY: Mutex<()> = Mutex::new(());
static IDS: AtomicU64 = AtomicU64::new(1);

struct PreviewJob {
    session: Weak<Session>,
    generation: u64,
    epoch: u64,
    onto: String,
    rebase: bool,
    instructions: Vec<awegit_git::RebaseStep>,
    context: Option<awegit_git::TimingContext>,
}

#[derive(Default)]
struct PreviewQueue {
    pending: Option<PreviewJob>,
    running: bool,
}

#[derive(Default)]
struct PreviewLane {
    queue: Mutex<PreviewQueue>,
    epoch: AtomicU64,
}

impl PreviewLane {
    fn submit(self: Arc<Self>, mut job: PreviewJob, run: impl Fn(&Self, PreviewJob) + Send + 'static) {
        let mut queue = self.queue.lock().unwrap_or_else(|error| error.into_inner());
        job.epoch = self.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        queue.pending = Some(job);
        if queue.running { return; }
        queue.running = true;
        drop(queue);
        std::thread::spawn(move || loop {
            let job = {
                let mut queue = self.queue.lock().unwrap_or_else(|error| error.into_inner());
                let Some(job) = queue.pending.take() else { queue.running = false; return };
                job
            };
            run(&self, job);
        });
    }
}

fn preview_lane(repo: &Path) -> Arc<PreviewLane> {
    static LANES: OnceLock<Mutex<HashMap<String, Weak<PreviewLane>>>> = OnceLock::new();
    let mut lanes = LANES.get_or_init(Default::default).lock().unwrap_or_else(|error| error.into_inner());
    lanes.retain(|_, lane| lane.strong_count() != 0);
    let entry = lanes.entry(awegit_git::repo_key(repo)).or_default();
    if let Some(lane) = entry.upgrade() { return lane; }
    let lane = Arc::new(PreviewLane::default());
    *entry = Arc::downgrade(&lane);
    lane
}

fn sessions() -> &'static Mutex<HashMap<PathBuf, Arc<Session>>> {
    SESSIONS.get_or_init(Default::default)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub repo: String,
    pub kind: String,
    pub state: String,
    pub sequence: u64,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub active_step: String,
    pub can_auto_close: bool,
    pub branch: String,
    pub before: Option<Commit>,
    pub after: Option<Commit>,
    pub progress: Option<InProgress>,
    pub pending_stash: String,
    pub recording_error: String,
    pub previous: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    #[serde(flatten)]
    pub event: Event,
    pub created_at: u64,
    pub round: u32,
    pub repeat_of: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delta {
    pub summary: Summary,
    pub node: Option<Node>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub summary: Summary,
    pub nodes: Vec<Node>,
}

struct Data {
    snapshot: Snapshot,
    round: u32,
    round_key: String,
    generation: u64,
    writing: bool,
    aborted: bool,
    failed: bool,
    request_key: String,
    last_checkpoint: Option<Checkpoint>,
    checkpoint_stamp: Option<CheckpointStamp>,
    checked_at: Option<Instant>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CheckpointStamp {
    revision: u64,
    files: Vec<Option<(u64, SystemTime)>>,
}

pub(crate) fn checkpoint_stamp(repo: &Path) -> Option<CheckpointStamp> {
    let git = flow::git_dir(repo).ok()?;
    let mut paths: Vec<_> = ["HEAD", "index", "MERGE_HEAD", "REBASE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "MERGE_MSG", "awegit", "rebase-merge", "rebase-apply", "sequencer", "rebase-merge/done", "rebase-merge/git-rebase-todo", "rebase-merge/stopped-sha", "rebase-apply/next", "sequencer/todo"]
        .into_iter().map(|path| git.join(path)).collect();
    let common = super::common_metadata_dir(&git);
    paths.push(common.join("packed-refs"));
    if let Some(name) = fs::read_to_string(git.join("HEAD")).ok().and_then(|head| head.trim().strip_prefix("ref: ").map(str::to_string)) {
        paths.push(common.join(name));
    }
    Some(CheckpointStamp {
        revision: super::changes::revision(repo),
        files: paths.into_iter().map(|path| fs::metadata(path).ok().and_then(|meta| Some((meta.len(), meta.modified().ok()?)))).collect(),
    })
}

pub struct Session {
    repo: PathBuf,
    directory: PathBuf,
    data: Mutex<Data>,
    refresh_lock: Mutex<()>,
}

pub fn install(app: tauri::AppHandle) {
    let _ = APP.set(app);
    std::thread::spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
        cleanup_known();
    });
    std::thread::spawn(cleanup_known);
}

fn directory(repo: &Path) -> Result<PathBuf, String> {
    flow::git_dir(repo)
        .map(|path| path.join("awegit-flow"))
        .map_err(|error| error.to_string())
}

fn broadcast(delta: &Delta) {
    if let Some(app) = APP.get() {
        let _ = app.emit("operation-flow", delta);
    }
}

#[cfg(not(test))]
fn open(repo: &Path, id: &str) {
    if !super::settings::load()
        .map(|settings| settings.flow_auto_open)
        .unwrap_or(true)
    {
        return;
    }
    let Some(app) = APP.get() else { return };
    let app = app.clone();
    let repo = repo.display().to_string();
    let id = id.to_string();
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if !super::settings::load()
            .map(|settings| settings.flow_auto_open)
            .unwrap_or(true)
        {
            return;
        }
        if let Err(error) = super::open_window(
            handle,
            Some("flow".into()),
            Some(repo),
            Some("live".into()),
            Some(id),
            Some(1280.0),
            Some(840.0),
        ) {
            super::perf::write("WARN", "operation-flow-window", &error);
        }
    });
}

#[cfg(test)]
fn open(repo: &Path, id: &str) {
    tests::OPENED
        .lock()
        .unwrap()
        .push((repo.to_path_buf(), id.into()));
}

impl Session {
    fn publish(&self, data: &mut Data, node: Option<Node>) {
        data.snapshot.summary.sequence += 1;
        let delta = Delta {
            summary: data.snapshot.summary.clone(),
            node,
        };
        let result = (|| -> Result<(), String> {
            fs::create_dir_all(&self.directory).map_err(|error| error.to_string())?;
            let path = self
                .directory
                .join(format!("{}.jsonl", data.snapshot.summary.id));
            let mut file = OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .map_err(|error| error.to_string())?;
            serde_json::to_writer(&mut file, &delta).map_err(|error| error.to_string())?;
            file.write_all(b"\n").map_err(|error| error.to_string())?;
            // The summary is a small index. The append-only journal is authoritative.
            let index = self
                .directory
                .join(format!("{}.json", data.snapshot.summary.id));
            let temporary = index.with_extension("tmp");
            fs::write(
                &temporary,
                serde_json::to_vec(&data.snapshot.summary).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            fs::rename(&temporary, &index).map_err(|error| error.to_string())?;
            let history = history_index(&self.directory);
            let mut history = history.lock().unwrap_or_else(|error| error.into_inner());
            history.paths.insert(index.clone());
            history.summaries.insert(index.clone(), (file_stamp(&index), data.snapshot.summary.clone()));
            Ok(())
        })();
        if let Err(error) = result {
            data.snapshot.summary.recording_error = error;
        }
        broadcast(&Delta {
            summary: data.snapshot.summary.clone(),
            node: delta.node,
        });
    }

    fn put(&self, data: &mut Data, mut event: Event) {
        if event.at == 0 {
            event.at = flow::now();
        }
        let at = data
            .snapshot
            .nodes
            .iter()
            .position(|node| node.event.key == event.key);
        let created_at = at
            .map(|index| data.snapshot.nodes[index].created_at)
            .unwrap_or(event.at);
        let round = at
            .map(|index| data.snapshot.nodes[index].round)
            .unwrap_or(data.round);
        let event_paths: HashSet<_> = event.files.iter().map(|file| file.path.as_str()).collect();
        let repeat_of = if let Some(index) = at {
            data.snapshot.nodes[index].repeat_of.clone()
        } else if event.kind == "conflict" {
            data.snapshot
                .nodes
                .iter()
                .rev()
                .find(|node| {
                    node.event.kind == "conflict"
                        && node.event.key != event.key
                        && node
                            .event
                            .files
                            .iter()
                            .any(|old| event_paths.contains(old.path.as_str()))
                })
                .map(|node| node.event.key.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        let node = Node {
            event,
            created_at,
            round,
            repeat_of,
        };
        if matches!(
            node.event.state.as_str(),
            "running" | "conflict" | "waiting"
        ) {
            data.snapshot.summary.active_step = node.event.key.clone();
        }
        if let Some(index) = at {
            data.snapshot.nodes[index] = node.clone();
        } else {
            data.snapshot.nodes.push(node.clone());
        }
        self.publish(data, Some(node));
    }

    pub fn observer(self: &Arc<Self>) -> Observer {
        let session = self.clone();
        Arc::new(move |mut event| {
            let fetched = event.detail == "git-child"
                && event.state == "running"
                && matches!(event.kind.as_str(), "rebase" | "merge");
            let rebase = event.kind == "rebase";
            if event.detail == "git-child"
                && (fetched || (event.kind == "fetch" && event.state == "completed"))
            {
                event.source = flow::commit(&session.repo, "FETCH_HEAD");
            }
            let mut data = session
                .data
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if fetched {
                event.target = data.snapshot.summary.before.clone();
            }
            // A real command is never overwritten by a late preview.
            if event.kind == "local-saved" {
                data.snapshot.summary.pending_stash = event.stash.clone();
                session.put(
                    &mut data,
                    Event {
                        key: format!("restore-{}", event.stash),
                        kind: "local-restore".into(),
                        state: "planned".into(),
                        stash: event.stash.clone(),
                        source: event.source.clone(),
                        base: event.base.clone(),
                        files: event.files.clone(),
                        detail: "restore-after-integration".into(),
                        ..Default::default()
                    },
                );
            }
            if event.kind == "local-restore" {
                if event.files.is_empty() {
                    event.files = data
                        .snapshot
                        .nodes
                        .iter()
                        .find(|node| {
                            node.event.kind == "local-saved" && node.event.stash == event.stash
                        })
                        .map(|node| node.event.files.clone())
                        .unwrap_or_default();
                }
                if matches!(event.state.as_str(), "completed" | "conflict") {
                    data.snapshot.summary.pending_stash.clear();
                }
                if event.state == "failed" {
                    data.failed = true;
                }
            }
            session.put(&mut data, event);
            drop(data);
            if fetched {
                session.preview("FETCH_HEAD".into(), rebase, Vec::new());
            }
        })
    }

    pub fn finish(&self, result: Result<(), String>, action: &str) -> Summary {
        let stamp = checkpoint_stamp(&self.repo);
        let checkpoint = flow::checkpoint(&self.repo);
        let mut data = self.data.lock().unwrap_or_else(|error| error.into_inner());
        data.checkpoint_stamp = stamp;
        data.checked_at = Some(Instant::now());
        data.writing = false;
        if action == "abort" && result.is_ok() {
            data.aborted = true;
        }
        let paused = checkpoint
            .as_ref()
            .is_ok_and(|point| point.progress.is_some() || !point.files.is_empty());
        let request_key = data.request_key.clone();
        if let Some(node) = data
            .snapshot
            .nodes
            .iter()
            .find(|node| node.event.key == request_key)
            .cloned()
        {
            let mut event = node.event;
            event.state = if paused {
                "waiting"
            } else if result.is_ok() {
                "completed"
            } else {
                "failed"
            }
            .into();
            self.put(&mut data, event);
        }
        if let Err(error) = &result {
            if !paused {
                data.failed = true;
            }
            let key = format!("failure-{}", data.snapshot.summary.sequence);
            self.put(
                &mut data,
                Event {
                    key,
                    kind: "error".into(),
                    state: if paused { "paused" } else { "failed" }.into(),
                    detail: flow::redact(error),
                    ..Default::default()
                },
            );
        }
        match checkpoint {
            Ok(checkpoint) => self.reconcile_locked(&mut data, checkpoint, action),
            Err(error) => {
                data.snapshot.summary.state = "failed".into();
                data.snapshot.summary.recording_error = error.to_string();
                self.publish(&mut data, None);
            }
        }
        data.snapshot.summary.clone()
    }

    fn reconcile_locked(&self, data: &mut Data, checkpoint: Checkpoint, action: &str) {
        data.last_checkpoint = Some(checkpoint.clone());
        data.snapshot.summary.after = checkpoint.head.clone();
        data.snapshot.summary.progress = checkpoint.progress;
        if !checkpoint.branch.is_empty() {
            data.snapshot.summary.branch = checkpoint.branch.clone();
        }
        // A failed restore retains its stash identity even after Git's pending marker clears.
        if !checkpoint.pending_stash.is_empty() {
            data.snapshot.summary.pending_stash = checkpoint.pending_stash.clone();
        }
        let paths: HashSet<_> = checkpoint
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        let existing = data
            .snapshot
            .nodes
            .iter()
            .position(|node| node.event.key == data.round_key);
        if let Some(index) = existing {
            let mut event = data.snapshot.nodes[index].event.clone();
            let mut changed = false;
            let current_source = checkpoint.replay.as_ref().or(checkpoint.incoming.as_ref());
            let same_replay = if checkpoint.progress.is_some() {
                event.source.as_ref().map(|commit| &commit.id)
                    == current_source.map(|commit| &commit.id)
            } else {
                // Stash apply has no sequencer HEAD. Stage identities preserve this round
                // while files are resolved one at a time.
                let old_files: HashMap<_, _> = event.files.iter().map(|file| (file.path.as_str(), file)).collect();
                checkpoint.files.iter().all(|file| old_files.get(file.path.as_str()).is_some_and(|old| {
                    old.base == file.base && old.ours == file.ours && old.theirs == file.theirs
                }))
            };
            for file in &mut event.files {
                let resolved = !same_replay || !paths.contains(file.path.as_str());
                if resolved != file.resolved {
                    file.resolved = resolved;
                    changed = true;
                }
            }
            if paths.is_empty() || !same_replay {
                let next = if paths.is_empty() && checkpoint.progress.is_some() && same_replay {
                    "waiting"
                } else if action == "skip" {
                    "skipped"
                } else if action == "abort" {
                    "aborted"
                } else {
                    "completed"
                };
                if event.state != next {
                    event.state = next.into();
                    changed = true;
                }
            }
            if changed {
                self.put(data, event);
            }
        }
        let current_exists = data
            .snapshot
            .nodes
            .iter()
            .any(|node| node.event.key == data.round_key && node.event.state == "conflict");
        if !checkpoint.files.is_empty() && !current_exists {
            data.round += 1;
            data.round_key = format!("conflict-{}-{}", data.round, data.snapshot.summary.sequence);
            let restore = data.snapshot.nodes.iter().rev().find(|node| {
                (node.event.kind == "local-restore" && node.event.state == "conflict")
                    || matches!(
                        node.event.kind.as_str(),
                        "stashApply" | "stashPop" | "stashBranch"
                    )
            });
            let stash_source = restore.and_then(|node| node.event.source.clone());
            let stash = restore
                .map(|node| node.event.stash.clone())
                .unwrap_or_default();
            let source = checkpoint
                .replay
                .clone()
                .or(checkpoint.incoming.clone())
                .or(stash_source);
            self.put(
                data,
                Event {
                    key: data.round_key.clone(),
                    kind: "conflict".into(),
                    state: "conflict".into(),
                    files: checkpoint.files.clone(),
                    source,
                    target: checkpoint.head.clone(),
                    base: checkpoint
                        .base
                        .clone()
                        .or_else(|| restore.and_then(|node| node.event.base.clone())),
                    stash,
                    detail: if checkpoint.progress.is_none() {
                        "local-vs-result"
                    } else if checkpoint.progress == Some(InProgress::Rebase) {
                        "replay-vs-accumulated"
                    } else if checkpoint.progress == Some(InProgress::Revert) {
                        "revert-vs-current"
                    } else {
                        "source-vs-target"
                    }
                    .into(),
                    ..Default::default()
                },
            );
            open(&self.repo, &data.snapshot.summary.id);
        }
        // Saved original commit ids from sequencer checkpoints remain distinct across rounds.
        for original in &checkpoint.done {
            if let Some(node) = data
                .snapshot
                .nodes
                .iter()
                .find(|node| node.event.key == format!("replay-{original}"))
                .cloned()
            {
                let mut event = node.event;
                event.state = if checkpoint
                    .replay
                    .as_ref()
                    .is_some_and(|commit| &commit.id == original)
                    && !checkpoint.files.is_empty()
                {
                    "conflict"
                } else {
                    "completed"
                }
                .into();
                if event.state
                    != data
                        .snapshot
                        .nodes
                        .iter()
                        .find(|node| node.event.key == event.key)
                        .unwrap()
                        .event
                        .state
                {
                    self.put(data, event);
                }
            }
        }
        if !checkpoint.files.is_empty() {
            data.failed = false; // Git's pause explains the unsuccessful command.
            data.snapshot.summary.state = "conflict".into();
            data.snapshot.summary.active_step = data.round_key.clone();
        } else if checkpoint.progress.is_some() || !checkpoint.pending_stash.is_empty() {
            data.snapshot.summary.state = "waiting".into();
            if let Some(node) = data
                .snapshot
                .nodes
                .iter()
                .find(|node| node.event.key == data.round_key)
            {
                data.snapshot.summary.active_step = node.event.key.clone();
            }
        } else if data.writing {
            data.snapshot.summary.state = "running".into();
        } else if data.aborted {
            data.snapshot.summary.state = "aborted".into();
            data.snapshot.summary.ended_at.get_or_insert_with(flow::now);
        } else if data.failed {
            data.snapshot.summary.state = "failed".into();
            data.snapshot.summary.ended_at.get_or_insert_with(flow::now);
        } else {
            data.snapshot.summary.state = "completed".into();
            data.snapshot.summary.ended_at.get_or_insert_with(flow::now);
            let remaining: Vec<_> = data
                .snapshot
                .nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.event.state.as_str(),
                        "planned" | "predicted" | "conditional" | "waiting" | "conflict"
                    )
                })
                .map(|node| node.event.clone())
                .collect();
            for mut event in remaining {
                event.state = if event.kind == "replay" {
                    "processed"
                } else if event.kind == "conflict" || event.kind == "local-restore" {
                    "completed"
                } else {
                    "superseded"
                }
                .into();
                if event.kind == "local-restore" {
                    event.detail = "stash-kept".into();
                }
                event.detail = if event.kind == "replay" {
                    "git-processed-sequence".into()
                } else {
                    event.detail
                };
                self.put(data, event);
            }
            data.snapshot.summary.pending_stash.clear();
            self.put(
                data,
                Event {
                    key: "finished".into(),
                    kind: "finished".into(),
                    state: "completed".into(),
                    target: checkpoint.head,
                    ..Default::default()
                },
            );
            data.snapshot.summary.active_step = "finished".into();
        }
        data.snapshot.summary.can_auto_close = data.snapshot.summary.state == "completed"
            && data.snapshot.summary.pending_stash.is_empty();
        self.publish(data, None);
    }

    pub fn refresh(&self) {
        let _reader = self.refresh_lock.lock().unwrap_or_else(|error| error.into_inner());
        let generation = {
            let data = self.data.lock().unwrap_or_else(|error| error.into_inner());
            (!data.writing
                && matches!(
                    data.snapshot.summary.state.as_str(),
                    "conflict" | "waiting" | "running"
                )).then_some(data.generation)
        };
        let Some(generation) = generation else { return };
        let stamp = checkpoint_stamp(&self.repo);
        {
            let data = self.data.lock().unwrap_or_else(|error| error.into_inner());
            if stamp.is_some() && data.checkpoint_stamp == stamp
                && data.checked_at.is_some_and(|checked| checked.elapsed() < Duration::from_secs(10)) {
                return;
            }
        }
        if let Ok(checkpoint) = flow::checkpoint(&self.repo) {
            let mut data = self.data.lock().unwrap_or_else(|error| error.into_inner());
            if !data.writing
                && data.generation == generation
                && matches!(
                    data.snapshot.summary.state.as_str(),
                    "conflict" | "waiting" | "running"
                )
            {
                data.checkpoint_stamp = stamp;
                data.checked_at = Some(Instant::now());
                if data.last_checkpoint.as_ref() != Some(&checkpoint) {
                    self.reconcile_locked(&mut data, checkpoint, "refresh");
                }
            }
        }
    }

    fn preview(
        self: &Arc<Self>,
        onto: String,
        rebase: bool,
        instructions: Vec<awegit_git::RebaseStep>,
    ) {
        let generation = self.data.lock().unwrap_or_else(|error| error.into_inner()).generation;
        let lane = preview_lane(&self.repo);
        lane.submit(PreviewJob { session: Arc::downgrade(self), generation, epoch: 0, onto, rebase, instructions, context: awegit_git::timing_context() }, |lane, job| {
            let Some(session) = job.session.upgrade() else { return };
            let cancelled = || {
                if lane.epoch.load(Ordering::Acquire) != job.epoch { return true; }
                let data = session.data.lock().unwrap_or_else(|error| error.into_inner());
                data.generation != job.generation || data.snapshot.summary.ended_at.is_some()
            };
            if cancelled() { return; }
            let context = job.context.map(|mut context| { context.phase = "preview".into(); context });
            let _timing = awegit_git::enter_timing(context);
            session.preview_now(job.onto, job.rebase, job.instructions, job.generation, &cancelled);
        });
    }

    fn preview_now(
        self: &Arc<Self>,
        onto: String,
        rebase: bool,
        instructions: Vec<awegit_git::RebaseStep>,
        generation: u64,
        cancelled: &dyn Fn() -> bool,
    ) {
        let Some(onto) = flow::commit(&self.repo, &onto) else {
            return;
        };
        let head = self
            .data
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .snapshot
            .summary
            .before
            .clone();
        let Some(head) = head else { return };
        let plan = if !rebase {
            vec![onto.clone()]
        } else if instructions.is_empty() {
            flow::replay_commits(&self.repo, &onto.id, &head.id).unwrap_or_default()
        } else {
            instructions
                .iter()
                .filter_map(|step| flow::commit(&self.repo, &step.rev))
                .collect()
        };
        if cancelled() { return; }
        {
            let mut data = self.data.lock().unwrap_or_else(|error| error.into_inner());
            if data.generation != generation || data.snapshot.summary.ended_at.is_some() { return; }
            for commit in plan {
                let key = format!("replay-{}", commit.id);
                if data.snapshot.nodes.iter().any(|node| node.event.key == key) {
                    continue;
                }
                self.put(
                    &mut data,
                    Event {
                        key,
                        kind: if rebase { "replay" } else { "integration" }.into(),
                        state: "planned".into(),
                        source: Some(commit),
                        target: Some(onto.clone()),
                        detail: "preview-conditional".into(),
                        ..Default::default()
                    },
                );
            }
        }
        let session = self;
        {
            let preview =
                flow::preview_at_cancellable(&session.repo, &onto.id, &head.id, rebase, &instructions, cancelled);
            if cancelled() { return; }
            let mut data = session
                .data
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if data.generation != generation || data.snapshot.summary.ended_at.is_some() {
                return;
            }
            match preview {
                Ok(preview) => {
                    for step in preview.steps {
                        let key = format!("replay-{}", step.commit.id);
                        if data.snapshot.nodes.iter().any(|node| {
                            node.event.key == key
                                && matches!(node.event.state.as_str(), "completed" | "conflict")
                        }) {
                            continue;
                        }
                        session.put(
                            &mut data,
                            Event {
                                key,
                                kind: if rebase { "replay" } else { "integration" }.into(),
                                state: if step.state == "dropped" {
                                    "skipped"
                                } else if matches!(step.state.as_str(), "conditional" | "unknown") {
                                    "conditional"
                                } else {
                                    "predicted"
                                }
                                .into(),
                                source: Some(step.commit),
                                target: preview.source.clone(),
                                detail: format!("preview-{}", step.state),
                                files: step
                                    .files
                                    .into_iter()
                                    .map(|path| File {
                                        path,
                                        ..Default::default()
                                    })
                                    .collect(),
                                ..Default::default()
                            },
                        );
                    }
                    if !preview.detail.is_empty() {
                        session.put(
                            &mut data,
                            Event {
                                key: "preview-note".into(),
                                kind: "preview".into(),
                                state: "conditional".into(),
                                detail: preview.detail,
                                ..Default::default()
                            },
                        );
                    }
                }
                Err(_) => session.put(
                    &mut data,
                    Event {
                        key: "preview-note".into(),
                        kind: "preview".into(),
                        state: "conditional".into(),
                        detail: "preview-unavailable".into(),
                        ..Default::default()
                    },
                ),
            }
            if !data.writing {
                if let Some(checkpoint) = data.last_checkpoint.clone() {
                    session.reconcile_locked(&mut data, checkpoint, "preview");
                }
            }
        }
    }
}

fn tracked(kind: &str) -> bool {
    matches!(
        kind,
        "fetch"
            | "stash"
            | "stashPaths"
            | "stashDrop"
            | "reword"
            | "gitFlowStart"
            | "pull"
            | "pullRef"
            | "push"
            | "pushRef"
            | "merge"
            | "rebase"
            | "rebaseInteractive"
            | "cherryPick"
            | "revert"
            | "reset"
            | "checkout"
            | "checkoutRemote"
            | "stashApply"
            | "stashPop"
            | "stashBranch"
            | "applyPatch"
            | "squash"
            | "gitFlowFinish"
    )
}

fn active(repo: &Path) -> Option<Arc<Session>> {
    let directory = directory(repo).ok()?;
    if let Some(session) = sessions()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&directory)
        .cloned()
    {
        return Some(session);
    }
    // Status refresh and a newly opened flow window can request recovery together.
    // Only one reader may repair and resume a journal.
    let _recovery = RECOVERY.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(session) = sessions()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&directory)
        .cloned()
    {
        return Some(session);
    }
    let summary = history(repo, 0, 1).ok()?.into_iter().next()?;
    if !matches!(summary.state.as_str(), "conflict" | "waiting" | "running") {
        return None;
    }
    let snapshot = read_snapshot(&directory, &summary.id).ok()?;
    repair_tail(&directory, &summary.id);
    let interrupted = snapshot.summary.state == "running";
    let round = snapshot
        .nodes
        .iter()
        .map(|node| node.round)
        .max()
        .unwrap_or(0);
    let round_key = snapshot
        .nodes
        .iter()
        .rev()
        .find(|node| node.event.kind == "conflict")
        .map(|node| node.event.key.clone())
        .unwrap_or_default();
    let session = Arc::new(Session {
        repo: repo.to_path_buf(),
        directory: directory.clone(),
        refresh_lock: Mutex::new(()),
        data: Mutex::new(Data {
            snapshot,
            round,
            round_key,
            generation: 0,
            writing: false,
            aborted: false,
            failed: interrupted,
            request_key: String::new(),
            last_checkpoint: None,
            checkpoint_stamp: None,
            checked_at: None,
        }),
    });
    if interrupted {
        let mut data = session
            .data
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let unfinished: Vec<_> = data
            .snapshot
            .nodes
            .iter()
            .filter(|node| node.event.state == "running")
            .map(|node| node.event.clone())
            .collect();
        for mut event in unfinished {
            event.state = "interrupted".into();
            event.detail = "recording-interrupted".into();
            session.put(&mut data, event);
        }
    }
    sessions()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(directory, session.clone());
    Some(session)
}

pub fn begin(repo: &Path, request: &Mutation) -> Option<Arc<Session>> {
    let kind = super::action_label(request);
    begin_kind(repo, kind, Some(request))
}

pub fn begin_kind(repo: &Path, kind: &str, request: Option<&Mutation>) -> Option<Arc<Session>> {
    let existing = active(repo);
    let continuation = matches!(
        kind,
        "continue"
            | "skip"
            | "abort"
            | "commit"
            | "resolve"
            | "resolvePaths"
            | "stage"
            | "stagePaths"
            | "stageHunk"
            | "stageLine"
            | "stageLines"
            | "stashDrop"
    );
    let reusable = existing.as_ref().is_some_and(|session| {
        let data = session
            .data
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        matches!(
            data.snapshot.summary.state.as_str(),
            "conflict" | "waiting" | "running"
        )
    });
    if !tracked(kind) && !(continuation && reusable) {
        return None;
    }
    let checkpoint = flow::checkpoint(repo).ok()?;
    let session = if reusable {
        existing.unwrap()
    } else {
        let directory = directory(repo).ok()?;
        let previous = history(repo, 0, 1)
            .ok()
            .and_then(|items| items.first().map(|summary| summary.id.clone()))
            .unwrap_or_default();
        let id = format!(
            "{}-{}-{}",
            flow::now(),
            std::process::id(),
            IDS.fetch_add(1, Ordering::Relaxed)
        );
        let summary = Summary {
            id,
            repo: repo.display().to_string(),
            kind: kind.into(),
            state: "running".into(),
            sequence: 0,
            started_at: flow::now(),
            ended_at: None,
            active_step: String::new(),
            can_auto_close: false,
            branch: checkpoint.branch.clone(),
            before: checkpoint.head.clone(),
            after: checkpoint.head.clone(),
            progress: checkpoint.progress,
            pending_stash: checkpoint.pending_stash.clone(),
            recording_error: String::new(),
            previous,
        };
        let session = Arc::new(Session {
            repo: repo.to_path_buf(),
            directory: directory.clone(),
            refresh_lock: Mutex::new(()),
            data: Mutex::new(Data {
                snapshot: Snapshot {
                    summary,
                    nodes: Vec::new(),
                },
                round: 0,
                round_key: String::new(),
                generation: 0,
                writing: false,
                aborted: false,
                failed: false,
                request_key: String::new(),
                last_checkpoint: None,
                checkpoint_stamp: None,
                checked_at: None,
            }),
        });
        sessions()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(directory, session.clone());
        session
    };
    {
        let mut data = session
            .data
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        data.generation += 1;
        data.writing = true;
        if !matches!(kind, "stage" | "resolve" | "resolvePaths") {
            data.failed = false;
        }
        data.snapshot.summary.ended_at = None;
        data.snapshot.summary.can_auto_close = false;
        data.snapshot.summary.state = "running".into();
        let key = format!("action-{}", data.snapshot.summary.sequence);
        data.request_key = key.clone();
        let source = request.and_then(|request| match request {
            Mutation::StashApply { name }
            | Mutation::StashPop { name }
            | Mutation::StashBranch { name, .. } => {
                flow::commit(repo, if name.is_empty() { "refs/stash" } else { name })
            }
            Mutation::CherryPick { rev, .. }
            | Mutation::Revert { rev }
            | Mutation::Reset { rev, .. } => flow::commit(repo, rev),
            Mutation::Merge { name, .. } => flow::commit(repo, name),
            Mutation::Rebase { onto, .. } | Mutation::RebaseInteractive { onto, .. } => {
                flow::commit(repo, onto)
            }
            _ => None,
        });
        let stash = if kind.starts_with("stash") {
            source
                .as_ref()
                .map(|commit| commit.id.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        let base = if !stash.is_empty() {
            flow::commit(repo, &format!("{stash}^1"))
        } else {
            None
        };
        session.put(
            &mut data,
            Event {
                key,
                kind: kind.into(),
                state: "running".into(),
                detail: match kind {
                    "reset" => "reset-baseline",
                    "revert" => "revert-new-commit",
                    _ => "requested",
                }
                .into(),
                source,
                stash,
                base,
                target: checkpoint.head,
                ..Default::default()
            },
        );
    }
    let preview = request.and_then(|request| match request {
        Mutation::Merge { name, .. } => Some((name.clone(), false, Vec::new())),
        Mutation::Rebase { onto, .. } => Some((onto.clone(), true, Vec::new())),
        Mutation::RebaseInteractive { onto, steps, .. } => {
            Some((onto.clone(), true, steps.clone()))
        }
        _ => None,
    });
    if let Some((onto, rebase, steps)) = preview {
        session.preview(onto, rebase, steps);
    }
    Some(session)
}

pub fn reconcile(repo: &Path) {
    if let Some(session) = active(repo) {
        session.refresh();
    }
}

pub fn resume(repo: &Path) {
    let _turn = super::repo_turn(repo);
    if let Some(session) = active(repo) {
        if let Ok(point) = flow::checkpoint(repo) {
            if point.progress.is_none() && point.files.is_empty() && !point.pending_stash.is_empty()
            {
                {
                    let mut data = session
                        .data
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    data.writing = true;
                    data.failed = false;
                }
                let result = flow::observe(session.observer(), || flow::restore_pending(repo))
                    .map_err(|error| error.to_string());
                session.finish(result, "restore");
            }
        }
        session.refresh();
        let data = session
            .data
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if data.snapshot.summary.state == "conflict" {
            open(repo, &data.snapshot.summary.id);
        }
    }
}

fn repair_tail(directory: &Path, id: &str) {
    let path = directory.join(format!("{id}.jsonl"));
    let Ok(file) = fs::File::open(&path) else {
        return;
    };
    let mut reader = BufReader::new(file);
    let mut valid = 0;
    loop {
        let mut line = Vec::new();
        let Ok(length) = reader.read_until(b'\n', &mut line) else {
            return;
        };
        if length == 0 {
            break;
        }
        if line.last() != Some(&b'\n') || serde_json::from_slice::<Delta>(&line).is_err() {
            break;
        }
        valid += length as u64;
    }
    if let Ok(file) = OpenOptions::new().write(true).open(path) {
        let _ = file.set_len(valid);
    }
}

fn read_snapshot(directory: &Path, id: &str) -> Result<Snapshot, String> {
    if id.is_empty() || !id.chars().all(|ch| ch.is_ascii_digit() || ch == '-') {
        return Err("invalid flow id".into());
    }
    let file =
        fs::File::open(directory.join(format!("{id}.jsonl"))).map_err(|error| error.to_string())?;
    let mut summary = None;
    let mut nodes: Vec<Node> = Vec::new();
    let mut positions = HashMap::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|error| error.to_string())?;
        // An interrupted final append does not invalidate the preceding journal.
        let Ok(delta) = serde_json::from_str::<Delta>(&line) else {
            break;
        };
        if let Some(node) = delta.node {
            if let Some(index) = positions.get(&node.event.key).copied() {
                nodes[index] = node;
            } else {
                positions.insert(node.event.key.clone(), nodes.len());
                nodes.push(node);
            }
        }
        summary = Some(delta.summary);
    }
    Ok(Snapshot {
        summary: summary.ok_or("empty flow journal")?,
        nodes,
    })
}

pub fn snapshot(repo: &Path, id: Option<String>) -> Result<Option<Snapshot>, String> {
    let directory = directory(repo)?;
    if let Some(session) = active(repo) {
        session.refresh();
        let data = session
            .data
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if id
            .as_ref()
            .is_none_or(|id| id.is_empty() || *id == data.snapshot.summary.id)
        {
            return Ok(Some(data.snapshot.clone()));
        }
    }
    let id = id.filter(|id| !id.is_empty()).or_else(|| {
        history(repo, 0, 1)
            .ok()?
            .first()
            .map(|summary| summary.id.clone())
    });
    id.map(|id| read_snapshot(&directory, &id)).transpose()
}

type FileStamp = Option<(u64, SystemTime)>;

fn file_stamp(path: &Path) -> FileStamp {
    fs::metadata(path).ok().and_then(|meta| Some((meta.len(), meta.modified().ok()?)))
}

#[derive(Default)]
struct HistoryIndex {
    paths: BTreeSet<PathBuf>,
    summaries: HashMap<PathBuf, (FileStamp, Summary)>,
    directory_stamp: FileStamp,
    verified_at: Option<Instant>,
    #[cfg(test)]
    reads: usize,
}

fn history_indexes() -> &'static Mutex<HashMap<PathBuf, Arc<Mutex<HistoryIndex>>>> {
    static HISTORY: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<HistoryIndex>>>>> = OnceLock::new();
    HISTORY.get_or_init(Default::default)
}

fn history_index(directory: &Path) -> Arc<Mutex<HistoryIndex>> {
    history_indexes().lock().unwrap_or_else(|error| error.into_inner())
        .entry(directory.to_path_buf()).or_default().clone()
}

pub fn history(repo: &Path, offset: usize, limit: usize) -> Result<Vec<Summary>, String> {
    let directory = directory(repo)?;
    let index = history_index(&directory);
    let mut index = index.lock().unwrap_or_else(|error| error.into_inner());
    let stamp = file_stamp(&directory);
    if index.verified_at.is_none_or(|at| at.elapsed() >= Duration::from_secs(30)) || index.directory_stamp != stamp {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.to_string()),
        };
        index.paths = entries.filter_map(Result::ok).map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json")).collect();
        let known = index.paths.clone();
        index.summaries.retain(|path, _| known.contains(path));
        index.directory_stamp = stamp;
        index.verified_at = Some(Instant::now());
    }
    let paths: Vec<_> = index.paths.iter().rev().skip(offset).take(limit.min(100)).cloned().collect();
    let mut summaries = Vec::with_capacity(paths.len());
    for path in paths {
        let stamp = file_stamp(&path);
        if let Some((cached_stamp, summary)) = index.summaries.get(&path) {
            if stamp.is_some() && *cached_stamp == stamp {
                summaries.push(summary.clone());
                continue;
            }
        }
        #[cfg(test)] { index.reads += 1; }
        if let Some(summary) = fs::read(&path).ok().and_then(|bytes| serde_json::from_slice::<Summary>(&bytes).ok()) {
            summaries.push(summary.clone());
            index.summaries.insert(path, (stamp, summary));
        }
    }
    Ok(summaries)
}

fn prune(directory: &Path, days: u32, now: u64) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let cutoff = now.saturating_sub(u64::from(days.clamp(1, 90)) * 86_400_000);
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let Some(summary) = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Summary>(&bytes).ok())
        else {
            continue;
        };
        if summary.ended_at.is_some_and(|ended| ended < cutoff) {
            let _ = fs::remove_file(path.with_extension("jsonl"));
            let _ = fs::remove_file(path);
        }
    }
    history_indexes().lock().unwrap_or_else(|error| error.into_inner()).remove(directory);
}

pub fn cleanup_known() {
    let values = super::settings::load().unwrap_or_default();
    let mut repos: HashSet<PathBuf> = values.recent.iter().map(PathBuf::from).collect();
    repos.extend(
        sessions()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .map(|session| session.repo.clone()),
    );
    for repo in repos {
        if let Ok(directory) = directory(&repo) {
            prune(&directory, values.flow_history_days, flow::now());
        }
    }
    let cutoff =
        flow::now().saturating_sub(u64::from(values.flow_history_days.clamp(1, 90)) * 86_400_000);
    sessions()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .retain(|_, session| {
            let data = session
                .data
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            data.writing
                || data
                    .snapshot
                    .summary
                    .ended_at
                    .is_none_or(|ended| ended >= cutoff)
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    pub(super) static OPENED: Mutex<Vec<(PathBuf, String)>> = Mutex::new(Vec::new());

    thread_local! {
        static GIT_CALLS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
        static GIT_TIMES: std::cell::RefCell<Option<HashMap<String, Vec<f64>>>> = const { std::cell::RefCell::new(None) };
    }

    fn count_git_calls<T>(action: impl FnOnce() -> T) -> (T, usize) {
        awegit_git::set_trace(|args, elapsed, _, _| {
            GIT_CALLS.with(|slot| {
                if let Some(count) = slot.get() {
                    slot.set(Some(count + 1));
                    GIT_TIMES.with(|times| {
                        if let Some(times) = times.borrow_mut().as_mut() {
                            let mut name = args.first().copied().unwrap_or("unknown").to_string();
                            if name == "stash" {
                                if let Some(verb) = args.get(1) { name.push(' '); name.push_str(verb); }
                            }
                            times.entry(name).or_default().push(elapsed.as_secs_f64() * 1000.0);
                        }
                    });
                }
            });
        });
        GIT_CALLS.with(|slot| slot.set(Some(0)));
        let result = action();
        let count = GIT_CALLS.with(|slot| slot.replace(None).unwrap());
        (result, count)
    }

    struct Repo(PathBuf);
    impl Repo {
        fn new() -> Self {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join(".lab/flow-tests");
            let path = root.join(format!(
                "{}-{}",
                flow::now(),
                IDS.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("global-config"), "").unwrap();
            let repo = Self(path);
            repo.git(&["init", "-b", "main"]);
            for (key, value) in [
                ("user.name", "Flow Test"),
                ("user.email", "flow@test.local"),
                ("commit.gpgsign", "false"),
                ("core.autocrlf", "false"),
                ("core.hooksPath", ".git/no-hooks"),
            ] {
                repo.git(&["config", key, value]);
            }
            repo.save("note", "base\n", "base");
            repo
        }
        fn git(&self, args: &[&str]) -> String {
            let output = Command::new("git")
                .current_dir(&self.0)
                .args(args)
                .env("GIT_CONFIG_GLOBAL", self.0.join("global-config"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "Flow Test")
                .env("GIT_AUTHOR_EMAIL", "flow@test.local")
                .env("GIT_COMMITTER_NAME", "Flow Test")
                .env("GIT_COMMITTER_EMAIL", "flow@test.local")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().into()
        }
        fn save(&self, file: &str, body: &str, message: &str) {
            fs::write(self.0.join(file), body).unwrap();
            self.git(&["add", "--", file]);
            self.git(&["commit", "-m", message]);
        }
        fn run(&self, request: Mutation) -> Summary {
            let kind = super::super::action_label(&request);
            let session = begin(&self.0, &request).unwrap();
            let result = flow::observe(session.observer(), || {
                awegit_git::perform(&self.0, request)?;
                if matches!(kind, "stagePaths" | "resolve" | "resolvePaths") {
                    flow::restore_pending(&self.0)?;
                }
                Ok::<_, awegit_git::Error>(())
            });
            session.finish(result.map_err(|error| error.to_string()), kind)
        }
        fn resolve(&self, body: &str) -> Summary {
            fs::write(self.0.join("note"), body).unwrap();
            self.run(Mutation::StagePaths {
                files: vec!["note".into()],
                unstage: false,
            })
        }
        fn snapshot(&self) -> Snapshot {
            snapshot(&self.0, None).unwrap().unwrap()
        }
        fn opened(&self) -> Vec<String> {
            OPENED
                .lock()
                .unwrap()
                .iter()
                .filter(|(repo, _)| repo == &self.0)
                .map(|(_, id)| id.clone())
                .collect()
        }
    }
    impl Drop for Repo {
        fn drop(&mut self) {
            if let Ok(directory) = directory(&self.0) {
                sessions().lock().unwrap().remove(&directory);
            }
            OPENED.lock().unwrap().retain(|(repo, _)| repo != &self.0);
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn merge_conflict() -> Repo {
        let repo = Repo::new();
        repo.git(&["branch", "incoming"]);
        repo.save("note", "local\n", "local");
        repo.git(&["checkout", "incoming"]);
        repo.save("note", "incoming\n", "incoming");
        repo.git(&["checkout", "main"]);
        assert_eq!(repo.run(Mutation::Merge {
            name: "incoming".into(), squash: false, no_ff: false, autostash: false,
            summary: String::new(), description: String::new(),
        }).state, "conflict");
        repo
    }

    #[test]
    fn commit_metadata_does_not_walk_merge_trees() {
        let repo = Repo::new();
        let parent = repo.git(&["rev-parse", "HEAD"]);
        repo.save("note", "next\n", "next");
        let head = repo.git(&["rev-parse", "HEAD"]);
        // A partial clone can have a commit before its tree is available. Metadata
        // must not calculate a combined merge diff or fetch the missing tree.
        let body = format!("tree {}\nparent {parent}\nparent {head}\nauthor Metadata Author <metadata@example.invalid> 1 +0000\ncommitter Metadata Author <metadata@example.invalid> 1 +0000\n\nmetadata only\n", "f".repeat(40));
        fs::write(repo.0.join("merge-object"), body).unwrap();
        let oid = repo.git(&["hash-object", "-w", "-t", "commit", "merge-object"]);
        let commit = flow::commit(&repo.0, &oid).expect("metadata must not require the merge tree");
        assert_eq!(commit.id, oid);
        assert_eq!(commit.summary, "metadata only");
        assert_eq!(commit.author, "Metadata Author");
        let detail = awegit_git::commit_detail(&repo.0, &oid).unwrap();
        assert_eq!(detail.id, oid);
        assert_eq!(detail.summary, "metadata only");
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), head);
    }

    #[test]
    fn unchanged_conflict_polling_skips_git_but_external_resolution_is_seen() {
        let repo = merge_conflict();
        let (_, calls) = count_git_calls(|| { for _ in 0..20 { assert_eq!(repo.snapshot().summary.state, "conflict"); } });
        assert_eq!(calls, 0, "unchanged conflict polling must reuse the checkpoint");
        fs::write(repo.0.join("note"), "external resolution\n").unwrap();
        repo.git(&["add", "note"]);
        let (snapshot, calls) = count_git_calls(|| repo.snapshot());
        assert!(calls > 0, "a changed index must invalidate the checkpoint even without a watcher");
        assert_eq!(snapshot.summary.state, "waiting");
        assert!(!snapshot.summary.can_auto_close);
        let (_, calls) = count_git_calls(|| repo.snapshot());
        assert_eq!(calls, 0);
        let session = active(&repo.0).unwrap();
        session.data.lock().unwrap().checked_at = Some(Instant::now() - Duration::from_secs(11));
        let (_, calls) = count_git_calls(|| repo.snapshot());
        assert!(calls > 0, "a fallback must recover missed metadata events");
    }

    #[test]
    fn history_pages_reuse_summaries_and_detect_external_replacement() {
        let repo = Repo::new();
        let summary = repo.run(Mutation::Reset { rev: "HEAD".into(), mode: awegit_git::ResetMode::Soft });
        let directory = directory(&repo.0).unwrap();
        history_indexes().lock().unwrap().remove(&directory);
        assert_eq!(history(&repo.0, 0, 1).unwrap()[0].id, summary.id);
        let cache = history_index(&directory);
        let reads = cache.lock().unwrap().reads;
        assert_eq!(history(&repo.0, 0, 1).unwrap()[0].id, summary.id);
        assert_eq!(cache.lock().unwrap().reads, reads);
        let mut replacement = summary.clone();
        replacement.kind = "external-replacement".into();
        fs::write(directory.join(format!("{}.json", summary.id)), serde_json::to_vec(&replacement).unwrap()).unwrap();
        assert_eq!(history(&repo.0, 0, 1).unwrap()[0].kind, "external-replacement");
        assert_eq!(cache.lock().unwrap().reads, reads + 1);
        let mut older = replacement.clone();
        older.id = "100-1-2".into();
        fs::write(directory.join("100-1-2.json"), serde_json::to_vec(&older).unwrap()).unwrap();
        assert_eq!(history(&repo.0, 1, 1).unwrap()[0].id, older.id);
        assert!(history(&repo.0, 2, 1).unwrap().is_empty());
    }

    #[test]
    fn checkpoints_group_a_thousand_conflicts_and_preserve_each_stage() {
        use std::process::Stdio;
        let repo = Repo::new();
        let oid = repo.git(&["rev-parse", "HEAD:note"]);
        let mut child = Command::new("git").current_dir(&repo.0)
            .args(["update-index", "--index-info"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let mut input = child.stdin.take().unwrap();
        for index in 0..1000 { for stage in 1..=3 { writeln!(input, "100644 {oid} {stage}\tconflict-{index:04}").unwrap(); } }
        drop(input);
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let point = flow::checkpoint(&repo.0).unwrap();
        assert_eq!(point.files.len(), 1000);
        for (index, file) in point.files.iter().enumerate() {
            assert_eq!(file.path, format!("conflict-{index:04}"));
            assert_eq!((&file.base, &file.ours, &file.theirs), (&oid, &oid, &oid));
        }
    }

    #[test]
    fn cancelled_preview_never_initializes_a_sandbox_or_touches_the_index() {
        let repo = Repo::new();
        let index = fs::read(repo.0.join(".git/index")).unwrap();
        let (preview, calls) = count_git_calls(|| flow::preview_at_cancellable(&repo.0, "HEAD", "HEAD", false, &[], &|| true).unwrap());
        assert_eq!(preview.detail, "preview-cancelled");
        assert_eq!(calls, 0);
        assert_eq!(fs::read(repo.0.join(".git/index")).unwrap(), index);
    }

    #[test]
    fn preview_lane_runs_one_job_and_keeps_only_the_latest_pending_job() {
        let lane = Arc::new(PreviewLane::default());
        let started = Arc::new(std::sync::Barrier::new(2));
        let released = Arc::new(std::sync::Barrier::new(2));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (done, completed) = std::sync::mpsc::channel();
        let run = {
            let (started, released, seen) = (started.clone(), released.clone(), seen.clone());
            move |_: &PreviewLane, job: PreviewJob| {
                seen.lock().unwrap().push(job.generation);
                if job.generation == 0 { started.wait(); released.wait(); }
                if job.generation == 100 { done.send(()).unwrap(); }
            }
        };
        let job = |generation| PreviewJob { session: Weak::new(), generation, epoch: 0, onto: "HEAD".into(), rebase: false, instructions: Vec::new(), context: None };
        lane.clone().submit(job(0), run.clone());
        started.wait();
        for generation in 1..=100 { lane.clone().submit(job(generation), run.clone()); }
        released.wait();
        completed.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(*seen.lock().unwrap(), [0, 100]);
    }

    #[test]
    fn clean_operations_and_failures_without_conflicts_do_not_open_the_observer() {
        let repo = Repo::new();
        let request = Mutation::Reset {
            rev: "HEAD".into(),
            mode: awegit_git::ResetMode::Soft,
        };
        let session = begin(&repo.0, &request).unwrap();
        assert!(
            repo.opened().is_empty(),
            "starting an operation must not open a window"
        );
        let result = flow::observe(session.observer(), || awegit_git::perform(&repo.0, request));
        let completed = session.finish(
            result.map(|_| ()).map_err(|error| error.to_string()),
            "reset",
        );
        assert_eq!(completed.state, "completed");
        resume(&repo.0);
        assert!(repo.opened().is_empty());

        let failed = repo.run(Mutation::Push {
            remote: None,
            set_upstream: false,
            tags: false,
            force_with_lease: false,
            force: false,
        });
        assert_eq!(failed.state, "failed");
        resume(&repo.0);
        assert!(
            repo.opened().is_empty(),
            "an ordinary Git failure has no conflicts to resolve"
        );
        assert_eq!(history(&repo.0, 0, 10).unwrap().len(), 2);
    }

    #[test]
    fn separated_conflict_rounds_and_final_stash_conflict_keep_one_live_workflow() {
        let repo = Repo::new();
        repo.git(&["branch", "remote"]);
        repo.save("note", "local one\n", "first local");
        let first = repo.git(&["rev-parse", "HEAD"]);
        repo.save("clean", "clean\n", "between conflicts");
        let clean = repo.git(&["rev-parse", "HEAD"]);
        repo.save("note", "local two\n", "second local");
        let second = repo.git(&["rev-parse", "HEAD"]);
        repo.git(&["checkout", "remote"]);
        repo.save("note", "remote\n", "incoming");
        repo.git(&["checkout", "main"]);
        fs::write(repo.0.join("note"), "uncommitted\n").unwrap();
        let started = repo.run(Mutation::Rebase {
            onto: "remote".into(),
            autostash: true,
        });
        assert_eq!(started.state, "conflict");
        assert_eq!(repo.opened(), vec![started.id.clone()]);
        assert!(!started.can_auto_close);
        let saved = started.pending_stash.clone();
        assert!(!saved.is_empty());
        assert_eq!(
            repo.snapshot()
                .nodes
                .iter()
                .find(|node| node.event.kind == "conflict")
                .unwrap()
                .event
                .source
                .as_ref()
                .unwrap()
                .id,
            first
        );
        let staged = repo.resolve("resolved first\n");
        assert_eq!(staged.state, "waiting");
        assert!(!staged.can_auto_close);
        resume(&repo.0);
        assert_eq!(
            repo.opened().len(),
            1,
            "resolved conflicts waiting for continue must not reopen the window"
        );
        let continued = repo.run(Mutation::Continue {
            message: String::new(),
        });
        assert_eq!(continued.id, started.id);
        assert_eq!(continued.state, "conflict");
        assert_eq!(repo.opened(), vec![started.id.clone(); 2]);
        let snapshot = repo.snapshot();
        let rounds: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.event.kind == "conflict")
            .collect();
        assert_eq!(rounds.len(), 2);
        assert_eq!(rounds[0].event.state, "completed");
        assert!(rounds[0].event.files[0].resolved);
        assert_eq!(rounds[1].event.source.as_ref().unwrap().id, second);
        assert_eq!(rounds[1].event.base.as_ref().unwrap().id, clean);
        assert_eq!(rounds[1].repeat_of, rounds[0].event.key);
        assert_eq!(snapshot.summary.active_step, rounds[1].event.key);
        assert!(snapshot
            .nodes
            .iter()
            .any(|node| node.event.key == format!("replay-{clean}")
                && node.event.state == "completed"));
        repo.resolve("resolved second\n");
        let restoring = repo.run(Mutation::Continue {
            message: String::new(),
        });
        assert!(restoring.progress.is_none());
        assert_eq!(restoring.state, "conflict");
        assert_eq!(repo.opened(), vec![started.id.clone(); 3]);
        assert!(!restoring.can_auto_close);
        let snapshot = repo.snapshot();
        let rounds: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.event.kind == "conflict")
            .collect();
        assert_eq!(rounds.len(), 3);
        assert_eq!(rounds[2].event.stash, saved);
        assert_eq!(rounds[2].event.source.as_ref().unwrap().id, saved);
        assert_eq!(snapshot.summary.active_step, rounds[2].event.key);
        let sequence = snapshot.summary.sequence;
        assert_eq!(
            repo.snapshot().summary.sequence,
            sequence,
            "unchanged polling must not append journal records"
        );
        let finished = repo.resolve("restored local work\n");
        assert_eq!(finished.state, "completed");
        assert!(finished.can_auto_close);
        assert!(finished.pending_stash.is_empty());
        resume(&repo.0);
        assert_eq!(repo.opened().len(), 3);
        assert_eq!(
            repo.git(&["rev-parse", "refs/stash"]),
            saved,
            "conflicted stash is retained after manual resolution"
        );
        assert_eq!(
            repo.snapshot()
                .nodes
                .iter()
                .filter(|node| node.event.kind == "local-saved")
                .count(),
            1
        );
    }

    #[test]
    fn stash_round_does_not_repeat_on_each_refresh_and_journal_survives_a_torn_tail() {
        let repo = Repo::new();
        fs::write(repo.0.join("note"), "saved\n").unwrap();
        repo.git(&["stash", "push"]);
        let stash = repo.git(&["rev-parse", "refs/stash"]);
        repo.save("note", "new base\n", "advance");
        let started = repo.run(Mutation::StashPop {
            name: stash.clone(),
        });
        assert_eq!(started.state, "conflict");
        let before = repo.snapshot();
        for _ in 0..3 {
            assert_eq!(repo.snapshot().summary.sequence, before.summary.sequence);
        }
        let directory = directory(&repo.0).unwrap();
        let path = directory.join(format!("{}.jsonl", started.id));
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"summary\":")
            .unwrap();
        sessions().lock().unwrap().remove(&directory);
        let restored = repo.snapshot();
        assert_eq!(restored.summary.id, started.id);
        resume(&repo.0);
        assert_eq!(
            repo.opened(),
            vec![started.id.clone(); 2],
            "recovered unresolved conflicts still open the observer"
        );
        let finished = repo.resolve("resolved\n");
        assert!(finished.can_auto_close);
        let disk = read_snapshot(&directory, &started.id).unwrap();
        assert_eq!(disk.summary.state, "completed");
        assert_eq!(
            disk.nodes
                .iter()
                .filter(|node| node.event.kind == "conflict")
                .count(),
            1
        );
    }

    #[test]
    fn revert_conflict_records_the_original_postimage_as_its_base() {
        let repo = Repo::new();
        repo.save("note", "original change\n", "original");
        let original = repo.git(&["rev-parse", "HEAD"]);
        repo.save("note", "later change\n", "later");
        let target = repo.git(&["rev-parse", "HEAD"]);
        let started = repo.run(Mutation::Revert {
            rev: original.clone(),
        });
        assert_eq!(started.state, "conflict");
        let snapshot = repo.snapshot();
        let round = snapshot
            .nodes
            .iter()
            .find(|node| node.event.kind == "conflict")
            .unwrap();
        assert_eq!(round.event.source.as_ref().unwrap().id, original);
        assert_eq!(round.event.base.as_ref().unwrap().id, original);
        assert_eq!(round.event.target.as_ref().unwrap().id, target);
        assert_eq!(
            round.event.files[0].base,
            repo.git(&["rev-parse", &format!("{original}:note")])
        );
        repo.run(Mutation::Abort);
    }

    #[test]
    fn skip_and_abort_are_retained_as_distinct_outcomes() {
        let repo = Repo::new();
        repo.git(&["branch", "remote"]);
        repo.save("note", "local\n", "local");
        repo.git(&["checkout", "remote"]);
        repo.save("note", "remote\n", "remote");
        repo.git(&["checkout", "main"]);
        repo.run(Mutation::Rebase {
            onto: "remote".into(),
            autostash: true,
        });
        let skipped = repo.run(Mutation::Skip);
        assert!(skipped.can_auto_close);
        assert!(repo
            .snapshot()
            .nodes
            .iter()
            .any(|node| node.event.kind == "conflict" && node.event.state == "skipped"));
        repo.save("note", "local again\n", "again");
        repo.git(&["checkout", "remote"]);
        repo.save("note", "remote again\n", "remote again");
        repo.git(&["checkout", "main"]);
        repo.run(Mutation::Rebase {
            onto: "remote".into(),
            autostash: true,
        });
        let aborted = repo.run(Mutation::Abort);
        assert_eq!(aborted.state, "aborted");
        assert!(!aborted.can_auto_close);
    }

    #[test]
    fn interrupted_write_is_not_left_running_after_recovery() {
        let repo = Repo::new();
        let request = Mutation::Reset {
            rev: "HEAD".into(),
            mode: awegit_git::ResetMode::Soft,
        };
        let session = begin(&repo.0, &request).unwrap();
        let id = session.data.lock().unwrap().snapshot.summary.id.clone();
        sessions()
            .lock()
            .unwrap()
            .remove(&directory(&repo.0).unwrap());
        let recovered = repo.snapshot();
        assert_eq!(recovered.summary.id, id);
        assert_eq!(recovered.summary.state, "failed");
        assert!(!recovered.summary.can_auto_close);
        assert!(recovered
            .nodes
            .iter()
            .any(|node| node.event.state == "interrupted"));
        assert!(!recovered
            .nodes
            .iter()
            .any(|node| node.event.state == "running"));
    }

    #[test]
    fn failed_push_and_following_pull_keep_separate_linked_histories() {
        let repo = Repo::new();
        let push = || Mutation::Push {
            remote: None,
            set_upstream: false,
            tags: false,
            force_with_lease: false,
            force: false,
        };
        let failed = repo.run(push());
        assert_eq!(failed.state, "failed");
        assert!(!failed.can_auto_close);
        let pulled = repo.run(Mutation::Pull {
            rebase: false,
            autostash: true,
        });
        assert_ne!(pulled.id, failed.id);
        assert_eq!(pulled.kind, "pull");
        assert_eq!(pulled.previous, failed.id);
        let retried = repo.run(push());
        assert_ne!(retried.id, pulled.id);
        assert_eq!(retried.kind, "push");
        assert_eq!(retried.previous, pulled.id);
    }

    #[test]
    fn retention_removes_only_expired_finished_workflows() {
        let repo = Repo::new();
        let summary = repo.run(Mutation::Reset {
            rev: "HEAD".into(),
            mode: awegit_git::ResetMode::Soft,
        });
        let directory = directory(&repo.0).unwrap();
        let mut ended = summary.clone();
        ended.ended_at = Some(1);
        ended.started_at = 1;
        fs::write(
            directory.join(format!("{}.json", ended.id)),
            serde_json::to_vec(&ended).unwrap(),
        )
        .unwrap();
        let mut active = ended.clone();
        active.id = "100-1-2".into();
        active.state = "conflict".into();
        active.ended_at = None;
        fs::write(
            directory.join("100-1-2.json"),
            serde_json::to_vec(&active).unwrap(),
        )
        .unwrap();
        prune(&directory, 3, 4 * 86_400_000);
        assert!(!directory.join(format!("{}.jsonl", ended.id)).exists());
        assert!(directory.join("100-1-2.json").exists());
    }

    fn benchmark_samples(default: usize) -> usize {
        std::env::var("AWEGIT_PERF_SAMPLES").ok().map(|value| value.parse::<usize>().unwrap().clamp(1, 1000)).unwrap_or(default)
    }

    fn measure(label: &str, iterations: usize, mut action: impl FnMut()) {
        let iterations = benchmark_samples(iterations);
        action(); // Report warm reads separately from data preparation.
        let mut times = Vec::with_capacity(iterations);
        let (_, calls) = count_git_calls(|| {
            for _ in 0..iterations {
                let start = Instant::now();
                action();
                times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
        });
        times.sort_by(f64::total_cmp);
        println!("PERF {}", serde_json::json!({
            "label": label, "samples": iterations,
            "meanMs": times.iter().sum::<f64>() / iterations as f64,
            "p50Ms": times[(iterations - 1) / 2],
            "p95Ms": times[(iterations * 95).div_ceil(100) - 1],
            "maxMs": times[iterations - 1], "gitProcesses": calls,
        }));
    }

    #[test]
    #[ignore = "manual backend benchmark; run with --ignored --nocapture"]
    fn backend_read_timings() {
        let repo = Repo::new();
        repo.run(Mutation::Reset {
            rev: "HEAD".into(),
            mode: awegit_git::ResetMode::Soft,
        });
        measure("git directory (warm)", 30, || {
            std::hint::black_box(flow::git_dir(&repo.0).unwrap());
        });
        measure("clean checkpoint", 30, || {
            std::hint::black_box(flow::checkpoint(&repo.0).unwrap());
        });
        measure("completed operation_flow", 30, || {
            std::hint::black_box(repo.snapshot());
        });
        measure("workspace_status after completion", 30, || {
            std::hint::black_box(
                super::super::workspace_status(Some(repo.0.display().to_string())).unwrap(),
            );
        });

        repo.git(&["branch", "incoming"]);
        repo.save("note", "local\n", "local");
        repo.git(&["checkout", "incoming"]);
        repo.save("note", "incoming\n", "incoming");
        repo.git(&["checkout", "main"]);
        assert_eq!(repo.run(Mutation::Merge {
                name: "incoming".into(),
                squash: false,
                no_ff: false,
                autostash: false,
                summary: String::new(),
                description: String::new(),
            }).state, "conflict");
        measure("stable active operation_flow", 30, || {
            std::hint::black_box(repo.snapshot());
        });
        measure("merge conflict checkpoint", 30, || {
            std::hint::black_box(flow::checkpoint(&repo.0).unwrap());
        });
        measure("merge conflict_view", 30, || {
            std::hint::black_box(awegit_git::conflict_view(&repo.0, "note").unwrap());
        });
        awegit_git::perform(&repo.0, Mutation::Abort).unwrap();
        assert!(awegit_git::perform(
            &repo.0,
            Mutation::Rebase {
                onto: "incoming".into(),
                autostash: false,
            },
        )
        .is_err());
        measure("rebase conflict_view", 30, || {
            std::hint::black_box(awegit_git::conflict_view(&repo.0, "note").unwrap());
        });
    }

    #[test]
    #[ignore = "optional large data benchmark; set AWEGIT_PERF_MANIFEST"]
    fn seeded_performance_timings() {
        thread_local! { static PHASES: std::cell::RefCell<HashMap<String, Vec<f64>>> = std::cell::RefCell::new(HashMap::new()); }
        awegit_git::set_timing_trace(|name, elapsed| PHASES.with(|phases| phases.borrow_mut().entry(name.into()).or_default().push(elapsed.as_secs_f64()*1000.0)));
        let Ok(manifest) = std::env::var("AWEGIT_PERF_MANIFEST") else { return };
        let value: serde_json::Value = serde_json::from_slice(&fs::read(manifest).unwrap()).unwrap();
        let workspace = PathBuf::from(value["workspace"].as_str().unwrap());
        let allowed = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join(".lab/performance").canonicalize().unwrap();
        assert!(workspace.canonicalize().unwrap().starts_with(&allowed));
        let repo = Repo(workspace.clone());
        // This seed belongs to the user and must remain available after the benchmark.
        let repo = std::mem::ManuallyDrop::new(repo);
        measure("seed porcelain pre-operation scan", 30, || {
            std::hint::black_box(repo.git(&["status", "--porcelain", "-z", "--untracked-files=normal"]));
            // Repo::git runs the external executable directly, outside the library trace hook.
            GIT_CALLS.with(|slot| { if let Some(n) = slot.get() { slot.set(Some(n+1)); } });
        });
        let count = value["untracked"].as_u64().unwrap() as usize;
        let ignored: Vec<String> = (0..count).map(|index| format!("ignored/{index}.tmp")).collect();
        fs::create_dir_all(workspace.join("ignored")).unwrap();
        for path in &ignored { fs::write(workspace.join(path), "build output\n").unwrap(); }
        measure("seed native ignore batch", 30, || {
            assert_eq!(awegit_git::ignored_paths(&workspace, &ignored).len(), count);
        });
        measure("seed workspace status", 30, || { std::hint::black_box(awegit_git::status(&workspace).unwrap()); });
        let directory = directory(&workspace).unwrap();
        let start = Instant::now();
        let first = history(&workspace, 0, 100).unwrap();
        println!("PERF {}", serde_json::json!({"label": "seed history first page", "samples": 1, "ms": start.elapsed().as_secs_f64()*1000.0, "summariesRead": history_index(&directory).lock().unwrap().reads, "returned": first.len()}));
        measure("seed history warm page", 30, || { std::hint::black_box(history(&workspace, 0, 100).unwrap()); });
        println!("PERF {}", serde_json::json!({"label": "seed history parse count after polls", "summariesRead": history_index(&directory).lock().unwrap().reads}));
        for scenario in value["conflicts"].as_array().unwrap() {
            let path = PathBuf::from(scenario["repo"].as_str().unwrap());
            let count = scenario["count"].as_u64().unwrap() as usize;
            measure(&format!("seed {count} conflicts checkpoint"), 30, || { assert_eq!(flow::checkpoint(&path).unwrap().files.len(), count); });
        }
        let blob_repo = PathBuf::from(value["blobs"].as_str().unwrap());
        for case in value["blobCases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            measure(&format!("seed blob {name}"), 30, || { std::hint::black_box(awegit_git::conflict_view(&blob_repo, name).unwrap()); });
        }
        if std::env::var_os("AWEGIT_PERF_WRITES").is_none() { return; }
        let base = value["base"].as_str().unwrap();
        for (label, untracked) in [("clean", 0), ("mixed", 3), ("untracked", count)] {
            // Previous benchmark runs can have left our generated local files.
            for index in 0..count.max(3) {
                let path = workspace.join(format!("untracked/{index}.txt"));
                if path.is_file() { fs::remove_file(path).unwrap(); }
            }
            PHASES.with(|phases| phases.borrow_mut().clear());
            GIT_TIMES.with(|times| *times.borrow_mut() = Some(HashMap::new()));
            let mut times = Vec::new();
            let mut calls = 0;
            let samples = benchmark_samples(30);
            for _ in 0..samples {
                // Restore the same local remote and index state outside the timed interval.
                repo.git(&["reset", "--hard", base]);
                if label != "clean" {
                    fs::write(workspace.join("tracked/0/file-0.txt"), "staged\n").unwrap();
                    repo.git(&["add", "tracked/0/file-0.txt"]);
                    fs::write(workspace.join("tracked/0/file-0.txt"), "working\n").unwrap();
                }
                fs::create_dir_all(workspace.join("untracked")).unwrap();
                for index in 0..untracked { fs::write(workspace.join(format!("untracked/{index}.txt")), "local\n").unwrap(); }
                let start = Instant::now();
                let (_, processes) = count_git_calls(|| {
                    awegit_git::perform(&workspace, Mutation::Pull { rebase: true, autostash: true }).unwrap();
                });
                times.push(start.elapsed().as_secs_f64()*1000.0);
                calls += processes;
                if label != "clean" {
                    assert_eq!(fs::read_to_string(workspace.join("tracked/0/file-0.txt")).unwrap(), "working\n");
                    assert_eq!(repo.git(&["show", ":tracked/0/file-0.txt"]), "staged");
                }
                assert_eq!(repo.git(&["rev-parse", "HEAD"]), value["incoming"].as_str().unwrap());
                for index in [0, untracked.saturating_sub(1)] {
                    if untracked > 0 { assert_eq!(fs::read_to_string(workspace.join(format!("untracked/{index}.txt"))).unwrap(), "local\n"); }
                }
                assert!(repo.git(&["stash", "list"]).is_empty());
            }
            times.sort_by(f64::total_cmp);
            println!("PERF {}", serde_json::json!({"label": format!("seed local pull {label}"), "samples": samples, "p50Ms": times[(samples-1)/2], "p95Ms": times[(samples*95).div_ceil(100)-1], "maxMs": times[samples-1], "gitProcesses": calls}));
            GIT_TIMES.with(|times| {
                for (name, mut times) in times.borrow_mut().take().unwrap_or_default() {
                    times.sort_by(f64::total_cmp);
                    let n = times.len();
                    println!("PERF {}", serde_json::json!({"label": format!("seed local pull {label} git {name}"), "samples": n, "totalMs": times.iter().sum::<f64>(), "p50Ms": times[(n-1)/2], "p95Ms": times[(n*95).div_ceil(100)-1], "maxMs": times[n-1]}));
                }
            });
            PHASES.with(|phases| {
                for (name, mut times) in phases.take() {
                    times.sort_by(f64::total_cmp);
                    let n = times.len();
                    println!("PERF {}", serde_json::json!({"label": format!("seed local pull {label} {name}"), "samples": n, "p50Ms": times[(n-1)/2], "p95Ms": times[(n*95).div_ceil(100)-1], "maxMs": times[n-1]}));
                }
            });
        }
    }

    #[test]
    fn finished_flow_reads_do_not_spawn_git() {
        let repo = Repo::new();
        let summary = repo.run(Mutation::Reset {
            rev: "HEAD".into(),
            mode: awegit_git::ResetMode::Soft,
        });
        let session = active(&repo.0).unwrap();
        for state in ["completed", "failed", "aborted"] {
            session.data.lock().unwrap().snapshot.summary.state = state.into();
            let (snapshot, calls) = count_git_calls(|| {
                reconcile(&repo.0);
                repo.snapshot()
            });
            assert_eq!(snapshot.summary.state, state);
            assert_eq!(snapshot.summary.sequence, summary.sequence);
            assert_eq!(calls, 0, "finished {state} flow must not read live Git state");
        }
    }

    #[test]
    fn git_directory_cache_is_shared_and_worktree_specific() {
        let repo = Repo::new();
        let linked = repo.0.join("linked");
        repo.git(&["worktree", "add", "-b", "linked", linked.to_str().unwrap()]);
        let main_dir = flow::git_dir(&repo.0).unwrap();
        let linked_dir = flow::git_dir(&linked).unwrap();
        assert_ne!(main_dir, linked_dir);
        assert_eq!(
            main_dir.canonicalize().unwrap(),
            repo.0.join(".git").canonicalize().unwrap()
        );
        let expected = repo.git(&[
            "-C",
            linked.to_str().unwrap(),
            "rev-parse",
            "--absolute-git-dir",
        ]);
        assert_eq!(
            linked_dir.canonicalize().unwrap(),
            Path::new(&expected).canonicalize().unwrap()
        );
        let (_, calls) = count_git_calls(|| {
            for (path, expected) in [(&repo.0, &main_dir), (&linked, &linked_dir)] {
                assert_eq!(flow::git_dir(path).unwrap(), *expected);
                assert_eq!(awegit_git::in_progress(path).unwrap(), None);
                assert_eq!(flow::git_dir(path).unwrap(), *expected);
            }
        });
        assert_eq!(calls, 0, "directory reads must share the same cache");
    }
}
