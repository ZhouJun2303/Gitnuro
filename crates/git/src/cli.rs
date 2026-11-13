//! Run `git` for changes that must match the real Git command line.
//!
//! Reads that only need the object database stay in gitoxide. Staging, committing,
//! and the text of a diff go through `git` so filters, hooks, and line endings match
//! what the user already has configured. Nothing here writes the global gitconfig.

use std::cell::{Cell, RefCell};
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::Error;

static BUNDLED_GIT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

struct Tracked {
    key: String,
    pid: u32,
    generation: u64,
    read: bool,
    group: crate::proc::Group,
}

static TRACKED: Mutex<Vec<Tracked>> = Mutex::new(Vec::new());
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
static TRACE: std::sync::OnceLock<Trace> = std::sync::OnceLock::new();
static PROGRESS: Mutex<Option<ProgressHook>> = Mutex::new(None);

type ProgressHook = Arc<dyn Fn(&str, u8) + Send + Sync>;

/// Called after each `git` child exits.
///
/// `ok` is false only when the exit is an error the caller will surface. Expected misses
/// such as a missing upstream stay successful so they do not look like failures.
/// `note` is a short stderr tail for a real failure, and empty otherwise.
pub type Trace = fn(&[&str], std::time::Duration, bool, &str);

pub fn set_trace(hook: Trace) {
    let _ = TRACE.set(hook);
}

/// Receive the latest `git` transfer percent (`0`–`100`) while a command is running.
///
/// Fetch, pull, push, clone, checkout, and submodule update force `--progress`
/// so the percent arrives even when stderr is not a terminal.
pub fn set_progress(hook: impl Fn(&str, u8) + Send + Sync + 'static) {
    if let Ok(mut slot) = PROGRESS.lock() {
        *slot = Some(Arc::new(hook));
    }
}

fn progress_hook() -> Option<ProgressHook> {
    PROGRESS.lock().ok().and_then(|guard| guard.clone())
}

/// Stable identity for one repository. Slash direction and a trailing slash do not matter.
pub fn repo_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

thread_local! {
    static OPERATION_REPO: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static READ_SCOPE: Cell<bool> = const { Cell::new(false) };
}

/// Mark `git` processes started inside `body` as reads, so a newer read can stop them
/// without touching a queued write. Nested scopes restore the previous value.
pub fn with_read<T>(body: impl FnOnce() -> T) -> T {
    READ_SCOPE.with(|slot| {
        let previous = slot.replace(true);
        let value = body();
        slot.set(previous);
        value
    })
}

/// Attribute git progress and cancellation to `path` while `body` runs on this thread.
pub fn with_operation_repo<T>(path: &Path, body: impl FnOnce() -> T) -> T {
    OPERATION_REPO.with(|slot| {
        let previous = slot.borrow().clone();
        *slot.borrow_mut() = Some(path.to_path_buf());
        let value = body();
        *slot.borrow_mut() = previous;
        value
    })
}

fn operation_path(fallback: &Path) -> PathBuf {
    OPERATION_REPO
        .with(|slot| slot.borrow().clone())
        .unwrap_or_else(|| fallback.to_path_buf())
}

thread_local! {
    static CAPTURE: Cell<bool> = const { Cell::new(false) };
    static CAPTURED: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Keep stdout and stderr from `git` started on this thread, then return that text.
pub fn capture_output<T>(body: impl FnOnce() -> T) -> (T, String) {
    CAPTURE.with(|flag| flag.set(true));
    CAPTURED.with(|slot| slot.borrow_mut().clear());
    let value = body();
    let text = CAPTURED.with(|slot| slot.borrow().trim().to_string());
    CAPTURE.with(|flag| flag.set(false));
    (value, text)
}

fn note_captured(stdout: &[u8], stderr: &[u8]) {
    if !CAPTURE.with(|flag| flag.get()) {
        return;
    }
    let out = String::from_utf8_lossy(stdout).trim().to_string();
    let err = settle_stderr(stderr);
    if out.is_empty() && err.is_empty() {
        return;
    }
    CAPTURED.with(|slot| {
        let mut text = slot.borrow_mut();
        if !text.is_empty() {
            text.push('\n');
        }
        if !out.is_empty() {
            text.push_str(&out);
        }
        if !err.is_empty() {
            if !out.is_empty() {
                text.push('\n');
            }
            text.push_str(&err);
        }
    });
}

/// Values applied to `git` child processes. Nothing here is written to gitconfig.
#[derive(Clone)]
pub struct Session {
    pub proxy: String,
    pub author_name: String,
    pub author_email: String,
    pub ssl_verify: bool,
    pub ssl_ca: String,
    pub sign_commits: bool,
    pub ask_user: String,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            proxy: String::new(),
            author_name: String::new(),
            author_email: String::new(),
            ssl_verify: true,
            ssl_ca: String::new(),
            sign_commits: false,
            ask_user: String::new(),
        }
    }
}

struct Process {
    session: Session,
    passphrase: String,
}

static PROCESS: Mutex<Process> = Mutex::new(Process {
    session: Session {
        proxy: String::new(),
        author_name: String::new(),
        author_email: String::new(),
        ssl_verify: true,
        ssl_ca: String::new(),
        sign_commits: false,
        ask_user: String::new(),
    },
    passphrase: String::new(),
});

/// Prefer a Portable Git `cmd` directory shipped beside the executable.
///
/// When `mingw64\bin\git.exe` sits beside that directory, later commands start it directly.
/// The `cmd\git.exe` wrapper would otherwise open a console for the real git.
pub fn use_bundled_git(dir: &Path) {
    let _ = BUNDLED_GIT.set(dir.display().to_string());
}

struct GitLaunch {
    program: OsString,
    path_prefix: Option<OsString>,
}

static LAUNCH: std::sync::OnceLock<GitLaunch> = std::sync::OnceLock::new();

/// Stop a console program from opening a window on Windows. A windowed program ignores the flag.
pub fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

fn git_command() -> Command {
    let launch = LAUNCH.get_or_init(resolve_launch);
    let mut command = Command::new(&launch.program);
    hide_console(&mut command);
    apply_path(&mut command, launch.path_prefix.as_deref());
    command
}

fn resolve_launch() -> GitLaunch {
    if let Some(bundled) = BUNDLED_GIT.get() {
        if let Some(found) = bundled_launch(Path::new(bundled)) {
            return found;
        }
    }
    #[cfg(windows)]
    if let Some(found) = launch_from_system_path() {
        return found;
    }
    GitLaunch {
        program: OsString::from("git"),
        path_prefix: None,
    }
}

/// Path of the `git` binary the next command will start.
pub fn git_program() -> String {
    let launch = LAUNCH.get_or_init(resolve_launch);
    launch.program.to_string_lossy().into_owned()
}

/// Git on this machine. `version` is empty when `git --version` cannot be read.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRuntime {
    pub program: String,
    pub version: String,
    pub merge_tree: bool,
    pub lfs: bool,
    pub minimum: bool,
}

/// Major and minor from `git version 2.39.1` or `2.56.0.windows.1`. Empty text is `None`.
pub fn parse_git_version(text: &str) -> Option<(u32, u32)> {
    let rest = text.trim();
    if rest.is_empty() {
        return None;
    }
    let rest = rest.strip_prefix("git version ").unwrap_or(rest);
    let mut numbers = rest
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty());
    let major = numbers.next()?.parse().ok()?;
    let minor = numbers.next()?.parse().ok()?;
    Some((major, minor))
}

