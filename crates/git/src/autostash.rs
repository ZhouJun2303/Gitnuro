//! The one stash an operation keeps while it rewrites the work tree.
//!
//! Git's own `--autostash` is hidden from `git stash list` while a rebase or merge
//! is paused, and it cannot restore the staged/unstaged split. AweGit keeps one
//! visible entry instead, records its commit id in `.git/awegit`, and puts it back
//! when the operation settles.

use std::path::Path;

use crate::cli::{run, run_stdin};
use crate::model::{in_progress, pending_autostash, pending_autostash_deletions, set_pending_autostash};
use crate::Error;

/// What happened to the stash a finished operation had kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Restored {
    /// No stash was pending.
    None,
    /// The contents were restored; a backup may remain for inspection.
    Applied,
    /// The operation is still paused, or its result still has unmerged paths.
    Kept,
    /// The stash could not apply cleanly; Git left the conflicts and kept the entry.
    Conflict,
    /// The recorded entry is gone, so there is nothing left to restore.
    Missing,
}

/// Stash tracked changes, optionally including untracked files except the root
/// `.fakeignore` that only AweGit reads. `None` when there is nothing to stash.
/// A native two-tree checkout/fast-forward carries disjoint local changes itself.
/// Decide before writing anything; never retry a partially failed checkout/merge.
pub(crate) fn push_for_tree(repo: &Path, label: &str, target: Option<&str>) -> Result<Option<String>, Error> {
    let include_untracked = crate::cli::session_snapshot().0.autostash_include_untracked;
    let incoming = target.and_then(|target| tree_paths(repo, target));
    if incoming.as_ref().is_some_and(Vec::is_empty) { return Ok(None); }
    let changes = {
        let _phase = crate::timing_phase("autostash-check");
        changed_paths(repo, include_untracked)?
    };
    if let Some(incoming) = incoming {
        // Include both sides of renames and parent/child collisions. Git still
        // performs the write and enforces untracked/ignored-file protection.
        // Case aliases can otherwise hide a collision on Windows/macOS. Native
        // Git remains the authority for non-ASCII filesystem case mappings.
        let ignore_case = crate::open_repo(repo).ok().and_then(|repo| repo.config_snapshot().boolean("core.ignoreCase")).unwrap_or(true);
        let ascii = changes.paths.iter().chain(&incoming).all(|path| path.is_ascii());
        let local: Vec<_> = changes.paths.iter().map(|path| if ignore_case { path.to_ascii_lowercase() } else { path.clone() }).collect();
        let incoming: Vec<_> = incoming.into_iter().map(|path| if ignore_case { path.to_ascii_lowercase() } else { path }).collect();
        if (!ignore_case || ascii) && disjoint(&local, &incoming) { return Ok(None); }
    }
    let Changes { paths, exclude_fakeignore, index_only } = changes;
    if paths.is_empty() {
        return Ok(None);
    }
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
    let before = stash_tip(repo);
    let mut args = vec![
        "stash",
        "push",
        "-m",
        message.as_str(),
    ];
    if include_untracked {
        args.push("--include-untracked");
    }
    if exclude_fakeignore {
        args.extend(["--", ".", ":(exclude).fakeignore"]);
    }
    let saved = { let _phase = crate::timing_phase("autostash-save"); run(repo, &args) };
    let after = stash_tip(repo);
    let oid = after.0.clone().unwrap_or_default();
    if let Err(error) = saved {
        // With an exclusion pathspec, Git can save and clean an index-only
        // change (for example a staged new file deleted from disk), then fail
        // while applying its empty cleanup patch. Accept only that exact case,
        // after proving a new stash exists and every included change is saved.
        let empty_cleanup = error.to_string().contains("No valid patches in input")
            && !oid.is_empty() && before != after
            && changed_paths(repo, include_untracked).map(|changes| {
                changes.paths.iter().all(|path| changes.exclude_fakeignore && path == ".fakeignore")
            }).unwrap_or(false);
        if !empty_cleanup {
            return Err(error);
        }
    }
    if oid.is_empty() || before == after {
        return Ok(None);
    }
    if let Err(error) = set_pending_autostash(repo, Some(&oid), &index_only) {
        // Do not leave the work tree empty behind an error the caller will report.
        if run(repo, &["stash", "apply", "--index", "stash@{0}"])
            .and_then(|_| restore_worktree_deletions(repo, &oid, &index_only)).is_ok() {
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
    // Cancelling the primary operation must still permit recovery of saved work.
    crate::cli::without_cancellation(|| finish_saved(repo))
}

fn finish_saved(repo: &Path) -> Result<Restored, Error> {
    let Some(oid) = pending_autostash(repo) else {
        return Ok(Restored::None);
    };
    if in_progress(repo)?.is_some() || has_unmerged(repo) {
        return Ok(Restored::Kept);
    }
    let Some(index) = stash_index(repo, &oid) else {
        set_pending_autostash(repo, None, &[])?;
        crate::flow::emit(crate::flow::Event {
            key: format!("restore-{oid}"), kind: "local-restore".into(), state: "failed".into(),
            stash: oid, detail: "stash-missing".into(), ..Default::default()
        });
        return Ok(Restored::Missing);
    };
    let selector = format!("stash@{{{index}}}");
    let index_only = pending_autostash_deletions(repo)?;
    let event = crate::flow::Event {
        key: format!("restore-{oid}"), kind: "local-restore".into(), state: "running".into(),
        stash: oid.clone(), source: crate::flow::commit(repo, &oid), target: crate::flow::commit(repo, "HEAD"),
        base: crate::flow::commit(repo, &format!("{oid}^1")), ..Default::default()
    };
    crate::flow::emit(event.clone());
    let mut restored_without_index = false;
    let applied = {
        let _phase = crate::timing_phase("autostash-restore");
        let indexed = run(repo, &["stash", "apply", "--index", &oid]);
        // Git applies the saved index patch before touching the worktree. This
        // specific failure means that patch no longer fits the new HEAD; Git
        // explicitly asks us to merge the saved contents without --index.
        // Never retry other errors: they may have already changed files.
        let applied = match indexed {
            Err(error) if error.to_string().contains("conflicts in index. Try without --index.") && !has_unmerged(repo) => {
                restored_without_index = true;
                run(repo, &["stash", "apply", &oid])
            }
            result => result,
        };
        applied
            .and_then(|output| restore_worktree_deletions(repo, &oid, &index_only).map(|_| output))
    };
    match applied {
        Ok(_) => {
            // A content merge cannot promise the original staged/unstaged
            // split. Keep that exact index in the backup for manual inspection.
            let dropped = !restored_without_index && run(repo, &["stash", "drop", &selector]).is_ok();
            set_pending_autostash(repo, None, &[])?;
            crate::flow::emit(crate::flow::Event {
                state: "completed".into(), detail: if restored_without_index { "stash-restored-without-index" } else if dropped { "stash-restored" } else { "stash-restored-kept" }.into(), ..event
            });
            Ok(Restored::Applied)
        }
        Err(error) => {
            if has_unmerged(repo) {
                set_pending_autostash(repo, None, &[])?;
                crate::flow::emit(crate::flow::Event { state: "conflict".into(), detail: "stash-kept".into(), ..event });
                Ok(Restored::Conflict)
            } else {
                crate::flow::emit(crate::flow::Event { state: "failed".into(), detail: "stash-kept".into(), ..event });
                Err(error)
            }
        }
    }
}

/// `stash apply --index` checks newly staged files out even when they were
/// deleted again before the stash. Reapply only those missing worktree entries;
/// Git validates their saved contents before removing them and leaves the index
/// alone. An error keeps the stash, including the original staged contents.
fn restore_worktree_deletions(repo: &Path, oid: &str, paths: &[String]) -> Result<(), Error> {
    let index = format!("{oid}^2");
    let base = format!("{oid}^1");
    for path in paths {
        crate::cli::check_path(path)?;
        match std::fs::symlink_metadata(repo.join(path)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(source) => return Err(Error::Read { path: path.clone(), source }),
            Ok(_) => {}
        }
        let patch = run(repo, &[
            "--literal-pathspecs", "diff", "--binary", "--no-color", "--no-ext-diff", "--no-textconv",
            "--no-renames", "--diff-filter=D", "--src-prefix=a/", "--dst-prefix=b/", &index, &base, "--", path,
        ])?;
        run_stdin(repo, &["apply", "--whitespace=nowarn", "-"], &patch.stdout)?;
    }
    Ok(())
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
    crate::flow::unmerged(repo).map(|files| !files.is_empty()).unwrap_or(true)
}

fn tree_paths(path: &Path, target: &str) -> Option<Vec<String>> {
    crate::cli::check_rev(target).ok()?;
    let repo = crate::open_repo(path).ok()?;
    // Sparse indexes, replacements and submodules require their native protection.
    if repo.git_dir() != repo.common_dir() || path.join(".gitmodules").exists()
        || repo.git_dir().join("info/grafts").exists()
        || repo.config_snapshot().boolean("core.sparseCheckout").unwrap_or(false)
        || repo.config_snapshot().boolean("submodule.recurse").unwrap_or(false) { return None; }
    let index = repo.index_or_empty().ok()?;
    if index.entries().iter().any(|entry| entry.mode == gix::index::entry::Mode::COMMIT || entry.stage_raw() != 0) { return None; }
    for reference in repo.references().ok()?.all().ok()? {
        if reference.ok()?.name().as_bstr().starts_with(b"refs/replace/") { return None; }
    }
    let output = run(path, &["diff", "--raw", "-z", "--no-renames", "--no-ext-diff", "--no-textconv", "HEAD", target, "--"]).ok()?;
    let mut fields = output.stdout.split(|byte| *byte == 0).filter(|field| !field.is_empty());
    let mut paths = Vec::new();
    while let Some(record) = fields.next() {
        let record = std::str::from_utf8(record).ok()?;
        let mut modes = record.strip_prefix(':')?.split_whitespace();
        if modes.next()? == "160000" || modes.next()? == "160000" { return None; }
        let path = std::str::from_utf8(fields.next()?).ok()?;
        if path == ".gitmodules" { return None; }
        paths.push(path.to_string());
    }
    Some(paths)
}

fn disjoint(local: &[String], incoming: &[String]) -> bool {
    let mut paths: Vec<_> = local.iter().map(|path| (path.trim_end_matches('/'), false))
        .chain(incoming.iter().map(|path| (path.as_str(), true))).collect();
    paths.sort_unstable();
    // Remember every ancestor of the opposite origin; an interleaved child must
    // not hide a directory/file collision from a later sibling.
    let mut local_paths = std::collections::HashSet::new();
    let mut incoming_paths = std::collections::HashSet::new();
    for (path, incoming) in paths {
        let other = if incoming { &local_paths } else { &incoming_paths };
        if other.contains(path) || path.match_indices('/').any(|(at, _)| other.contains(&path[..at])) { return false; }
        if incoming { incoming_paths.insert(path); } else { local_paths.insert(path); }
    }
    true
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

struct Changes {
    paths: Vec<String>,
    exclude_fakeignore: bool,
    // Git's stash worktree tree can include these index-only additions. Keep
    // their absence separately, including across an interrupted rebase.
    index_only: Vec<String>,
}

/// Work-tree paths from `status --porcelain -z`, in the order Git prints them.
fn changed_paths(repo: &Path, include_untracked: bool) -> Result<Changes, Error> {
    if let Ok(changes) = changed_paths_native(repo, include_untracked) { return Ok(changes); }
    let output = run(
        repo,
        &["status", "--porcelain", "-z", if include_untracked { "--untracked-files=normal" } else { "--untracked-files=no" }],
    )?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut entries = text.split('\0');
    let mut paths = Vec::new();
    let mut index_only = Vec::new();
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        let status = &entry[..2];
        paths.push(entry[3..].to_string());
        if matches!(status.as_bytes(), [b'A' | b'R' | b'C', b'D']) {
            index_only.push(entry[3..].to_string());
        }
        if status.contains(['R', 'C']) {
            // A rename or copy prints its source path as the next entry.
            if let Some(source) = entries.next() { paths.push(source.to_string()); }
        }
    }
    Ok(Changes { paths, exclude_fakeignore: include_untracked && fakeignore_is_untracked(repo), index_only })
}

fn changed_paths_native(path: &Path, include_untracked: bool) -> Result<Changes, Error> {
    let repo = crate::open_repo(path)?;
    if repo.git_dir() != repo.common_dir() { return Err(Error::Git("use porcelain for linked-worktree exclude rules".into())); }
    let index = repo.index_or_empty().map_err(|error| Error::Git(error.to_string()))?;
    let exclude_fakeignore = include_untracked && path.join(".fakeignore").is_file()
        && index.entry_by_path(gix::bstr::BStr::new(b".fakeignore")).is_none();
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let iter = repo.status(gix::progress::Discard)?
        .untracked_files(if include_untracked { gix::status::UntrackedFiles::Collapsed } else { gix::status::UntrackedFiles::None })
        .into_iter(Vec::<gix::bstr::BString>::new())?;
    for item in iter {
        match item? {
            gix::status::Item::TreeIndex(change) => crate::push_staged(&mut staged, change),
            gix::status::Item::IndexWorktree(change) => crate::push_unstaged(&mut unstaged, change),
        }
    }
    let deleted: std::collections::HashSet<_> = unstaged.iter()
        .filter(|change| change.kind == crate::ChangeKind::Deleted).map(|change| &change.path).collect();
    let index_only = staged.iter().filter(|change| {
        matches!(change.kind, crate::ChangeKind::Added | crate::ChangeKind::Renamed) && deleted.contains(&change.path)
    }).map(|change| change.path.clone()).collect();
    let mut paths: Vec<_> = staged.into_iter().chain(unstaged)
        .flat_map(|change| std::iter::once(change.path).chain(change.previous_path)).collect();
    paths.sort();
    paths.dedup();
    Ok(Changes { paths, exclude_fakeignore, index_only })
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

fn stash_tip(path: &Path) -> (Option<String>, Option<(u64, std::time::SystemTime)>) {
    let repo = crate::open_repo(path).ok();
    let native = repo.as_ref().and_then(|repo| match repo.try_find_reference("refs/stash") {
        Ok(None) => Some(None),
        Ok(Some(mut reference)) => reference.peel_to_id_in_place().ok().map(|id| Some(id.to_string())),
        Err(_) => None,
    });
    let stamp = repo.as_ref().and_then(|repo| std::fs::metadata(repo.common_dir().join("logs/refs/stash")).ok())
        .and_then(|meta| Some((meta.len(), meta.modified().ok()?)));
    let oid = native.unwrap_or_else(|| run(path, &["rev-parse", "--verify", "--quiet", "refs/stash"]).ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string()).filter(|oid| !oid.is_empty()));
    (oid, stamp)
}

fn stash_index(repo: &Path, oid: &str) -> Option<usize> {
    let listed = run(repo, &["stash", "list", "--format=%H"]).ok()?;
    String::from_utf8_lossy(&listed.stdout)
        .lines()
        .position(|line| line.trim() == oid)
}

#[cfg(test)]
mod tests {
    use super::disjoint;

    #[test]
    fn carry_check_catches_both_rename_sides_and_interleaved_directory_collisions() {
        let paths = |items: &[&str]| items.iter().map(|item| item.to_string()).collect::<Vec<_>>();
        for (local, target, expected) in [
            (vec!["a", "z"], vec!["b", "c"], true),
            (vec!["a", "b"], vec!["b"], false),
            (vec!["folder/"], vec!["folder/a", "folder/b"], false),
            (vec!["folder/a", "folder/b"], vec!["folder"], false),
            (vec!["folder/a"], vec!["folder-b"], true),
            (vec!["old", "new"], vec!["old"], false),
            (vec!["a", "a/b"], vec!["a/c"], false),
        ] { assert_eq!(disjoint(&paths(&local), &paths(&target)), expected, "{local:?} {target:?}"); }
    }
}
