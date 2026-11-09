mod ai;
mod forge;
mod http;
mod lab;
mod perf;
mod secrets;
mod settings;
mod tools;
mod updates;

pub use lab::run_lab;

use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{Emitter, Manager};

static FAKE_IGNORE: AtomicBool = AtomicBool::new(true);
static FAKE_IGNORE_KNOWN: AtomicBool = AtomicBool::new(false);
static SETTINGS_LANE: OnceLock<Sender<Job>> = OnceLock::new();
static WATCH_RECOVERIES: AtomicU64 = AtomicU64::new(0);

const GATE_KEEP: usize = 24;
const GATE_IDLE_SECS: u64 = 15 * 60;

struct GateSlot {
    gate: Arc<RepoGate>,
    used: Instant,
}

fn repo_gates() -> &'static Mutex<HashMap<String, GateSlot>> {
    static GATES: OnceLock<Mutex<HashMap<String, GateSlot>>> = OnceLock::new();
    GATES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Drop an idle gate only when the map is over the cap and nothing else holds it.
fn idle_for_reap(age_secs: u64, strong: usize, len: usize) -> bool {
    len > GATE_KEEP && age_secs >= GATE_IDLE_SECS && strong == 1
}

struct CachedFacts {
    stamp: String,
    full: bool,
    facts: awegit_git::RepoFacts,
}

fn facts_cache() -> &'static Mutex<HashMap<String, CachedFacts>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedFacts>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

const WATCH_QUIET: Duration = Duration::from_millis(300);
const WATCH_MAX_WAIT: Duration = Duration::from_secs(2);
const WATCH_BATCH: usize = 2_000;

type Job = Box<dyn FnOnce() + Send>;

struct WatchSlot {
    watcher: Mutex<Option<RecommendedWatcher>>,
}

struct RepoWatch(Mutex<Option<Arc<WatchSlot>>>);

/// What a batch of file events changed. `refs` means branches, `HEAD`, or an operation in progress.
#[derive(Clone, serde::Serialize)]
struct RepoChange {
    refs: bool,
}

/// Transfer percent for one repository. The shell shows it only while that repository is open.
#[derive(Clone, serde::Serialize)]
struct GitProgress {
    path: String,
    percent: u8,
}

/// One repository's writes run in the order they were asked for. Other repositories do not wait.
struct RepoGate {
    inner: Mutex<RepoTicket>,
    cv: Condvar,
}

struct RepoTicket {
    next: u64,
    serving: u64,
}

struct RepoTurn {
    gate: Arc<RepoGate>,
}

impl Drop for RepoTurn {
    fn drop(&mut self) {
        let mut guard = self.gate.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard.serving = guard.serving.saturating_add(1);
        self.gate.cv.notify_all();
    }
}

fn repo_turn(repo: &Path) -> RepoTurn {
    let key = awegit_git::repo_key(repo);
    let gate = {
        let mut map = repo_gates().lock().unwrap_or_else(|err| err.into_inner());
        let now = Instant::now();
        let slot = map.entry(key).or_insert_with(|| GateSlot {
            gate: Arc::new(RepoGate {
                inner: Mutex::new(RepoTicket { next: 0, serving: 0 }),
                cv: Condvar::new(),
            }),
            used: now,
        });
        slot.used = now;
        let gate = Arc::clone(&slot.gate);
        let len = map.len();
        if len > GATE_KEEP {
            map.retain(|_, slot| !idle_for_reap(slot.used.elapsed().as_secs(), Arc::strong_count(&slot.gate), len));
        }
        gate
    };
    let ticket = {
        let mut guard = gate.inner.lock().unwrap_or_else(|err| err.into_inner());
        let ticket = guard.next;
        guard.next = guard.next.saturating_add(1);
        ticket
    };
    let mut guard = gate.inner.lock().unwrap_or_else(|err| err.into_inner());
    while guard.serving != ticket {
        guard = gate.cv.wait(guard).unwrap_or_else(|err| err.into_inner());
    }
    drop(guard);
    RepoTurn { gate }
}

fn report_path(given: Option<String>, repo: &Path) -> PathBuf {
    match given {
        Some(path) if !path.trim().is_empty() => PathBuf::from(path),
        _ => repo.to_path_buf(),
    }
}

/// Commands never run on the main thread, which also drives the window and webview input.
/// Settings keep their call order. Writes run together, and each repository keeps its own order.
fn dispatch(command: &str, job: Job) {
    let lane = match command {
        "load_settings" | "save_settings" | "settings_path" | "import_fork_settings" | "watch_repository" | "set_passphrase" | "read_ssh_config" | "write_ssh_config"
        | "log_client" => Some((&SETTINGS_LANE, "awegit-settings")),
        _ => None,
    };
    let Some((slot, name)) = lane else {
        tauri::async_runtime::spawn_blocking(job);
        return;
    };
    let sender = slot.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<Job>();
        let _ = std::thread::Builder::new().name(name.into()).spawn(move || {
            for job in receiver {
                job();
            }
        });
        sender
    });
    if let Err(mpsc::SendError(job)) = sender.send(job) {
        tauri::async_runtime::spawn_blocking(job);
    }
}

fn git_cmd(dir: &Path) -> Option<PathBuf> {
    let cmd = dir.join("git").join("cmd");
    if cmd.join("git.exe").is_file() || cmd.join("git").is_file() {
        Some(cmd)
    } else {
        None
    }
}

fn prefer_bundled_git() {
    let Ok(exe) = std::env::current_exe() else { return };
    let Some(parent) = exe.parent() else { return };
    if let Some(cmd) = git_cmd(parent).or_else(|| git_cmd(&parent.join("resources"))) {
        awegit_git::use_bundled_git(&cmd);
    }
}

fn repo_from(path: Option<String>) -> Result<PathBuf, String> {
    let start = match path {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => {
            if lab::lab_guarded() {
                return Err("lab command needs a repository path".into());
            }
            std::env::current_dir().map_err(|error| error.to_string())?
        }
    };
    if !start.exists() {
        return Err(format!("could not open repository at {}: missing", start.display()));
    }
    let found = awegit_git::discover(&start).unwrap_or_else(|| start.clone());
    lab::ensure_lab_path(&start)?;
    lab::ensure_lab_path(&found)?;
    Ok(found)
}

fn read_status(repo: &Path) -> Result<awegit_git::StatusSnapshot, String> {
    awegit_git::status_with(repo, use_fakeignore()).map_err(|error| error.to_string())
}

fn use_fakeignore() -> bool {
    if FAKE_IGNORE_KNOWN.load(Ordering::Acquire) {
        return FAKE_IGNORE.load(Ordering::Relaxed);
    }
    let enabled = settings::load().map(|values| values.fake_ignore).unwrap_or(true);
    remember_fakeignore(enabled);
    enabled
}

fn remember_fakeignore(enabled: bool) {
    FAKE_IGNORE.store(enabled, Ordering::Relaxed);
    FAKE_IGNORE_KNOWN.store(true, Ordering::Release);
}

fn net_options() -> http::NetOptions {
    settings::load().map(|values| http::NetOptions::from_settings(&values)).unwrap_or_else(|_| http::NetOptions::direct())
}

fn apply_session() {
    let values = settings::load().unwrap_or_default();
    awegit_git::configure(awegit_git::Session {
        proxy: http::compose_proxy(&values),
        author_name: values.author_name.trim().to_string(),
        author_email: values.author_email.trim().to_string(),
        ssl_verify: values.ssl_verify,
        ssl_ca: values.ssl_ca_file.trim().to_string(),
        sign_commits: values.sign_commits,
        ask_user: String::new(),
    });
}

fn write_then_status(path: Option<String>, write: impl FnOnce(&Path) -> Result<(), awegit_git::Error>) -> Result<awegit_git::StatusSnapshot, String> {
    let given = path.clone();
    let repo = repo_from(path)?;
    let _turn = repo_turn(&repo);
    apply_session();
    let report = report_path(given, &repo);
    awegit_git::with_operation_repo(&report, || write(&repo)).map_err(|error| error.to_string())?;
    read_status(&repo)
}

#[tauri::command]
fn workspace_status(path: Option<String>) -> Result<awegit_git::StatusSnapshot, String> {
    read_status(&repo_from(path)?)
}

#[tauri::command]
fn stage_all(path: Option<String>) -> Result<awegit_git::StatusSnapshot, String> {
    let peel = use_fakeignore();
    write_then_status(path, |repo| {
        awegit_git::stage_all(repo)?;
        if peel {
            awegit_git::unstage_fakeignored(repo)?;
        }
        Ok(())
    })
}

#[tauri::command]
fn unstage_all(path: Option<String>) -> Result<awegit_git::StatusSnapshot, String> {
    write_then_status(path, awegit_git::unstage_all)
}

#[tauri::command]
fn stage_path(path: Option<String>, file: String) -> Result<awegit_git::StatusSnapshot, String> {
    write_then_status(path, |repo| awegit_git::stage_paths(repo, &[file]))
}

#[tauri::command]
fn unstage_path(path: Option<String>, file: String) -> Result<awegit_git::StatusSnapshot, String> {
    write_then_status(path, |repo| awegit_git::unstage_paths(repo, &[file]))
}

#[tauri::command]
fn commit_changes(
    path: Option<String>,
    summary: String,
    description: String,
    amend: bool,
    sign_off: bool,
    skip_hooks: Option<bool>,
) -> Result<CommitReport, String> {
    let repo = repo_from(path.clone())?;
    let sign_off_format = if sign_off {
        let local = awegit_git::repo_sign_off_format(&repo);
        if local.trim().is_empty() {
            settings::load().map(|values| values.sign_off_format).unwrap_or_default()
        } else {
            local
        }
    } else {
        String::new()
    };
    let peel = use_fakeignore();
    let skip_hooks = skip_hooks.unwrap_or(false);
    let (result, log) = awegit_git::capture_output(|| {
        write_then_status(path, |repo| {
            if peel {
                awegit_git::unstage_fakeignored(repo)?;
            }
            awegit_git::commit(
                repo,
                awegit_git::CommitRequest {
                    summary,
                    description,
                    amend,
                    sign_off,
                    sign_off_format,
                    skip_hooks,
                    convention: !skip_hooks,
                },
            )
        })
    });
    Ok(CommitReport { status: result?, log })
}