fn version_at_least(parsed: Option<(u32, u32)>, major: u32, minor: u32) -> bool {
    parsed.is_some_and(|(found_major, found_minor)| {
        found_major > major || (found_major == major && found_minor >= minor)
    })
}

/// Probe Git once for the about dialog. Startup still opens when Git is missing or old.
pub fn git_runtime() -> GitRuntime {
    let program = git_program();
    let text = git_version_text();
    let parsed = parse_git_version(&text);
    let version = parsed
        .map(|_| {
            text.trim()
                .strip_prefix("git version ")
                .unwrap_or(text.trim())
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_default();
    GitRuntime {
        program,
        version,
        merge_tree: version_at_least(parsed, 2, 38),
        lfs: lfs_runs(),
        minimum: version_at_least(parsed, 2, 39),
    }
}

fn git_version_text() -> String {
    let mut command = git_command();
    command.arg("--version");
    match command.output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => String::new(),
    }
}

fn lfs_runs() -> bool {
    let mut command = git_command();
    command.args(["lfs", "version"]);
    command
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// How to start the Git under `dir` (its `cmd` folder), when that copy can run.
fn bundled_launch(dir: &Path) -> Option<GitLaunch> {
    if let Some(found) = launch_beside(dir) {
        return Some(found);
    }
    // Only the shim is left. Start it by absolute path: a bare `git` would be
    // looked up in the parent's PATH, which often has no Git at all.
    let shim = dir.join(if cfg!(windows) { "git.exe" } else { "git" });
    shim.is_file().then(|| GitLaunch {
        program: shim.into_os_string(),
        path_prefix: Some(dir.as_os_str().to_os_string()),
    })
}

/// `dir` is Git's `cmd` folder. The real binary sits in `<arch>/bin/git.exe`;
/// recent MinGit uses `ucrt64`, older installs use `mingw64`.
fn launch_beside(dir: &Path) -> Option<GitLaunch> {
    let root = dir.parent()?;
    for arch in ["ucrt64", "mingw64", "clang64", "clangarm64", "mingw32"] {
        let bin = root.join(arch).join("bin");
        let real = bin.join("git.exe");
        if real.is_file() {
            return Some(GitLaunch {
                program: real.into_os_string(),
                path_prefix: Some(helper_path(&bin)),
            });
        }
    }
    None
}

fn helper_path(bin: &Path) -> OsString {
    let mut dirs = vec![bin.to_path_buf()];
    if let Some(root) = bin.parent().and_then(|parent| parent.parent()) {
        let usr = root.join("usr").join("bin");
        if usr.is_dir() {
            dirs.push(usr);
        }
    }
    std::env::join_paths(&dirs).unwrap_or_else(|_| bin.as_os_str().to_os_string())
}

#[cfg(windows)]
fn launch_from_system_path() -> Option<GitLaunch> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if !dir.join("git.exe").is_file() {
            continue;
        }
        if let Some(found) = launch_beside(&dir) {
            return Some(found);
        }
        return Some(GitLaunch {
            program: dir.join("git.exe").into_os_string(),
            path_prefix: Some(dir.into_os_string()),
        });
    }
    None
}

fn apply_path(command: &mut Command, prefix: Option<&OsStr>) {
    let Some(prefix) = prefix.filter(|prefix| !prefix.is_empty()) else {
        return;
    };
    let mut path = OsString::from(prefix);
    path.push(if cfg!(windows) { ";" } else { ":" });
    if let Some(current) = std::env::var_os("PATH") {
        path.push(current);
    }
    command.env("PATH", path);
}

/// `origin` URL, or empty when this repository has no such remote.
pub fn origin_url(repo: &Path) -> String {
    run_output(repo, &["remote", "get-url", "origin"])
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Remote name and fetch URL. Push-only URLs are kept when that remote has no fetch URL.
pub fn remote_list(repo: &Path) -> Vec<(String, String)> {
    let Ok(output) = run_output(repo, &["remote", "-v"]) else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut seen: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else { continue };
        let Some(url) = parts.next() else { continue };
        let kind = parts.next().unwrap_or("(fetch)");
        if let Some(slot) = seen.iter_mut().find(|(item, _)| item == name) {
            if kind.contains("fetch") {
                slot.1 = url.to_string();
            }
            continue;
        }
        seen.push((name.to_string(), url.to_string()));
    }
    seen
}

/// Ask `git ls-remote` whether this HTTPS URL accepts the credentials already set.
pub fn ls_remote_head(url: &str) -> Result<(), Error> {
    if url.is_empty() || url.starts_with('-') || url.contains(['\n', '\r', '\0', ' ']) {
        return Err(Error::Rev("remote url".into()));
    }
    run(&std::env::temp_dir(), &["ls-remote", url, "HEAD"]).map(|_| ())
}

/// One HTTPS password offered to `git` for a single host. It is not written to gitconfig.
#[derive(Clone)]
pub struct HttpsCredential {
    pub host: String,
    pub username: String,
    pub secret: String,
    pub preferred: bool,
}

thread_local! {
    static HTTPS_CREDENTIALS: RefCell<Vec<HttpsCredential>> = const { RefCell::new(Vec::new()) };
}

/// Replace the HTTPS accounts the next network command may use. Empty clears them.
pub fn set_https_credentials(items: Vec<HttpsCredential>) {
    HTTPS_CREDENTIALS.with(|slot| *slot.borrow_mut() = items);
}

/// Replace the session used by later `git` processes. The in-memory passphrase is kept.
pub fn configure(session: Session) {
    if let Ok(mut guard) = PROCESS.lock() {
        let ask_user = guard.session.ask_user.clone();
        guard.session = session;
        guard.session.ask_user = ask_user;
    }
}

/// Remember a signing or askpass secret for this process only. It is not written to settings.
pub fn set_passphrase(user: String, secret: String) {
    if secret.contains(['\n', '\r', '\0']) || user.contains(['\n', '\r', '\0']) {
        return;
    }
    let clear = secret.is_empty();
    if let Ok(mut guard) = PROCESS.lock() {
        guard.session.ask_user = user;
        guard.passphrase = secret;
    }
    if clear {
        if let Some(dir) = askpass_dir() {
            let _ = std::fs::remove_file(dir.join("secret"));
        }
    }
}

/// Proxy used by later `git` processes. Empty clears it. This does not write gitconfig.
pub fn set_http_proxy(value: Option<String>) {
    if let Ok(mut guard) = PROCESS.lock() {
        guard.session.proxy = value.unwrap_or_default();
    }
}

pub(crate) fn session_snapshot() -> (Session, String) {
    PROCESS
        .lock()
        .map(|guard| (guard.session.clone(), guard.passphrase.clone()))
        .unwrap_or_else(|_| (Session::default(), String::new()))
}

fn lock_tracked() -> std::sync::MutexGuard<'static, Vec<Tracked>> {
    TRACKED.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn track(key: String, pid: u32, read: bool, group: crate::proc::Group) -> u64 {
    let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
    lock_tracked().push(Tracked {
        key,
        pid,
        generation,
        read,
        group,
    });
    generation
}

fn stop_generation(generation: u64) {
    let entry = {
        let mut guard = lock_tracked();
        let index = guard.iter().position(|item| item.generation == generation);
        index.map(|index| guard.remove(index))
    };
    if let Some(entry) = entry {
        entry.group.kill_and_wait(entry.pid);
    }
}

