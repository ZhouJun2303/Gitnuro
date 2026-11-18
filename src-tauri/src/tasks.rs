//! Process-wide task identities, dependency checks and cancellation, shared by all windows.
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tauri::Emitter;

mod progress;
mod intent;
use intent::Intent;
use progress::Progress;

type Job = Box<dyn FnOnce() + Send>;
const DEPENDENCY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub path: String,
    pub action: String,
    pub label: String,
    pub preview: HashMap<String, String>,
    pub targets: Vec<String>,
    pub target_count: usize,
    pub state: String,
    pub phase: String,
    pub percent: u8,
    pub depends_on: Vec<String>,
    pub revision: u64,
    pub sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_scope: Option<String>,
    pub queued_ms: u64,
    pub execute_ms: u64,
    pub verify_ms: u64,
}

struct Task {
    snapshot: Snapshot,
    progress: Progress,
    after: Option<String>,
    lane: String,
    intent: Result<Option<Arc<Intent>>, String>,
    commit: Option<(String, String)>,
    queued: Instant,
    started: Option<Instant>,
    verifying: Option<Instant>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
struct Registry {
    tasks: HashMap<String, Task>,
    finished: VecDeque<String>,
    lanes: HashMap<String, VecDeque<String>>,
    dependents: HashMap<String, HashSet<String>>,
    jobs: HashMap<String, Job>,
    cancelled_before_enqueue: HashSet<String>,
}

fn registry() -> &'static (Mutex<Registry>, Condvar) {
    static REGISTRY: OnceLock<(Mutex<Registry>, Condvar)> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
static SEQUENCE: AtomicU64 = AtomicU64::new(1);
static ADMISSIONS: OnceLock<std::sync::mpsc::Sender<Job>> = OnceLock::new();
thread_local! { static CURRENT: RefCell<Option<String>> = const { RefCell::new(None) }; }

pub fn install(app: tauri::AppHandle) { let _ = APP.set(app); install_progress(); }

pub fn install_progress() {
    awegit_git::set_progress(|path, percent| {
        let key = awegit_git::repo_key(Path::new(path));
        let id = {
            let registry = registry().0.lock().unwrap_or_else(|error| error.into_inner());
            registry.tasks.values().find(|task| matches!(task.snapshot.state.as_str(), "running" | "cancelling")
                && awegit_git::repo_key(Path::new(&task.snapshot.path)) == key).map(|task| task.snapshot.id.clone())
        };
        if let Some(id) = id { update_progress(&id, |progress| progress.transfer(percent)); }
        if let Some(app) = APP.get() { let _ = app.emit("git-progress", super::GitProgress { path: path.into(), percent }); }
    });
}
fn emit(snapshot: &Snapshot) { if let Some(app) = APP.get() { let _ = app.emit("operation-task", snapshot); } }
pub fn terminal(state: &str) -> bool { matches!(state, "completed" | "conflict" | "failed" | "cancelled" | "blocked") }

fn touch(snapshot: &mut Snapshot) { snapshot.sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed); }

// Route IPC arrivals in order without doing file verification on the router.
fn admissions() -> &'static std::sync::mpsc::Sender<Job> {
    ADMISSIONS.get_or_init(|| {
        let (send, receive) = std::sync::mpsc::channel::<Job>();
        std::thread::Builder::new().name("awegit-admissions".into()).spawn(move || loop {
            match receive.recv_timeout(Duration::from_millis(250)) {
                Ok(admit) => admit(),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => dispatch_ready(),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }).expect("start repository admission worker");
        send
    })
}

fn admission_lanes() -> &'static Mutex<HashMap<String, VecDeque<Job>>> {
    static LANES: OnceLock<Mutex<HashMap<String, VecDeque<Job>>>> = OnceLock::new();
    LANES.get_or_init(Default::default)
}

fn queue_admission(lane: String, job: Job) {
    use std::collections::hash_map::Entry;
    {
        let mut lanes = admission_lanes().lock().unwrap_or_else(|error| error.into_inner());
        match lanes.entry(lane.clone()) {
            Entry::Occupied(mut active) => { active.get_mut().push_back(job); return; }
            Entry::Vacant(idle) => { idle.insert(VecDeque::from([job])); }
        }
    }
    // The entry stays present while its worker runs, even with no pending jobs.
    tauri::async_runtime::spawn_blocking(move || loop {
        let job = {
            let mut lanes = admission_lanes().lock().unwrap_or_else(|error| error.into_inner());
            match lanes.get_mut(&lane).and_then(VecDeque::pop_front) {
                Some(job) => job,
                None => { lanes.remove(&lane); return; }
            }
        };
        job();
    });
}

/// Admit requests in repository order and park writes until they are runnable.
pub fn submit(command: String, args: Value, complete: impl FnOnce(Result<Value, String>) + Send + 'static) {
    let route = Box::new(move || {
        let (target, lane) = match task_repository(&args) {
            Ok(repository) => repository,
            Err(error) => { complete(Err(error)); return; }
        };
        queue_admission(lane.clone(), Box::new(move || {
            let prepared = {
                let _span = super::perf::command_with_id("enqueue_operation", args.get("__perf").and_then(Value::as_str));
                awegit_git::timing_repository(&target);
                enqueue_in_lane(&command, &args, target, lane)
            };
            let id = match prepared {
                Ok(id) => id,
                Err(error) => { complete(Err(error)); return; }
            };
            let job_id = id.clone();
            let job: Job = Box::new(move || {
                let _span = super::perf::command_with_id(&command, args.get("__perf").and_then(Value::as_str));
                complete(run(&job_id, || execute(&command, &args), |value| outcome(&command, &args, value)));
            });
            let mut state = registry().0.lock().unwrap_or_else(|error| error.into_inner());
            if state.tasks[&id].cancelled.load(Ordering::Acquire) {
                drop(state);
                tauri::async_runtime::spawn_blocking(job);
            } else {
                state.jobs.insert(id, job);
                drop(state);
                dispatch_ready();
            }
        }));
    });
    if let Err(error) = admissions().send(route) { (error.0)(); }
}

fn await_admissions(lane: &str) {
    let Some(sender) = ADMISSIONS.get() else { return };
    let (done, admitted) = std::sync::mpsc::channel();
    let lane = lane.to_string();
    if sender.send(Box::new(move || {
        let mut lanes = admission_lanes().lock().unwrap_or_else(|error| error.into_inner());
        if let Some(queue) = lanes.get_mut(&lane) {
            queue.push_back(Box::new(move || { let _ = done.send(()); }));
        } // Dropping the sender also releases a reader of an idle lane.
    })).is_ok() { let _ = admitted.recv(); }
}

fn dispatch_ready() {
    let jobs = {
        let mut registry = registry().0.lock().unwrap_or_else(|error| error.into_inner());
        let ready: Vec<_> = registry.lanes.values().filter_map(|lane| lane.front()).filter(|id| {
            if !registry.jobs.contains_key(*id) { return false; }
            let task = &registry.tasks[*id];
            dependency_failed(&registry, task) || !waiting(&registry, task)
        }).cloned().collect();
        ready.into_iter().filter_map(|id| registry.jobs.remove(&id)).collect::<Vec<_>>()
    };
    for job in jobs { tauri::async_runtime::spawn_blocking(job); }
}

