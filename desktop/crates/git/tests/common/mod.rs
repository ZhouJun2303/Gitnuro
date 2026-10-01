use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TempRepo {
    pub path: PathBuf,
    /// Empty file used as `GIT_CONFIG_GLOBAL` so the test does not read or write the user gitconfig.
    gitconfig: PathBuf,
}

impl TempRepo {
    pub fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("awegit-git-{nanos}"));
        let gitconfig = std::env::temp_dir().join(format!("awegit-git-{nanos}.gitconfig"));
        fs::create_dir_all(&path).unwrap();
        fs::write(&gitconfig, "").unwrap();
        Self { path, gitconfig }
    }

    pub fn git(&self, args: &[&str]) {
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

    #[allow(dead_code)]
    pub fn output(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &self.gitconfig)
            .output()
            .unwrap_or_else(|error| panic!("git is not on PATH: {error}"));
        assert!(
            output.status.success(),
            "git {args:?} failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
        let _ = fs::remove_file(&self.gitconfig);
    }
}
