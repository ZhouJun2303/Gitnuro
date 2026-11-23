use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInspection {
    pub kinds: Vec<&'static str>,
    pub files: u64,
    pub directories: u64,
    pub worktree_bytes: u64,
    pub git_bytes: u64,
    pub skipped: u64,
    pub remotes: Vec<(String, String)>,
}

fn has_any(root: &Path, paths: &[&str]) -> bool {
    paths.iter().any(|path| root.join(path).is_file())
}

fn metadata_dir(root: &Path) -> Option<PathBuf> {
    let marker = root.join(".git");
    if marker.is_dir() { return Some(marker); }
    let pointer = std::fs::read_to_string(marker).ok()?;
    let location = pointer.trim().strip_prefix("gitdir:")?.trim();
    let directory = Path::new(location);
    let directory = if directory.is_absolute() { directory.to_path_buf() } else { root.join(directory) };
    let common = std::fs::read_to_string(directory.join("commondir")).ok();
    let directory = common.map(|value| directory.join(value.trim())).unwrap_or(directory);
    directory.is_dir().then_some(directory)
}

#[derive(Default)]
struct TreeStats {
    files: u64,
    directories: u64,
    bytes: u64,
    skipped: u64,
}

fn scan_tree(root: &Path, skip: Option<&Path>, count_directories: bool) -> TreeStats {
    let mut result = TreeStats::default();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => {
                result.skipped += 1;
                continue;
            }
        };
        for entry in entries {
            let Ok(entry) = entry else {
                result.skipped += 1;
                continue;
            };
            let path = entry.path();
            if skip.is_some_and(|marker| path == marker) {
                continue;
            }
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) => {
                    result.skipped += 1;
                    continue;
                }
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                if count_directories {
                    result.directories += 1;
                }
                stack.push(path);
            } else if metadata.is_file() {
                result.files += 1;
                result.bytes = result.bytes.saturating_add(metadata.len());
            }
        }
    }
    result
}

fn inspect(root: &Path, git_dir: Option<&Path>) -> RepoInspection {
    let git_marker = root.join(".git");
    let mut kinds = Vec::new();
    if root.join("Assets").is_dir() && root.join("ProjectSettings").is_dir()
        && has_any(root, &["ProjectSettings/ProjectVersion.txt", "Packages/manifest.json"]) {
        kinds.push("unity");
    }
    if has_any(root, &["settings.gradle", "settings.gradle.kts"])
        && has_any(root, &["build.gradle", "build.gradle.kts"])
        && (root.join("gradlew").is_file() || root.join("gradlew.bat").is_file()
            || root.join("app/src/main/AndroidManifest.xml").is_file()) {
        kinds.push("android");
    }
    let (worktree, git) = std::thread::scope(|scope| {
        let worktree = scope.spawn(|| scan_tree(root, Some(&git_marker), true));
        let git = git_dir
            .filter(|dir| dir.is_dir())
            .map(|dir| scope.spawn(|| scan_tree(dir, None, false)));
        let worktree = worktree.join().unwrap_or_default();
        let git = git.and_then(|thread| thread.join().ok()).unwrap_or_default();
        (worktree, git)
    });
    let result = RepoInspection {
        kinds,
        files: worktree.files,
        directories: worktree.directories,
        worktree_bytes: worktree.bytes,
        git_bytes: git.bytes,
        skipped: worktree.skipped.saturating_add(git.skipped),
        remotes: Vec::new(),
    };
    result
}

#[tauri::command]
pub fn repository_inspection(path: Option<String>) -> Result<RepoInspection, String> {
    let root = super::repo_from(path)?;
    let git_dir = metadata_dir(&root);
    if let Some(ref directory) = git_dir { super::lab::ensure_lab_path(directory)?; }
    let (mut result, remotes) = std::thread::scope(|scope| {
        let inspection = scope.spawn(|| inspect(&root, git_dir.as_deref()));
        let remotes = scope.spawn(|| awegit_git::remote_list(&root));
        let inspection = inspection.join().map_err(|_| "repository inspection stopped unexpectedly".to_string())?;
        let remotes = remotes.join().unwrap_or_default();
        Ok::<_, String>((inspection, remotes))
    })?;
    result.remotes = remotes;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_markers_and_excludes_git_metadata() {
        let root = std::env::temp_dir().join(format!("awegit-inspect-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(root.join("ProjectSettings")).unwrap();
        std::fs::create_dir_all(root.join("Assets")).unwrap();
        std::fs::create_dir_all(root.join("app/src/main")).unwrap();
        std::fs::create_dir_all(root.join(".git/objects")).unwrap();
        for (name, content) in [("ProjectSettings/ProjectVersion.txt", "unity"), ("settings.gradle.kts", "a"), ("build.gradle.kts", "b"), ("app/src/main/AndroidManifest.xml", "c"), (".git/objects/one", "git")] {
            std::fs::write(root.join(name), content).unwrap();
        }
        let result = inspect(&root, Some(&root.join(".git")));
        assert_eq!(result.kinds, ["unity", "android"]);
        assert_eq!(result.files, 4);
        assert_eq!(result.worktree_bytes, 8);
        assert_eq!(result.git_bytes, 3);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn linked_worktree_uses_shared_metadata() {
        let base = std::env::temp_dir().join(format!("awegit-linked-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let root = base.join("repo");
        let linked = base.join("common/worktrees/repo");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&linked).unwrap();
        std::fs::write(root.join(".git"), "gitdir: ../common/worktrees/repo\n").unwrap();
        std::fs::write(linked.join("commondir"), "../..\n").unwrap();
        std::fs::write(base.join("common/objects"), "objects").unwrap();
        let git = metadata_dir(&root).unwrap();
        assert_eq!(inspect(&root, Some(&git)).git_bytes, 7 + 6);
        std::fs::remove_dir_all(base).unwrap();
    }
}