fn dependency_failed(registry: &Registry, task: &Task) -> bool {
    let expired = task.queued.elapsed() >= DEPENDENCY_TIMEOUT;
    task.snapshot.depends_on.iter().any(|dependency| match registry.tasks.get(dependency) {
        Some(task) => terminal(&task.snapshot.state) && task.snapshot.state != "completed",
        None => expired,
    })
}

fn waiting(registry: &Registry, task: &Task) -> bool {
    task.snapshot.depends_on.iter().chain(task.after.iter()).any(|id| match registry.tasks.get(id) {
        Some(task) => !terminal(&task.snapshot.state),
        None => task.queued.elapsed() < DEPENDENCY_TIMEOUT,
    })
}

fn dependency_reaches(registry: &Registry, task: &str, prerequisite: &str) -> bool {
    let mut pending = vec![task];
    let mut visited = HashSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) { continue; }
        if let Some(task) = registry.tasks.get(id) {
            for dependency in task.snapshot.depends_on.iter().chain(task.after.iter()) {
                if dependency == prerequisite { return true; }
                pending.push(dependency);
            }
        }
    }
    false
}

/// IPC delivery can interleave a chain arbitrarily. Keep independent writes in
/// arrival order while putting every delivered prerequisite before its users.
fn order_lane(registry: &mut Registry, lane: &str) {
    let Some(queue) = registry.lanes.get(lane) else { return };
    let positions: HashMap<_, _> = queue.iter().enumerate().map(|(index, id)| (id, index)).collect();
    let mut dependents = vec![Vec::new(); queue.len()];
    let mut remaining = vec![0; queue.len()];
    let mut ready = BinaryHeap::new();
    for (index, id) in queue.iter().enumerate() {
        let task = &registry.tasks[id];
        for dependency in task.snapshot.depends_on.iter().chain(task.after.iter()) {
            if let Some(&earlier) = positions.get(dependency) {
                remaining[index] += 1;
                dependents[earlier].push(index);
            }
        }
        if remaining[index] == 0 { ready.push(Reverse(index)); }
    }
    let mut ordered = VecDeque::new();
    while let Some(Reverse(index)) = ready.pop() {
        ordered.push_back(queue[index].clone());
        for &next in &dependents[index] {
            remaining[next] -= 1;
            if remaining[next] == 0 { ready.push(Reverse(next)); }
        }
    }
    if ordered.len() == queue.len() { registry.lanes.insert(lane.to_string(), ordered); }
}

pub fn handles(command: &str) -> bool {
    matches!(command, "mutate" | "stage_all" | "unstage_all" | "stage_path" | "unstage_path" | "commit_changes"
        | "save_hidden_refs" | "save_expanded_groups" | "save_sign_off" | "set_repo_author" | "save_remote_account" | "approve_credential" | "remove_index_lock" | "remove_stale_lock")
}

fn task_repository(args: &Value) -> Result<(PathBuf, String), String> {
    let path = args.get("path").and_then(Value::as_str).unwrap_or("");
    let request = args.get("request").unwrap_or(args);
    let target = match request.get("destination").and_then(Value::as_str).filter(|_| matches!(request.get("action").and_then(Value::as_str), Some("clone" | "init"))) {
        Some(destination) => PathBuf::from(destination),
        None => super::repo_from((!path.is_empty()).then(|| path.to_string()))?,
    };
    super::lab::ensure_lab_path(&target)?;
    let lane = awegit_git::common_repo_key(&target);
    Ok((target, lane))
}

pub fn enqueue(command: &str, args: &Value) -> Result<String, String> {
    let (target, lane) = task_repository(args)?;
    enqueue_in_lane(command, args, target, lane)
}

fn enqueue_in_lane(command: &str, args: &Value, target: PathBuf, lane: String) -> Result<String, String> {
    let request = args.get("request").unwrap_or(args);
    let id = args.get("__task").and_then(Value::as_str).map(str::to_string)
        .unwrap_or_else(|| format!("task-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let action = request.get("action").and_then(Value::as_str).unwrap_or(command).to_string();
    let depends_on: Vec<String> = args.get("__dependsOn").and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let after = args.get("__after").and_then(Value::as_str).map(str::to_string).filter(|earlier| !depends_on.contains(earlier));
    let intent = Intent::capture(command, &target, args).map(|intent| intent.map(Arc::new));
    let mut targets: Vec<String> = request.get("files").and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    if let Some(selection) = args.get("__selection").and_then(Value::as_array) { targets.extend(selection.iter().filter_map(Value::as_str).map(str::to_string)); }
    if args.get("request").is_some() { if let Some(path) = request.get("path").and_then(Value::as_str) { targets.push(path.to_string()); } }
    for key in ["file", "name", "to", "branch", "rev", "onto", "destination", "remote"] {
        if let Some(value) = request.get(key).and_then(Value::as_str).filter(|value| !value.is_empty()) { targets.push(value.chars().take(1024).collect()); }
    }
    targets.sort(); targets.dedup();
    let target_count = targets.len();
    targets.truncate(16);
    let label = super::perf::redact(&args.get("__taskLabel").and_then(Value::as_str).unwrap_or("").chars().take(1024).collect::<String>());
    let mut preview: HashMap<String, String> = ["name", "to", "branch", "rev", "start", "remote", "path"].into_iter().filter_map(|key| request.get(key).and_then(Value::as_str).map(|value| (key.to_string(), value.chars().take(1024).collect()))).collect();
    if action == "stashRename" { if let Some(message) = request.get("message").and_then(Value::as_str) { preview.insert("message".into(), message.chars().take(1024).collect()); } }
    let mut snapshot = Snapshot { id: id.clone(), path: target.to_string_lossy().into_owned(), action, label, preview, targets, target_count,
        state: "queued".into(), phase: String::new(), percent: 0, depends_on, revision: super::changes::revision(&target), sequence: 0,
        refresh_scope: None, queued_ms: 0, execute_ms: 0, verify_ms: 0 };
    {
        let mut registry = registry().0.lock().unwrap_or_else(|error| error.into_inner());
        if registry.tasks.contains_key(&id) { return Err("Duplicate task identity".into()); }
        if snapshot.depends_on.iter().chain(after.iter()).any(|dependency| dependency == &id || dependency_reaches(&registry, dependency, &id)) {
            return Err("Task dependency cycle".into());
        }
        // A normal arrival only appends. Reorder when IPC delivers a prerequisite late.
        for dependency in snapshot.depends_on.iter().chain(after.iter()) {
            registry.dependents.entry(dependency.clone()).or_default().insert(id.clone());
        }
        let reorder = registry.dependents.get(&id).is_some_and(|ids|
            ids.iter().any(|id| registry.tasks.get(id).is_some_and(|task| task.lane == lane)));
        touch(&mut snapshot);
        registry.lanes.entry(lane.clone()).or_default().push_back(id.clone());
        let cancelled = registry.cancelled_before_enqueue.remove(&id);
        registry.tasks.insert(id.clone(), Task { progress: Progress::new(&snapshot.action), snapshot: snapshot.clone(), after, lane: lane.clone(), intent, commit: None,
            queued: Instant::now(), started: None, verifying: None, cancelled: Arc::new(AtomicBool::new(cancelled)) });
        if reorder { order_lane(&mut registry, &lane); }
        self::registry().1.notify_all();
    }
    emit(&snapshot);
    Ok(id)
}

pub fn run<T>(id: &str, body: impl FnOnce() -> Result<T, String>, outcome: impl FnOnce(&T) -> &str) -> Result<T, String> {
    let token = {
        let (lock, ready) = registry();
        let mut registry = lock.lock().unwrap_or_else(|error| error.into_inner());
        loop {
            let task = registry.tasks.get(id).ok_or("Unknown task")?;
            if task.cancelled.load(Ordering::Acquire) {
                drop(registry); finish(id, "cancelled"); return Err("awegit:cancelled:".into());
            }
            if dependency_failed(&registry, task) { drop(registry); finish(id, "blocked"); return Err("awegit:cancelled:Task dependency did not complete successfully".into()); }
            let first = registry.lanes.get(&task.lane).and_then(|lane| lane.front()).is_some_and(|first| first == id);
            if first && !waiting(&registry, task) { break; }
            let missing = task.snapshot.depends_on.iter().chain(task.after.iter()).any(|id| !registry.tasks.contains_key(id));
            let remaining = DEPENDENCY_TIMEOUT.saturating_sub(task.queued.elapsed());
            registry = if missing && !remaining.is_zero() {
                ready.wait_timeout(registry, remaining).unwrap_or_else(|error| error.into_inner()).0
            } else { ready.wait(registry).unwrap_or_else(|error| error.into_inner()) };
        }
        let task = registry.tasks.get_mut(id).unwrap();
        task.started = Some(Instant::now()); task.snapshot.queued_ms = task.queued.elapsed().as_millis() as u64; task.snapshot.state = "running".into();
        touch(&mut task.snapshot);
        let token = task.cancelled.clone(); let snapshot = task.snapshot.clone();
        drop(registry); emit(&snapshot); token
    };
    struct Scope(Option<String>);
    impl Drop for Scope { fn drop(&mut self) { CURRENT.with(|current| *current.borrow_mut() = self.0.take()); } }
    let _scope = Scope(CURRENT.with(|current| current.replace(Some(id.to_string()))));
    update_progress(id, Progress::prepared);
    let task_id = id.to_string();
    let observer = Arc::new(move |event: awegit_git::flow::Event| observe(&task_id, &event));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| awegit_git::flow::observe(observer, || awegit_git::with_cancellation(token.clone(), body))))
        .unwrap_or_else(|_| Err("Task stopped unexpectedly".into()));
    let state = match &result {
        Ok(value) => outcome(value),
        Err(error) if error.starts_with("awegit:cancelled:") => "cancelled",
        Err(_) => "failed",
    };
    finish(id, state);
    result
}

