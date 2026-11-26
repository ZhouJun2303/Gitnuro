use std::path::Path;

use gix::bstr::{BStr, BString, ByteSlice};

use crate::{cli, push, ChangeKind, Error, FileChange};

pub(crate) fn can_retry(error: &Error) -> bool {
    use gix::index::init::from_tree::Error::InvalidComponent;
    use gix::repository::index_from_tree::Error::IndexFromTree;
    use gix::status::{index_worktree, iter, plumbing, tree_index};

    match error {
        Error::Item(iter::Error::IndexWorktree(
            index_worktree::Error::IndexAsWorktreeWithRenames(
                plumbing::index_as_worktree_with_renames::Error::TrackedFileModifications(error),
            ),
        )) => matches!(
            error,
            plumbing::index_as_worktree::Error::SubmoduleStatus { .. }
        ),
        // Git can diff a tree containing a path that cannot be checked out on this platform.
        Error::Item(iter::Error::TreeIndex(tree_index::Error::IndexFromMTree(IndexFromTree {
            source,
            ..
        }))) => matches!(source, InvalidComponent { .. }),
        _ => false,
    }
}

pub(crate) fn read_changes(
    repo: &gix::Repository,
    worktree: &Path,
    patterns: &[BString],
) -> Result<(Vec<FileChange>, Vec<FileChange>), Error> {
    // Git status has no pathspec-from-file option. Leave room for Windows argument quoting.
    let scope_after_read = cfg!(windows)
        && patterns
            .iter()
            .map(|pattern| pattern.len() + 3)
            .sum::<usize>()
            > 12_000;
    let mut args = Vec::new();
    if !patterns.is_empty()
        && !scope_after_read
        && repo
            .config_snapshot()
            .boolean("core.ignorecase")
            .unwrap_or(false)
    {
        args.push("--icase-pathspecs");
    }
    args.extend([
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--",
    ]);
    if !scope_after_read {
        for pattern in patterns {
            args.push(
                pattern
                    .to_str()
                    .map_err(|error| Error::Git(error.to_string()))?,
            );
        }
    }
    let output = cli::run_env(worktree, &args, &[("GIT_OPTIONAL_LOCKS", "0")])?;
    let (mut staged, mut unstaged) = parse_status(&output.stdout)?;
    if scope_after_read {
        let index = repo
            .index_or_empty()
            .map_err(|error| Error::Git(error.to_string()))?;
        let mut scope = repo
            .pathspec(
                false,
                patterns,
                true,
                &index,
                gix::worktree::stack::state::attributes::Source::WorktreeThenIdMapping,
            )
            .map_err(|error| Error::Git(error.to_string()))?;
        restrict_to_scope(&mut staged, &mut scope, ChangeKind::Added);
        restrict_to_scope(&mut unstaged, &mut scope, ChangeKind::Untracked);
    }
    Ok((staged, unstaged))
}

fn parse_status(bytes: &[u8]) -> Result<(Vec<FileChange>, Vec<FileChange>), Error> {
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut fields = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());
    let invalid_output = || Error::Git("invalid Git status output".into());
    while let Some(field) = fields.next() {
        let Some((&[x, y, b' '], path)) = field.split_first_chunk::<3>() else {
            return Err(invalid_output());
        };
        let path = BStr::new(path.strip_suffix(b"/").unwrap_or(path));
        match [x, y] {
            [b'?', b'?'] => push(&mut unstaged, path, ChangeKind::Untracked, None),
            [b'D', b'D'] | [b'A', b'A'] | [b'U', _] | [_, b'U'] => {
                push(&mut unstaged, path, ChangeKind::Conflict, None);
            }
            _ => {
                // With -z, a rename or copy is followed by its original path.
                let previous = if [x, y].iter().any(|code| matches!(code, b'R' | b'C')) {
                    Some(BStr::new(fields.next().ok_or_else(invalid_output)?))
                } else {
                    None
                };
                if let Some(kind) = change_kind(x, ChangeKind::Added) {
                    push(&mut staged, path, kind, previous.filter(|_| x == b'R'));
                }
                if let Some(kind) = change_kind(y, ChangeKind::Untracked) {
                    push(&mut unstaged, path, kind, previous.filter(|_| y == b'R'));
                }
            }
        }
    }
    Ok((staged, unstaged))
}

fn change_kind(code: u8, copy: ChangeKind) -> Option<ChangeKind> {
    match code {
        b'A' => Some(ChangeKind::Added),
        b'M' | b'T' => Some(ChangeKind::Modified),
        b'D' => Some(ChangeKind::Deleted),
        b'R' => Some(ChangeKind::Renamed),
        b'C' => Some(copy),
        _ => None,
    }
}

fn restrict_to_scope(
    changes: &mut Vec<FileChange>,
    scope: &mut gix::Pathspec<'_>,
    added: ChangeKind,
) {
    changes.retain_mut(|change| {
        let included = scope.is_included(BStr::new(&change.path), None);
        let Some(previous) = change.previous_path.as_deref() else {
            return included;
        };
        let source_included = scope.is_included(BStr::new(previous), None);
        match (source_included, included) {
            (true, false) => {
                change.path = previous.to_string();
                change.kind = ChangeKind::Deleted;
                change.previous_path = None;
                true
            }
            (false, true) => {
                change.kind = added;
                change.previous_path = None;
                true
            }
            _ => included,
        }
    });
}