#[tauri::command]
fn file_diff(path: Option<String>, file: String, staged: bool, unified: Option<u32>, ignore_space: Option<bool>, load: Option<bool>) -> Result<awegit_git::FileDiff, String> {
    let repo = repo_from(path)?;
    awegit_git::file_diff_with(&repo, &file, staged, unified.unwrap_or(3), ignore_space.unwrap_or(false), load.unwrap_or(false))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn commit_log(
    path: Option<String>,
    limit: Option<usize>,
    all: Option<bool>,
    order: Option<String>,
    revs: Option<Vec<String>>,
) -> Result<Vec<awegit_git::CommitRow>, String> {
    let repo = repo_from(path)?;
    let limit = limit.unwrap_or(500);
    let topo = order.as_deref() == Some("topo");
    if let Some(revs) = revs.as_ref().filter(|items| !items.is_empty()) {
        return awegit_git::commits_from(&repo, limit, revs, topo).map_err(|error| error.to_string());
    }
    let rows = if all.unwrap_or(true) {
        awegit_git::commit_log_sorted(&repo, limit, topo)
    } else {
        awegit_git::branch_commits_sorted(&repo, limit, topo)
    };
    rows.map_err(|error| error.to_string())
}

#[tauri::command]
fn commit_detail(path: Option<String>, id: String) -> Result<awegit_git::CommitDetail, String> {
    let repo = repo_from(path)?;
    awegit_git::commit_detail(&repo, &id).map_err(|error| error.to_string())
}

#[tauri::command]
fn locate_ref(path: Option<String>, name: String, order: Option<String>) -> Result<awegit_git::LocatedRef, String> {
    let repo = repo_from(path)?;
    let topo = order.as_deref() == Some("topo");
    awegit_git::locate_ref(&repo, &name, topo).map_err(|error| error.to_string())
}

#[tauri::command]
fn repository_refs(path: Option<String>) -> Result<awegit_git::RefSnapshot, String> {
    let repo = repo_from(path)?;
    awegit_git::repository_refs(&repo).map_err(|error| error.to_string())
}

#[tauri::command]
fn commit_files(path: Option<String>, id: String) -> Result<Vec<awegit_git::FileChange>, String> {
    let repo = repo_from(path)?;
    awegit_git::commit_files(&repo, &id).map_err(|error| error.to_string())
}

#[tauri::command]
fn show_commit_file(path: Option<String>, id: String, file: String, unified: Option<u32>) -> Result<awegit_git::FileDiff, String> {
    let repo = repo_from(path)?;
    awegit_git::show_commit_file_with(&repo, &id, &file, unified.unwrap_or(3)).map_err(|error| error.to_string())
}

#[tauri::command]
fn in_progress(path: Option<String>) -> Result<Option<awegit_git::InProgress>, String> {
    let repo = repo_from(path)?;
    awegit_git::in_progress(&repo).map_err(|error| error.to_string())
}

#[tauri::command]
fn file_history(path: Option<String>, file: String, limit: Option<usize>) -> Result<Vec<awegit_git::CommitRow>, String> {
    let repo = repo_from(path)?;
    awegit_git::file_history(&repo, &file, limit.unwrap_or(200)).map_err(|error| error.to_string())
}

#[tauri::command]
fn blame_file(path: Option<String>, file: String) -> Result<Vec<awegit_git::BlameLine>, String> {
    let repo = repo_from(path)?;
    awegit_git::blame_file(&repo, &file).map_err(|error| error.to_string())
}

#[derive(serde::Serialize)]
struct MutationReport {
    #[serde(flatten)]
    status: awegit_git::StatusSnapshot,
    #[serde(skip_serializing_if = "String::is_empty")]
    output: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    log: String,
}

#[derive(serde::Serialize)]
struct CommitReport {
    #[serde(flatten)]
    status: awegit_git::StatusSnapshot,
    #[serde(skip_serializing_if = "String::is_empty")]
    log: String,
}

#[tauri::command]
fn mutate(path: Option<String>, request: awegit_git::Mutation) -> Result<MutationReport, String> {
    let given = path.clone();
    let repo = repo_from(path)?;
    let next = match &request {
        awegit_git::Mutation::Clone { destination, .. } | awegit_git::Mutation::Init { destination } => {
            PathBuf::from(destination)
        }
        _ => repo.clone(),
    };
    let _turn = repo_turn(&next);
    apply_session();
    record_action(&request);
    let report = report_path(given, &repo);
    let (result, log) = awegit_git::capture_output(|| awegit_git::with_operation_repo(&report, || awegit_git::perform(&repo, request)));
    let output = result.map_err(|error| error.to_string())?;
    Ok(MutationReport {
        status: read_status(&next)?,
        output,
        log,
    })
}

fn record_action(request: &awegit_git::Mutation) {
    let Ok(values) = settings::load() else { return };
    let directory = values.log_directory.trim();
    if directory.is_empty() {
        return;
    }
    let label = action_label(request);
    let detail = action_detail(request);
    let _ = std::fs::create_dir_all(directory);
    let path = PathBuf::from(directory).join("awegit.log");
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let line = if detail.is_empty() {
        format!("{unix}\t{label}\n")
    } else {
        format!("{unix}\t{label}\t{detail}\n")
    };
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = file.write_all(line.as_bytes());
    }
}

fn action_label(request: &awegit_git::Mutation) -> &'static str {
    match request {
        awegit_git::Mutation::Fetch { .. } => "fetch",
        awegit_git::Mutation::Pull { .. } => "pull",
        awegit_git::Mutation::Push { .. } => "push",
        awegit_git::Mutation::Stash { .. } => "stash",
        awegit_git::Mutation::StashPop { .. } => "stashPop",
        awegit_git::Mutation::StashApply { .. } => "stashApply",
        awegit_git::Mutation::StashDrop { .. } => "stashDrop",
        awegit_git::Mutation::StashRename { .. } => "stashRename",
        awegit_git::Mutation::StashBranch { .. } => "stashBranch",
        awegit_git::Mutation::StashPatch { .. } => "stashPatch",
        awegit_git::Mutation::StashPaths { .. } => "stashPaths",
        awegit_git::Mutation::Checkout { .. } => "checkout",
        awegit_git::Mutation::CreateBranch { .. } => "createBranch",
        awegit_git::Mutation::DeleteBranch { .. } => "deleteBranch",
        awegit_git::Mutation::Merge { .. } => "merge",
        awegit_git::Mutation::Rebase { .. } => "rebase",
        awegit_git::Mutation::RebaseInteractive { .. } => "rebaseInteractive",
        awegit_git::Mutation::Abort => "abort",
        awegit_git::Mutation::Continue { .. } => "continue",
        awegit_git::Mutation::Skip => "skip",
        awegit_git::Mutation::Reset { .. } => "reset",
        awegit_git::Mutation::CherryPick { .. } => "cherryPick",
        awegit_git::Mutation::Revert { .. } => "revert",
        awegit_git::Mutation::Reword { .. } => "reword",
        awegit_git::Mutation::Tag { .. } => "tag",
        awegit_git::Mutation::DeleteTag { .. } => "deleteTag",
        awegit_git::Mutation::DeleteRemoteTag { .. } => "deleteRemoteTag",
        awegit_git::Mutation::RestoreFile { .. } => "restoreFile",
        awegit_git::Mutation::Bisect { .. } => "bisect",
        awegit_git::Mutation::WriteGitignore { .. } => "writeGitignore",
        awegit_git::Mutation::AppendIgnore { .. } => "appendIgnore",
        awegit_git::Mutation::WriteMailmap { .. } => "writeMailmap",
        awegit_git::Mutation::StageHunk { .. } => "stageHunk",
        awegit_git::Mutation::StageLine { .. } => "stageLine",
        awegit_git::Mutation::StageLines { .. } => "stageLines",
        awegit_git::Mutation::StagePaths { .. } => "stagePaths",
        awegit_git::Mutation::DiscardHunk { .. } => "discardHunk",
        awegit_git::Mutation::DiscardLine { .. } => "discardLine",
        awegit_git::Mutation::DiscardLines { .. } => "discardLines",
        awegit_git::Mutation::Resolve { .. } => "resolve",
        awegit_git::Mutation::ResolvePaths { .. } => "resolvePaths",
        awegit_git::Mutation::RenameBranch { .. } => "renameBranch",
        awegit_git::Mutation::SetUpstream { .. } => "setUpstream",
        awegit_git::Mutation::DeleteRemoteBranch { .. } => "deleteRemoteBranch",
        awegit_git::Mutation::CheckoutRemote { .. } => "checkoutRemote",
        awegit_git::Mutation::PushRef { .. } => "pushRef",
        awegit_git::Mutation::PullRef { .. } => "pullRef",
        awegit_git::Mutation::Discard { .. } | awegit_git::Mutation::DiscardPaths { .. } => "discard",
        awegit_git::Mutation::Delete { .. } => "delete",
        awegit_git::Mutation::DeletePaths { .. } => "deletePaths",
        awegit_git::Mutation::ApplyPatch { .. } => "applyPatch",
        awegit_git::Mutation::AddRemote { .. } => "addRemote",
        awegit_git::Mutation::RemoveRemote { .. } => "removeRemote",
        awegit_git::Mutation::SetRemoteUrl { .. } => "setRemoteUrl",
        awegit_git::Mutation::AddWorktree { .. } => "addWorktree",
        awegit_git::Mutation::RemoveWorktree { .. } => "removeWorktree",
        awegit_git::Mutation::SubmoduleUpdate => "submoduleUpdate",
        awegit_git::Mutation::SubmoduleAdd { .. } => "submoduleAdd",
        awegit_git::Mutation::SubmoduleInit { .. } => "submoduleInit",
        awegit_git::Mutation::SubmoduleSync { .. } => "submoduleSync",
        awegit_git::Mutation::SubmoduleRemove { .. } => "submoduleRemove",
        awegit_git::Mutation::GitFlowInit { .. } => "gitFlowInit",
        awegit_git::Mutation::GitFlowStart { .. } => "gitFlowStart",
        awegit_git::Mutation::GitFlowFinish { .. } => "gitFlowFinish",
        awegit_git::Mutation::Squash { .. } => "squash",
        awegit_git::Mutation::Clone { .. } => "clone",
        awegit_git::Mutation::Init { .. } => "init",
        awegit_git::Mutation::LfsPull => "lfsPull",
        awegit_git::Mutation::LfsPush => "lfsPush",
        awegit_git::Mutation::LfsLock { .. } => "lfsLock",
        awegit_git::Mutation::LfsUnlock { .. } => "lfsUnlock",
        awegit_git::Mutation::SetHidden { .. } => "setHidden",
        awegit_git::Mutation::SetExpanded { .. } => "setExpanded",
        awegit_git::Mutation::SetSignOff { .. } => "setSignOff",
        awegit_git::Mutation::Custom { .. } => "custom",
    }
}

