//! `.fakeignore` hides local test edits from AweGit's change lists.
//!
//! The file sits at the work-tree root and uses `.gitignore` syntax. Git's
//! index and exclude rules are left alone. A parent directory excluded by a
//! non-negative pattern hides everything inside it, same as Git.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use gix::bstr::ByteSlice;
use gix::glob::pattern::Case;

use crate::{unstage_paths, ChangeKind, Error, FileChange};

const MAX_BYTES: u64 = 1024 * 1024;

struct Rules {
    search: Option<gix::ignore::Search>,
    case: Case,
}

struct Cached {
    root: PathBuf,
    case: Case,
    present: bool,
    modified: Option<SystemTime>,
    len: u64,
    rules: Rules,
}

pub(crate) fn conceal(
    root: &Path,
    repo: &gix::Repository,
    staged: &mut Vec<FileChange>,
    unstaged: &mut Vec<FileChange>,
) {
    if staged.is_empty() && unstaged.is_empty() {
        return;
    }
    with_rules(root, ignore_case(repo), |rules| {
        if rules.is_empty() {
            return;
        }
        retain(rules, staged);
        retain(rules, unstaged);
    });
}

/// Take matching paths back out of the index so stage-all and commit skip them.
pub(crate) fn unstage(repo: &Path) -> Result<(), Error> {
    let root = crate::discover(repo).unwrap_or_else(|| repo.to_path_buf());
    if !root.join(".fakeignore").is_file() {
        return Ok(());
    }
    let opened = gix::open(&root).map_err(|source| Error::Open {
        path: root.display().to_string(),
        source: Box::new(source),
    })?;
    let case = ignore_case(&opened);
    drop(opened);

    let snapshot = crate::status(&root)?;
    let paths = with_rules(&root, case, |rules| {
        let mut paths = Vec::new();
        if rules.is_empty() {
            return paths;
        }
        for change in snapshot.staged {
            if change.kind == ChangeKind::Conflict || !rules.hides(&change.path) {
                continue;
            }
            push_unique(&mut paths, change.path);
            if let Some(previous) = change.previous_path {
                push_unique(&mut paths, previous);
            }
        }
        paths
    });
    for chunk in paths.chunks(32) {
        unstage_paths(&root, chunk)?;
    }
    Ok(())
}

fn retain(rules: &Rules, changes: &mut Vec<FileChange>) {
    changes.retain(|change| change.kind == ChangeKind::Conflict || !rules.hides(&change.path));
}

fn with_rules<T>(root: &Path, case: Case, body: impl FnOnce(&Rules) -> T) -> T {
    let mut slot = cache().lock().unwrap_or_else(|error| error.into_inner());
    if !slot_matches(slot.as_ref(), root, case) {
        *slot = Some(load_cached(root, case));
    }
    let rules = &slot.as_ref().expect("loaded above").rules;
    body(rules)
}

fn slot_matches(slot: Option<&Cached>, root: &Path, case: Case) -> bool {
    let Some(slot) = slot else {
        return false;
    };
    if slot.root != root || slot.case != case {
        return false;
    }
    match std::fs::metadata(root.join(".fakeignore")) {
        Ok(meta) if meta.is_file() && meta.len() <= MAX_BYTES => {
            slot.present && slot.len == meta.len() && slot.modified == meta.modified().ok()
        }
        _ => !slot.present,
    }
}

fn load_cached(root: &Path, case: Case) -> Cached {
    let path = root.join(".fakeignore");
    let meta = std::fs::metadata(&path)
        .ok()
        .filter(|meta| meta.is_file() && meta.len() <= MAX_BYTES);
    let (present, modified, len, rules) = match meta {
        Some(meta) => {
            let modified = meta.modified().ok();
            let len = meta.len();
            let rules = std::fs::read(&path)
                .map(|bytes| Rules::parse(&path, root, &bytes, case))
                .unwrap_or_else(|_| Rules::empty(case));
            (true, modified, len, rules)
        }
        None => (false, None, 0, Rules::empty(case)),
    };
    Cached {
        root: root.to_path_buf(),
        case,
        present,
        modified,
        len,
        rules,
    }
}

fn cache() -> &'static Mutex<Option<Cached>> {
    static CACHE: OnceLock<Mutex<Option<Cached>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn ignore_case(repo: &gix::Repository) -> Case {
    if repo
        .config_snapshot()
        .boolean("core.ignoreCase")
        .unwrap_or(false)
    {
        Case::Fold
    } else {
        Case::Sensitive
    }
}

fn push_unique(paths: &mut Vec<String>, path: String) {
    if !paths.iter().any(|have| have == &path) {
        paths.push(path);
    }
}

impl Rules {
    fn empty(case: Case) -> Self {
        Self { search: None, case }
    }

    fn parse(source: &Path, root: &Path, bytes: &[u8], case: Case) -> Self {
        let mut search = gix::ignore::Search::default();
        search.add_patterns_buffer(
            bytes,
            source,
            Some(root),
            gix::ignore::search::Ignore {
                support_precious: false,
            },
        );
        Self {
            search: Some(search),
            case,
        }
    }

    fn is_empty(&self) -> bool {
        self.search
            .as_ref()
            .is_none_or(|search| search.patterns.iter().all(|list| list.patterns.is_empty()))
    }

    fn hides(&self, path: &str) -> bool {
        let Some(search) = &self.search else {
            return false;
        };
        if path.as_bytes().contains(&b'\\') {
            return hides_unix(search, &path.replace('\\', "/"), self.case);
        }
        hides_unix(search, path, self.case)
    }
}

fn hides_unix(search: &gix::ignore::Search, path: &str, case: Case) -> bool {
    let bytes = path.as_bytes();
    let mut index = 0;
    while let Some(offset) = bytes[index..].iter().position(|byte| *byte == b'/') {
        let end = index + offset;
        if end > index && excluded_dir(search, &bytes[..end], case) {
            return true;
        }
        index = end + 1;
    }
    search
        .pattern_matching_relative_path(bytes.as_bstr(), Some(false), case)
        .is_some_and(|hit| !hit.pattern.is_negative())
}

fn excluded_dir(search: &gix::ignore::Search, dir: &[u8], case: Case) -> bool {
    search
        .pattern_matching_relative_path(dir.as_bstr(), Some(true), case)
        .is_some_and(|hit| !hit.pattern.is_negative())
}
