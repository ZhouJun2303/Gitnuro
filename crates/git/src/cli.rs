//! Run `git` for changes that must match the real Git command line.
//!
//! Reads that only need the object database stay in gitoxide. Staging, committing,
//! and the text of a diff go through `git` so filters, hooks, and line endings match
//! what the user already has configured. Nothing here writes the global gitconfig.

use std::cell::{Cell, RefCell};
use std::io::Read;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::Error;

static BUNDLED_GIT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static CHILDREN: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());
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
    path.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase()
}

thread_local! {
    static OPERATION_REPO: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
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
    OPERATION_REPO.with(|slot| slot.borrow().clone()).unwrap_or_else(|| fallback.to_path_buf())
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
        let dir = PathBuf::from(bundled);
        if let Some(found) = launch_beside(&dir) {
            return found;
        }
        return GitLaunch {
            program: OsString::from("git"),
            path_prefix: Some(dir.into_os_string()),
        };
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

/// `dir` is Git's `cmd` folder. The real binary is `../mingw64/bin/git.exe`.
fn launch_beside(dir: &Path) -> Option<GitLaunch> {
    let mingw = dir.join("..").join("mingw64").join("bin");
    let real = mingw.join("git.exe");
    if !real.is_file() {
        return None;
    }
    Some(GitLaunch {
        program: real.into_os_string(),
        path_prefix: Some(helper_path(&mingw)),
    })
}

fn helper_path(mingw_bin: &Path) -> OsString {
    let mut dirs = vec![mingw_bin.to_path_buf()];
    if let Some(root) = mingw_bin.parent().and_then(|parent| parent.parent()) {
        let usr = root.join("usr").join("bin");
        if usr.is_dir() {
            dirs.push(usr);
        }
    }
    std::env::join_paths(&dirs).unwrap_or_else(|_| mingw_bin.as_os_str().to_os_string())
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
    let Some(prefix) = prefix.filter(|prefix| !prefix.is_empty()) else { return };
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
    if let Ok(mut guard) = PROCESS.lock() {
        guard.session.ask_user = user;
        guard.passphrase = secret;
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

/// Stop every `git` process this library started, including grandchildren.
pub fn cancel_running() {
    let pids = CHILDREN.lock().map(|guard| guard.clone()).unwrap_or_default();
    for (_, pid) in pids {
        stop_process_tree(pid);
    }
}

/// Stop `git` processes started for this repository. Other repositories keep running.
pub fn cancel_repo(repo: &Path) {
    let key = repo_key(repo);
    let pids = CHILDREN
        .lock()
        .map(|guard| guard.iter().filter(|(item, _)| item == &key).map(|(_, pid)| *pid).collect::<Vec<_>>())
        .unwrap_or_default();
    for pid in pids {
        stop_process_tree(pid);
    }
}

pub fn stop_process_tree(pid: u32) {
    if cfg!(windows) {
        let mut command = Command::new("taskkill");
        hide_console(&mut command);
        let _ = command.args(["/F", "/T", "/PID", &pid.to_string()]).output();
    } else {
        let _ = Command::new("kill").args(["-TERM", &format!("-{pid}")]).output();
        let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).output();
    }
}

pub fn run(repo: &Path, args: &[&str]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], None, true, false)
}

/// Like `run`, but a non-zero exit still returns the output. Spawn failures stay errors.
pub fn run_output(repo: &Path, args: &[&str]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], None, true, true)
}

pub fn run_env(repo: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, env, None, true, false)
}

pub fn run_stdin(repo: &Path, args: &[&str], stdin: &[u8]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], Some(stdin), true, false)
}

/// Like `run_stdin`, but a non-zero exit still returns the output.
pub(crate) fn run_stdin_output(repo: &Path, args: &[&str], stdin: &[u8]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], Some(stdin), true, true)
}

/// Run git without replacing a repository-local `user.name`.
pub(crate) fn run_repo_author(repo: &Path, args: &[&str]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], None, false, false)
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
        Ok(output) => output.status.success() && output.stdout.iter().any(|byte| !byte.is_ascii_whitespace()),
        Err(_) => false,
    }
}