/// Names, paths, and verbs only. Passwords, command text, and patch bodies stay out.
fn action_detail(request: &awegit_git::Mutation) -> String {
    use awegit_git::Mutation;
    let text = match request {
        Mutation::Fetch { remote, prune, tags } => join([opt(remote), flag("prune", *prune), flag("tags", *tags)]),
        Mutation::Pull { rebase, autostash } => join([flag("rebase", *rebase), flag("autostash", *autostash)]),
        Mutation::Push { remote, set_upstream, tags, force_with_lease, force } => {
            join([opt(remote), flag("setUpstream", *set_upstream), flag("tags", *tags), flag("forceWithLease", *force_with_lease), flag("force", *force)])
        }
        Mutation::Stash { message, include_untracked } => join([bytes("message", message), flag("untracked", *include_untracked)]),
        Mutation::StashPop { name } => scrub(name),
        Mutation::StashApply { name } | Mutation::StashDrop { name } | Mutation::StashPatch { name } => scrub(name),
        Mutation::StashRename { name, message } => join([scrub(name), bytes("message", message)]),
        Mutation::StashBranch { name, branch } => join([scrub(name), scrub(branch)]),
        Mutation::StashPaths { files, message, include_untracked } => {
            join([files.iter().cloned().map(|file| scrub(&file)).collect::<Vec<_>>().join(","), bytes("message", message), flag("untracked", *include_untracked)])
        }
        Mutation::Checkout { name } => scrub(name),
        Mutation::DeleteBranch { name, force } => join([scrub(name), flag("force", *force)]),
        Mutation::CreateBranch { name, start } => join([scrub(name), opt(start)]),
        Mutation::Merge { name, squash, no_ff, autostash, summary, description } => {
            join([scrub(name), flag("squash", *squash), flag("noFf", *no_ff), flag("autostash", *autostash), bytes("summary", summary), bytes("description", description)])
        }
        Mutation::Rebase { onto, autostash } => join([scrub(onto), flag("autostash", *autostash)]),
        Mutation::RebaseInteractive { onto, drop, steps, autostash, update_refs } => {
            join([scrub(onto), format!("drop={}", drop.len()), format!("steps={}", steps.len()), flag("autostash", *autostash), flag("updateRefs", *update_refs)])
        }
        Mutation::Reset { rev, mode } => join([scrub(rev), format!("{mode:?}")]),
        Mutation::CherryPick { rev, record } => join([scrub(rev), flag("record", *record)]),
        Mutation::Revert { rev } => scrub(rev),
        Mutation::Reword { rev, summary } => join([scrub(rev), bytes("summary", summary)]),
        Mutation::Tag { name, rev, message } => join([scrub(name), scrub(rev), bytes("message", message)]),
        Mutation::DeleteTag { name } | Mutation::RemoveRemote { name } => scrub(name),
        Mutation::DeleteRemoteTag { remote, name } => join([scrub(remote), scrub(name)]),
        Mutation::RestoreFile { rev, file } => join([scrub(rev), scrub(file)]),
        Mutation::Bisect { verb, rev } => join([scrub(verb), scrub(rev)]),
        Mutation::WriteGitignore { text } | Mutation::WriteMailmap { text } => format!("bytes={}", text.len()),
        Mutation::AppendIgnore { file, files, fake, directory } => {
            let names = if files.is_empty() {
                scrub(file)
            } else {
                files.iter().map(|item| scrub(item)).collect::<Vec<_>>().join(",")
            };
            join([names, flag("fake", *fake), flag("directory", *directory)])
        }
        Mutation::StageHunk { file, index, unstage, .. } => join([scrub(file), format!("hunk={index}"), flag("unstage", *unstage)]),
        Mutation::StageLine { file, unstage, .. } => join([scrub(file), flag("unstage", *unstage)]),
        Mutation::StageLines { file, lines, unstage } => join([scrub(file), format!("lines={}", lines.len()), flag("unstage", *unstage)]),
        Mutation::StagePaths { files, unstage } => join([files.iter().cloned().map(|file| scrub(&file)).collect::<Vec<_>>().join(","), flag("unstage", *unstage)]),
        Mutation::DiscardHunk { file, index, .. } => join([scrub(file), format!("hunk={index}")]),
        Mutation::DiscardLine { file, .. } | Mutation::Discard { file } | Mutation::Delete { file } => scrub(file),
        Mutation::DiscardLines { file, lines } => join([scrub(file), format!("lines={}", lines.len())]),
        Mutation::DiscardPaths { files } => format!("files={}", files.len()),
        Mutation::DeletePaths { files } => format!("files={}", files.len()),
        Mutation::Resolve { file, side, text } => join([scrub(file), scrub(side), format!("bytes={}", text.len())]),
        Mutation::ResolvePaths { files, side } => join([format!("files={}", files.len()), scrub(side)]),
        Mutation::RenameBranch { name, to } => join([scrub(name), scrub(to)]),
        Mutation::SetUpstream { branch, upstream } => join([scrub(branch), scrub(upstream)]),
        Mutation::DeleteRemoteBranch { remote, branch }
        | Mutation::CheckoutRemote { remote, branch }
        | Mutation::PushRef { remote, branch, .. }
        | Mutation::PullRef { remote, branch, .. } => join([scrub(remote), scrub(branch)]),
        Mutation::ApplyPatch { patch } => format!("bytes={}", patch.len()),
        Mutation::AddRemote { name, url } | Mutation::SetRemoteUrl { name, url } => join([scrub(name), public_url(url)]),
        Mutation::AddWorktree { path, branch } => join([scrub(path), scrub(branch)]),
        Mutation::RemoveWorktree { path } => scrub(path),
        Mutation::SubmoduleAdd { url, path } => join([scrub(path), public_url(url)]),
        Mutation::SubmoduleInit { path } | Mutation::SubmoduleSync { path } | Mutation::SubmoduleRemove { path } => scrub(path),
        Mutation::GitFlowStart { name, kind } | Mutation::GitFlowFinish { name, kind } => join([scrub(name), scrub(kind)]),
        Mutation::GitFlowInit { master, develop, feature, release, hotfix, support } => {
            join([scrub(master), scrub(develop), scrub(feature), scrub(release), scrub(hotfix), scrub(support)])
        }
        Mutation::Squash { from, to, summary } => join([scrub(from), scrub(to), bytes("summary", summary)]),
        Mutation::Clone { url, destination } => join([public_url(url), scrub(destination)]),
        Mutation::Init { destination } => scrub(destination),
        Mutation::SetHidden { names } | Mutation::SetExpanded { names } => names.iter().cloned().map(|name| scrub(&name)).collect::<Vec<_>>().join(","),
        Mutation::SetSignOff { enabled, .. } => flag("enabled", *enabled),
        Mutation::Continue { message } => bytes("message", message),
        Mutation::Custom { .. } => String::new(),
        Mutation::LfsLock { path } | Mutation::LfsUnlock { path } => scrub(path),
        Mutation::Abort | Mutation::Skip | Mutation::SubmoduleUpdate | Mutation::LfsPull | Mutation::LfsPush => String::new(),
    };
    text.replace(['\n', '\r', '\t', '\0'], " ")
}

fn join(parts: impl IntoIterator<Item = String>) -> String {
    parts.into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
}

fn scrub(value: &str) -> String {
    value.replace(['\n', '\r', '\t', '\0'], " ")
}

fn opt(value: &Option<String>) -> String {
    value.as_deref().map(scrub).unwrap_or_default()
}

fn flag(name: &str, on: bool) -> String {
    if on { name.to_string() } else { String::new() }
}

fn bytes(name: &str, value: &str) -> String {
    if value.is_empty() { String::new() } else { format!("{name}Bytes={}", value.len()) }
}

fn public_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return scrub(url.split_once('@').map(|(_, host)| host).unwrap_or(url));
    };
    let host = rest.split_once('@').map(|(_, host)| host).unwrap_or(rest);
    scrub(&format!("{scheme}://{host}"))
}

#[tauri::command]
fn load_settings() -> Result<settings::Settings, String> {
    let values = settings::load()?;
    remember_fakeignore(values.fake_ignore);
    Ok(values)
}

#[tauri::command]
fn save_settings(mut values: settings::Settings) -> Result<settings::Settings, String> {
    values.recovery_notice.clear();
    settings::save(&values)?;
    remember_fakeignore(values.fake_ignore);
    Ok(values)
}

#[tauri::command]
fn settings_path() -> Result<String, String> {
    Ok(settings::path()?.display().to_string())
}

#[tauri::command]
fn import_fork_settings(path: Option<String>) -> Result<settings::ForkImport, String> {
    let chosen = path.filter(|item| !item.trim().is_empty());
    let file = match chosen {
        Some(path) => PathBuf::from(path),
        None => tools::fork_settings_file().ok_or_else(|| "fork-settings-missing".to_string())?,
    };
    let imported = settings::import_fork_at(&file)?;
    remember_fakeignore(imported.settings.fake_ignore);
    Ok(imported)
}

#[tauri::command]
fn external_tools() -> tools::ExternalTools {
    tools::installed()
}

#[tauri::command]
fn program_exists(path: String) -> bool {
    tools::program_exists(&path)
}