fn finish(id: &str, state: &str) {
    let (lock, ready) = registry();
    let mut registry = lock.lock().unwrap_or_else(|error| error.into_inner());
    let Some(task) = registry.tasks.get_mut(id) else { return };
    task.snapshot.state = state.to_string();
    task.progress.finish(state == "completed");
    task.snapshot.percent = task.progress.percent;
    if task.started.is_none() { task.snapshot.queued_ms = task.queued.elapsed().as_millis() as u64; }
    task.snapshot.revision = super::changes::revision(Path::new(&task.snapshot.path));
    if let Some(start) = task.started { task.snapshot.execute_ms = task.verifying.unwrap_or_else(Instant::now).duration_since(start).as_millis() as u64; }
    if let Some(start) = task.verifying { task.snapshot.verify_ms = start.elapsed().as_millis() as u64; }
    touch(&mut task.snapshot);
    let lane = task.lane.clone(); let snapshot = task.snapshot.clone();
    let predecessors: Vec<_> = task.snapshot.depends_on.iter().chain(task.after.iter()).cloned().collect();
    for dependency in predecessors {
        if let Some(ids) = registry.dependents.get_mut(&dependency) {
            ids.remove(id);
            if ids.is_empty() { registry.dependents.remove(&dependency); }
        }
    }
    if let Some(queue) = registry.lanes.get_mut(&lane) {
        if queue.front().is_some_and(|first| first == id) { queue.pop_front(); }
        else { queue.retain(|task| task != id); }
        if queue.is_empty() { registry.lanes.remove(&lane); }
    }
    registry.finished.push_back(id.into());
    // Keep outcomes needed by dependent writes, even under a large backlog.
    while registry.finished.len() > 256 {
        let Some(index) = registry.finished.iter().position(|id| !registry.dependents.contains_key(id)) else { break; };
        let old = registry.finished.remove(index).unwrap();
        registry.tasks.remove(&old);
    }
    ready.notify_all(); drop(registry); emit(&snapshot);
    dispatch_ready();
}

pub fn verifying() {
    let Some(id) = CURRENT.with(|current| current.borrow().clone()) else { return };
    let mut registry = registry().0.lock().unwrap_or_else(|error| error.into_inner());
    let Some(task) = registry.tasks.get_mut(&id) else { return };
    if task.verifying.is_none() { task.verifying = Some(Instant::now()); }
    task.progress.verifying();
    task.snapshot.percent = task.progress.percent;
    task.snapshot.phase = task.progress.phase.into();
    task.snapshot.state = if task.cancelled.load(Ordering::Acquire) { "cancelling" } else { "verifying" }.into();
    touch(&mut task.snapshot);
    let snapshot = task.snapshot.clone(); drop(registry); emit(&snapshot);
}

pub fn current_id() -> Option<String> { CURRENT.with(|current| current.borrow().clone()) }

pub fn observe(id: &str, event: &awegit_git::flow::Event) {
    update_progress(id, |progress| progress.observe(event));
}

fn update_progress(id: &str, update: impl FnOnce(&mut Progress)) {
    let mut registry = registry().0.lock().unwrap_or_else(|error| error.into_inner());
    let Some(task) = registry.tasks.get_mut(id) else { return };
    if terminal(&task.snapshot.state) { return; }
    update(&mut task.progress);
    if task.snapshot.phase == task.progress.phase && task.snapshot.percent == task.progress.percent { return; }
    task.snapshot.phase = task.progress.phase.into();
    task.snapshot.percent = task.progress.percent;
    touch(&mut task.snapshot);
    let snapshot = task.snapshot.clone(); drop(registry); emit(&snapshot);
}

#[cfg(test)]
pub fn list() -> Vec<Snapshot> { list_after(None) }