/// Stop every `git` process this library started, including grandchildren.
/// Returns true when a write (not just a read) was among them.
pub fn cancel_running() -> bool {
    let entries: Vec<(u64, bool)> = lock_tracked()
        .iter()
        .map(|item| (item.generation, item.read))
        .collect();
    let stopped_write = entries.iter().any(|(_, read)| !read);
    for (generation, _) in entries {
        stop_generation(generation);
    }
    stopped_write
}

/// Stop `git` processes started for this repository. Other repositories keep running.
///
/// Only the generations recorded at this moment are stopped. A command that starts
/// afterwards has a new generation and is left running.
pub fn cancel_repo(repo: &Path) -> bool {
    let key = repo_key(repo);
    let entries: Vec<(u64, bool)> = lock_tracked()
        .iter()
        .filter(|item| item.key == key)
        .map(|item| (item.generation, item.read))
        .collect();
    let stopped_write = entries.iter().any(|(_, read)| !read);
    for (generation, _) in entries {
        stop_generation(generation);
    }
    stopped_write
}

/// Stop reads (history, diff, blame, search) for this repository. Writes keep running.
pub fn cancel_reads(repo: &Path) {
    let key = repo_key(repo);
    let generations: Vec<u64> = lock_tracked()
        .iter()
        .filter(|item| item.read && item.key == key)
        .map(|item| item.generation)
        .collect();
    for generation in generations {
        stop_generation(generation);
    }
}

/// Stop one process and the children it has started, then wait until it has exited.
///
/// When the process was started here, its job or process group is signaled and the
/// tracking entry is removed first. A later command that reused the same process id
/// has a different entry and is not signaled.
pub fn stop_process_tree(pid: u32) {
    let entry = {
        let mut guard = lock_tracked();
        let index = guard.iter().position(|item| item.pid == pid);
        index.map(|index| guard.remove(index))
    };
    if let Some(entry) = entry {
        entry.group.kill_and_wait(entry.pid);
        return;
    }
    crate::proc::kill_pid(pid);
}

/// Which identity variables a child process receives. Rebase, cherry-pick, and
/// revert keep the original author, so they only replace the committer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthorMode {
    /// Author and committer, for commits this app creates.
    Both,
    /// Committer only, for history rewrites that must keep their authors.
    CommitterOnly,
    /// Neither. Used when a repository-local author should win.
    Repo,
}

pub fn run(repo: &Path, args: &[&str]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], None, AuthorMode::Both, false)
}

/// Like `run`, but a non-zero exit still returns the output. Spawn failures stay errors.
pub fn run_output(repo: &Path, args: &[&str]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], None, AuthorMode::Both, true)
}

pub fn run_env(
    repo: &Path,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, env, None, AuthorMode::Both, false)
}

/// Like [`run_env`], but leaves the author of rewritten commits alone.
pub(crate) fn run_env_committer_only(
    repo: &Path,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, env, None, AuthorMode::CommitterOnly, false)
}

pub fn run_stdin(repo: &Path, args: &[&str], stdin: &[u8]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], Some(stdin), AuthorMode::Both, false)
}

/// Like `run_stdin`, but a non-zero exit still returns the output.
pub(crate) fn run_stdin_output(
    repo: &Path,
    args: &[&str],
    stdin: &[u8],
) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], Some(stdin), AuthorMode::Both, true)
}

/// Like `run_stdin`, but keeps a repository-local `user.name`.
pub(crate) fn run_stdin_repo_author(
    repo: &Path,
    args: &[&str],
    stdin: &[u8],
) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], Some(stdin), AuthorMode::Repo, false)
}

/// Run git without replacing a repository-local `user.name`.
pub(crate) fn run_repo_author(repo: &Path, args: &[&str]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], None, AuthorMode::Repo, false)
}