#[tauri::command]
fn watch_repository(app: tauri::AppHandle, state: tauri::State<'_, RepoWatch>, path: Option<String>) -> Result<(), String> {
    let repo = repo_from(path)?;
    let mut slot = state.0.lock().map_err(|_| "watcher lock".to_string())?;
    if let Some(previous) = slot.take() {
        if let Ok(mut watcher) = previous.watcher.lock() {
            watcher.take();
        }
    }
    let (sender, receiver) = mpsc::channel::<Vec<PathBuf>>();
    let mut watcher = notify::recommended_watcher(move |result: Result<Event, notify::Error>| {
        if let Ok(event) = result {
            let _ = sender.send(event.paths);
        }
    })
    .map_err(|error| error.to_string())?;
    let started = Instant::now();
    let mut watched = WatchSet::default();
    let git_dir = git_metadata_dir(&repo);
    watch_one(&mut watcher, &repo, RecursiveMode::NonRecursive, &mut watched)?;
    if let Some(git) = &git_dir {
        watch_git_metadata(&mut watcher, git, &mut watched);
    }
    let tracked = awegit_git::tracked_directories(&repo, 4_000);
    if tracked.capped {
        perf::write("WARN", "watch", "tracked directories capped at 4000");
    }
    for dir in tracked.paths {
        if skipped_path(&dir) {
            continue;
        }
        if let Err(error) = watch_one(&mut watcher, &dir, RecursiveMode::NonRecursive, &mut watched) {
            perf::write("WARN", "watch", &error);
        }
    }
    perf::write(
        "INFO",
        "watch",
        &format!(
            "armed watches={} recursive={} recoveries={} {}ms",
            watched.exact.len(),
            watched.recursive.len(),
            WATCH_RECOVERIES.load(Ordering::Relaxed),
            started.elapsed().as_millis()
        ),
    );
    let _ = app.emit("repo-changed", RepoChange { refs: true });
    let shared = Arc::new(WatchSlot {
        watcher: Mutex::new(Some(watcher)),
    });
    let thread_slot = Arc::clone(&shared);
    let root = repo.clone();
    std::thread::Builder::new()
        .name("awegit-watch".into())
        .spawn(move || settle_changes(&app, &root, git_dir, receiver, watched, thread_slot))
        .map_err(|error| error.to_string())?;
    *slot = Some(shared);
    Ok(())
}

/// Directories that are huge or rewrite themselves. A recursive watch of the work tree
/// would include them, and on Windows that stalls the process while the watch is armed.
fn heavy_dir(name: &str) -> bool {
    const NAMES: &[&str] = &[
        ".git",
        "node_modules",
        "target",
        "dist",
        "dist-ssr",
        ".turbo",
        ".svelte-kit",
        ".next",
        ".pnpm-store",
        ".rust-analyzer",
        "storybook-static",
        "playwright-report",
        "coverage",
        ".direnv",
        "vendor",
        "out",
        "build",
    ];
    NAMES.iter().any(|item| name.eq_ignore_ascii_case(item)) || awegit_git::codex_scratch_name(name)
}

fn skipped_path(path: &Path) -> bool {
    path.components().any(|component| match component {
        std::path::Component::Normal(name) => heavy_dir(&name.to_string_lossy()),
        _ => false,
    })
}

fn normalize_watch_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(text.as_ref()))
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

fn watch_key(path: &Path) -> PathBuf {
    let normal = normalize_watch_path(path);
    #[cfg(windows)]
    {
        PathBuf::from(normal.to_string_lossy().to_ascii_lowercase())
    }
    #[cfg(not(windows))]
    {
        normal
    }
}

#[derive(Default)]
struct WatchSet {
    exact: HashSet<PathBuf>,
    recursive: Vec<PathBuf>,
}

impl WatchSet {
    fn covers(&self, path: &Path) -> bool {
        let key = watch_key(path);
        if self.exact.contains(&key) {
            return true;
        }
        self.recursive.iter().any(|root| key.starts_with(root))
    }
}

struct WatchNode {
    parent: Option<usize>,
    path: PathBuf,
    children: Vec<usize>,
    heavy: bool,
}

/// Work-tree directories, without entering `node_modules`, `target`, and the other heavy names.
fn scan_worktree(root: &Path) -> Vec<WatchNode> {
    let mut nodes = vec![WatchNode {
        parent: None,
        path: root.to_path_buf(),
        children: Vec::new(),
        heavy: false,
    }];
    let mut stack = vec![0usize];
    while let Some(index) = stack.pop() {
        let path = nodes[index].path.clone();
        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => {
                nodes[index].heavy = true;
                continue;
            }
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            if !kind.is_dir() || kind.is_symlink() {
                continue;
            }
            if heavy_dir(&entry.file_name().to_string_lossy()) {
                nodes[index].heavy = true;
                continue;
            }
            let child = nodes.len();
            nodes.push(WatchNode {
                parent: Some(index),
                path: entry.path(),
                children: Vec::new(),
                heavy: false,
            });
            nodes[index].children.push(child);
            stack.push(child);
        }
    }
    for index in (0..nodes.len()).rev() {
        if nodes[index].heavy {
            if let Some(parent) = nodes[index].parent {
                nodes[parent].heavy = true;
            }
        }
    }
    nodes
}

#[cfg(test)]
fn watch_modes(nodes: &[WatchNode], index: usize, out: &mut Vec<(PathBuf, bool)>) {
    let recursive = !nodes[index].heavy;
    out.push((nodes[index].path.clone(), recursive));
    if !recursive {
        for child in nodes[index].children.clone() {
            watch_modes(nodes, child, out);
        }
    }
}

fn watch_one(watcher: &mut RecommendedWatcher, path: &Path, mode: RecursiveMode, set: &mut WatchSet) -> Result<(), String> {
    let key = watch_key(path);
    if !set.exact.insert(key.clone()) {
        return Ok(());
    }
    if let Err(error) = watcher.watch(path, mode) {
        set.exact.remove(&key);
        return Err(format!("{}: {error}", path.display()));
    }
    if mode == RecursiveMode::Recursive {
        set.recursive.push(key);
    }
    Ok(())
}

