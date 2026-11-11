//! The one stash an operation keeps while it rewrites the work tree.
//!
//! Git's own `--autostash` is hidden from `git stash list` while a rebase or merge
//! is paused, and it cannot restore the staged/unstaged split. AweGit keeps one
//! visible entry instead, records its commit id in `.git/awegit`, and puts it back
//! when the operation settles.

use std::collections::HashSet;
use std::path::Path;

use crate::cli::run;
use crate::model::{in_progress, pending_autostash, set_pending_autostash};
use crate::Error;

/// What happened to the stash a finished operation had kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Restored {
    /// No stash was pending.
    None,
    /// The stash was applied and dropped.
    Applied,
    /// The operation is still paused, or its result still has unmerged paths.
    Kept,
    /// The stash could not apply cleanly; Git left the conflicts and kept the entry.
    Conflict,
    /// The recorded entry is gone, so there is nothing left to restore.
    Missing,
}

/// Stash every work-tree change, including untracked files, except the root
/// `.fakeignore` that only AweGit reads. `None` when the tree has nothing to stash.
pub(crate) fn push(repo: &Path, label: &str) -> Result<Option<String>, Error> {
    let paths = changed_paths(repo)?;
    if paths.is_empty() {
        return Ok(None);
    }
    let exclude_fakeignore = fakeignore_is_untracked(repo);
    let listed: Vec<String> = if exclude_fakeignore {
        paths
            .iter()
            .filter(|path| path.as_str() != ".fakeignore")
            .cloned()
            .collect()
    } else {
        paths.clone()
    };
    if listed.is_empty() {
        return Ok(None);
    }
    let message = stash_message("AweGit autostash", label, Some(describe_paths(&listed)));
    let before = stash_hashes(repo);
    let mut args = vec![
        "stash",
        "push",
        "--include-untracked",
        "-m",
        message.as_str(),
    ];
    if exclude_fakeignore {
        args.extend(["--", ".", ":(exclude).fakeignore"]);
    }
    run(repo, &args)?;
    let output = run(repo, &["rev-parse", "--verify", "refs/stash"]).ok();
    let oid = output
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();
    if oid.is_empty() || before.contains(&oid) {
        return Ok(None);
    }
    if let Err(error) = set_pending_autostash(repo, Some(&oid)) {
        // Do not leave the work tree empty behind an error the caller will report.
        if run(repo, &["stash", "apply", "--index", "stash@{0}"]).is_ok() {
            let _ = run(repo, &["stash", "drop", "stash@{0}"]);
        }
        return Err(error);
    }
    crate::flow::emit(crate::flow::Event {
        key: format!("saved-{oid}"), kind: "local-saved".into(), state: "completed".into(),
        stash: oid.clone(), source: crate::flow::commit(repo, &oid),
        base: crate::flow::commit(repo, &format!("{oid}^1")),
        files: listed.into_iter().map(|path| crate::flow::File { path, ..Default::default() }).collect(),
        ..Default::default()
    });
    Ok(Some(oid))
}

/// Put the recorded stash back if no operation is paused and the index has no
/// unmerged paths. A conflicting apply leaves Git's conflict markers in place and
/// keeps the entry for the user to drop after resolving.
pub(crate) fn finish_if_settled(repo: &Path) -> Result<Restored, Error> {
    let Some(oid) = pending_autostash(repo) else {
        return Ok(Restored::None);
    };
    if in_progress(repo)?.is_some() || has_unmerged(repo) {
        return Ok(Restored::Kept);
    }
    let Some(index) = stash_index(repo, &oid) else {
        set_pending_autostash(repo, None)?;
        crate::flow::emit(crate::flow::Event {
            key: format!("restore-{oid}"), kind: "local-restore".into(), state: "failed".into(),
            stash: oid, detail: "stash-missing".into(), ..Default::default()
        });
        return Ok(Restored::Missing);
    };
    let selector = format!("stash@{{{index}}}");
    let event = crate::flow::Event {
        key: format!("restore-{oid}"), kind: "local-restore".into(), state: "running".into(),
        stash: oid.clone(), source: crate::flow::commit(repo, &oid), target: crate::flow::commit(repo, "HEAD"),
        base: crate::flow::commit(repo, &format!("{oid}^1")), ..Default::default()
    };
    crate::flow::emit(event.clone());
    match run(repo, &["stash", "apply", "--index", &selector]) {
        Ok(_) => {
            let dropped = run(repo, &["stash", "drop", &selector]).is_ok();
            set_pending_autostash(repo, None)?;
            crate::flow::emit(crate::flow::Event {
                state: "completed".into(), detail: if dropped { "stash-restored" } else { "stash-restored-kept" }.into(), ..event
            });
            Ok(Restored::Applied)
        }
        Err(error) => {
            set_pending_autostash(repo, None)?;
            if has_unmerged(repo) {
                crate::flow::emit(crate::flow::Event { state: "conflict".into(), detail: "stash-kept".into(), ..event });
                Ok(Restored::Conflict)
            } else {
                crate::flow::emit(crate::flow::Event { state: "failed".into(), detail: "stash-kept".into(), ..event });
                Err(error)
            }
        }
    }
}

