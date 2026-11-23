//! A status fingerprint reads `HEAD`, the branch ref, and the index trailer.
//!
//! It does not stat the work tree. A matching fingerprint means the branch and
//! the index are unchanged. Work tree edits stay the file watcher's job.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Error;

/// Identity of `HEAD` and the index. Equal values mean neither has changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusFingerprint {
    pub head: String,
    pub ref_id: String,
    pub index_len: u64,
    pub index_sum: String,
}

/// Read the fingerprint. A few small files, never the work tree.
pub fn status_fingerprint(repo: &Path) -> Result<StatusFingerprint, Error> {
    let git = git_metadata(repo)?;
    let head = read_capped(&git.join("HEAD"), 512)?;
    let head = head.trim().to_string();
    let ref_id = if let Some(name) = head.strip_prefix("ref:") {
        loose_ref(&git, name.trim()).unwrap_or_else(|| packed_stat(&git))
    } else {
        head.clone()
    };
    let (index_len, index_sum) = index_tail(&git.join("index"));
    Ok(StatusFingerprint { head, ref_id, index_len, index_sum })
}

fn git_metadata(repo: &Path) -> Result<PathBuf, Error> {
    let dot = repo.join(".git");
    if dot.is_dir() {
        return Ok(dot);
    }
    if !dot.is_file() {
        return Err(Error::Missing { path: repo.display().to_string() });
    }
    let text = std::fs::read_to_string(&dot).map_err(|source| Error::Read { path: dot.display().to_string(), source })?;
    let line = text.lines().find(|line| !line.trim().is_empty()).unwrap_or("");
    let rest = line.trim().strip_prefix("gitdir:").ok_or_else(|| Error::Git("missing gitdir".into()))?.trim();
    if rest.is_empty() || rest.contains('\0') {
        return Err(Error::Git("missing gitdir".into()));
    }
    let path = PathBuf::from(rest);
    let full = if path.is_absolute() { path } else { repo.join(path) };
    if full.is_dir() {
        Ok(full)
    } else {
        Err(Error::Git("gitdir is not a directory".into()))
    }
}

fn common_dir(git: &Path) -> PathBuf {
    let Ok(text) = std::fs::read_to_string(git.join("commondir")) else {
        return git.to_path_buf();
    };
    let path = PathBuf::from(text.trim());
    if path.as_os_str().is_empty() {
        return git.to_path_buf();
    }
    let full = if path.is_absolute() { path } else { git.join(path) };
    if full.is_dir() { full } else { git.to_path_buf() }
}

/// The loose ref only. `packed-refs` can be large, so a missing loose ref records its size and mtime.
fn loose_ref(git: &Path, name: &str) -> Option<String> {
    if !safe_ref(name) {
        return Some(String::new());
    }
    if let Some(text) = read_ref_file(&git.join(name)) {
        return Some(text);
    }
    let common = common_dir(git);
    if common != git {
        if let Some(text) = read_ref_file(&common.join(name)) {
            return Some(text);
        }
    }
    None
}

fn safe_ref(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.starts_with('\\')
        && !name.contains('\0')
        && !name.split(['/', '\\']).any(|part| part.is_empty() || part == "." || part == "..")
}

fn read_ref_file(path: &Path) -> Option<String> {
    let text = read_capped(path, 256).ok()?;
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn packed_stat(git: &Path) -> String {
    let path = common_dir(git).join("packed-refs");
    match path.metadata() {
        Ok(meta) => {
            let modified = meta.modified().ok().and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok());
            let nanos = modified.map(|time| time.as_nanos()).unwrap_or(0);
            format!("packed:{}:{nanos}", meta.len())
        }
        Err(_) => "packed:missing".into(),
    }
}

/// Last 32 bytes of the index. That covers the SHA-1 trailer and a SHA-256 trailer without reading the body.
fn index_tail(path: &Path) -> (u64, String) {
    let Ok(mut file) = File::open(path) else {
        return (0, String::new());
    };
    let Ok(meta) = file.metadata() else {
        return (0, String::new());
    };
    let len = meta.len();
    if len == 0 {
        return (0, String::new());
    }
    let start = len.saturating_sub(32);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return (len, String::new());
    }
    let mut buf = [0u8; 32];
    let take = (len - start) as usize;
    match file.read(&mut buf[..take]) {
        Ok(read) if read > 0 => (len, hex(&buf[..read])),
        _ => (len, String::new()),
    }
}