fn run_prepared(
    repo: &Path,
    args: &[&str],
    env: &[(&str, &str)],
    stdin_bytes: Option<&[u8]>,
    apply_author: bool,
    allow_failure: bool,
) -> Result<std::process::Output, Error> {
    let progress = progress_hook();
    let mut command = git_command();
    command.current_dir(repo).env("GIT_TERMINAL_PROMPT", "0");
    if progress.is_some() {
        command.env("GIT_LFS_FORCE_PROGRESS", "1");
        command.args(progress_args(args));
    } else {
        command.args(args);
    }
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
    if apply_author && !session.author_name.is_empty() {
        command.env("GIT_AUTHOR_NAME", &session.author_name);
        command.env("GIT_COMMITTER_NAME", &session.author_name);
    }
    if apply_author && !session.author_email.is_empty() {
        command.env("GIT_AUTHOR_EMAIL", &session.author_email);
        command.env("GIT_COMMITTER_EMAIL", &session.author_email);
    }
    if !session.ssl_verify {
        command.env("GIT_SSL_NO_VERIFY", "1");
    }
    if !session.ssl_ca.is_empty() {
        command.env("GIT_SSL_CAINFO", &session.ssl_ca);
    }
    if !passphrase.is_empty() {
        if let Some(script) = askpass_script() {
            command.env("GIT_ASKPASS", &script);
            command.env("SSH_ASKPASS", &script);
            command.env("SSH_ASKPASS_REQUIRE", "force");
            command.env("AWE_ASKPASS", &passphrase);
            command.env("AWE_ASK_USER", &session.ask_user);
        }
    }
    for (key, value) in env {
        command.env(key, value);
    }
    let started = std::time::Instant::now();
    let owned = operation_path(repo);
    let mut child = command.spawn().map_err(Error::GitMissing)?;
    let pid = child.id();
    if let Ok(mut guard) = CHILDREN.lock() {
        guard.push((repo_key(&owned), pid));
    }
    if let Some(bytes) = stdin_bytes {
        use std::io::Write;
        let write_result = child.stdin.as_mut().map(|stdin| stdin.write_all(bytes));
        if let Some(Err(source)) = write_result {
            stop_process_tree(pid);
            let _ = child.wait();
            untrack(pid);
            return Err(Error::Read {
                path: "git stdin".to_string(),
                source,
            });
        }
        drop(child.stdin.take());
    }
    let output = if let Some(progress) = progress {
        stream_output(&mut child, progress, owned.display().to_string())?
    } else {
        child.wait_with_output().map_err(Error::GitMissing)?
    };
    untrack(pid);
    let note = failure_note(output.status.success(), allow_failure, &output.stderr);
    if let Some(hook) = TRACE.get() {
        hook(args, started.elapsed(), output.status.success() || allow_failure, &note);
    }
    note_captured(&output.stdout, &output.stderr);
    if output.status.success() || allow_failure {
        return Ok(output);
    }
    let stderr = settle_stderr(&output.stderr);
    Err(Error::Git(if stderr.is_empty() {
        format!("git {} failed", args.join(" "))
    } else {
        stderr
    }))
}

