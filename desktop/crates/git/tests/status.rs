use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use awegit_git::{status, ChangeKind, FileChange};

#[test]
fn status_splits_staged_and_unstaged_changes() {
    let repo = TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "status.showUntrackedFiles", "all"]);
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "initial"]);

    fs::write(repo.path.join("a.txt"), "two\n").unwrap();
    fs::write(repo.path.join("b.txt"), "new\n").unwrap();

    let before_add = status(&repo.path).unwrap();
    assert_eq!(before_add.branch.as_deref(), Some("main"));
    assert!(before_add.staged.is_empty(), "{:?}", before_add.staged);
    assert_has(&before_add.unstaged, "a.txt", ChangeKind::Modified);
    assert_has(&before_add.unstaged, "b.txt", ChangeKind::Untracked);

    repo.git(&["add", "b.txt"]);
    let after_add = status(&repo.path).unwrap();
    assert_has(&after_add.staged, "b.txt", ChangeKind::Added);
    assert_has(&after_add.unstaged, "a.txt", ChangeKind::Modified);
    assert!(!after_add.unstaged.iter().any(|change| change.path == "b.txt"));
}

fn assert_has(changes: &[FileChange], path: &str, kind: ChangeKind) {
    assert!(
        changes.iter().any(|change| change.path == path && change.kind == kind),
        "missing {path} {kind:?} in {changes:?}"
    );
}

struct TempRepo {
    path: PathBuf,
    /// Empty file used as `GIT_CONFIG_GLOBAL` so the test does not read or write the user gitconfig.
    gitconfig: PathBuf,
}

impl TempRepo {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("awegit-status-{nanos}"));
        let gitconfig = std::env::temp_dir().join(format!("awegit-status-{nanos}.gitconfig"));
        fs::create_dir_all(&path).unwrap();
        fs::write(&gitconfig, "").unwrap();
        Self { path, gitconfig }
    }

    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &self.gitconfig)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .unwrap_or_else(|error| panic!("git is not on PATH: {error}"));
        assert!(
            output.status.success(),
            "git {args:?} failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
        let _ = fs::remove_file(&self.gitconfig);
    }
}