fn read_capped(path: &Path, cap: usize) -> Result<String, Error> {
    let mut file = File::open(path).map_err(|source| Error::Read { path: path.display().to_string(), source })?;
    let mut buf = vec![0u8; cap];
    let read = file.read(&mut buf).map_err(|source| Error::Read { path: path.display().to_string(), source })?;
    Ok(String::from_utf8_lossy(&buf[..read]).into_owned())
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Instant;

    struct TempRepo {
        path: PathBuf,
        gitconfig: PathBuf,
    }

    impl TempRepo {
        fn new() -> Self {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("awegit-fp-{nanos}"));
            let gitconfig = std::env::temp_dir().join(format!("awegit-fp-{nanos}.gitconfig"));
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(&gitconfig, "").unwrap();
            let repo = Self { path, gitconfig };
            repo.git(&["init", "-b", "main"]);
            repo.git(&["config", "user.name", "Test"]);
            repo.git(&["config", "user.email", "test@example.com"]);
            repo
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
            assert!(output.status.success(), "git {args:?} failed\n{}", String::from_utf8_lossy(&output.stderr));
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
            let _ = std::fs::remove_file(&self.gitconfig);
        }
    }

    #[test]
    fn worktree_edits_do_not_change_the_fingerprint() {
        let repo = TempRepo::new();
        std::fs::write(repo.path.join("tracked.txt"), "one\n").unwrap();
        repo.git(&["add", "tracked.txt"]);
        repo.git(&["commit", "-m", "one"]);
        let first = status_fingerprint(&repo.path).unwrap();
        std::fs::write(repo.path.join("tracked.txt"), "two\n").unwrap();
        std::fs::write(repo.path.join("untracked.txt"), "new\n").unwrap();
        let second = status_fingerprint(&repo.path).unwrap();
        assert_eq!(first, second);
        assert!(first.index_len > 32);
        assert_eq!(first.index_sum.len(), 64);
        assert!(first.head.contains("refs/heads/main"));
        assert_eq!(first.ref_id.len(), 40);
    }

    #[test]
    fn index_and_head_changes_move_the_fingerprint() {
        let repo = TempRepo::new();
        std::fs::write(repo.path.join("tracked.txt"), "one\n").unwrap();
        repo.git(&["add", "tracked.txt"]);
        repo.git(&["commit", "-m", "one"]);
        let committed = status_fingerprint(&repo.path).unwrap();
        std::fs::write(repo.path.join("tracked.txt"), "two\n").unwrap();
        repo.git(&["add", "tracked.txt"]);
        let staged = status_fingerprint(&repo.path).unwrap();
        assert_ne!(committed.index_sum, staged.index_sum);
        assert_eq!(committed.ref_id, staged.ref_id);
        repo.git(&["commit", "-m", "two"]);
        let next = status_fingerprint(&repo.path).unwrap();
        assert_ne!(staged.ref_id, next.ref_id);
    }

    #[test]
    fn index_tail_reads_the_trailer_not_the_body() {
        let dir = std::env::temp_dir().join(format!(
            "awegit-fp-index-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let index = dir.join(".git").join("index");
        let mut body = vec![0u8; 8 * 1024 * 1024];
        body.extend(b"0123456789abcdef0123456789abcdef");
        std::fs::write(&index, &body).unwrap();
        std::fs::write(dir.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
        let started = Instant::now();
        let fingerprint = status_fingerprint(&dir).unwrap();
        let elapsed = started.elapsed();
        assert_eq!(fingerprint.index_len, body.len() as u64);
        assert_eq!(fingerprint.index_sum, hex(b"0123456789abcdef0123456789abcdef"));
        assert!(elapsed.as_millis() < 1500, "index fingerprint took {}ms", elapsed.as_millis());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsafe_ref_names_are_not_joined_onto_the_git_dir() {
        assert!(!safe_ref("refs/../HEAD"));
        assert!(!safe_ref("/etc/passwd"));
        assert!(safe_ref("refs/heads/main"));
    }
}