pub fn list_after(after: Option<u64>) -> Vec<Snapshot> {
    let registry = registry().0.lock().unwrap_or_else(|error| error.into_inner());
    if after.is_some_and(|after| after >= SEQUENCE.load(Ordering::Relaxed) - 1) { return Vec::new(); }
    let mut tasks: Vec<_> = registry.tasks.values().filter(|task| task.snapshot.sequence > after.unwrap_or(0))
        .map(|task| task.snapshot.clone()).collect();
    tasks.sort_unstable_by_key(|task| task.sequence);
    tasks
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum TaskFeed {
    Updates { session: &'static str, tasks: Vec<Snapshot> },
    Full(Vec<Snapshot>),
}

pub fn feed(after: Option<u64>, session: Option<&str>) -> TaskFeed {
    let Some(after) = after else { return TaskFeed::Full(list_after(None)); };
    static SESSION: OnceLock<String> = OnceLock::new();
    let current = SESSION.get_or_init(|| format!("{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));
    TaskFeed::Updates { session: current, tasks: list_after((session == Some(current)).then_some(after)) }
}

pub fn snapshot(id: &str) -> Option<Snapshot> {
    registry().0.lock().unwrap_or_else(|error| error.into_inner()).tasks.get(id).map(|task| task.snapshot.clone())
}

/// Mutable reads wait for writes already submitted, without delaying later writes.
pub fn wait_for_writes(repo: &Path) {
    if CURRENT.with(|current| current.borrow().is_some()) { return; }
    let lane = awegit_git::common_repo_key(repo);
    await_admissions(&lane);
    let (lock, ready) = registry();
    let mut registry = lock.lock().unwrap_or_else(|error| error.into_inner());
    let pending = registry.lanes.get(&lane).cloned().unwrap_or_default();
    while pending.iter().any(|id| registry.tasks.get(id).is_some_and(|task| !terminal(&task.snapshot.state))) {
        registry = ready.wait(registry).unwrap_or_else(|error| error.into_inner());
    }
}

pub fn cancel(id: &str) -> Result<(), String> {
    let (lock, ready) = registry();
    let mut registry = lock.lock().unwrap_or_else(|error| error.into_inner());
    if !registry.tasks.contains_key(id) {
        if registry.cancelled_before_enqueue.len() >= 256 { return Err("Too many pending cancellations".into()); }
        registry.cancelled_before_enqueue.insert(id.to_string());
        return Ok(());
    }
    let task = registry.tasks.get_mut(id).unwrap();
    if terminal(&task.snapshot.state) { return Ok(()); }
    task.cancelled.store(true, Ordering::Release);
    let running = task.started.is_some();
    let token = task.cancelled.clone();
    task.snapshot.state = if running { "cancelling" } else { "cancelled" }.into();
    touch(&mut task.snapshot);
    let snapshot = task.snapshot.clone();
    let job = registry.jobs.remove(id);
    ready.notify_all(); drop(registry); emit(&snapshot);
    if running { awegit_git::cancel_token(&token); }
    if let Some(job) = job { tauri::async_runtime::spawn_blocking(job); }
    Ok(())
}

pub fn cancel_path(path: &Path) {
    let key = awegit_git::common_repo_key(path);
    let ids: Vec<_> = {
        let registry = registry().0.lock().unwrap_or_else(|error| error.into_inner());
        registry.tasks.values().filter(|task| task.lane == key && !terminal(&task.snapshot.state)).map(|task| task.snapshot.id.clone()).collect()
    };
    for id in ids { let _ = cancel(&id); }
}

fn arg<T: serde::de::DeserializeOwned>(args: &Value, name: &str) -> Result<T, String> {
    serde_json::from_value(args.get(name).cloned().unwrap_or(Value::Null)).map_err(|error| format!("{name}: {error}"))
}
fn value<T: Serialize>(result: Result<T, String>) -> Result<Value, String> { serde_json::to_value(result?).map_err(|error| error.to_string()) }

fn current_intent() -> Result<Option<Arc<Intent>>, String> {
    let Some(id) = current_id() else { return Ok(None) };
    registry().0.lock().unwrap_or_else(|error| error.into_inner()).tasks.get(&id).map_or(Ok(None), |task| task.intent.clone())
}

pub fn validate_intent(repo: &Path) -> Result<(), String> {
    if let Some(intent) = current_intent()? { intent.check(repo)?; }
    Ok(())
}

fn verified_push(repo: &Path, prerequisite: &str, force_with_lease: bool) -> Result<(awegit_git::Mutation, serde_json::Value), String> {
    let (branch, head) = {
        let registry = registry().0.lock().unwrap_or_else(|error| error.into_inner());
        let task = registry.tasks.get(prerequisite).filter(|task| task.snapshot.state == "completed"
            && awegit_git::repo_key(Path::new(&task.snapshot.path)) == awegit_git::repo_key(repo));
        task.and_then(|task| task.commit.clone()).ok_or("The prerequisite did not produce a commit on this branch. / 前置任务没有在此分支生成提交。")?
    };
    if awegit_git::current_branch(repo).map_err(|error| error.to_string())?.as_deref() != Some(&branch)
        || awegit_git::commit_intent(repo).map_err(|error| error.to_string())?.head.as_deref() != Some(&head) {
        return Err("The committed branch or HEAD changed before push. / 提交所在分支或 HEAD 在推送前已变化。".into());
    }
    let (remote, remote_branch, set_upstream) = awegit_git::push_commit_target(repo, &branch).map_err(|error| error.to_string())?;
    let target = serde_json::json!({ "remote": remote, "branch": branch, "remoteBranch": remote_branch });
    Ok((awegit_git::Mutation::PushRef { remote, branch, source_commit: Some(head), remote_branch, set_upstream, force_with_lease, force: false, tags: false }, target))
}

/// Keep the original IPC names so desktop and lab exercise exactly the same commands.
pub fn execute(command: &str, args: &Value) -> Result<Value, String> {
    if matches!(args.pointer("/request/action").and_then(Value::as_str), Some("push" | "pull")) && args.get("__pushCommit").is_none() {
        if let Some(branch) = args.get("__branch") {
            let repo = super::repo_from(arg(args, "path")?)?;
            let current = awegit_git::current_branch(&repo).map_err(|error| error.to_string())?;
            if *branch != serde_json::to_value(current).unwrap_or(Value::Null) {
                return Err("The selected branch changed before execution. / 当前分支已改变，请核对后重试。".into());
            }
        }
    }
    validate_diff(args)?;
    if let Some(expected) = args.get("__expectedConflict") {
        let report = super::conflict_sides(arg(args, "path")?, arg(expected, "file")?)?;
        if expected.get("version").and_then(Value::as_str) != Some(report.version.as_str()) {
            return Err("Conflict content changed outside this editor. Reload before saving. / 冲突文件已在外部改变，请重新加载后保存。".into());
        }
    }
    let path = || arg::<Option<String>>(args, "path");
    let result = match command {
        "mutate" => {
            let mut target = None;
            let request = if let Some(prerequisite) = args.get("__pushCommit").and_then(Value::as_str) {
                let repo = super::repo_from(path()?)?;
                let (request, resolved) = verified_push(&repo, prerequisite, args["request"]["forceWithLease"].as_bool().unwrap_or(false))?;
                target = Some(resolved);
                request
            } else { arg(args, "request")? };
            value(super::mutate(path()?, request, arg(args, "retryStash")?)).map(|mut report| {
                if let Some(target) = target { report["pushTarget"] = target; }
                report
            })
        }
        "stage_all" | "unstage_all" if args.get("__selection").is_some() => value(super::mutate(path()?, awegit_git::Mutation::StagePaths {
            files: arg(args, "__selection")?, unstage: command == "unstage_all",
        }, None)),
        "stage_all" => value(super::stage_all(path()?)),
        "unstage_all" => value(super::unstage_all(path()?)),
        "stage_path" => value(super::stage_path(path()?, arg(args, "file")?, arg(args, "files")?, arg(args, "filtered")?, arg(args, "incremental")?)),
        "unstage_path" => value(super::unstage_path(path()?, arg(args, "file")?, arg(args, "files")?, arg(args, "filtered")?, arg(args, "incremental")?)),
        "commit_changes" => {
            let result = value(super::commit_changes(path()?, arg(args, "summary")?, arg(args, "description")?, arg(args, "amend")?, arg(args, "signOff")?, arg(args, "skipHooks")?));
            if result.as_ref().is_err_and(|error| error.starts_with("awegit:cancelled:")) {
                let repo = super::repo_from(path()?)?;
                let checked = awegit_git::without_cancellation(|| super::read_status(&repo));
                let intent = current_intent()?;
                if let Some(Intent::Commit { expected, .. }) = intent.as_deref() {
                    match checked {
                        Ok(status) if status.commit_intent.head != expected.head => {
                            return Err("Git changed HEAD before cancellation. Check the new commit before retrying. / Git 在取消前已写入提交，请检查新提交后再操作。".into());
                        }
                        Err(_) => return Err("Commit interrupted; could not verify HEAD. Check the repository before retrying. / 提交中断且无法核对 HEAD，请先检查仓库。".into()),
                        _ => {}
                    }
                }
            }
            result
        }
        "save_hidden_refs" => value(super::save_hidden_refs(path()?, arg(args, "names")?)),
        "save_expanded_groups" => value(super::save_expanded_groups(path()?, arg(args, "names")?)),
        "save_sign_off" => value(super::save_sign_off(path()?, arg(args, "enabled")?, arg(args, "format")?)),
        "set_repo_author" => value(super::set_repo_author(path()?, arg(args, "name")?, arg(args, "email")?)),
        "save_remote_account" => value(super::save_remote_account(path()?, arg(args, "remote")?, arg(args, "accountId")?)),
        "approve_credential" => value(super::approve_credential(path()?, arg(args, "protocol")?, arg(args, "host")?, arg(args, "username")?, arg(args, "password")?)),
        "remove_index_lock" => value(super::remove_index_lock(path()?)),
        "remove_stale_lock" => value(super::remove_stale_lock(path()?, arg(args, "lock")?)),
        _ => Err(format!("Unknown task command: {command}")),
    };
    if let (Some(id), Ok(report)) = (current_id(), &result) {
        if let Some(task) = registry().0.lock().unwrap_or_else(|error| error.into_inner()).tasks.get_mut(&id) {
            task.snapshot.refresh_scope = report.get("refreshScope").and_then(Value::as_str).map(str::to_string);
            if command == "commit_changes" {
                task.commit = report["branch"].as_str().zip(report["commitIntent"]["head"].as_str()).map(|(branch, head)| (branch.into(), head.into()));
            }
        }
    }
    result
}

pub fn outcome(_command: &str, _args: &Value, value: &Value) -> &'static str {
    if value["error"].as_str().is_some_and(|error| error.starts_with("awegit:cancelled:")) { return "cancelled"; }
    match value["outcome"].as_str() {
        Some("conflict") => "conflict",
        Some("failed") => "failed",
        _ => "completed",
    }
}

pub fn diff_version(diff: &awegit_git::FileDiff) -> String {
    content_version(diff)
}

pub fn content_version(value: &impl Serialize) -> String {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_vec(value).unwrap_or_default().hash(&mut hash);
    format!("{:016x}", hash.finish())
}

fn validate_diff(args: &Value) -> Result<(), String> {
    let Some(expected) = args.get("__expectedDiff") else { return Ok(()) };
    let report = super::file_diff(arg(args, "path")?, arg(expected, "file")?, arg(expected, "staged")?, arg(expected, "unified")?, arg(expected, "ignoreSpace")?, arg(expected, "load")?)?;
    if expected.get("version").and_then(Value::as_str) != Some(report.version.as_str()) {
        return Err("The displayed diff has changed. Reload it before applying this edit. / 差异内容已变化，请刷新后重试。".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::mpsc;

    struct Repo(PathBuf);
    impl Repo {
        fn new() -> Self {
            let repo = Self(std::env::temp_dir().join(format!("awegit-task-test-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
            std::fs::create_dir_all(&repo.0).unwrap();
            repo.git(&["init", "-b", "main"]); repo
        }
        fn git(&self, args: &[&str]) -> String {
            let output = std::process::Command::new(awegit_git::git_program()).current_dir(&self.0)
                .args(["-c", "user.name=Task Test", "-c", "user.email=task@example.test", "-c", "commit.gpgsign=false"])
                .args(args).output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        fn args(&self) -> Value { json!({ "path": self.0, "request": { "action": "fetch" } }) }
    }
    impl Drop for Repo { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    fn ok(_: &Value) -> &str { "completed" }

    #[test]
    fn slow_admission_only_delays_its_own_repository() {
        let slow = Repo::new();
        let fast = Repo::new();
        for repo in [&slow, &fast] { repo.git(&["commit", "--allow-empty", "-qm", "test: seed"]); }
        let slow_lane = awegit_git::common_repo_key(&slow.0);
        let timeout = Duration::from_secs(10);
        let (started, preparing) = mpsc::channel();
        let (release, held) = mpsc::channel();
        // Hold admission at the point where a file intent hashes disk contents.
        assert!(admissions().send(Box::new(move || queue_admission(slow_lane, Box::new(move || {
            let _ = started.send(());
            let _ = held.recv();
        })))).is_ok());
        preparing.recv_timeout(timeout).unwrap();

        let (first_done, first) = mpsc::channel();
        submit("mutate".into(), json!({ "path": slow.0, "request": { "action": "createBranch", "name": "before" } }),
            move |result| { let _ = first_done.send(result); });
        let (second_done, second) = mpsc::channel();
        submit("mutate".into(), json!({ "path": slow.0, "request": { "action": "renameBranch", "name": "before", "to": "after" } }),
            move |result| { let _ = second_done.send(result); });
        let (fast_done, fast_write) = mpsc::channel();
        submit("mutate".into(), json!({ "path": fast.0, "request": { "action": "createBranch", "name": "unrelated" } }),
            move |result| { let _ = fast_done.send(result); });

        let read = |repo: &Repo| {
            let path = repo.0.to_string_lossy().into_owned();
            let (done, result) = mpsc::channel();
            std::thread::spawn(move || { let _ = done.send(super::super::repository_refs(Some(path))); });
            result
        };
        let slow_read = read(&slow);
        let fast_read = read(&fast);
        let refs = fast_read.recv_timeout(timeout).expect("unrelated read blocked on admission").unwrap();
        assert!(refs.branches.iter().any(|branch| branch.name == "unrelated"));
        assert_eq!(fast_write.recv_timeout(timeout).unwrap().unwrap()["outcome"], "completed");
        assert!(slow_read.try_recv().is_err());
        assert!(first.try_recv().is_err());
        assert!(second.try_recv().is_err());

        release.send(()).unwrap();
        assert_eq!(first.recv_timeout(timeout).unwrap().unwrap()["outcome"], "completed");
        assert_eq!(second.recv_timeout(timeout).unwrap().unwrap()["outcome"], "completed");
        let refs = slow_read.recv_timeout(timeout).unwrap().unwrap();
        assert!(refs.branches.iter().any(|branch| branch.name == "after"));
        assert!(!refs.branches.iter().any(|branch| branch.name == "before"));
    }

    #[test]
    fn real_fetch_commands_report_phase_progress_without_transfer_output() {
        let repo = Repo::new();
        std::fs::write(repo.0.join("note.txt"), "one\n").unwrap();
        repo.git(&["add", "note.txt"]);
        repo.git(&["commit", "-qm", "test: seed command progress"]);
        // Fetch from the same local object store: Git has no objects to transfer.
        repo.git(&["remote", "add", "origin", repo.0.to_str().unwrap()]);
        for (remote, succeeds) in [("origin", true), ("missing", false)] {
            let args = json!({ "path": repo.0, "request": { "action": "fetch", "remote": remote, "prune": false, "tags": false } });
            let id = enqueue("mutate", &args).unwrap();
            let result = run(&id, || {
                assert_eq!(list().iter().find(|task| task.id == id).unwrap().percent, 33);
                let value = execute("mutate", &args)?;
                let snapshot = list().into_iter().find(|task| task.id == id).unwrap();
                assert_eq!(snapshot.phase, if succeeds { "fetch" } else { "finished" });
                assert_eq!(snapshot.percent, 66);
                Ok(value)
            }, |value| outcome("mutate", &args, value));
            assert!(result.is_ok());
            let snapshot = list().into_iter().find(|task| task.id == id).unwrap();
            assert_eq!(snapshot.state, if succeeds { "completed" } else { "failed" });
            assert_eq!(snapshot.percent, if succeeds { 100 } else { 66 });
        }
    }

    #[test]
    fn a_noop_pull_reports_its_command_phases_and_finishes_after_verification() {
        let repo = Repo::new();
        std::fs::write(repo.0.join("note.txt"), "one\n").unwrap();
        repo.git(&["add", "note.txt"]);
        repo.git(&["commit", "-qm", "test: seed pull progress"]);
        repo.git(&["remote", "add", "origin", repo.0.to_str().unwrap()]);
        repo.git(&["fetch", "origin"]);
        repo.git(&["branch", "--set-upstream-to=origin/main", "main"]);
        std::fs::write(repo.0.join("note.txt"), "local work\n").unwrap();
        let args = json!({ "path": repo.0, "request": { "action": "pull", "rebase": false, "autostash": true } });
        let id = enqueue("mutate", &args).unwrap();
        let result = run(&id, || {
            assert_eq!(list().iter().find(|task| task.id == id).unwrap().percent, 16);
            let value = execute("mutate", &args)?;
            let snapshot = list().into_iter().find(|task| task.id == id).unwrap();
            assert_eq!(snapshot.state, "verifying");
            assert_eq!(snapshot.percent, 83);
            Ok(value)
        }, |value| outcome("mutate", &args, value)).unwrap();
        assert_eq!(result["outcome"], "completed");
        assert_eq!(list().iter().find(|task| task.id == id).unwrap().percent, 100);
        assert_eq!(std::fs::read_to_string(repo.0.join("note.txt")).unwrap(), "local work\n");
    }

    #[test]
    fn later_arrival_of_a_prerequisite_does_not_form_a_dependency_cycle() {
        let repo = Repo::new();
        let prerequisite = format!("late-{}", NEXT.fetch_add(1, Ordering::Relaxed));
        let mut next_args = repo.args(); next_args["__dependsOn"] = json!([prerequisite]);
        let next = enqueue("mutate", &next_args).unwrap();
        let mut first_args = repo.args(); first_args["__task"] = json!(prerequisite);
        enqueue("mutate", &first_args).unwrap();
        assert_eq!(run(&prerequisite, || Ok(json!(1)), ok).unwrap(), json!(1));
        assert_eq!(run(&next, || Ok(json!(2)), ok).unwrap(), json!(2));
    }

    #[test]
    fn reverse_delivery_of_a_four_write_chain_preserves_client_order() {
        let repo = Repo::new();
        let prefix = NEXT.fetch_add(1, Ordering::Relaxed);
        let ids: Vec<_> = (0..4).map(|index| format!("reverse-{prefix}-{index}")).collect();
        for index in (0..4).rev() {
            let mut args = repo.args(); args["__client"] = json!("same-window"); args["__task"] = json!(ids[index]);
            args["__dependsOn"] = if index == 0 { json!([]) } else { json!([ids[index - 1]]) };
            enqueue("mutate", &args).unwrap();
        }
        for id in ids { assert!(run(&id, || Ok(json!(true)), ok).is_ok()); }
    }

    #[test]
    fn interleaved_delivery_sorts_transitive_prerequisites_and_avoids_cross_window_cycles() {
        for cross_window in [false, true] {
            let repo = Repo::new(); let prefix = NEXT.fetch_add(1, Ordering::Relaxed);
            let ids: Vec<_> = (0..4).map(|index| format!("interleaved-{prefix}-{index}")).collect();
            for index in [3, 1, 2, 0] {
                let mut args = repo.args(); args["__client"] = json!(if cross_window && index == 3 { "other" } else { "window" });
                args["__task"] = json!(ids[index]);
                args["__dependsOn"] = if index == 0 { json!([]) } else { json!([ids[index - 1]]) };
                enqueue("mutate", &args).unwrap();
            }
            let lane = awegit_git::common_repo_key(&repo.0);
            assert_eq!(registry().0.lock().unwrap().lanes[&lane], ids);
            for id in ids { assert!(run(&id, || Ok(json!(true)), ok).is_ok()); }
        }
    }

    #[test]
    fn another_window_observes_failure_and_independent_writes_continue() {
        let repo = Repo::new();
        let first = enqueue("mutate", &repo.args()).unwrap();
        let mut args = repo.args();
        args["__after"] = json!(first);
        let second = enqueue("mutate", &args).unwrap();
        assert!(run::<Value>(&first, || Err("conflict".into()), ok).is_err());
        assert_eq!(snapshot(&first).unwrap().state, "failed");
        assert!(run(&second, || Ok(json!(true)), ok).is_ok());
        let mut args = repo.args(); args["request"]["action"] = json!("abort");
        let recovery = enqueue("mutate", &args).unwrap();
        assert!(run(&recovery, || Ok(json!(true)), ok).is_ok());
    }

    #[test]
    fn queued_discard_preserves_later_disk_and_index_changes() {
        let repo = Repo::new();
        let file = repo.0.join("tracked.txt");
        std::fs::write(&file, "base\n").unwrap();
        repo.git(&["add", "tracked.txt"]);
        repo.git(&["commit", "-m", "test: seed"]);
        for staged in [false, true] {
            std::fs::write(&file, "selected version\n").unwrap();
            let args = json!({ "path": repo.0, "request": { "action": "discard", "file": "tracked.txt" } });
            let earlier = enqueue("mutate", &repo.args()).unwrap();
            let id = enqueue("mutate", &args).unwrap();
            run(&earlier, || {
                if staged { repo.git(&["add", "tracked.txt"]); }
                else { std::fs::write(&file, "new work\n").unwrap(); }
                Ok(json!(true))
            }, ok).unwrap();
            assert!(run(&id, || execute("mutate", &args), ok).unwrap_err().contains("selected files changed"));
            assert_eq!(std::fs::read_to_string(&file).unwrap(), if staged { "selected version\n" } else { "new work\n" });
        }
    }

    fn commit_args(repo: &Repo) -> Value {
        json!({ "path": repo.0, "summary": "test: queued commit", "description": "", "amend": false,
            "signOff": false, "skipHooks": true, "__branch": awegit_git::current_branch(&repo.0).unwrap(),
            "__expectedCommit": awegit_git::commit_intent(&repo.0).unwrap() })
    }

    #[test]
    fn queued_commit_does_not_include_paths_staged_after_confirmation() {
        let repo = Repo::new(); repo.git(&["commit", "--allow-empty", "-m", "test: seed"]);
        for file in ["selected.txt", "other.txt"] { std::fs::write(repo.0.join(file), file).unwrap(); }
        repo.git(&["add", "selected.txt"]);
        let args = commit_args(&repo);
        let earlier = enqueue("mutate", &repo.args()).unwrap();
        let id = enqueue("commit_changes", &args).unwrap();
        run(&earlier, || { repo.git(&["add", "other.txt"]); Ok(json!(true)) }, ok).unwrap();
        assert!(run(&id, || execute("commit_changes", &args), ok).unwrap_err().contains("index changed"));
        assert_eq!(repo.git(&["log", "-1", "--format=%s"]).trim(), "test: seed");
    }

    #[test]
    fn cancelling_a_commit_from_a_subdirectory_reports_existing_head() {
        let repo = Repo::new(); repo.git(&["commit", "--allow-empty", "-m", "test: seed"]);
        repo.git(&["config", "user.name", "Task Test"]);
        repo.git(&["config", "user.email", "task@example.test"]);
        repo.git(&["config", "core.hooksPath", ".git/hooks"]);
        let earlier = repo.git(&["rev-parse", "HEAD"]);
        let child = repo.0.join("sub");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(child.join("file.txt"), "new\n").unwrap();
        repo.git(&["add", "sub/file.txt"]);
        let hook = repo.0.join(".git/hooks/post-commit");
        let marker = repo.0.join(".git/commit-started");
        let release = repo.0.join(".git/commit-release");
        std::fs::write(&hook, "#!/bin/sh\n: > .git/commit-started\nwhile test ! -f .git/commit-release; do sleep 0.1; done\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut args = commit_args(&repo);
        args["path"] = json!(child);
        args["skipHooks"] = json!(false);
        let id = enqueue("commit_changes", &args).unwrap();
        let task = id.clone();
        let (done, result) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let response = run(&task, || execute("commit_changes", &args), ok);
            let _ = done.send(response);
        });
        let deadline = Instant::now() + Duration::from_secs(8);
        while !marker.exists() && Instant::now() < deadline { std::thread::sleep(Duration::from_millis(20)); }
        let started = marker.exists();
        cancel(&id).unwrap();
        let response = result.recv_timeout(Duration::from_secs(4));
        std::fs::write(release, "").unwrap();
        worker.join().unwrap();
        assert!(started, "post-commit verification hook did not run: {response:?}");
        let error = response.expect("cancellation did not stop the Git process").unwrap_err();
        assert!(error.contains("changed HEAD before cancellation"), "{error}");
        assert_eq!(snapshot(&id).unwrap().state, "failed");
        assert_ne!(repo.git(&["rev-parse", "HEAD"]), earlier);
    }

    #[test]
    fn dependent_push_uses_the_confirmed_commit_and_rejects_a_moved_head() {
        let repo = Repo::new(); repo.git(&["commit", "--allow-empty", "-m", "test: seed"]);
        let remote = repo.0.join(".git").join("test-remote.git");
        repo.git(&["init", "--bare", remote.to_str().unwrap()]);
        repo.git(&["remote", "add", "origin", remote.to_str().unwrap()]);
        repo.git(&["push", "origin", "main"]);
        let main = repo.git(&["rev-parse", "main"]);
        repo.git(&["checkout", "-b", "feature"]);
        repo.git(&["config", "branch.feature.remote", "origin"]);
        repo.git(&["config", "branch.feature.merge", "refs/heads/published"]);
        repo.git(&["config", "user.name", "Task Test"]);
        repo.git(&["config", "user.email", "task@example.test"]);
        for move_head in [false, true] {
            std::fs::write(repo.0.join("feature.txt"), format!("{move_head}\n")).unwrap();
            repo.git(&["add", "feature.txt"]);
            let commit = commit_args(&repo);
            let committed = enqueue("commit_changes", &commit).unwrap();
            let push = json!({ "path": repo.0, "request": { "action": "push" }, "__pushCommit": committed, "__dependsOn": [committed] });
            let pushed = enqueue("mutate", &push).unwrap();
            let saved = run(&committed, || execute("commit_changes", &commit), ok).unwrap();
            let head = saved["commitIntent"]["head"].as_str().unwrap();
            if move_head {
                repo.git(&["commit", "--allow-empty", "-m", "test: external change"]);
                assert!(run(&pushed, || execute("mutate", &push), ok).unwrap_err().contains("HEAD changed before push"));
                assert_ne!(repo.git(&["ls-remote", "origin", "refs/heads/published"]).split_whitespace().next(), Some(head));
            } else {
                let result = run(&pushed, || execute("mutate", &push), |value| outcome("mutate", &push, value)).unwrap();
                assert_eq!(result["outcome"], "completed", "{result}");
                assert_eq!(result["pushTarget"], json!({ "remote": "origin", "branch": "feature", "remoteBranch": "published" }));
                assert_eq!(repo.git(&["ls-remote", "origin", "refs/heads/published"]).split_whitespace().next(), Some(head));
            }
        }
        assert_eq!(repo.git(&["ls-remote", "origin", "refs/heads/main"]).split_whitespace().next(), Some(main.trim()));
    }

    #[test]
    fn stage_verification_is_scoped_and_does_not_fail_for_unrelated_conflicts() {
        let repo = Repo::new();
        for file in ["conflict.txt", "stage.txt", "untouched.txt"] { std::fs::write(repo.0.join(file), "base\n").unwrap(); }
        repo.git(&["add", "."]); repo.git(&["commit", "-m", "test: seed"]);
        repo.git(&["checkout", "-b", "other"]);
        std::fs::write(repo.0.join("conflict.txt"), "other\n").unwrap();
        repo.git(&["commit", "-am", "test: other"]);
        repo.git(&["checkout", "main"]);
        std::fs::write(repo.0.join("conflict.txt"), "main\n").unwrap();
        repo.git(&["commit", "-am", "test: main"]);
        assert!(awegit_git::perform(&repo.0, awegit_git::Mutation::Merge { name: "other".into(), no_ff: false, squash: false, autostash: false, summary: String::new(), description: String::new() }).is_err());
        for file in ["stage.txt", "untouched.txt"] { std::fs::write(repo.0.join(file), "new\n").unwrap(); }
        let args = json!({ "path": repo.0, "request": { "action": "stagePaths", "files": ["stage.txt"], "unstage": false } });
        let id = enqueue("mutate", &args).unwrap();
        let result = run(&id, || execute("mutate", &args), |value| outcome("mutate", &args, value)).unwrap();
        assert_eq!(snapshot(&id).unwrap().state, "completed", "{result}");
        assert!(result["staged"].as_array().unwrap().iter().any(|row| row["path"] == "stage.txt"));
        assert!(repo.git(&["ls-files", "-u"]).contains("conflict.txt"));
        repo.git(&["merge", "--abort"]);
        let id = enqueue("mutate", &args).unwrap();
        let result = run(&id, || execute("mutate", &args), |value| outcome("mutate", &args, value)).unwrap();
        assert_eq!(result["statusPaths"], json!(["stage.txt"]), "{result}");
        assert!(!result["unstaged"].as_array().unwrap().iter().any(|row| row["path"] == "untouched.txt"));
    }

    #[test]
    fn task_deltas_bound_targets_and_omit_unchanged_entries() {
        let repo = Repo::new();
        let files: Vec<_> = (0..10_000).map(|index| format!("file-{index}.txt")).collect();
        let args = json!({ "path": repo.0, "request": { "action": "stagePaths", "files": files } });
        let id = enqueue("mutate", &args).unwrap();
        let queued = snapshot(&id).unwrap();
        assert_eq!(queued.target_count, 10_000);
        assert_eq!(queued.targets.len(), 16);
        assert!(list_after(Some(queued.sequence)).is_empty());
        run(&id, || Ok(json!(true)), ok).unwrap();
        let changes = list_after(Some(queued.sequence));
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].id, id);
        assert_eq!(changes[0].state, "completed");
        assert!(serde_json::to_vec(&changes).unwrap().len() < 2048);
        assert!(list_after(Some(changes[0].sequence)).is_empty());
    }

    #[test]
    fn stale_conflict_editor_cannot_overwrite_an_external_file_edit() {
        let repo = Repo::new(); let file = repo.0.join("file.txt");
        std::fs::write(&file, "original\n").unwrap();
        let sides = super::super::conflict_sides(Some(repo.0.to_string_lossy().into()), "file.txt".into()).unwrap();
        std::fs::write(&file, "external\n").unwrap();
        let args = json!({ "path": repo.0, "request": { "action": "resolve", "file": "file.txt", "side": "text", "text": "from stale editor" },
            "__expectedConflict": { "file": "file.txt", "version": sides.version } });
        let id = enqueue("mutate", &args).unwrap();
        assert!(run(&id, || execute("mutate", &args), ok).unwrap_err().contains("Conflict content changed"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "external\n");
    }

    #[test]
    fn a_commit_does_not_follow_a_queued_checkout_into_another_branch() {
        let repo = Repo::new(); repo.git(&["commit", "--allow-empty", "-m", "test: base"]);
        let args = commit_args(&repo);
        let id = enqueue("commit_changes", &args).unwrap();
        repo.git(&["checkout", "-b", "other"]);
        assert!(run(&id, || execute("commit_changes", &args), ok).unwrap_err().contains("selected branch changed"));
        assert!(repo.git(&["log", "-1", "--format=%s"]).contains("test: base"));
    }

    #[test]
    fn tasks_preserve_write_order_without_blocking_another_repository() {
        let repo = Repo::new(); let other = Repo::new();
        let first = enqueue("mutate", &repo.args()).unwrap();
        let second = enqueue("mutate", &repo.args()).unwrap();
        let independent = enqueue("mutate", &other.args()).unwrap();
        let (started, ready) = mpsc::channel(); let (release, released) = mpsc::channel();
        let handle = std::thread::spawn(move || run(&first, || { started.send(()).unwrap(); released.recv().unwrap(); Ok(json!(1)) }, ok));
        ready.recv().unwrap();
        let (called, calls) = mpsc::channel();
        let next = std::thread::spawn(move || run(&second, || { called.send(()).unwrap(); Ok(json!(2)) }, ok));
        assert_eq!(run(&independent, || Ok(json!(3)), ok).unwrap(), json!(3));
        assert!(calls.try_recv().is_err());
        release.send(()).unwrap();
        assert_eq!(handle.join().unwrap().unwrap(), json!(1));
        assert_eq!(next.join().unwrap().unwrap(), json!(2));
    }

    #[test]
    fn failed_or_cancelled_dependencies_never_execute_the_following_write() {
        let repo = Repo::new();
        for cancel_first in [false, true] {
            let first = enqueue("mutate", &repo.args()).unwrap();
            let mut args = repo.args(); args["__dependsOn"] = json!([first]);
            let second = enqueue("mutate", &args).unwrap();
            if cancel_first { cancel(&first).unwrap(); }
            assert!(run::<Value>(&first, || Err("write failed".into()), ok).is_err());
            assert!(run(&second, || panic!("dependent write ran"), ok).is_err());
            assert_eq!(list().iter().find(|task| task.id == second).unwrap().state, "blocked");
        }
    }

    #[test]
    fn cancellation_before_delivery_and_during_execution_prevents_later_commands() {
        let repo = Repo::new(); let mut args = repo.args();
        let early = format!("early-{}", NEXT.fetch_add(1, Ordering::Relaxed));
        cancel(&early).unwrap(); args["__task"] = json!(early);
        let id = enqueue("mutate", &args).unwrap();
        assert!(run(&id, || panic!("cancelled task ran"), ok).is_err());
        let id = enqueue("mutate", &repo.args()).unwrap(); let stopped = id.clone();
        let (started, ready) = mpsc::channel(); let (resume, resumed) = mpsc::channel(); let path = repo.0.clone();
        let task = std::thread::spawn(move || run(&id, || {
            started.send(()).unwrap(); resumed.recv().unwrap();
            awegit_git::perform(&path, awegit_git::Mutation::CreateBranch { name: "must-not-exist".into(), start: None }).map_err(|error| error.to_string())?;
            Ok(json!(true))
        }, ok));
        ready.recv().unwrap(); cancel(&stopped).unwrap(); resume.send(()).unwrap();
        assert!(task.join().unwrap().is_err());
        assert_eq!(repo.git(&["branch", "--list", "must-not-exist"]), "");
    }

    #[test]
    fn linked_worktrees_share_the_write_lane() {
        let repo = Repo::new(); repo.git(&["commit", "--allow-empty", "-m", "test: initial"]);
        let worktree = repo.0.join("linked"); repo.git(&["worktree", "add", "-b", "side", worktree.to_str().unwrap()]);
        assert_eq!(awegit_git::common_repo_key(&repo.0), awegit_git::common_repo_key(&worktree));
        let first = enqueue("mutate", &repo.args()).unwrap();
        let second = enqueue("mutate", &json!({ "path": worktree, "request": { "action": "tag" } })).unwrap();
        let registry = registry().0.lock().unwrap();
        assert_eq!(registry.tasks[&first].lane, registry.tasks[&second].lane);
        drop(registry); cancel(&first).unwrap(); cancel(&second).unwrap();
        let _ = run(&first, || Ok(Value::Null), ok); let _ = run(&second, || Ok(Value::Null), ok);
    }

    #[test]
    fn stale_patch_is_rejected_without_changing_the_index() {
        let repo = Repo::new(); let file = repo.0.join("file.txt");
        std::fs::write(&file, "base\n").unwrap(); repo.git(&["add", "file.txt"]); repo.git(&["commit", "-m", "test: initial"]);
        std::fs::write(&file, "first\n").unwrap();
        let report = super::super::file_diff(Some(repo.0.to_string_lossy().into()), "file.txt".into(), false, None, None, None).unwrap();
        let args = json!({ "path": repo.0, "request": { "action": "stageHunk", "file": "file.txt", "index": 0 },
            "__expectedDiff": { "file": "file.txt", "staged": false, "version": report.version } });
        std::fs::write(&file, "second\n").unwrap();
        let id = enqueue("mutate", &args).unwrap();
        let result = run(&id, || execute("mutate", &args), |value| outcome("mutate", &args, value));
        assert!(result.unwrap_err().contains("diff has changed"));
        assert_eq!(repo.git(&["diff", "--cached", "--name-only"]), "");
    }

    #[test]
    fn metadata_tasks_do_not_require_a_workspace_snapshot() {
        let repo = Repo::new(); repo.git(&["commit", "--allow-empty", "-m", "test: initial"]);
        let args = json!({ "path": repo.0, "request": { "action": "createBranch", "name": "created" } });
        let id = enqueue("mutate", &args).unwrap();
        let value = run(&id, || execute("mutate", &args), |value| outcome("mutate", &args, value)).unwrap();
        assert!(value.get("staged").is_none());
        assert_eq!(value["refreshAreas"], json!(["refs"]));
        assert_eq!(value["outcome"], "completed");
    }

    #[test]
    fn initializing_a_repository_does_not_require_an_open_repository() {
        let repo = Repo::new(); let target = repo.0.join("new").join("repository");
        let args = json!({ "request": { "action": "init", "destination": target } });
        let id = enqueue("mutate", &args).unwrap();
        let value = run(&id, || execute("mutate", &args), |value| outcome("mutate", &args, value)).unwrap();
        assert_eq!(value["outcome"], "completed");
        assert!(target.join(".git").is_dir());
        assert!(repo.git(&["status", "--porcelain"]).contains("new/"));
    }
}