/// True when this repository has its own `user.name`.
pub(crate) fn has_local_author(repo: &Path) -> bool {
    let mut command = git_command();
    command
        .current_dir(repo)
        .args(["config", "--local", "--get", "user.name"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    match command.output() {
        Ok(output) => {
            output.status.success() && output.stdout.iter().any(|byte| !byte.is_ascii_whitespace())
        }
        Err(_) => false,
    }
}

fn run_prepared(
    repo: &Path,
    args: &[&str],
    env: &[(&str, &str)],
    stdin_bytes: Option<&[u8]>,
    author: AuthorMode,
    allow_failure: bool,
) -> Result<std::process::Output, Error> {
    run_prepared_capped(repo, args, env, stdin_bytes, author, allow_failure, None)
        .map(|(output, _)| output)
}

/// Like `run`, but stops reading after `limit` bytes and kills the child.
///
/// A huge diff, blame, or log would otherwise be buffered in full before the UI can
/// truncate it. The returned flag says the output is incomplete.
pub fn run_capped(
    repo: &Path,
    args: &[&str],
    limit: usize,
) -> Result<(std::process::Output, bool), Error> {
    run_prepared_capped(repo, args, &[], None, AuthorMode::Both, false, Some(limit))
}

/// Run a command with many pathspecs. Short lists stay on the command line; long
/// ones go through `--pathspec-from-file=-` so Windows never hits the 32K limit.
pub(crate) fn run_paths(
    repo: &Path,
    prefix: &[&str],
    paths: &[String],
) -> Result<std::process::Output, Error> {
    if paths.is_empty() {
        return run(repo, prefix);
    }
    for path in paths {
        check_path(path)?;
    }
    let prefix = prefix.strip_suffix(&["--"]).unwrap_or(prefix);
    let bytes: usize = paths.iter().map(|path| path.len() + 1).sum();
    if paths.len() <= 40 && bytes <= 4_000 {
        let mut owned: Vec<String> = Vec::with_capacity(prefix.len() + paths.len() + 2);
        owned.push("--literal-pathspecs".into());
        owned.extend(prefix.iter().map(|part| (*part).to_string()));
        owned.push("--".into());
        owned.extend(paths.iter().cloned());
        let borrowed: Vec<&str> = owned.iter().map(String::as_str).collect();
        return run(repo, &borrowed);
    }
    let mut owned: Vec<String> = Vec::with_capacity(prefix.len() + 4);
    owned.push("--literal-pathspecs".into());
    owned.extend(prefix.iter().map(|part| (*part).to_string()));
    owned.push("--pathspec-from-file=-".into());
    owned.push("--pathspec-file-nul".into());
    let mut input = Vec::with_capacity(bytes);
    for path in paths {
        input.extend_from_slice(path.as_bytes());
        input.push(0);
    }
    let borrowed: Vec<&str> = owned.iter().map(String::as_str).collect();
    run_stdin(repo, &borrowed, &input)
}

#[allow(clippy::too_many_arguments)]
fn run_prepared_capped(
    repo: &Path,
    args: &[&str],
    env: &[(&str, &str)],
    stdin_bytes: Option<&[u8]>,
    author: AuthorMode,
    allow_failure: bool,
    cap: Option<usize>,
) -> Result<(std::process::Output, bool), Error> {
    let mut flow_command = crate::flow::CommandSpan::start(args);
    let progress = progress_hook();
    let (credential_args, helper_dir) = https_prefix(args);
    let _remove_helper = RemoveDir(helper_dir);
    let mut command = git_command();
    command.current_dir(repo).env("GIT_TERMINAL_PROMPT", "0");
    let progressed = if progress.is_some() && cap.is_none() {
        command.env("GIT_LFS_FORCE_PROGRESS", "1");
        progress_args(args)
    } else {
        args.iter().map(|arg| (*arg).to_string()).collect::<Vec<_>>()
    };
    let mut full = credential_args;
    full.extend(progressed);
    command.args(&full);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (session, passphrase) = session_snapshot();
    if !session.proxy.is_empty() {
        command.env("http_proxy", &session.proxy);
        command.env("https_proxy", &session.proxy);
        command.env("HTTP_PROXY", &session.proxy);
        command.env("HTTPS_PROXY", &session.proxy);
    }
    // The identity is decided here, not by whatever shell started the app.
    command
        .env_remove("GIT_AUTHOR_NAME")
        .env_remove("GIT_AUTHOR_EMAIL")
        .env_remove("GIT_COMMITTER_NAME")
        .env_remove("GIT_COMMITTER_EMAIL");
    if author == AuthorMode::Both && !session.author_name.is_empty() {
        command.env("GIT_AUTHOR_NAME", &session.author_name);
    }
    if author != AuthorMode::Repo && !session.author_name.is_empty() {
        command.env("GIT_COMMITTER_NAME", &session.author_name);
    }
    if author == AuthorMode::Both && !session.author_email.is_empty() {
        command.env("GIT_AUTHOR_EMAIL", &session.author_email);
    }
    if author != AuthorMode::Repo && !session.author_email.is_empty() {
        command.env("GIT_COMMITTER_EMAIL", &session.author_email);
    }
    if !session.ssl_verify {
        command.env("GIT_SSL_NO_VERIFY", "1");
    }
    if !session.ssl_ca.is_empty() {
        command.env("GIT_SSL_CAINFO", &session.ssl_ca);
    }
    if !passphrase.is_empty() && needs_passphrase(args) {
        if let Some(script) = askpass_script(&passphrase) {
            command.env("GIT_ASKPASS", &script);
            command.env("SSH_ASKPASS", &script);
            command.env("SSH_ASKPASS_REQUIRE", "force");
            command.env("AWE_ASK_USER", &session.ask_user);
        }
    }
    for (key, value) in env {
        command.env(key, value);
    }
    // Retry the individual Windows filesystem operation inside Git. Re-running
    // a whole pull/stash/checkout after a partial write can lose local work.
    #[cfg(windows)]
    let _file_retry = if needs_passphrase(args) {
        crate::locks::FileRetry::configure(&mut command)
    } else {
        None
    };
    flow_command.configure(&mut command);
    let started = std::time::Instant::now();
    let owned = operation_path(repo);
    let (mut child, group) =
        crate::proc::spawn_prepared(&mut command).map_err(Error::GitMissing)?;
    let pid = child.id();
    let key = repo_key(&owned);
    let is_read = READ_SCOPE.with(|slot| slot.get());
    track(key, pid, is_read, group);
    let (output, write_result, truncated) = std::thread::scope(|scope| {
        // Feed stdin while stdout is drained. Writing the whole input first deadlocks
        // as soon as the child fills its stdout pipe and stops reading.
        let writer = stdin_bytes.and_then(|bytes| {
            child.stdin.take().map(|mut stdin| {
                scope.spawn(move || {
                    use std::io::Write;
                    let result = stdin.write_all(bytes);
                    drop(stdin);
                    result
                })
            })
        });
        let (output, truncated) = if let Some(limit) = cap {
            let (output, truncated) = stream_output_capped(&mut child, limit);
            (output, truncated)
        } else if let Some(progress) = progress {
            (
                stream_output(&mut child, progress, owned.display().to_string()),
                false,
            )
        } else {
            (child.wait_with_output().map_err(Error::GitMissing), false)
        };
        if output.is_err() {
            // Unblock the writer before joining it if reading output failed mid-flight.
            stop_process_tree(pid);
        }
        let write_result = writer.map(|handle| {
            handle.join().unwrap_or_else(|_| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "git stdin writer panicked",
                ))
            })
        });
        (output, write_result, truncated)
    });
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            untrack(pid);
            return Err(error);
        }
    };
    // A command that exits before reading all of stdin produces a broken pipe; that is
    // not more interesting than the command's own exit status.
    if let Some(Err(source)) = write_result {
        if source.kind() != std::io::ErrorKind::BrokenPipe {
            untrack(pid);
            return Err(Error::Read {
                path: "git stdin".to_string(),
                source,
            });
        }
    }
    untrack(pid);
    // A capped read is intentionally incomplete. Do not turn it into a lock error.
    if truncated {
        flow_command.finish(output.status.success(), &output.stderr);
        let note = failure_note(output.status.success(), true, &output.stdout, &output.stderr);
        if let Some(hook) = TRACE.get() {
            hook(args, started.elapsed(), true, &note);
        }
        note_captured(&output.stdout, &output.stderr);
        return Ok((output, true));
    }
    let stderr_text = settle_stderr(&output.stderr);
    // Git for Windows can exit 0 after `checkout` while leaving the locked file
    // unchanged. The unlink message is still the failure the user has to see.
    let locked = if !allow_failure && output.status.success() {
        crate::locks::explain(repo, &stderr_text)
    } else {
        None
    };
    let succeeded = output.status.success() && locked.is_none();
    flow_command.finish(succeeded, &output.stderr);
    let note = failure_note(succeeded, allow_failure, &output.stdout, &output.stderr);
    if let Some(hook) = TRACE.get() {
        hook(args, started.elapsed(), succeeded || allow_failure, &note);
    }
    note_captured(&output.stdout, &output.stderr);
    if succeeded || allow_failure {
        return Ok((output, false));
    }
    let detail = command_failure_detail(&output.stdout, &stderr_text, args);
    let explained = locked.or_else(|| crate::locks::explain(repo, &detail));
    Err(Error::Git(match explained {
        Some(message) => format!("{message}\n{detail}"),
        None => detail,
    }))
}

/// Read stdout until `limit` bytes, then stop the child. Stderr is drained in parallel.
fn stream_output_capped(
    child: &mut Child,
    limit: usize,
) -> (Result<std::process::Output, Error>, bool) {
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stderr_task = std::thread::Builder::new()
        .name("awegit-output".into())
        .spawn(move || drain(stderr_pipe));
    let mut stdout = Vec::new();
    let mut truncated = false;
    if let Some(mut pipe) = stdout_pipe {
        let mut chunk = [0u8; 16 * 1024];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    let room = limit.saturating_sub(stdout.len());
                    if read > room {
                        stdout.extend_from_slice(&chunk[..room]);
                        truncated = true;
                        stop_process_tree(child.id());
                        break;
                    }
                    stdout.extend_from_slice(&chunk[..read]);
                }
                Err(_) => break,
            }
        }
    }
    let status = match child.wait() {
        Ok(status) => status,
        Err(source) => return (Err(Error::GitMissing(source)), truncated),
    };
    let stderr = stderr_task
        .ok()
        .and_then(|task| task.join().ok())
        .unwrap_or_default();
    (
        Ok(std::process::Output {
            status,
            stdout,
            stderr,
        }),
        truncated,
    )
}

fn drain(pipe: Option<impl Read>) -> Vec<u8> {
    let Some(mut pipe) = pipe else {
        return Vec::new();
    };
    let mut buf = Vec::new();
    let _ = pipe.read_to_end(&mut buf);
    buf
}

