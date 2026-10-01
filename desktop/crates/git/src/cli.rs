//! Run `git` for changes that must match the real Git command line.
//!
//! Reads that only need the object database stay in gitoxide. Staging, committing,
//! and the text of a diff go through `git` so filters, hooks, and line endings match
//! what the user already has configured. Nothing here writes the global gitconfig.

use std::path::Path;
use std::process::Command;

use crate::Error;

pub fn run(repo: &Path, args: &[&str]) -> Result<std::process::Output, Error> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(Error::GitMissing)?;
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