/// Read both pipes while `git` runs. Stderr is scanned for `45% (12/30)` lines.
fn stream_output(child: &mut Child, progress: ProgressHook, repo: String) -> Result<std::process::Output, Error> {
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stderr_task = match std::thread::Builder::new().name("awegit-progress".into()).spawn(move || read_stderr(stderr_pipe, progress, repo)) {
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
    Ok(std::process::Output { status, stdout, stderr })
}

fn read_stderr(pipe: Option<impl Read>, progress: ProgressHook, repo: String) -> Vec<u8> {
    let Some(mut pipe) = pipe else { return Vec::new() };
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

fn drain_progress(pending: &mut Vec<u8>, last: &mut Option<u8>, progress: &ProgressHook, repo: &str) {
    loop {
        let Some(index) = pending.iter().position(|byte| *byte == b'\r' || *byte == b'\n') else { break };
        let piece: Vec<u8> = pending.drain(..=index).collect();
        report_percent(&piece, last, progress, repo);
    }
}

fn report_percent(piece: &[u8], last: &mut Option<u8>, progress: &ProgressHook, repo: &str) {
    let text = String::from_utf8_lossy(piece);
    let Some(percent) = percent_in(text.trim()) else { return };
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
        let Ok(percent) = text[start..index].parse::<u8>() else { continue };
        if percent <= 100 {
            found = Some(percent);
        }
    }
    found
}

/// Stderr tail for a failure the caller will surface. Expected non-zero exits stay empty.
fn failure_note(success: bool, allow_failure: bool, stderr: &[u8]) -> String {
    if success || allow_failure {
        return String::new();
    }
    tail_text(&settle_stderr(stderr), 500)
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
            let kept = line.trim_end_matches(['\r', ' ']).rsplit('\r').next().unwrap_or("").trim();
            if kept.is_empty() { None } else { Some(kept.to_string()) }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Insert `--progress` for commands that can report a transfer or checkout percent.
fn progress_args(args: &[&str]) -> Vec<String> {
    let mut owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    if owned.iter().any(|arg| arg == "--progress" || arg == "--no-progress") {
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
        Some("fetch" | "pull" | "push" | "clone" | "checkout" | "switch" | "restore") => Some(index + 1),
        Some("submodule") if owned.get(index + 1).map(String::as_str) == Some("update") => Some(index + 2),
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
    if let Ok(mut guard) = CHILDREN.lock() {
        guard.retain(|(_, item)| *item != pid);
    }
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
        if let Some(program) = gpg_wrapper() {
            prefix.push("-c".into());
            prefix.push(format!("gpg.program={program}"));
        }
    }
    prefix
}

fn askpass_script() -> Option<String> {
    let dir = std::env::temp_dir().join("awegit-askpass");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("ask.sh");
    let body = "#!/bin/sh\ncase \"$1\" in\n*[Uu]sername*)\n  printf '%s\\n' \"$AWE_ASK_USER\"\n  ;;\n*)\n  printf '%s\\n' \"$AWE_ASKPASS\"\n  ;;\nesac\n";
    std::fs::write(&path, body).ok()?;
    Some(path.display().to_string().replace('\\', "/"))
}

fn gpg_wrapper() -> Option<String> {
    let dir = std::env::temp_dir().join("awegit-askpass");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("gpgwrap.sh");
    let body = "#!/bin/sh\ntmp=\"${TMP:-/tmp}/awegit-pass-$$\"\nprintf '%s\\n' \"$AWE_ASKPASS\" > \"$tmp\"\ngpg --batch --yes --pinentry-mode loopback --passphrase-file \"$tmp\" \"$@\"\ncode=$?\nrm -f \"$tmp\"\nexit $code\n";
    std::fs::write(&path, body).ok()?;
    Some(path.display().to_string().replace('\\', "/"))
}

/// A commit id or a revision such as `HEAD~2`. Rejects option-like values.
pub fn check_rev(rev: &str) -> Result<&str, Error> {
    let safe = !rev.is_empty()
        && !rev.starts_with('-')
        && !rev.contains("..")
        && !rev.contains([' ', '\n', '\r', '\\'])
        && rev.chars().all(|c| name_char(c) || matches!(c, '~' | '^' | '@' | '{' | '}'));
    if safe {
        Ok(rev)
    } else {
        Err(Error::Rev(rev.to_string()))
    }
}

/// Characters Git accepts in a ref name that cannot change how the command line is read.
/// Any letter or digit is fine, so branch and tag names in other languages work.
fn name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '/' | '_' | '.' | '-' | '+' | '#' | ',' | '=' | '%' | '!' | '&' | '(' | ')' | '@')
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
pub fn check_path(path: &str) -> Result<&str, Error> {
    let escapes = path.is_empty()
        || path.starts_with(['/', '\\'])
        || path.contains(':')
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
        assert_eq!(percent_in("Receiving objects:  45% (10/22), 1 KiB | 1 MiB/s"), Some(45));
        assert_eq!(percent_in("remote: Counting objects: 100% (15/15), done."), Some(100));
        assert_eq!(percent_in("Writing objects: 100% (11/11), done.\rReceiving objects: 3% (1/30)"), Some(3));
        assert_eq!(percent_in("Downloading LFS objects:   0% (0/2), 0 B | 0 B/s"), Some(0));
        assert_eq!(percent_in("nothing to report"), None);
        assert_eq!(percent_in("already 100% complete"), None);
    }

    #[test]
    fn keeps_the_last_progress_line() {
        let raw = b"Receiving objects:  10% (1/10)\rReceiving objects:  40% (4/10)\r\nfatal: early EOF\n";
        assert_eq!(settle_stderr(raw), "Receiving objects:  40% (4/10)\nfatal: early EOF");
    }

    #[test]
    fn adds_progress_only_where_git_can_report_it() {
        assert_eq!(progress_args(&["fetch", "--all"]), vec!["fetch", "--progress", "--all"]);
        assert_eq!(progress_args(&["pull", "--rebase"]), vec!["pull", "--progress", "--rebase"]);
        assert_eq!(progress_args(&["push", "--force-with-lease"]), vec!["push", "--progress", "--force-with-lease"]);
        assert_eq!(progress_args(&["clone", "https://example.com/a.git", "a"]), vec!["clone", "--progress", "https://example.com/a.git", "a"]);
        assert_eq!(
            progress_args(&["submodule", "update", "--init", "--recursive"]),
            vec!["submodule", "update", "--progress", "--init", "--recursive"]
        );
        assert_eq!(progress_args(&["submodule", "sync", "--", "path"]), vec!["submodule", "sync", "--", "path"]);
        assert_eq!(
            progress_args(&["-c", "commit.gpgsign=false", "commit", "-m", "hi"]),
            vec!["-c", "commit.gpgsign=false", "commit", "-m", "hi"]
        );
        assert_eq!(progress_args(&["checkout", "--ours", "--", "file"]), vec!["checkout", "--progress", "--ours", "--", "file"]);
        assert_eq!(progress_args(&["fetch", "--progress"]), vec!["fetch", "--progress"]);
    }

    #[test]
    fn failure_note_keeps_stderr_only_for_real_errors() {
        let stderr = b"Receiving objects:  10% (1/10)\rReceiving objects:  40% (4/10)\r\nfatal: could not read Username\n";
        assert_eq!(failure_note(true, false, stderr), "");
        assert_eq!(failure_note(false, true, stderr), "");
        let note = failure_note(false, false, stderr);
        assert!(note.contains("could not read Username"));
        assert!(note.contains("40%"));
    }
}

#[cfg(test)]
mod launch_tests {
    use std::path::Path;

    use super::launch_beside;

    #[test]
    fn prefers_the_real_git_beside_the_cmd_wrapper() {
        let root = std::env::temp_dir().join(format!("awegit-git-launch-{}", std::process::id()));
        let cmd = root.join("cmd");
        let mingw = root.join("mingw64").join("bin");
        let usr = root.join("usr").join("bin");
        std::fs::create_dir_all(&cmd).unwrap();
        std::fs::create_dir_all(&mingw).unwrap();
        std::fs::create_dir_all(&usr).unwrap();
        std::fs::write(cmd.join("git.exe"), b"wrapper").unwrap();
        std::fs::write(mingw.join("git.exe"), b"real").unwrap();
        let launch = launch_beside(&cmd).expect("mingw git");
        let program = Path::new(&launch.program);
        assert!(program.is_file());
        assert!(program.components().any(|part| part.as_os_str() == "mingw64"));
        let prefix = launch.path_prefix.expect("helper path");
        let text = prefix.to_string_lossy();
        assert!(text.contains("mingw64"));
        assert!(text.contains("usr"));
        assert!(launch_beside(&root).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