/// Read both pipes while `git` runs. Stderr is scanned for `45% (12/30)` lines.
fn stream_output(
    child: &mut Child,
    progress: ProgressHook,
    repo: String,
) -> Result<std::process::Output, Error> {
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stderr_task = match std::thread::Builder::new()
        .name("awegit-progress".into())
        .spawn(move || read_stderr(stderr_pipe, progress, repo))
    {
        Ok(task) => task,
        Err(source) => {
            stop_process_tree(child.id());
            let _ = child.wait();
            return Err(Error::GitMissing(source));
        }
    };
    let mut stdout = Vec::new();
    if let Some(mut pipe) = stdout_pipe {
        let _ = pipe.read_to_end(&mut stdout);
    }
    let status = child.wait().map_err(Error::GitMissing)?;
    let stderr = stderr_task.join().unwrap_or_default();
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

fn read_stderr(pipe: Option<impl Read>, progress: ProgressHook, repo: String) -> Vec<u8> {
    let Some(mut pipe) = pipe else {
        return Vec::new();
    };
    let mut buf = Vec::new();
    let mut pending = Vec::new();
    let mut last = None;
    let mut chunk = [0u8; 1024];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                pending.extend_from_slice(&chunk[..n]);
                drain_progress(&mut pending, &mut last, &progress, &repo);
            }
        }
    }
    if !pending.is_empty() {
        report_percent(&pending, &mut last, &progress, &repo);
    }
    buf
}

fn drain_progress(
    pending: &mut Vec<u8>,
    last: &mut Option<u8>,
    progress: &ProgressHook,
    repo: &str,
) {
    loop {
        let Some(index) = pending
            .iter()
            .position(|byte| *byte == b'\r' || *byte == b'\n')
        else {
            break;
        };
        let piece: Vec<u8> = pending.drain(..=index).collect();
        report_percent(&piece, last, progress, repo);
    }
}

fn report_percent(piece: &[u8], last: &mut Option<u8>, progress: &ProgressHook, repo: &str) {
    let text = String::from_utf8_lossy(piece);
    let Some(percent) = percent_in(text.trim()) else {
        return;
    };
    if *last == Some(percent) {
        return;
    }
    *last = Some(percent);
    progress(repo, percent);
}

/// Last `N% (` on a git progress line. Other text is ignored.
fn percent_in(text: &str) -> Option<u8> {
    let bytes = text.as_bytes();
    let mut found = None;
    for index in 0..bytes.len() {
        if bytes[index] != b'%' {
            continue;
        }
        let mut start = index;
        while start > 0 && bytes[start - 1].is_ascii_digit() {
            start -= 1;
        }
        if start == index {
            continue;
        }
        let after = text[index + 1..].trim_start();
        if !after.starts_with('(') {
            continue;
        }
        let Ok(percent) = text[start..index].parse::<u8>() else {
            continue;
        };
        if percent <= 100 {
            found = Some(percent);
        }
    }
    found
}

/// Output tail for a failure the caller will surface. Expected non-zero exits stay empty.
///
/// Git often prints the useful sentence on stdout (`nothing to commit`) and leaves
/// stderr empty. Prefer stderr when both exist.
fn failure_note(success: bool, allow_failure: bool, stdout: &[u8], stderr: &[u8]) -> String {
    if success || allow_failure {
        return String::new();
    }
    tail_text(&command_failure_text(stdout, stderr), 500)
}

fn command_failure_detail(stdout: &[u8], stderr_text: &str, args: &[&str]) -> String {
    if !stderr_text.is_empty() {
        return stderr_text.to_string();
    }
    let stdout_text = settle_stderr(stdout);
    if stdout_text.is_empty() {
        format!("git {} failed", args.join(" "))
    } else {
        tail_text(&stdout_text, 500)
    }
}

fn command_failure_text(stdout: &[u8], stderr: &[u8]) -> String {
    let stderr_text = settle_stderr(stderr);
    if !stderr_text.is_empty() {
        stderr_text
    } else {
        settle_stderr(stdout)
    }
}

fn tail_text(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    text.chars().skip(count - max_chars).collect()
}

/// Drop overwritten `\r` updates and keep the last copy of each line.
fn settle_stderr(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .split('\n')
        .filter_map(|line| {
            let kept = line
                .trim_end_matches(['\r', ' '])
                .rsplit('\r')
                .next()
                .unwrap_or("")
                .trim();
            if kept.is_empty() {
                None
            } else {
                Some(kept.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Insert `--progress` for commands that can report a transfer or checkout percent.
fn progress_args(args: &[&str]) -> Vec<String> {
    let mut owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    if owned
        .iter()
        .any(|arg| arg == "--progress" || arg == "--no-progress")
    {
        return owned;
    }
    let mut index = 0;
    while index < owned.len() {
        if owned[index] == "-c" {
            index += 2;
            continue;
        }
        if owned[index].starts_with('-') {
            index += 1;
            continue;
        }
        break;
    }
    let insert_at = match owned.get(index).map(String::as_str) {
        Some("fetch" | "pull" | "push" | "clone" | "checkout" | "switch" | "restore") => {
            Some(index + 1)
        }
        Some("submodule") if owned.get(index + 1).map(String::as_str) == Some("update") => {
            Some(index + 2)
        }
        _ => None,
    };
    if let Some(at) = insert_at {
        if at <= owned.len() {
            owned.insert(at, "--progress".to_string());
        }
    }
    owned
}

fn untrack(pid: u32) {
    lock_tracked().retain(|item| item.pid != pid);
}

struct RemoveDir(Option<PathBuf>);

impl Drop for RemoveDir {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

fn command_verb<'a>(args: &'a [&'a str]) -> Option<&'a str> {
    let mut index = 0;
    while index < args.len() {
        match args[index] {
            "-c" | "--git-dir" | "--work-tree" | "--namespace" | "--config-env" | "--exec-path" => {
                index += 2;
            }
            arg if arg.starts_with('-') => index += 1,
            arg => return Some(arg),
        }
    }
    None
}

fn wants_https_credentials(args: &[&str]) -> bool {
    let Some(verb) = command_verb(args) else {
        return false;
    };
    match verb {
        "fetch" | "pull" | "push" | "clone" | "ls-remote" => true,
        "submodule" => args.iter().any(|arg| *arg == "update" || *arg == "sync"),
        "lfs" => args
            .iter()
            .any(|arg| matches!(*arg, "fetch" | "pull" | "push" | "clone")),
        _ => false,
    }
}

fn safe_credential_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() < 200
        && host
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
        && !host.starts_with('-')
        && !host.contains("..")
}

fn shell_single(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// `-c` arguments that answer HTTPS passwords for this command only.
/// The token stays in the helper script, not in the argument list.
fn https_prefix(args: &[&str]) -> (Vec<String>, Option<PathBuf>) {
    if !wants_https_credentials(args) {
        return (Vec::new(), None);
    }
    let creds = HTTPS_CREDENTIALS.with(|slot| slot.borrow().clone());
    let creds: Vec<_> = creds
        .into_iter()
        .filter(|item| {
            safe_credential_host(&item.host)
                && !item.secret.is_empty()
                && !item.secret.contains(['\n', '\r', '\0'])
                && !item.username.contains(['\n', '\r', '\0', '\''])
        })
        .collect();
    if creds.is_empty() {
        return (Vec::new(), None);
    }
    let Some(dir) = https_helper_dir() else {
        return (Vec::new(), None);
    };
    let script = dir.join("helper.sh");
    let body = https_helper_script(&creds);
    if write_private(&script, &body).is_none() {
        let _ = std::fs::remove_dir_all(&dir);
        return (Vec::new(), None);
    }
    let path = script.display().to_string().replace('\\', "/");
    let helper = format!("!sh {}", shell_single(&path));
    let mut hosts = Vec::new();
    for item in &creds {
        if !hosts.iter().any(|host| host == &item.host) {
            hosts.push(item.host.clone());
        }
    }
    let mut prefix = Vec::new();
    for host in hosts {
        for scheme in ["https", "http"] {
            prefix.push("-c".to_string());
            prefix.push(format!("credential.{scheme}://{host}.helper={helper}"));
        }
    }
    (prefix, Some(dir))
}

fn https_helper_dir() -> Option<PathBuf> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "awegit-https-{}-{stamp}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Some(dir)
}

fn https_helper_script(creds: &[HttpsCredential]) -> String {
    let mut body = String::from(
        "#!/bin/sh\nif [ \"$1\" != \"get\" ]; then\n  exit 0\nfi\nhost=\nuser=\nwhile IFS= read -r line; do\n  case \"$line\" in\n    host=*) host=${line#host=} ;;\n    username=*) user=${line#username=} ;;\n    \"\") break ;;\n  esac\ndone\nhost=${host%%/*}\nhost=${host%%:*}\n",
    );
    let mut hosts = Vec::new();
    for item in creds {
        if !hosts.iter().any(|host| host == &item.host) {
            hosts.push(item.host.clone());
        }
    }
    for host in hosts {
        let matched: Vec<_> = creds.iter().filter(|item| item.host == host).collect();
        let fallback = matched.iter().find(|item| item.preferred).copied();
        body.push_str(&format!("if [ \"$host\" = {} ]; then\n", shell_single(&host)));
        for item in &matched {
            body.push_str(&format!("  if [ \"$user\" = {} ]; then\n", shell_single(&item.username)));
            body.push_str(&format!("    printf 'username=%s\\n' {}\n", shell_single(&item.username)));
            body.push_str(&format!("    printf 'password=%s\\n' {}\n", shell_single(&item.secret)));
            body.push_str("    exit 0\n  fi\n");
        }
        if let Some(item) = fallback {
            body.push_str("  if [ -z \"$user\" ]; then\n");
            body.push_str(&format!("    printf 'username=%s\\n' {}\n", shell_single(&item.username)));
            body.push_str(&format!("    printf 'password=%s\\n' {}\n", shell_single(&item.secret)));
            body.push_str("    exit 0\n  fi\n");
        }
        body.push_str("fi\n");
    }
    body.push_str("exit 0\n");
    body
}

/// `-c` arguments that keep commit signing on only when the session asked for it.
pub(crate) fn commit_config(repo: &Path) -> Vec<String> {
    let (session, passphrase) = session_snapshot();
    if !session.sign_commits {
        return vec!["-c".into(), "commit.gpgsign=false".into()];
    }
    let mut prefix = vec!["-c".into(), "commit.gpgsign=true".into()];
    if passphrase.is_empty() {
        return prefix;
    }
    let format = run(repo, &["config", "--get", "gpg.format"])
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();
    if format != "ssh" {
        if let Some(program) = gpg_wrapper(&passphrase) {
            prefix.push("-c".into());
            prefix.push(format!("gpg.program={program}"));
        }
    }
    prefix
}

fn askpass_dir() -> Option<&'static std::path::Path> {
    static DIR: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let root = std::env::temp_dir();
        prune_askpass_dirs(&root);
        for attempt in 0..4 {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0);
            let dir = root.join(format!(
                "awegit-askpass-{}-{stamp}-{attempt}",
                std::process::id()
            ));
            match std::fs::create_dir(&dir) {
                Ok(()) => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let _ =
                            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
                    }
                    return Some(dir);
                }
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return None,
            }
        }
        None
    })
    .as_deref()
}

