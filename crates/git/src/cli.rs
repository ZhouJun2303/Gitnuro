//! Run `git` for changes that must match the real Git command line.
//!
//! Reads that only need the object database stay in gitoxide. Staging, committing,
//! and the text of a diff go through `git` so filters, hooks, and line endings match
//! what the user already has configured. Nothing here writes the global gitconfig.

use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use crate::Error;

static BUNDLED_GIT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static CHILDREN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

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
pub fn use_bundled_git(dir: &Path) {
    let _ = BUNDLED_GIT.set(dir.display().to_string());
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
    for pid in pids {
        stop_process_tree(pid);
    }
}

pub fn stop_process_tree(pid: u32) {
    if cfg!(windows) {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .output();
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

/// Run git without replacing a repository-local `user.name`.
pub(crate) fn run_repo_author(repo: &Path, args: &[&str]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], None, false, false)
}

/// True when this repository has its own `user.name`.
pub(crate) fn has_local_author(repo: &Path) -> bool {
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .args(["config", "--local", "--get", "user.name"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(dir) = BUNDLED_GIT.get() {
        let mut path = std::ffi::OsString::from(dir);
        path.push(";");
        if let Some(current) = std::env::var_os("PATH") {
            path.push(current);
        }
        command.env("PATH", path);
    }
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
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(dir) = BUNDLED_GIT.get() {
        let mut path = std::ffi::OsString::from(dir);
        path.push(";");
        if let Some(current) = std::env::var_os("PATH") {
            path.push(current);
        }
        command.env("PATH", path);
    }
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
    let mut child = command.spawn().map_err(Error::GitMissing)?;
    let pid = child.id();
    if let Ok(mut guard) = CHILDREN.lock() {
        guard.push(pid);
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
    let output = child.wait_with_output().map_err(Error::GitMissing)?;
    untrack(pid);
    if output.status.success() || allow_failure {
        return Ok(output);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(Error::Git(if stderr.is_empty() {
        format!("git {} failed", args.join(" "))
    } else {
        stderr
    }))
}

fn untrack(pid: u32) {
    if let Ok(mut guard) = CHILDREN.lock() {
        guard.retain(|item| *item != pid);
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
        && rev
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '~' | '^' | '@' | '{' | '}' | '/' | '_' | '.' | '-'));
    if safe {
        Ok(rev)
    } else {
        Err(Error::Rev(rev.to_string()))
    }
}

/// Branch, tag, and remote names passed to `git`.
pub fn check_name(name: &str) -> Result<&str, Error> {
    let safe = !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('/')
        && !name.ends_with('.')
        && !name.contains("..")
        && !name.contains('@')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '.' | '-'));
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