#[allow(dead_code)]
fn place_watches(watcher: &mut RecommendedWatcher, nodes: &[WatchNode], index: usize, set: &mut WatchSet) -> Result<(), String> {
    let heavy = nodes[index].heavy;
    let path = nodes[index].path.clone();
    let children = nodes[index].children.clone();
    if !heavy {
        return watch_one(watcher, &path, RecursiveMode::Recursive, set);
    }
    watch_one(watcher, &path, RecursiveMode::NonRecursive, set)?;
    for child in children {
        if let Err(error) = place_watches(watcher, nodes, child, set) {
            perf::write("WARN", "watch", &error);
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn watch_worktree(watcher: &mut RecommendedWatcher, root: &Path, set: &mut WatchSet) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    let nodes = scan_worktree(root);
    if nodes.is_empty() {
        return Ok(());
    }
    place_watches(watcher, &nodes, 0, set)
}

fn git_metadata_dir(repo: &Path) -> Option<PathBuf> {
    let git = repo.join(".git");
    if git.is_dir() {
        return Some(git);
    }
    let text = std::fs::read_to_string(&git).ok()?;
    let line = text.lines().find(|line| !line.trim().is_empty())?;
    let rest = line.trim().strip_prefix("gitdir:")?.trim();
    if rest.is_empty() {
        return None;
    }
    let path = PathBuf::from(rest);
    let full = if path.is_absolute() { path } else { repo.join(path) };
    full.is_dir().then_some(full)
}

fn watch_git_metadata(watcher: &mut RecommendedWatcher, git: &Path, set: &mut WatchSet) {
    if let Err(error) = watch_one(watcher, git, RecursiveMode::NonRecursive, set) {
        perf::write("WARN", "watch", &error);
    }
    for name in ["refs", "logs", "worktrees"] {
        let path = git.join(name);
        if path.is_dir() {
            if let Err(error) = watch_one(watcher, &path, RecursiveMode::Recursive, set) {
                perf::write("WARN", "watch", &error);
            }
        }
    }
}

fn directory_to_watch(path: &Path) -> Option<PathBuf> {
    let path = normalize_watch_path(path);
    if path.is_dir() {
        Some(path)
    } else {
        path.parent().map(Path::to_path_buf)
    }
}

fn attach_new_dirs(repo: &Path, git_dir: Option<&Path>, paths: &[PathBuf], set: &mut WatchSet, slot: &WatchSlot) {
    let Ok(mut guard) = slot.watcher.lock() else { return };
    let Some(watcher) = guard.as_mut() else { return };
    for path in paths {
        let Some(dir) = directory_to_watch(path) else { continue };
        if !dir.starts_with(repo) || git_dir.is_some_and(|git| dir.starts_with(git)) || set.covers(&dir) || skipped_path(&dir) {
            continue;
        }
        if let Err(error) = watch_one(watcher, &dir, RecursiveMode::NonRecursive, set) {
            perf::write("WARN", "watch", &error);
        }
    }
}

/// Waits for a quiet gap, then reports the batch once. Ends when the watcher is replaced.
fn settle_changes(
    app: &tauri::AppHandle,
    repo: &Path,
    git_dir: Option<PathBuf>,
    receiver: Receiver<Vec<PathBuf>>,
    mut watched: WatchSet,
    slot: Arc<WatchSlot>,
) {
    while let Ok(mut paths) = receiver.recv() {
        let started = Instant::now();
        let mut overflow = false;
        while started.elapsed() < WATCH_MAX_WAIT {
            if paths.len() >= WATCH_BATCH {
                overflow = true;
                while receiver.try_recv().is_ok() {}
                break;
            }
            match receiver.recv_timeout(WATCH_QUIET) {
                Ok(more) => paths.extend(more),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        attach_new_dirs(repo, git_dir.as_deref(), &paths, &mut watched, &slot);
        if overflow {
            // The dropped events may have included refs; a full refresh is the safe answer.
            let recoveries = WATCH_RECOVERIES.fetch_add(1, Ordering::Relaxed) + 1;
            perf::write("INFO", "watch", &format!("repo-changed refs=true paths=overflow recoveries={recoveries}"));
            let _ = app.emit("repo-changed", RepoChange { refs: true });
            continue;
        }
        let Some(change) = classify_changes(repo, &paths) else { continue };
        perf::write("INFO", "watch", &format!("repo-changed refs={} paths={}", change.refs, paths.len()));
        let _ = app.emit("repo-changed", change);
    }
}

/// `None` when every path is Git bookkeeping the UI does not show, or matched by ignore rules.
fn classify_changes(repo: &Path, paths: &[PathBuf]) -> Option<RepoChange> {
    let mut status = false;
    let mut files = Vec::new();
    for path in paths {
        let Ok(relative) = path.strip_prefix(repo) else { continue };
        let relative = relative.to_string_lossy().replace('\\', "/");
        if relative.is_empty() || relative.ends_with(".lock") {
            continue;
        }
        if let Some(inner) = relative.strip_prefix(".git/") {
            match inner.split('/').next().unwrap_or("") {
                "index" => status = true,
                "objects" | "modules" | "lfs" | "logs" | "FETCH_HEAD" | "COMMIT_EDITMSG" | "awegit" | "gitnuro" | "fsmonitor--daemon"
                    if inner != "logs/refs/stash" => {}
                _ => return Some(RepoChange { refs: true }),
            }
            continue;
        }
        let skip = relative.split('/').any(heavy_dir);
        if !skip {
            files.push(relative);
        }
    }
    if !status && !files.is_empty() {
        files.sort();
        files.dedup();
        let ignored: HashSet<String> = awegit_git::ignored_paths(repo, &files).into_iter().collect();
        status = files.iter().any(|file| !ignored.contains(file));
    }
    status.then_some(RepoChange { refs: false })
}

#[tauri::command]
fn open_terminal(path: Option<String>) -> Result<(), String> {
    let repo = repo_from(path)?;
    let loaded = settings::load().ok();
    if let Some(values) = loaded.as_ref() {
        if values.shell_kind != "default" && !values.shell_path.trim().is_empty() {
            let mut command = std::process::Command::new(&values.shell_path);
            if !values.shell_args.trim().is_empty() {
                command.args(split_command_args(&values.shell_args));
            }
            command.current_dir(&repo).spawn().map_err(|error| error.to_string())?;
            return Ok(());
        }
    }
    let configured = loaded.map(|settings| settings.terminal).unwrap_or_default();
    let mut command = std::process::Command::new("cmd");
    awegit_git::hide_console(&mut command);
    if configured.trim().is_empty() {
        command.args(["/C", "start", "cmd"]);
    } else {
        command.arg("/C").arg(configured);
    }
    command.current_dir(&repo).spawn().map_err(|error| error.to_string())?;
    Ok(())
}

fn split_command_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in text.chars() {
        match (quote, ch) {
            (None, ' ' | '\t') => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            (None, '"' | '\'') => quote = Some(ch),
            (Some(mark), found) if found == mark => quote = None,
            (_, found) => current.push(found),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

#[tauri::command]
fn clipboard_text() -> String {
    read_clipboard()
}

fn read_clipboard() -> String {
    #[cfg(windows)]
    {
        return read_clipboard_windows();
    }
    #[cfg(target_os = "macos")]
    {
        return read_command_text("pbpaste", &[]);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let wayland = read_command_text("wl-paste", &["--no-newline"]);
        if !wayland.is_empty() {
            return wayland;
        }
        return read_command_text("xclip", &["-selection", "clipboard", "-o"]);
    }
    #[cfg(not(any(windows, unix)))]
    String::new()
}

#[cfg(any(target_os = "macos", all(unix, not(target_os = "macos"))))]
fn read_command_text(program: &str, args: &[&str]) -> String {
    std::process::Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default()
}

#[cfg(windows)]
fn read_clipboard_windows() -> String {
    use std::ffi::c_void;
    const CF_UNICODETEXT: u32 = 13;
    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(hwnd: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(format: u32) -> *mut c_void;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalLock(hmem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(hmem: *mut c_void) -> i32;
    }
    unsafe {
        let mut opened = false;
        for _ in 0..8 {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        if !opened {
            return String::new();
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle.is_null() {
            CloseClipboard();
            return String::new();
        }
        let ptr = GlobalLock(handle) as *const u16;
        if ptr.is_null() {
            CloseClipboard();
            return String::new();
        }
        let mut len = 0usize;
        while len < 8192 && *ptr.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
        GlobalUnlock(handle);
        CloseClipboard();
        text
    }
}

#[tauri::command]
fn pick_directory(initial: Option<String>) -> Result<Option<String>, String> {
    let start = initial.unwrap_or_default();
    #[cfg(windows)]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms | Out-Null
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.ShowNewFolderButton = $true
$start = $env:AWEGIT_PICK_START
if ($start -and (Test-Path -LiteralPath $start)) { $dialog.SelectedPath = $start }
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $dialog.SelectedPath }
"#;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-STA", "-Command", script])
            .env("AWEGIT_PICK_START", &start)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if err.is_empty() { "Could not open the folder picker.".into() } else { err });
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok(if text.is_empty() { None } else { Some(text) });
    }
    #[cfg(target_os = "macos")]
    {
        let choose = if start.is_empty() {
            "POSIX path of (choose folder)".to_string()
        } else {
            format!(
                "POSIX path of (choose folder default location POSIX file \"{}\")",
                start.replace('"', "\\\"")
            )
        };
        let output = std::process::Command::new("osascript")
            .args(["-e", "try", "-e", &choose, "-e", "on error number -128", "-e", "\"\"", "-e", "end try"])
            .output()
            .map_err(|error| error.to_string())?;
        let text = String::from_utf8_lossy(&output.stdout).trim().trim_end_matches('/').to_string();
        return Ok(if text.is_empty() { None } else { Some(text) });
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = start;
        Err("This system has no folder picker.".into())
    }
}

#[tauri::command]
fn pick_file(initial: Option<String>) -> Result<Option<String>, String> {
    let start = initial.unwrap_or_default();
    #[cfg(windows)]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms | Out-Null
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Filter = "JSON (*.json)|*.json|All files (*.*)|*.*"
$dialog.CheckFileExists = $true
$start = $env:AWEGIT_PICK_START
if ($start -and (Test-Path -LiteralPath $start)) {
  $item = Get-Item -LiteralPath $start
  if ($item.PSIsContainer) { $dialog.InitialDirectory = $item.FullName }
  else { $dialog.InitialDirectory = $item.DirectoryName; $dialog.FileName = $item.Name }
}
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $dialog.FileName }
"#;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-STA", "-Command", script])
            .env("AWEGIT_PICK_START", &start)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if err.is_empty() { "Could not open the file picker.".into() } else { err });
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok(if text.is_empty() { None } else { Some(text) });
    }
    #[cfg(target_os = "macos")]
    {
        let _ = start;
        let output = std::process::Command::new("osascript")
            .args(["-e", "try", "-e", "POSIX path of (choose file of type {\"public.json\"})", "-e", "on error number -128", "-e", "\"\"", "-e", "end try"])
            .output()
            .map_err(|error| error.to_string())?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok(if text.is_empty() { None } else { Some(text) });
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = start;
        Err("This system has no file picker.".into())
    }
}