fn prune_askpass_dirs(root: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let Some(cutoff) =
        std::time::SystemTime::now().checked_sub(std::time::Duration::from_secs(24 * 60 * 60))
    else {
        return;
    };
    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with("awegit-askpass-")
        {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() && meta.modified().map(|at| at < cutoff).unwrap_or(false) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

fn write_private(path: &std::path::Path, body: &str) -> Option<()> {
    std::fs::write(path, body).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Some(())
}

/// Write the passphrase where the helper scripts can read it. Keeping it out of the
/// child environment stops it from appearing in process listings and crash dumps.
fn write_secret(path: &std::path::Path, value: &str) -> Option<()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let _guard = LOCK.lock().ok()?;
    let tmp = path.with_file_name(format!(
        ".secret-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&tmp, value).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    if std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return None;
    }
    Some(())
}

fn shell_quote(value: &str) -> String {
    let escaped = value
        .replace('\\', "/")
        .replace('$', "\\$")
        .replace('`', "\\`")
        .replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Reads never need the signing passphrase, and leaving it out of their environment
/// also keeps it away from external drivers a read can start.
fn needs_passphrase(args: &[&str]) -> bool {
    let mut index = 0;
    while index < args.len() {
        let arg = args[index];
        if matches!(
            arg,
            "-c" | "--git-dir" | "--work-tree" | "--namespace" | "--config-env" | "--exec-path"
        ) {
            index += 2;
            continue;
        }
        if arg.starts_with('-') {
            index += 1;
            continue;
        }
        return !matches!(
            arg,
            "status"
                | "diff"
                | "log"
                | "show"
                | "blame"
                | "rev-parse"
                | "rev-list"
                | "cat-file"
                | "ls-files"
                | "ls-tree"
                | "for-each-ref"
                | "merge-base"
                | "shortlog"
                | "describe"
                | "grep"
                | "check-ignore"
                | "hash-object"
                | "symbolic-ref"
                | "count-objects"
                | "var"
                | "version"
                | "help"
                | "show-ref"
                | "name-rev"
                | "config"
        );
    }
    true
}

fn askpass_script(passphrase: &str) -> Option<String> {
    let dir = askpass_dir()?;
    let secret = dir.join("secret");
    write_secret(&secret, &format!("{passphrase}\n"))?;
    let path = dir.join("ask.sh");
    let body = format!(
        "#!/bin/sh\ncase \"$1\" in\n*[Uu]sername*)\n  printf '%s\\n' \"$AWE_ASK_USER\"\n  ;;\n*)\n  cat {}\n  ;;\nesac\n",
        shell_quote(&secret.display().to_string())
    );
    write_private(&path, &body)?;
    Some(path.display().to_string().replace('\\', "/"))
}

fn gpg_wrapper(passphrase: &str) -> Option<String> {
    let dir = askpass_dir()?;
    let secret = dir.join("secret");
    write_secret(&secret, &format!("{passphrase}\n"))?;
    let path = dir.join("gpgwrap.sh");
    let body = format!(
        "#!/bin/sh\ngpg --batch --yes --pinentry-mode loopback --passphrase-file {} \"$@\"\n",
        shell_quote(&secret.display().to_string())
    );
    write_private(&path, &body)?;
    Some(path.display().to_string().replace('\\', "/"))
}

/// A commit id or a revision such as `HEAD~2`. Rejects option-like values.
pub fn check_rev(rev: &str) -> Result<&str, Error> {
    let safe = !rev.is_empty()
        && !rev.starts_with('-')
        && !rev.contains("..")
        && !rev.contains([' ', '\n', '\r', '\\'])
        && rev
            .chars()
            .all(|c| name_char(c) || matches!(c, '~' | '^' | '@' | '{' | '}'));
    if safe {
        Ok(rev)
    } else {
        Err(Error::Rev(rev.to_string()))
    }
}

/// Characters Git accepts in a ref name that cannot change how the command line is read.
/// Any letter or digit is fine, so branch and tag names in other languages work.
fn name_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            '/' | '_' | '.' | '-' | '+' | '#' | ',' | '=' | '%' | '!' | '&' | '(' | ')' | '@'
        )
}

/// Branch, tag, and remote names passed to `git`.
pub fn check_name(name: &str) -> Result<&str, Error> {
    let safe = !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('/')
        && !name.ends_with('.')
        && !name.contains("..")
        && name != "@"
        && !name.contains("@{")
        && name.chars().all(name_char);
    if safe {
        Ok(name)
    } else {
        Err(Error::Rev(name.to_string()))
    }
}

/// Reject absolute paths and `..` so a command cannot leave the work tree.
///
/// Newlines and NUL cannot be sent through line-based porcelain, and a lossy
/// non-UTF-8 conversion would point at a different file; both are refused instead
/// of risking a write to the wrong path.
pub fn check_path(path: &str) -> Result<&str, Error> {
    let escapes = path.is_empty()
        || path.starts_with(['/', '\\'])
        || path.contains(':')
        || path.contains(['\n', '\r', '\0', '\u{FFFD}'])
        || path
            .split(['/', '\\'])
            .any(|part| part.is_empty() || part == "." || part == "..");
    if escapes {
        Err(Error::Path(path.to_string()))
    } else {
        Ok(path)
    }
}

#[cfg(test)]
mod progress_tests {
    use super::{failure_note, percent_in, progress_args, settle_stderr};

    #[test]
    fn reads_the_latest_git_percent() {
        assert_eq!(
            percent_in("Receiving objects:  45% (10/22), 1 KiB | 1 MiB/s"),
            Some(45)
        );
        assert_eq!(
            percent_in("remote: Counting objects: 100% (15/15), done."),
            Some(100)
        );
        assert_eq!(
            percent_in("Writing objects: 100% (11/11), done.\rReceiving objects: 3% (1/30)"),
            Some(3)
        );
        assert_eq!(
            percent_in("Downloading LFS objects:   0% (0/2), 0 B | 0 B/s"),
            Some(0)
        );
        assert_eq!(percent_in("nothing to report"), None);
        assert_eq!(percent_in("already 100% complete"), None);
    }

    #[test]
    fn keeps_the_last_progress_line() {
        let raw =
            b"Receiving objects:  10% (1/10)\rReceiving objects:  40% (4/10)\r\nfatal: early EOF\n";
        assert_eq!(
            settle_stderr(raw),
            "Receiving objects:  40% (4/10)\nfatal: early EOF"
        );
    }

    #[test]
    fn adds_progress_only_where_git_can_report_it() {
        assert_eq!(
            progress_args(&["fetch", "--all"]),
            vec!["fetch", "--progress", "--all"]
        );
        assert_eq!(
            progress_args(&["pull", "--rebase"]),
            vec!["pull", "--progress", "--rebase"]
        );
        assert_eq!(
            progress_args(&["push", "--force-with-lease"]),
            vec!["push", "--progress", "--force-with-lease"]
        );
        assert_eq!(
            progress_args(&["clone", "https://example.com/a.git", "a"]),
            vec!["clone", "--progress", "https://example.com/a.git", "a"]
        );
        assert_eq!(
            progress_args(&["submodule", "update", "--init", "--recursive"]),
            vec!["submodule", "update", "--progress", "--init", "--recursive"]
        );
        assert_eq!(
            progress_args(&["submodule", "sync", "--", "path"]),
            vec!["submodule", "sync", "--", "path"]
        );
        assert_eq!(
            progress_args(&["-c", "commit.gpgsign=false", "commit", "-m", "hi"]),
            vec!["-c", "commit.gpgsign=false", "commit", "-m", "hi"]
        );
        assert_eq!(
            progress_args(&["checkout", "--ours", "--", "file"]),
            vec!["checkout", "--progress", "--ours", "--", "file"]
        );
        assert_eq!(
            progress_args(&["fetch", "--progress"]),
            vec!["fetch", "--progress"]
        );
    }

    #[test]
    fn failure_note_keeps_stderr_only_for_real_errors() {
        let stderr = b"Receiving objects:  10% (1/10)\rReceiving objects:  40% (4/10)\r\nfatal: could not read Username\n";
        assert_eq!(failure_note(true, false, b"nothing to commit\n", stderr), "");
        assert_eq!(failure_note(false, true, b"nothing to commit\n", stderr), "");
        let note = failure_note(false, false, b"nothing to commit\n", stderr);
        assert!(note.contains("could not read Username"));
        assert!(note.contains("40%"));
        assert!(!note.contains("nothing to commit"));
        let stdout_note = failure_note(false, false, b"nothing to commit, working tree clean\n", b"");
        assert!(stdout_note.contains("nothing to commit"));
    }
}

#[cfg(test)]
mod failure_tests {
    use super::run;

    #[test]
    fn empty_commit_failure_includes_the_stdout_explanation() {
        let repo = std::env::temp_dir().join(format!(
            "awegit-empty-commit-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            run(&repo, args).unwrap_or_else(|error| panic!("git {}: {error}", args.join(" ")));
        };
        git(&["init", "-q", "-b", "main", "--template="]);
        git(&["config", "user.name", "Test"]);
        git(&["config", "user.email", "test@example.com"]);
        git(&["config", "commit.gpgsign", "false"]);
        git(&["config", "core.hooksPath", ".git/no-hooks"]);
        let message = run(&repo, &["commit", "-m", "test: empty"])
            .unwrap_err()
            .to_string();
        let lower = message.to_ascii_lowercase();
        assert!(
            lower.contains("nothing to commit") || lower.contains("working tree clean"),
            "{message}"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }
}

#[cfg(test)]
mod launch_tests {
    use std::path::{Path, PathBuf};

    use super::{bundled_launch, launch_beside};

    fn bundle(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("awegit-git-launch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("cmd")).unwrap();
        std::fs::create_dir_all(root.join("usr").join("bin")).unwrap();
        std::fs::write(root.join("cmd").join("git.exe"), b"shim").unwrap();
        root
    }

    fn real_git(root: &Path, arch: &str) -> PathBuf {
        let bin = root.join(arch).join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let real = bin.join("git.exe");
        std::fs::write(&real, b"real").unwrap();
        real
    }

    #[test]
    fn finds_the_real_git_in_every_layout() {
        for arch in ["ucrt64", "mingw64", "clang64"] {
            let root = bundle(arch);
            let real = real_git(&root, arch);
            let launch = launch_beside(&root.join("cmd")).expect("real git");
            assert_eq!(Path::new(&launch.program), real);
            let prefix = launch
                .path_prefix
                .expect("helper path")
                .to_string_lossy()
                .to_string();
            assert!(prefix.contains(arch));
            assert!(prefix.contains("usr"));
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn ucrt64_wins_when_several_layouts_exist() {
        let root = bundle("both");
        real_git(&root, "mingw64");
        let real = real_git(&root, "ucrt64");
        let launch = launch_beside(&root.join("cmd")).expect("real git");
        assert_eq!(Path::new(&launch.program), real);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn falls_back_to_the_shim_by_absolute_path() {
        let root = bundle("shim");
        let launch = bundled_launch(&root.join("cmd")).expect("shim");
        assert_eq!(Path::new(&launch.program), root.join("cmd").join("git.exe"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn gives_up_when_the_bundle_has_no_git() {
        let root = std::env::temp_dir().join(format!("awegit-git-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("cmd")).unwrap();
        assert!(bundled_launch(&root.join("cmd")).is_none());
        assert!(launch_beside(&root).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod secret_tests {
    use super::{askpass_dir, askpass_script, needs_passphrase};

    #[test]
    fn read_commands_do_not_get_the_signing_passphrase() {
        assert!(!needs_passphrase(&["status", "--porcelain"]));
        assert!(!needs_passphrase(&[
            "-c",
            "core.quotepath=false",
            "diff",
            "--cached"
        ]));
        assert!(!needs_passphrase(&["--no-pager", "log", "-5"]));
        assert!(needs_passphrase(&["commit", "-m", "message"]));
        assert!(needs_passphrase(&["push", "origin", "main"]));
        assert!(needs_passphrase(&[
            "-c",
            "commit.gpgsign=true",
            "tag",
            "-s",
            "v1"
        ]));
    }

    #[test]
    fn askpass_script_reads_the_passphrase_from_a_private_file() {
        let script = askpass_script("s3cret").expect("askpass script");
        let secret = askpass_dir().expect("askpass dir").join("secret");
        assert_eq!(std::fs::read_to_string(&secret).unwrap(), "s3cret\n");
        let body = std::fs::read_to_string(script).unwrap();
        assert!(!body.contains("AWE_ASKPASS"), "{body}");
        assert!(body.contains("cat "), "{body}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&secret).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = std::fs::remove_file(secret);
    }
}

#[cfg(test)]
mod cancel_tests {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::{cancel_reads, cancel_repo, lock_tracked, repo_key, run, stop_generation, track};

    fn sleeper() -> Command {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("ping");
            command.args(["-n", "30", "127.0.0.1"]);
            command
        } else {
            let mut command = Command::new("sleep");
            command.arg("30");
            command
        };
        command.stdout(Stdio::null()).stderr(Stdio::null());
        command
    }

    /// Kills the generation when the test unwinds, so a failed assertion does not leave a sleeper.
    struct StopLater(u64);

    impl Drop for StopLater {
        fn drop(&mut self) {
            if self.0 != 0 {
                stop_generation(self.0);
            }
        }
    }

    #[test]
    fn cancel_repo_does_not_kill_a_later_command() {
        let repo = std::path::Path::new("repo-cancel-later");
        let (mut older, older_group) = crate::proc::spawn_prepared(&mut sleeper()).unwrap();
        let older_gen = track(repo_key(repo), older.id(), false, older_group);
        let _stop_older = StopLater(older_gen);
        let (mut newer, newer_group) = crate::proc::spawn_prepared(&mut sleeper()).unwrap();
        let newer_gen = track(repo_key(repo), newer.id(), false, newer_group);
        let _stop_newer = StopLater(newer_gen);
        let started = Instant::now();
        stop_generation(older_gen);
        let status = older.wait().unwrap();
        assert!(!status.success(), "{status}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "cancel took {:?}",
            started.elapsed()
        );
        assert!(
            newer.try_wait().unwrap().is_none(),
            "later command was killed"
        );
        stop_generation(newer_gen);
        let status = newer.wait().unwrap();
        assert!(!status.success(), "{status}");
    }

    #[test]
    fn cancel_reads_leaves_a_write_running() {
        let repo = std::path::Path::new("repo-cancel-reads");
        let (mut write, write_group) = crate::proc::spawn_prepared(&mut sleeper()).unwrap();
        let write_gen = track(repo_key(repo), write.id(), false, write_group);
        let _stop_write = StopLater(write_gen);
        let (mut read, read_group) = crate::proc::spawn_prepared(&mut sleeper()).unwrap();
        let read_gen = track(repo_key(repo), read.id(), true, read_group);
        let _stop_read = StopLater(read_gen);
        cancel_reads(repo);
        assert!(write.try_wait().unwrap().is_none(), "write was killed");
        let status = read.wait().unwrap();
        assert!(!status.success(), "{status}");
        cancel_repo(repo);
        let status = write.wait().unwrap();
        assert!(!status.success(), "{status}");
    }

    #[test]
    fn cancel_repo_stops_a_git_child_quickly() {
        let dir = std::env::temp_dir().join(format!("awegit-cancel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let init = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(
            init.status.success(),
            "{}",
            String::from_utf8_lossy(&init.stderr)
        );
        let repo = dir.clone();
        let handle = std::thread::spawn(move || {
            let hang = if cfg!(windows) {
                "!ping -n 30 127.0.0.1"
            } else {
                "!sleep 30"
            };
            let alias = format!("alias.hang={hang}");
            run(&repo, &["-c", &alias, "hang"])
        });
        let started = Instant::now();
        let mut saw = false;
        while started.elapsed() < Duration::from_secs(5) {
            let key = repo_key(&dir);
            let running = lock_tracked().iter().any(|item| item.key == key);
            if running {
                saw = true;
                break;
            }
            if handle.is_finished() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(saw, "git hang did not stay running");
        let killed = Instant::now();
        cancel_repo(&dir);
        let result = handle.join().expect("hang thread");
        assert!(
            killed.elapsed() < Duration::from_secs(5),
            "cancel took {:?}",
            killed.elapsed()
        );
        assert!(result.is_err(), "hanging git should fail once cancelled");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod version_tests {
    use super::parse_git_version;

    #[test]
    fn reads_supported_git_versions() {
        assert_eq!(parse_git_version("git version 2.39.1"), Some((2, 39)));
        assert_eq!(parse_git_version("git version 2.40.0"), Some((2, 40)));
        assert_eq!(
            parse_git_version("git version 2.56.0.windows.1"),
            Some((2, 56))
        );
        assert_eq!(parse_git_version(""), None);
        assert_eq!(parse_git_version("git version"), None);
    }
}

#[cfg(test)]
mod https_credential_tests {
    use super::{https_prefix, set_https_credentials, HttpsCredential};

    fn cred(host: &str, username: &str, secret: &str, preferred: bool) -> HttpsCredential {
        HttpsCredential {
            host: host.into(),
            username: username.into(),
            secret: secret.into(),
            preferred,
        }
    }

    #[test]
    fn helper_args_leave_the_token_in_the_script() {
        let secret = "ghp_test_secret_value";
        set_https_credentials(vec![cred("github.com", "octocat", secret, true)]);
        let (args, dir) = https_prefix(&["fetch", "origin"]);
        let joined = args.join("\n");
        assert!(!joined.contains(secret), "{joined}");
        assert!(joined.contains("credential.https://github.com.helper="), "{joined}");
        assert!(joined.contains("credential.http://github.com.helper="), "{joined}");
        let dir = dir.expect("helper dir");
        let body = std::fs::read_to_string(dir.join("helper.sh")).unwrap();
        assert!(body.contains(secret), "{body}");
        assert!(body.contains("-z \"$user\""), "{body}");
        set_https_credentials(Vec::new());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unpreferred_account_does_not_answer_an_empty_username() {
        set_https_credentials(vec![cred("github.com", "octocat", "ghp_other_secret", false)]);
        let (args, dir) = https_prefix(&["push", "origin"]);
        assert!(!args.join("\n").contains("ghp_other_secret"));
        let dir = dir.expect("helper dir");
        let body = std::fs::read_to_string(dir.join("helper.sh")).unwrap();
        assert!(!body.contains("-z \"$user\""), "{body}");
        assert!(body.contains("octocat"), "{body}");
        set_https_credentials(Vec::new());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn status_does_not_install_an_https_helper() {
        set_https_credentials(vec![cred("github.com", "octocat", "ghp_status_secret", true)]);
        let (args, dir) = https_prefix(&["status", "--porcelain"]);
        assert!(args.is_empty());
        assert!(dir.is_none());
        set_https_credentials(Vec::new());
    }
}
