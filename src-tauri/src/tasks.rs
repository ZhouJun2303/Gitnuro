//! Process-wide task identities, dependency checks and cancellation, shared by all windows.
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Instant;

use serde::Serialize;
use serde_json::Value;
use tauri::Emitter;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub path: String,
    pub action: String,
    pub label: String,
    pub preview: HashMap<String, String>,
    pub targets: Vec<String>,
    pub state: String,
    pub phase: String,
    pub depends_on: Vec<String>,
    pub revision: u64,
    pub queued_ms: u64,
    pub execute_ms: u64,
    pub verify_ms: u64,
}

struct Task {
    snapshot: Snapshot,
    client: String,
    lane: String,
    queued: Instant,
    started: Option<Instant>,
    verifying: Option<Instant>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
struct Registry {
    tasks: HashMap<String, Task>,
    order: VecDeque<String>,
    lanes: HashMap<String, VecDeque<String>>,
    cancelled_before_enqueue: HashSet<String>,
}

fn registry() -> &'static (Mutex<Registry>, Condvar) {
    static REGISTRY: OnceLock<(Mutex<Registry>, Condvar)> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
thread_local! { static CURRENT: RefCell<Option<String>> = const { RefCell::new(None) }; }

pub fn install(app: tauri::AppHandle) { let _ = APP.set(app); }
fn emit(snapshot: &Snapshot) { if let Some(app) = APP.get() { let _ = app.emit("operation-task", snapshot); } }
pub fn terminal(state: &str) -> bool { matches!(state, "completed" | "conflict" | "failed" | "cancelled" | "blocked") }

fn dependency_reaches(registry: &Registry, task: &str, prerequisite: &str) -> bool {
    let mut pending = vec![task];
    let mut visited = HashSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) { continue; }
        if let Some(task) = registry.tasks.get(id) {
            for dependency in &task.snapshot.depends_on {
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
    let mut pending: Vec<_> = queue.iter().cloned().collect();
    let mut ordered = VecDeque::new();
    while !pending.is_empty() {
        let next = pending.iter().position(|id| registry.tasks.get(id).is_none_or(|task|
            task.snapshot.depends_on.iter().all(|dependency| !pending.contains(dependency))));
        let Some(index) = next else { ordered.extend(pending); break; };
        ordered.push_back(pending.remove(index));
    }
    registry.lanes.insert(lane.to_string(), ordered);
}

pub fn handles(command: &str) -> bool {
    matches!(command, "mutate" | "stage_all" | "unstage_all" | "stage_path" | "unstage_path" | "commit_changes"
        | "save_hidden_refs" | "save_expanded_groups" | "save_sign_off" | "set_repo_author" | "save_remote_account" | "approve_credential" | "remove_index_lock" | "remove_stale_lock")
}

pub fn enqueue(command: &str, args: &Value) -> Result<String, String> {
    let path = args.get("path").and_then(Value::as_str).unwrap_or("");
    let request = args.get("request").unwrap_or(args);
    let target = match request.get("destination").and_then(Value::as_str).filter(|_| matches!(request.get("action").and_then(Value::as_str), Some("clone" | "init"))) {
        Some(destination) => PathBuf::from(destination),
        None => super::repo_from((!path.is_empty()).then(|| path.to_string()))?,
    };
    super::lab::ensure_lab_path(&target)?;
    let lane = awegit_git::common_repo_key(&target);
    let id = args.get("__task").and_then(Value::as_str).map(str::to_string)
        .unwrap_or_else(|| format!("task-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let action = request.get("action").and_then(Value::as_str).unwrap_or(command).to_string();
    let client = args.get("__client").and_then(Value::as_str).unwrap_or("").to_string();
    let mut depends_on: Vec<String> = args.get("__dependsOn").and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let mut targets: Vec<String> = request.get("files").and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    if let Some(selection) = args.get("__selection").and_then(Value::as_array) { targets.extend(selection.iter().filter_map(Value::as_str).map(str::to_string)); }
    if args.get("request").is_some() { if let Some(path) = request.get("path").and_then(Value::as_str) { targets.push(path.to_string()); } }
    for key in ["file", "name", "to", "branch", "rev", "onto", "destination", "remote"] {
        if let Some(value) = request.get(key).and_then(Value::as_str).filter(|value| !value.is_empty()) { targets.push(value.chars().take(1024).collect()); }
    }
    targets.sort(); targets.dedup();
    let label = super::perf::redact(&args.get("__taskLabel").and_then(Value::as_str).unwrap_or("").chars().take(1024).collect::<String>());
    let mut preview: HashMap<String, String> = ["name", "to", "branch", "rev", "start", "remote", "path"].into_iter().filter_map(|key| request.get(key).and_then(Value::as_str).map(|value| (key.to_string(), value.chars().take(1024).collect()))).collect();
    if action == "stashRename" { if let Some(message) = request.get("message").and_then(Value::as_str) { preview.insert("message".into(), message.chars().take(1024).collect()); } }
    let mut snapshot = Snapshot { id: id.clone(), path: target.to_string_lossy().into_owned(), action, label, preview, targets,
        state: "queued".into(), phase: String::new(), depends_on, revision: super::changes::revision(&target), queued_ms: 0, execute_ms: 0, verify_ms: 0 };
    {
        let mut registry = registry().0.lock().unwrap_or_else(|error| error.into_inner());
        if registry.tasks.contains_key(&id) { return Err("Duplicate task identity".into()); }
        // Retain dependency outcomes while any queued task can still need them.
        while registry.order.len() > 256 {
            let old = registry.order.iter().find(|id| registry.tasks.get(*id).is_some_and(|task| terminal(&task.snapshot.state))
                && !registry.tasks.values().any(|task| !terminal(&task.snapshot.state) && task.snapshot.depends_on.contains(id))).cloned();
            let Some(old) = old else { break };
            registry.order.retain(|id| id != &old); registry.tasks.remove(&old);
        }
        // Windows and linked worktrees share dependency outcomes as well as the lock.
        if snapshot.depends_on.is_empty() && !matches!(snapshot.action.as_str(), "resolve" | "resolvePaths" | "abort" | "continue" | "skip") {
            if let Some(previous) = registry.lanes.get(&lane).and_then(|queue| queue.back()).and_then(|id| registry.tasks.get(id)).filter(|task| !terminal(&task.snapshot.state) && !dependency_reaches(&registry, &task.snapshot.id, &id) && (client.is_empty() || task.client != client)) {
                depends_on = vec![previous.snapshot.id.clone()]; snapshot.depends_on = depends_on;
            }
        }
        registry.order.push_back(id.clone());
        registry.lanes.entry(lane.clone()).or_default().push_back(id.clone());
        let cancelled = registry.cancelled_before_enqueue.remove(&id);
        registry.tasks.insert(id.clone(), Task { snapshot: snapshot.clone(), client, lane: lane.clone(), queued: Instant::now(), started: None, verifying: None, cancelled: Arc::new(AtomicBool::new(cancelled)) });
        order_lane(&mut registry, &lane);
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
            let failed = dependency_reaches(&registry, id, id) || task.snapshot.depends_on.iter().any(|dependency| registry.tasks.get(dependency).is_some_and(|task| terminal(&task.snapshot.state) && task.snapshot.state != "completed")
                || (!registry.tasks.contains_key(dependency) && task.queued.elapsed().as_secs() >= 10));
            if failed { drop(registry); finish(id, "blocked"); return Err("awegit:cancelled:Task dependency did not complete successfully".into()); }
            let waiting = task.snapshot.depends_on.iter().any(|dependency| registry.tasks.get(dependency).is_none_or(|task| !terminal(&task.snapshot.state)));
            let first = registry.lanes.get(&task.lane).and_then(|lane| lane.front()).is_some_and(|first| first == id);
            if first && !waiting { break; }
            registry = ready.wait_timeout(registry, std::time::Duration::from_millis(100)).unwrap_or_else(|error| error.into_inner()).0;
        }
        let task = registry.tasks.get_mut(id).unwrap();
        task.started = Some(Instant::now()); task.snapshot.queued_ms = task.queued.elapsed().as_millis() as u64; task.snapshot.state = "running".into();
        let token = task.cancelled.clone(); let snapshot = task.snapshot.clone();
        drop(registry); emit(&snapshot); token
    };
    struct Scope(Option<String>);
    impl Drop for Scope { fn drop(&mut self) { CURRENT.with(|current| *current.borrow_mut() = self.0.take()); } }
    let _scope = Scope(CURRENT.with(|current| current.replace(Some(id.to_string()))));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| awegit_git::with_cancellation(token.clone(), body)))
        .unwrap_or_else(|_| Err("Task stopped unexpectedly".into()));
    let state = if token.load(Ordering::Acquire) { "cancelled" } else { result.as_ref().map(outcome).unwrap_or("failed") };
    finish(id, state);
    if token.load(Ordering::Acquire) { Err("awegit:cancelled:".into()) } else { result }
}

fn finish(id: &str, state: &str) {
    let (lock, ready) = registry();
    let mut registry = lock.lock().unwrap_or_else(|error| error.into_inner());
    let Some(task) = registry.tasks.get_mut(id) else { return };
    task.snapshot.state = state.to_string();
    if task.started.is_none() { task.snapshot.queued_ms = task.queued.elapsed().as_millis() as u64; }
    task.snapshot.revision = super::changes::revision(Path::new(&task.snapshot.path));
    if let Some(start) = task.started { task.snapshot.execute_ms = task.verifying.unwrap_or_else(Instant::now).duration_since(start).as_millis() as u64; }
    if let Some(start) = task.verifying { task.snapshot.verify_ms = start.elapsed().as_millis() as u64; }
    let lane = task.lane.clone(); let snapshot = task.snapshot.clone();
    if let Some(queue) = registry.lanes.get_mut(&lane) { queue.retain(|task| task != id); if queue.is_empty() { registry.lanes.remove(&lane); } }
    ready.notify_all(); drop(registry); emit(&snapshot);
}

pub fn verifying() {
    let Some(id) = CURRENT.with(|current| current.borrow().clone()) else { return };
    let mut registry = registry().0.lock().unwrap_or_else(|error| error.into_inner());
    let Some(task) = registry.tasks.get_mut(&id) else { return };
    if task.verifying.is_none() { task.verifying = Some(Instant::now()); }
    task.snapshot.state = if task.cancelled.load(Ordering::Acquire) { "cancelling" } else { "verifying" }.into(); let snapshot = task.snapshot.clone(); drop(registry); emit(&snapshot);
}

pub fn current_id() -> Option<String> { CURRENT.with(|current| current.borrow().clone()) }

pub fn phase(id: &str, phase: &str) {
    let mut registry = registry().0.lock().unwrap_or_else(|error| error.into_inner());
    let Some(task) = registry.tasks.get_mut(id) else { return };
    if terminal(&task.snapshot.state) || task.snapshot.phase == phase { return; }
    task.snapshot.phase = phase.into();
    let snapshot = task.snapshot.clone(); drop(registry); emit(&snapshot);
}

pub fn list() -> Vec<Snapshot> {
    let registry = registry().0.lock().unwrap_or_else(|error| error.into_inner());
    registry.order.iter().filter_map(|id| registry.tasks.get(id).map(|task| task.snapshot.clone())).collect()
}

/// Mutable reads wait for writes already submitted, without delaying later writes.
pub fn wait_for_writes(repo: &Path) {
    if CURRENT.with(|current| current.borrow().is_some()) { return; }
    let lane = awegit_git::common_repo_key(repo);
    let (lock, ready) = registry();
    let mut registry = lock.lock().unwrap_or_else(|error| error.into_inner());
    let pending: Vec<_> = registry.tasks.values().filter(|task| task.lane == lane && !terminal(&task.snapshot.state)).map(|task| task.snapshot.id.clone()).collect();
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
    task.snapshot.state = if running { "cancelling" } else { "cancelled" }.into();
    let snapshot = task.snapshot.clone();
    ready.notify_all(); drop(registry); emit(&snapshot);
    if running { awegit_git::cancel_repo(Path::new(&snapshot.path)); }
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

/// Keep the original IPC names so desktop and lab exercise exactly the same commands.
pub fn execute(command: &str, args: &Value) -> Result<Value, String> {
    if command == "commit_changes" || matches!(args.pointer("/request/action").and_then(Value::as_str), Some("push" | "pull")) {
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
    match command {
        "mutate" => value(super::mutate(path()?, arg(args, "request")?, arg(args, "retryStash")?)),
        "stage_all" | "unstage_all" if args.get("__selection").is_some() => value(super::mutate(path()?, awegit_git::Mutation::StagePaths {
            files: arg(args, "__selection")?, unstage: command == "unstage_all",
        }, None)),
        "stage_all" => value(super::stage_all(path()?)),
        "unstage_all" => value(super::unstage_all(path()?)),
        "stage_path" => value(super::stage_path(path()?, arg(args, "file")?, arg(args, "files")?, arg(args, "filtered")?, arg(args, "incremental")?)),
        "unstage_path" => value(super::unstage_path(path()?, arg(args, "file")?, arg(args, "files")?, arg(args, "filtered")?, arg(args, "incremental")?)),
        "commit_changes" => value(super::commit_changes(path()?, arg(args, "summary")?, arg(args, "description")?, arg(args, "amend")?, arg(args, "signOff")?, arg(args, "skipHooks")?)),
        "save_hidden_refs" => value(super::save_hidden_refs(path()?, arg(args, "names")?)),
        "save_expanded_groups" => value(super::save_expanded_groups(path()?, arg(args, "names")?)),
        "save_sign_off" => value(super::save_sign_off(path()?, arg(args, "enabled")?, arg(args, "format")?)),
        "set_repo_author" => value(super::set_repo_author(path()?, arg(args, "name")?, arg(args, "email")?)),
        "save_remote_account" => value(super::save_remote_account(path()?, arg(args, "remote")?, arg(args, "accountId")?)),
        "approve_credential" => value(super::approve_credential(path()?, arg(args, "protocol")?, arg(args, "host")?, arg(args, "username")?, arg(args, "password")?)),
        "remove_index_lock" => value(super::remove_index_lock(path()?)),
        "remove_stale_lock" => value(super::remove_stale_lock(path()?, arg(args, "lock")?)),
        _ => Err(format!("Unknown task command: {command}")),
    }
}

pub fn outcome(command: &str, args: &Value, value: &Value) -> &'static str {
    let action = args.pointer("/request/action").and_then(Value::as_str).unwrap_or(command);
    let conflict = value.get("outcome").and_then(Value::as_str) == Some("conflict");
    let failed = value.get("outcome").and_then(Value::as_str) == Some("failed") || value.get("error").and_then(Value::as_str).is_some_and(|error| !error.is_empty());
    if failed { return if conflict { "conflict" } else { "failed" }; }
    if matches!(action, "stagePaths" | "stageHunk" | "stageLine" | "stageLines" | "resolve" | "resolvePaths") { return "completed"; }
    let unmerged = ["staged", "unstaged"].iter().any(|key| value.get(key).and_then(Value::as_array).is_some_and(|files| files.iter().any(|file| file.get("kind").and_then(Value::as_str) == Some("conflict"))));
    if conflict || unmerged { "conflict" } else { "completed" }
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
    fn another_window_observes_failure_dependencies_and_recovery_can_still_run() {
        let repo = Repo::new();
        let first = enqueue("mutate", &repo.args()).unwrap();
        let second = enqueue("mutate", &repo.args()).unwrap();
        assert!(run::<Value>(&first, || Err("conflict".into()), ok).is_err());
        assert!(run(&second, || panic!("write ignored previous failure"), ok).is_err());
        let mut args = repo.args(); args["request"]["action"] = json!("abort");
        let recovery = enqueue("mutate", &args).unwrap();
        assert!(run(&recovery, || Ok(json!(true)), ok).is_ok());
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
        let args = json!({ "path": repo.0, "__branch": "main", "summary": "test: wrong branch" });
        repo.git(&["checkout", "-b", "other"]);
        let id = enqueue("commit_changes", &args).unwrap();
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