#[tauri::command]
fn file_preview(path: Option<String>, file: String) -> Result<Option<awegit_git::FilePreview>, String> {
    let repo = repo_from(path)?;
    awegit_git::file_preview(&repo, &file).map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_operation(path: Option<String>) -> Result<(), String> {
    match path {
        Some(path) if !path.trim().is_empty() => awegit_git::cancel_repo(Path::new(&path)),
        _ => awegit_git::cancel_running(),
    }
    Ok(())
}

/// Stop history/diff/blame reads without touching a queued write.
#[tauri::command]
fn cancel_reads(path: Option<String>) -> Result<(), String> {
    if let Some(path) = path.filter(|path| !path.trim().is_empty()) {
        awegit_git::cancel_reads(Path::new(&path));
    }
    Ok(())
}

/// Sidebar preferences live in `.git/awegit`; none of them changes the work tree,
/// so these skip the status scan that `mutate` would run.
#[tauri::command]
fn save_hidden_refs(path: Option<String>, names: Vec<String>) -> Result<(), String> {
    let repo = repo_from(path)?;
    let _turn = repo_turn(&repo);
    awegit_git::write_hidden_refs(&repo, &names).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_expanded_groups(path: Option<String>, names: Vec<String>) -> Result<(), String> {
    let repo = repo_from(path)?;
    let _turn = repo_turn(&repo);
    awegit_git::write_expanded_groups(&repo, &names).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_sign_off(path: Option<String>, enabled: bool, format: String) -> Result<(), String> {
    let repo = repo_from(path)?;
    let _turn = repo_turn(&repo);
    awegit_git::write_sign_off(&repo, enabled, &format).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_passphrase(user: String, secret: String) -> Result<(), String> {
    awegit_git::set_passphrase(user, secret);
    Ok(())
}

#[tauri::command]
fn approve_credential(path: Option<String>, protocol: String, host: String, username: String, password: String) -> Result<(), String> {
    let given = path.clone();
    let repo = repo_from(path)?;
    let _turn = repo_turn(&repo);
    apply_session();
    let report = report_path(given, &repo);
    awegit_git::with_operation_repo(&report, || awegit_git::approve_credential(&repo, &protocol, &host, &username, &password)).map_err(|error| error.to_string())
}

#[tauri::command]
fn check_for_update() -> Result<Option<updates::UpdateNotice>, String> {
    http::with_net(net_options(), updates::check)
}

#[tauri::command]
fn app_version() -> String {
    updates::current_version().to_string()
}

#[tauri::command]
fn update_release_page() -> String {
    updates::release_page().to_string()
}

#[tauri::command]
fn download_update(app: tauri::AppHandle, url: String, sha256: Option<String>) -> Result<(), String> {
    http::with_net(net_options(), || {
        updates::download_and_open(&url, sha256.as_deref(), |progress| {
            let _ = app.emit("update-progress", progress);
        })
    })
}

#[tauri::command]
fn cancel_update() {
    updates::cancel();
}

#[tauri::command]
fn conflict_sides(path: Option<String>, file: String) -> Result<awegit_git::ConflictSides, String> {
    let repo = repo_from(path)?;
    awegit_git::conflict_sides(&repo, &file).map_err(|error| error.to_string())
}

#[tauri::command]
fn conflict_view(path: Option<String>, file: String) -> Result<awegit_git::ConflictView, String> {
    let repo = repo_from(path)?;
    awegit_git::conflict_view(&repo, &file).map_err(|error| error.to_string())
}

#[tauri::command]
fn compare_files(path: Option<String>, from: String, to: String) -> Result<Vec<awegit_git::FileChange>, String> {
    let repo = repo_from(path)?;
    awegit_git::compare_files(&repo, &from, &to).map_err(|error| error.to_string())
}

#[tauri::command]
fn compare_file(path: Option<String>, from: String, to: String, file: String, unified: Option<u32>) -> Result<awegit_git::FileDiff, String> {
    let repo = repo_from(path)?;
    awegit_git::compare_file(&repo, &from, &to, &file, unified.unwrap_or(3)).map_err(|error| error.to_string())
}

#[tauri::command]
fn revision_preview(path: Option<String>, rev: String, file: String) -> Result<Option<awegit_git::FilePreview>, String> {
    let repo = repo_from(path)?;
    awegit_git::blob_preview(&repo, &rev, &file).map_err(|error| error.to_string())
}

#[tauri::command]
fn blob_view(path: Option<String>, rev: String, file: String) -> Result<awegit_git::BlobView, String> {
    let repo = repo_from(path)?;
    awegit_git::blob_view(&repo, &rev, &file).map_err(|error| error.to_string())
}

#[tauri::command]
fn reflog(path: Option<String>, limit: Option<usize>) -> Result<Vec<awegit_git::ReflogRow>, String> {
    let repo = repo_from(path)?;
    awegit_git::reflog(&repo, limit.unwrap_or(200)).map_err(|error| error.to_string())
}

#[tauri::command]
fn commit_tree(path: Option<String>, rev: String) -> Result<Vec<String>, String> {
    let repo = repo_from(path)?;
    awegit_git::commit_tree(&repo, &rev).map_err(|error| error.to_string())
}

#[tauri::command]
fn repository_summary(path: String) -> Result<awegit_git::RepoSummary, String> {
    awegit_git::repository_summary(std::path::Path::new(&path)).map_err(|error| error.to_string())
}

#[tauri::command]
fn rebase_conflicts(path: Option<String>, onto: String) -> Result<Vec<String>, String> {
    let repo = repo_from(path)?;
    awegit_git::rebase_conflicts(&repo, &onto).map_err(|error| error.to_string())
}

/// `list: false` only reports whether `git lfs` runs, without asking the server for locks.
#[tauri::command]
fn lfs_locks(path: Option<String>, list: Option<bool>) -> Result<LfsLockReport, String> {
    let repo = repo_from(path)?;
    if !list.unwrap_or(true) {
        return Ok(LfsLockReport { available: awegit_git::lfs_available(&repo), locks: Vec::new() });
    }
    let (available, locks) = awegit_git::lfs_locks(&repo).map_err(|error| error.to_string())?;
    Ok(LfsLockReport { available, locks })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LfsLockReport {
    available: bool,
    locks: Vec<awegit_git::LfsLockRow>,
}

#[tauri::command]
fn launch_tool(path: Option<String>, kind: String, file: String, tool: Option<String>, left: Option<String>, right: Option<String>) -> Result<(), String> {
    let repo = repo_from(path)?;
    let values = settings::load()?;
    let result = if kind == "merge" {
        awegit_git::launch_merge_tool(&repo, &values.merge_tool, &file)
    } else if let (Some(left), Some(right)) = (left.filter(|item| !item.is_empty()), right.filter(|item| !item.is_empty())) {
        let template = tool.filter(|item| !item.trim().is_empty()).unwrap_or(values.diff_tool);
        awegit_git::launch_diff_pair(&repo, &template, &file, &left, &right)
    } else {
        awegit_git::launch_diff_tool(&repo, &values.diff_tool, &file, false)
    };
    result.map_err(|error| error.to_string())
}

#[tauri::command]
fn account_token(values: &settings::Settings, forge_name: &str, fallback: &str) -> String {
    if !fallback.trim().is_empty() {
        return fallback.to_string();
    }
    values
        .accounts
        .iter()
        .find(|item| item.forge == forge_name && !item.token.trim().is_empty())
        .map(|item| item.token.clone())
        .unwrap_or_default()
}

#[tauri::command]
fn forge_notifications() -> Result<Vec<forge::Notice>, String> {
    let values = settings::load()?;
    http::with_net(http::NetOptions::from_settings(&values), || {
        forge::notifications(&account_token(&values, "github", &values.github_token))
    })
}

#[tauri::command]
fn forge_pulls(path: Option<String>) -> Result<Vec<forge::PullRequest>, String> {
    let repo = repo_from(path)?;
    let values = settings::load()?;
    let url = awegit_git::origin_url(&repo);
    http::with_net(http::NetOptions::from_settings(&values), || {
        forge::pulls(
            &url,
            &account_token(&values, "github", &values.github_token),
            &account_token(&values, "gitlab", &values.gitlab_token),
            &values.gitlab_host,
        )
    })
}

/// A stored snapshot can answer this request when HEAD and the index still match.
/// A full snapshot can serve a later brief request. A brief snapshot cannot serve a full one.
fn facts_reusable(stored_stamp: &str, stored_full: bool, stamp: &str, full: bool) -> bool {
    stored_stamp == stamp && (stored_full || !full)
}

/// `full: false` skips the ignored-file list, statistics, and LFS files.
/// A full result can answer a later brief request when HEAD and the index are unchanged.
#[tauri::command]
fn repo_facts(path: Option<String>, full: Option<bool>) -> Result<awegit_git::RepoFacts, String> {
    let repo = repo_from(path)?;
    let full = full.unwrap_or(true);
    let key = awegit_git::repo_key(&repo);
    let stamp = facts_stamp(&repo);
    if let Ok(cache) = facts_cache().lock() {
        if let Some(hit) = cache.get(&key) {
            if facts_reusable(&hit.stamp, hit.full, &stamp, full) {
                return Ok(hit.facts.clone());
            }
        }
    }
    let facts = if full { awegit_git::repo_facts(&repo) } else { awegit_git::repo_facts_brief(&repo) };
    let facts = facts.map_err(|error| error.to_string())?;
    if let Ok(mut cache) = facts_cache().lock() {
        cache.insert(key, CachedFacts { stamp, full, facts: facts.clone() });
    }
    Ok(facts)
}

fn facts_stamp(repo: &Path) -> String {
    let Some(git) = git_metadata_dir(repo) else {
        return format!("missing:{}", repo.display());
    };
    let mut stamp = file_mark(&git.join("HEAD"));
    if let Ok(text) = std::fs::read_to_string(git.join("HEAD")) {
        if let Some(rest) = text.trim().strip_prefix("ref:") {
            let rel = rest.trim();
            if !rel.is_empty() {
                stamp.push('|');
                stamp.push_str(&file_mark(&git.join(rel)));
            }
        }
    }
    stamp.push('|');
    stamp.push_str(&file_mark(&git.join("index")));
    stamp.push('|');
    stamp.push_str(repo.to_string_lossy().as_ref());
    stamp
}

fn file_mark(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(meta) => {
            let modified = meta.modified().ok().and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok()).map(|duration| duration.as_nanos()).unwrap_or(0);
            format!("{}:{}:{modified}", path.display(), meta.len())
        }
        Err(_) => format!("{}:missing", path.display()),
    }
}

#[tauri::command]
fn scan_repositories(root: String, depth: Option<u32>) -> Result<awegit_git::ScanResult, String> {
    let limit = depth.unwrap_or(5) as usize;
    awegit_git::scan_repositories(std::path::Path::new(&root), limit).map_err(|error| error.to_string())
}

#[tauri::command]
fn mark_notification(id: String) -> Result<(), String> {
    let values = settings::load()?;
    http::with_net(http::NetOptions::from_settings(&values), || {
        forge::mark_read(&account_token(&values, "github", &values.github_token), &id)
    })
}

#[tauri::command]
fn create_github_repo(name: String, private_repo: bool) -> Result<String, String> {
    let values = settings::load()?;
    http::with_net(http::NetOptions::from_settings(&values), || {
        forge::create_repo(&account_token(&values, "github", &values.github_token), &name, private_repo)
    })
}

#[tauri::command]
fn generate_ssh_key() -> Result<String, String> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map_err(|_| "home directory is missing".to_string())?;
    let dir = std::path::PathBuf::from(home).join(".ssh");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let key = dir.join("id_ed25519");
    if key.exists() {
        return Err("id_ed25519 already exists".into());
    }
    let mut command = std::process::Command::new("ssh-keygen");
    awegit_git::hide_console(&mut command);
    let output = command
        .args(["-t", "ed25519", "-f"])
        .arg(&key)
        .args(["-N", "", "-C", "awegit"])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    std::fs::read_to_string(dir.join("id_ed25519.pub")).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_repo_author(path: Option<String>, name: String, email: String) -> Result<(), String> {
    let given = path.clone();
    let repo = repo_from(path)?;
    let _turn = repo_turn(&repo);
    let report = report_path(given, &repo);
    awegit_git::with_operation_repo(&report, || awegit_git::set_repo_author(&repo, &name, &email)).map_err(|error| error.to_string())
}

#[tauri::command]
fn search_commits(path: Option<String>, kind: String, needle: String) -> Result<Vec<awegit_git::SearchHit>, String> {
    let repo = repo_from(path)?;
    awegit_git::search_commits(&repo, &kind, &needle).map_err(|error| error.to_string())
}

#[tauri::command]
fn search_log(
    path: Option<String>,
    field: String,
    scope: String,
    query: String,
    limit: Option<usize>,
    order: Option<String>,
) -> Result<Vec<awegit_git::CommitRow>, String> {
    let repo = repo_from(path)?;
    let topo = order.as_deref() == Some("topo");
    awegit_git::search_log(&repo, &field, &scope, &query, limit.unwrap_or(500), topo).map_err(|error| error.to_string())
}

#[tauri::command]
fn cherry_preview(path: Option<String>, rev: String) -> Result<String, String> {
    let repo = repo_from(path)?;
    awegit_git::cherry_preview(&repo, &rev).map_err(|error| error.to_string())
}

#[tauri::command]
fn merge_preview(path: Option<String>, name: String) -> Result<awegit_git::MergePreview, String> {
    let repo = repo_from(path)?;
    awegit_git::merge_preview(&repo, &name).map_err(|error| error.to_string())
}

#[tauri::command]
fn merge_state(path: Option<String>) -> Result<Option<awegit_git::MergeState>, String> {
    let repo = repo_from(path)?;
    awegit_git::merge_state(&repo).map_err(|error| error.to_string())
}

#[tauri::command]
fn merged_branches(path: Option<String>) -> Result<Vec<String>, String> {
    let repo = repo_from(path)?;
    awegit_git::merged_branches(&repo).map_err(|error| error.to_string())
}

#[tauri::command]
fn pick_history(path: Option<String>, file: String, text: String) -> Result<String, String> {
    let repo = repo_from(path)?;
    awegit_git::pick_history(&repo, &file, &text).map_err(|error| error.to_string())
}

#[tauri::command]
fn activity_log() -> Vec<String> {
    perf::activity()
}

#[tauri::command]
fn create_hosted_repo(forge_name: String, name: String, private_repo: bool, host: String, org: String) -> Result<String, String> {
    let values = settings::load()?;
    let fallback = match forge_name.as_str() {
        "gitlab" => values.gitlab_token.clone(),
        "bitbucket" => values.bitbucket_token.clone(),
        "azure" => values.azure_token.clone(),
        _ => values.github_token.clone(),
    };
    let token = account_token(&values, &forge_name, &fallback);
    http::with_net(http::NetOptions::from_settings(&values), || {
        forge::create_hosted(&forge_name, &token, &host, &name, private_repo, &org)
    })
}

#[tauri::command]
fn device_start(forge_name: String, client_id: String, host: String) -> Result<forge::DeviceStart, String> {
    http::with_net(net_options(), || forge::device_start(&forge_name, &client_id, &host))
}

#[tauri::command]
fn device_poll() -> Result<String, String> {
    http::with_net(net_options(), forge::device_poll)
}

#[tauri::command]
fn review_branch(path: Option<String>, base: String) -> Result<String, String> {
    let repo = repo_from(path)?;
    let values = settings::load()?;
    http::with_net(http::NetOptions::from_settings(&values), || ai::review(&repo, &values, &base))
}

#[tauri::command]
fn read_ssh_config() -> Result<String, String> {
    let path = ssh_config_path()?;
    if !path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(path).map_err(|error| error.to_string())
}

#[tauri::command]
fn write_ssh_config(text: String) -> Result<(), String> {
    if text.contains('\0') {
        return Err("ssh config contains a null".into());
    }
    let path = ssh_config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, text).map_err(|error| error.to_string())
}

fn ssh_config_path() -> Result<std::path::PathBuf, String> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map_err(|_| "home directory is missing".to_string())?;
    Ok(std::path::PathBuf::from(home).join(".ssh").join("config"))
}

#[tauri::command]
fn open_editor(path: Option<String>, file: String, line: u32) -> Result<(), String> {
    let repo = repo_from(path)?;
    let full = repo.join(&file);
    let spec = format!("{}:{line}", full.display());
    for bin in ["cursor", "code"] {
        if std::process::Command::new(bin).arg("-g").arg(&spec).spawn().is_ok() {
            return Ok(());
        }
    }
    std::process::Command::new("notepad").arg(&full).spawn().map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowNav {
    view: String,
    repo: String,
    file: String,
    rev: String,
}

#[derive(Clone, serde::Serialize)]
struct RevealCommit {
    repo: String,
    id: String,
}

#[tauri::command]
fn reveal_commit(app: tauri::AppHandle, repo: String, id: String) -> Result<(), String> {
    app.emit("awegit-reveal", RevealCommit { repo, id }).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_revision(path: Option<String>, rev: String, file: String) -> Result<(), String> {
    let repo = repo_from(path)?;
    let bytes = awegit_git::revision_bytes(&repo, &rev, &file).map_err(|error| error.to_string())?;
    let name = file.rsplit(['/', '\\']).next().filter(|item| !item.is_empty()).unwrap_or("file");
    let Some(dest) = pick_save(name)? else {
        return Ok(());
    };
    std::fs::write(dest, bytes).map_err(|error| error.to_string())
}

fn pick_save(name: &str) -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms | Out-Null
$dialog = New-Object System.Windows.Forms.SaveFileDialog
$dialog.Filter = "All files (*.*)|*.*"
$dialog.FileName = $env:AWEGIT_SAVE_NAME
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $dialog.FileName }
"#;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-STA", "-Command", script])
            .env("AWEGIT_SAVE_NAME", name)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if err.is_empty() { "Could not open the save dialog.".into() } else { err });
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok(if text.is_empty() { None } else { Some(text) });
    }
    #[cfg(target_os = "macos")]
    {
        let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
        let choose = format!("POSIX path of (choose file name default name \"{escaped}\")");
        let output = std::process::Command::new("osascript")
            .args(["-e", "try", "-e", &choose, "-e", "on error number -128", "-e", "\"\"", "-e", "end try"])
            .output()
            .map_err(|error| error.to_string())?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok(if text.is_empty() { None } else { Some(text) });
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = name;
        Err("This system has no save dialog.".into())
    }
}

