//! Run `git` for changes that must match the real Git command line.
//!
//! Reads that only need the object database stay in gitoxide. Staging, committing,
//! and the text of a diff go through `git` so filters, hooks, and line endings match
//! what the user already has configured. Nothing here writes the global gitconfig.

use std::cell::RefCell;
use std::path::Path;
use std::process::Command;

use crate::Error;

thread_local! {
    static HTTP_PROXY: RefCell<String> = const { RefCell::new(String::new()) };
}

static BUNDLED_GIT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Prefer a Portable Git `cmd` directory shipped beside the executable.
pub fn use_bundled_git(dir: &Path) {
    let _ = BUNDLED_GIT.set(dir.display().to_string());
}

/// Proxy used by the next `git` processes on this thread. Empty clears it.
/// This does not write gitconfig.
pub fn set_http_proxy(value: Option<String>) {
    HTTP_PROXY.with(|slot| *slot.borrow_mut() = value.unwrap_or_default());
}

pub fn run(repo: &Path, args: &[&str]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], None)
}

pub fn run_env(repo: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, env, None)
}

pub fn run_stdin(repo: &Path, args: &[&str], stdin: &[u8]) -> Result<std::process::Output, Error> {
    run_prepared(repo, args, &[], Some(stdin))
}

fn run_prepared(
    repo: &Path,
    args: &[&str],
    env: &[(&str, &str)],
    stdin_bytes: Option<&[u8]>,
) -> Result<std::process::Output, Error> {
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0");
    if let Some(dir) = BUNDLED_GIT.get() {
        let mut path = std::ffi::OsString::from(dir);
        path.push(";");
        if let Some(current) = std::env::var_os("PATH") {
            path.push(current);
        }
        command.env("PATH", path);
    }
    let proxy = HTTP_PROXY.with(|slot| slot.borrow().clone());
    if !proxy.is_empty() {
        command.env("http_proxy", &proxy);
        command.env("https_proxy", &proxy);
        command.env("HTTP_PROXY", &proxy);
        command.env("HTTPS_PROXY", &proxy);
    }
    for (key, value) in env {
        command.env(key, value);
    }
    let output = if let Some(bytes) = stdin_bytes {
        let mut child = command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(Error::GitMissing)?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(bytes).map_err(|source| Error::Read {
                path: "git stdin".to_string(),
                source,
            })?;
        }
        child.wait_with_output().map_err(Error::GitMissing)?
    } else {
        command.output().map_err(Error::GitMissing)?
    };
    if output.status.success() {
        return Ok(output);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(Error::Git(if stderr.is_empty() {
        format!("git {} failed", args.join(" "))
    } else {
        stderr
    }))
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