/// Refuse to start an operation while an earlier one still has a stash to restore
/// or is paused. This also recovers a pending stash left by a restart.
pub(crate) fn settle_before(repo: &Path) -> Result<(), Error> {
    if pending_autostash(repo).is_none() {
        return Ok(());
    }
    if in_progress(repo)?.is_some() || has_unmerged(repo) {
        return Err(Error::Git(
            "finish the current rebase, merge, cherry-pick, or revert before starting another operation".into(),
        ));
    }
    match finish_if_settled(repo)? {
        Restored::Applied | Restored::None | Restored::Missing => Ok(()),
        Restored::Kept => Err(Error::Git("local changes are still being restored".into())),
        Restored::Conflict => Err(Error::Git(
            "local changes from an earlier operation conflict with the work tree; resolve them first".into(),
        )),
    }
}

pub(crate) fn has_unmerged(repo: &Path) -> bool {
    run(repo, &["ls-files", "--unmerged"])
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(false)
}

/// Files held by one stash commit, ready for a stash message.
pub(crate) fn stash_files(repo: &Path, hash: &str) -> Option<String> {
    let output = run(
        repo,
        &[
            "stash",
            "show",
            "--include-untracked",
            "--name-only",
            "-z",
            hash,
        ],
    )
    .or_else(|_| run(repo, &["stash", "show", "--name-only", "-z", hash]))
    .ok()?;
    let paths: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect();
    (!paths.is_empty()).then(|| describe_paths(&paths))
}

pub(crate) fn stash_message(prefix: &str, label: &str, files: Option<String>) -> String {
    match files {
        Some(files) => format!("{prefix}: {label} ({files})"),
        None => format!("{prefix}: {label}"),
    }
}

/// Work-tree paths from `status --porcelain -z`, in the order Git prints them.
fn changed_paths(repo: &Path) -> Result<Vec<String>, Error> {
    let output = run(
        repo,
        &["status", "--porcelain", "-z", "--untracked-files=normal"],
    )?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut entries = text.split('\0');
    let mut paths = Vec::new();
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        let status = &entry[..2];
        paths.push(entry[3..].to_string());
        if status.contains(['R', 'C']) {
            // A rename or copy prints its source path as the next entry.
            entries.next();
        }
    }
    Ok(paths)
}

fn describe_paths(paths: &[String]) -> String {
    const SHOWN: usize = 3;
    let shown = paths
        .iter()
        .take(SHOWN)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    match paths.len() {
        0 => String::new(),
        1 => format!("1 file: {shown}"),
        count if count > SHOWN => format!("{count} files: {shown}, +{}", count - SHOWN),
        count => format!("{count} files: {shown}"),
    }
}

/// A tracked `.fakeignore` is a normal repository file; only the untracked one is
/// AweGit's private rule list and stays out of automatic stashes.
fn fakeignore_is_untracked(repo: &Path) -> bool {
    repo.join(".fakeignore").is_file()
        && run(repo, &["ls-files", "--error-unmatch", "--", ".fakeignore"]).is_err()
}

fn stash_hashes(repo: &Path) -> HashSet<String> {
    run(repo, &["stash", "list", "--format=%H"])
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn stash_index(repo: &Path, oid: &str) -> Option<usize> {
    let listed = run(repo, &["stash", "list", "--format=%H"]).ok()?;
    String::from_utf8_lossy(&listed.stdout)
        .lines()
        .position(|line| line.trim() == oid)
}
