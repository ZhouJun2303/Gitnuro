//! Watch events get a version when received, before the debounce delay.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn versions() -> &'static Mutex<HashMap<String, u64>> {
    static VERSIONS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    VERSIONS.get_or_init(Default::default)
}

pub fn revision(repo: &Path) -> u64 {
    versions()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&awegit_git::repo_key(repo))
        .copied()
        .unwrap_or(0)
}

pub fn changed(repo: &Path) -> u64 {
    let mut versions = versions().lock().unwrap_or_else(|error| error.into_inner());
    let revision = NEXT.fetch_add(1, Ordering::Relaxed);
    versions.insert(awegit_git::repo_key(repo), revision);
    revision
}