#[tauri::command]
fn open_window(app: tauri::AppHandle, view: Option<String>, repo: Option<String>, file: Option<String>, rev: Option<String>, width: Option<f64>, height: Option<f64>) -> Result<(), String> {
    let view_name = view.unwrap_or_default();
    let repo_name = repo.unwrap_or_default();
    let file_name = file.unwrap_or_default();
    let rev_name = rev.unwrap_or_default();
    let label = match view_name.as_str() {
        "blame" => "blame",
        "history" => "history",
        "blob" => "blob",
        "merge" => "merge",
        _ => "desk",
    };
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_focus();
        if label == "history" || label == "blob" || label == "merge" {
            let _ = app.emit(
                "awegit-navigate",
                WindowNav { view: view_name, repo: repo_name, file: file_name, rev: rev_name },
            );
        }
        return Ok(());
    }
    let mut query = String::new();
    for (key, value) in [("view", view_name.as_str()), ("repo", repo_name.as_str()), ("file", file_name.as_str()), ("rev", rev_name.as_str())] {
        if value.is_empty() {
            continue;
        }
        if !query.is_empty() {
            query.push('&');
        }
        query.push_str(key);
        query.push('=');
        query.push_str(&encode_query(value));
    }
    // Keep the root path: SvelteKit's dev server and client router both 404 on `/index.html`.
    // A query-only relative reference resolves against the dev server or the tauri protocol base.
    let url = if query.is_empty() { "index.html".to_string() } else { format!("?{query}") };
    let (width, height) = if label == "merge" {
        (width.unwrap_or(1280.0), height.unwrap_or(840.0))
    } else {
        (1100.0, 720.0)
    };
    let window = tauri::webview::WebviewWindowBuilder::new(&app, label, tauri::WebviewUrl::App(url.into()))
        .title("AweGit")
        .inner_size(width.clamp(480.0, 4000.0), height.clamp(360.0, 4000.0))
        .resizable(true)
        .build()
        .map_err(|error| error.to_string())?;
    show_devtools(&window);
    Ok(())
}

fn show_devtools(window: &tauri::WebviewWindow) {
    #[cfg(debug_assertions)]
    window.open_devtools();
    #[cfg(not(debug_assertions))]
    let _ = window;
}

fn encode_query(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[tauri::command]
fn suggest_commit_message(path: Option<String>, style: Option<String>, language: Option<String>) -> Result<ai::Suggestion, String> {
    let repo = repo_from(path)?;
    let mut values = settings::load()?;
    if let Some(style) = style.as_deref().map(str::trim).filter(|style| !style.is_empty()) {
        if style.eq_ignore_ascii_case("long") || style.eq_ignore_ascii_case("short") {
            values.ai_prompt_style = if style.eq_ignore_ascii_case("long") {
                "long".to_string()
            } else {
                "short".to_string()
            };
        }
    }
    if let Some(language) = language.as_deref().map(str::trim).filter(|language| !language.is_empty()) {
        values.ai_language = language.to_string();
    }
    http::with_net(http::NetOptions::from_settings(&values), || ai::suggest(&repo, &values))
}

#[tauri::command]
fn test_ai_endpoint(base_url: String, api_key: String) -> Result<u32, String> {
    http::with_net(net_options(), || Ok(ai::list_models(&base_url, &api_key)?.len() as u32))
}

#[tauri::command]
fn list_ai_models(base_url: String, api_key: String) -> Result<Vec<String>, String> {
    http::with_net(net_options(), || ai::list_models(&base_url, &api_key))
}

#[tauri::command]
fn log_client(entries: Vec<perf::ClientEntry>) {
    perf::client(entries);
}

#[tauri::command]
fn perf_snapshot() -> perf::Snapshot {
    perf::snapshot()
}

#[tauri::command]
fn diagnostics() -> String {
    let runtime = awegit_git::git_runtime();
    format!("{}\ngit {} {}", perf::diagnostics(), runtime.version, runtime.program)
}

#[tauri::command]
fn git_runtime() -> awegit_git::GitRuntime {
    awegit_git::git_runtime()
}

#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("awegit:git:path cannot be revealed".into());
    }
    let target = PathBuf::from(path);
    if !path_is_revealable(&target, &reveal_roots()) {
        return Err("awegit:git:path cannot be revealed".into());
    }
    let full = std::fs::canonicalize(&target).unwrap_or(target);
    reveal_in_file_manager(&full)
}

fn reveal_roots() -> Vec<PathBuf> {
    let Ok(settings) = settings::load() else { return Vec::new() };
    settings
        .recent
        .iter()
        .chain(settings.source_directories.iter())
        .filter(|path| !path.trim().is_empty())
        .map(|path| PathBuf::from(path))
        .collect()
}

fn system_dir(path: &Path) -> bool {
    let text = path.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    text.contains("\\windows\\system32") || text.contains("\\windows\\syswow64")
}

fn inside_git_worktree(path: &Path) -> bool {
    let mut cursor = if path.is_dir() { Some(path) } else { path.parent() };
    while let Some(dir) = cursor {
        if dir.join(".git").exists() {
            return true;
        }
        cursor = dir.parent();
    }
    false
}

fn root_allows(path: &Path, root: &Path) -> bool {
    if system_dir(root) {
        return false;
    }
    let full = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    if system_dir(&full) {
        return false;
    }
    path.starts_with(&full)
}

/// A path can be shown in the file manager when it sits in a work tree or a known root.
/// `Windows\System32` and `SysWOW64` are refused even when one of them is passed as a root.
fn path_is_revealable(path: &Path, roots: &[PathBuf]) -> bool {
    if path.as_os_str().is_empty() {
        return false;
    }
    let full = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if system_dir(&full) || system_dir(path) {
        return false;
    }
    if inside_git_worktree(&full) {
        return true;
    }
    roots.iter().any(|root| root_allows(&full, root))
}

fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let text = path.display().to_string();
        return std::process::Command::new("explorer")
            .arg(format!("/select,{text}"))
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string());
    }
    #[cfg(target_os = "macos")]
    {
        return std::process::Command::new("open").arg("-R").arg(path).spawn().map(|_| ()).map_err(|error| error.to_string());
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let parent = path.parent().unwrap_or(path);
        std::process::Command::new("xdg-open").arg(parent).spawn().map(|_| ()).map_err(|error| error.to_string())
    }
}

#[tauri::command]
fn open_log_folder() -> Result<(), String> {
    let dir = perf::dir();
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let program = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    std::process::Command::new(program).arg(&dir).spawn().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    perf::init();
    awegit_git::set_trace(perf::git);
    prefer_bundled_git();
    let handler: std::sync::Arc<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync> = std::sync::Arc::new(tauri::generate_handler![
            workspace_status,
            stage_all,
            unstage_all,
            stage_path,
            unstage_path,
            commit_changes,
            file_diff,
            commit_log,
            commit_detail,
            locate_ref,
            repository_refs,
            commit_files,
            show_commit_file,
            in_progress,
            mutate,
            file_history,
            blame_file,
            load_settings,
            save_settings,
            settings_path,
            import_fork_settings,
            external_tools,
            program_exists,
            watch_repository,
            open_terminal,
            clipboard_text,
            pick_directory,
            pick_file,
            suggest_commit_message,
            test_ai_endpoint,
            list_ai_models,
            file_preview,
            cancel_operation,
            cancel_reads,
            save_hidden_refs,
            save_expanded_groups,
            save_sign_off,
            set_passphrase,
            approve_credential,
            check_for_update,
            app_version,
            update_release_page,
            download_update,
            cancel_update,
            conflict_sides,
            conflict_view,
            compare_files,
            compare_file,
            set_repo_author,
            revision_preview,
            blob_view,
            reflog,
            commit_tree,
            repository_summary,
            rebase_conflicts,
            lfs_locks,
            launch_tool,
            forge_notifications,
            forge_pulls,
            repo_facts,
            scan_repositories,
            mark_notification,
            create_github_repo,
            generate_ssh_key,
            search_commits,
            search_log,
            cherry_preview,
            merge_preview,
            merge_state,
            merged_branches,
            pick_history,
            activity_log,
            create_hosted_repo,
            device_start,
            device_poll,
            review_branch,
            read_ssh_config,
            write_ssh_config,
            open_editor,
            open_window,
            reveal_commit,
            save_revision,
            log_client,
            perf_snapshot,
            diagnostics,
            open_log_folder,
            reveal_path,
            git_runtime
        ]);
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(RepoWatch(Mutex::new(None)))
        .setup(|app| {
            perf::mark("setup");
            if let Ok(dir) = app.path().resource_dir() {
                if let Some(cmd) = git_cmd(&dir) {
                    awegit_git::use_bundled_git(&cmd);
                }
            }
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                show_devtools(&window);
            }
            let handle = app.handle().clone();
            perf::watch_main_thread(handle.clone());
            awegit_git::set_progress(move |path, percent| {
                let _ = handle.emit("git-progress", GitProgress { path: path.to_string(), percent });
            });
            Ok(())
        })
        .on_page_load(|_, payload| {
            perf::mark(&format!("page {:?} {}", payload.event(), payload.url()));
        })
        .invoke_handler(move |invoke| {
            let name = invoke.message.command().to_string();
            let handler = handler.clone();
            let resolver = invoke.resolver.clone();
            dispatch(&name.clone(), Box::new(move || {
                let _span = (!matches!(name.as_str(), "log_client" | "perf_snapshot")).then(|| perf::command(&name));
                match std::panic::catch_unwind(AssertUnwindSafe(|| handler(invoke))) {
                    Ok(true) => {}
                    Ok(false) => resolver.reject(format!("Command {name} not found")),
                    Err(_) => resolver.reject(format!("{name} stopped unexpectedly")),
                }
            }));
            true
        })
        .build(tauri::generate_context!())
        .expect("error while building AweGit")
        .run(|_, event| {
            if let tauri::RunEvent::Exit = event {
                perf::summary();
            }
        });
}

#[cfg(test)]
mod watch_plan_tests {
    use super::{heavy_dir, scan_worktree, watch_modes};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn heavy_names_skip_dependency_directories() {
        assert!(heavy_dir("node_modules"));
        assert!(heavy_dir("Node_Modules"));
        assert!(heavy_dir("target"));
        assert!(!heavy_dir("src"));
        assert!(!heavy_dir("crates"));
    }

    #[test]
    fn worktree_plan_does_not_watch_node_modules() {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_nanos()).unwrap_or(0);
        let root = std::env::temp_dir().join(format!("awegit-watch-{nanos}"));
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("node_modules").join("left-pad")).unwrap();
        fs::create_dir_all(root.join("src").join("lib")).unwrap();
        fs::write(root.join("src").join("lib").join("main.ts"), "export {}\n").unwrap();
        fs::create_dir_all(root.join("apps").join("web").join("node_modules").join("pkg")).unwrap();
        fs::create_dir_all(root.join("apps").join("web").join("src")).unwrap();
        let nodes = scan_worktree(&root);
        let mut modes = Vec::new();
        watch_modes(&nodes, 0, &mut modes);
        let joined = modes
            .iter()
            .map(|(path, recursive)| format!("{} {}", path.display(), if *recursive { "recursive" } else { "flat" }))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("flat"), "{joined}");
        assert!(!joined.to_ascii_lowercase().contains("node_modules"), "{joined}");
        assert!(modes.iter().any(|(path, recursive)| *recursive && path.ends_with("src") && path.components().any(|part| part.as_os_str() == "src") && !path.ends_with("web")), "{joined}");
        let web_src = modes.iter().find(|(path, _)| path.ends_with(std::path::Path::new("apps").join("web").join("src")));
        assert!(web_src.is_some_and(|(_, recursive)| *recursive), "{joined}");
        let apps = modes.iter().find(|(path, _)| path.ends_with("apps") && !path.ends_with("web"));
        assert!(apps.is_some_and(|(_, recursive)| !*recursive), "{joined}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn idle_gates_reap_only_when_the_map_is_large() {
        assert!(!super::idle_for_reap(16 * 60, 1, 24));
        assert!(super::idle_for_reap(15 * 60, 1, 25));
        assert!(!super::idle_for_reap(15 * 60, 2, 25));
        assert!(!super::idle_for_reap(14 * 60, 1, 25));
    }

    #[test]
    fn facts_cache_reuses_a_full_snapshot_until_head_or_the_index_changes() {
        assert!(super::facts_reusable("stamp", true, "stamp", false));
        assert!(super::facts_reusable("stamp", true, "stamp", true));
        assert!(super::facts_reusable("stamp", false, "stamp", false));
        assert!(!super::facts_reusable("stamp", false, "stamp", true));
        assert!(!super::facts_reusable("stamp", true, "moved", false));

        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_nanos()).unwrap_or(0);
        let root = std::env::temp_dir().join(format!("awegit-facts-{nanos}"));
        let git = root.join(".git");
        fs::create_dir_all(git.join("refs").join("heads")).unwrap();
        fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(git.join("refs").join("heads").join("main"), "abc\n").unwrap();
        fs::write(git.join("index"), b"v1").unwrap();
        let path = root.display().to_string();
        let repo = PathBuf::from(&path);
        let key = awegit_git::repo_key(&repo);
        let stamp = super::facts_stamp(&repo);
        fs::write(root.join("note.txt"), "worktree").unwrap();
        assert_eq!(super::facts_stamp(&repo), stamp, "a worktree file does not invalidate facts");

        let marker = "cached-full-marker";
        let facts = awegit_git::RepoFacts {
            ignored: Vec::new(),
            bisect_active: true,
            bisect_log: marker.into(),
            template: String::new(),
            head_message: String::new(),
            weekday: Vec::new(),
            hour: Vec::new(),
            lfs: String::new(),
            hot: Vec::new(),
        };
        super::facts_cache().lock().unwrap().insert(key.clone(), super::CachedFacts { stamp: stamp.clone(), full: true, facts: facts.clone() });
        let brief = super::repo_facts(Some(path.clone()), Some(false)).unwrap();
        assert_eq!(brief.bisect_log, marker);
        assert!(brief.bisect_active);

        super::facts_cache().lock().unwrap().insert(key.clone(), super::CachedFacts { stamp: stamp.clone(), full: false, facts });
        let full = super::repo_facts(Some(path.clone()), Some(true)).unwrap();
        assert_ne!(full.bisect_log, marker, "a brief snapshot must not answer a full request");

        fs::write(git.join("index"), b"v1-changed").unwrap();
        assert_ne!(super::facts_stamp(&repo), stamp);
        let missed = super::repo_facts(Some(path), Some(false)).unwrap();
        assert_ne!(missed.bisect_log, marker, "a new index must miss the cache");
        super::facts_cache().lock().unwrap().remove(&key);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn system_directories_are_not_revealable() {
        let path = Path::new(r"C:\Windows\System32\cmd.exe");
        let roots = vec![PathBuf::from(r"C:\Windows\System32"), PathBuf::from(r"C:\Windows\SysWOW64")];
        assert!(!super::path_is_revealable(path, &roots));
    }

    #[test]
    fn a_work_tree_or_source_root_can_be_revealed() {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_nanos()).unwrap_or(0);
        let root = std::env::temp_dir().join(format!("awegit-reveal-{nanos}"));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src").join("note.txt"), "x").unwrap();
        let note = root.join("src").join("note.txt");
        assert!(!super::path_is_revealable(&note, &[]));
        assert!(super::path_is_revealable(&note, &[root.clone()]));
        fs::create_dir_all(root.join(".git")).unwrap();
        assert!(super::path_is_revealable(&note, &[]));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn viewer_windows_cannot_run_writes() {
        let viewers = include_str!("../capabilities/viewers.json");
        let defaults = include_str!("../capabilities/default.json");
        for name in [
            "allow-mutate",
            "allow-open-terminal",
            "allow-write-ssh-config",
            "allow-download-update",
            "allow-generate-ssh-key",
            "allow-set-passphrase",
            "allow-approve-credential",
            "allow-create-hosted-repo",
            "allow-create-github-repo",
            "allow-device-start",
            "allow-device-poll",
            "allow-import-fork-settings",
            "allow-open-editor",
            "allow-launch-tool",
            "allow-save-revision",
            "allow-open-path",
            "opener:allow-reveal-item-in-dir",
        ] {
            assert!(!viewers.contains(name), "{name} is granted to a viewer window");
        }
        assert!(viewers.contains("allow-stage-all"));
        assert!(viewers.contains("allow-reveal-path"));
        assert!(viewers.contains("allow-git-runtime"));
        assert!(!viewers.contains("**"));
        assert!(!defaults.contains("**"));
        assert!(!defaults.contains("allow-open-path"));
        assert!(!defaults.contains("opener:allow-reveal-item-in-dir"));
    }
}
