//! Last status list for one repository. It lives beside settings, never inside `.git`.
//!
//! The file is the change list plus the HEAD/index fingerprint. A cold start paints it,
//! then a live watcher decides whether it is still true. Writing happens off the status thread.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use awegit_git::{CommitIntent, FileChange, StatusFingerprint, StatusSnapshot};

const MAX_PATHS: usize = 4_000;
const MAX_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredStatus {
    pub path: String,
    pub branch: Option<String>,
    pub commit_intent: CommitIntent,
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
    pub fingerprint: StatusFingerprint,
    pub overflow: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusProbe {
    pub fingerprint: StatusFingerprint,
    pub revision: u64,
    pub watcher_alive: bool,
}

pub fn probe(repo: &Path, watcher_alive: bool) -> Result<StatusProbe, String> {
    Ok(StatusProbe {
        fingerprint: awegit_git::status_fingerprint(repo).map_err(|error| error.to_string())?,
        revision: crate::changes::revision(repo),
        watcher_alive,
    })
}

pub fn load(repo: &Path) -> Option<StoredStatus> {
    let dir = cache_dir().ok()?;
    load_from(&dir, repo)
}

pub fn load_from(dir: &Path, repo: &Path) -> Option<StoredStatus> {
    let path = dir.join(file_name(repo));
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() == 0 || meta.len() > MAX_BYTES {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Queue a snapshot write. The status command does not wait for the disk.
pub fn remember(repo: &Path, status: &StatusSnapshot, fingerprint: &StatusFingerprint) {
    let Ok(dir) = cache_dir() else { return };
    let Ok(bytes) = serde_json::to_vec(&stored_from(status, fingerprint)) else { return };
    let _ = sender().send(Job { dir, name: file_name(repo), bytes });
}

#[cfg(test)]
pub fn write_to(dir: &Path, repo: &Path, status: &StatusSnapshot, fingerprint: &StatusFingerprint) -> Result<(), String> {
    let bytes = serde_json::to_vec(&stored_from(status, fingerprint)).map_err(|error| error.to_string())?;
    write_bytes(&dir.join(file_name(repo)), &bytes)
}

fn stored_from(status: &StatusSnapshot, fingerprint: &StatusFingerprint) -> StoredStatus {
    let overflow = status.staged.len().saturating_add(status.unstaged.len()) > MAX_PATHS;
    StoredStatus {
        path: status.path.clone(),
        branch: status.branch.clone(),
        commit_intent: status.commit_intent.clone(),
        staged: if overflow { Vec::new() } else { status.staged.clone() },
        unstaged: if overflow { Vec::new() } else { status.unstaged.clone() },
        fingerprint: fingerprint.clone(),
        overflow,
    }
}

fn cache_dir() -> Result<PathBuf, String> {
    let settings = crate::settings::path()?;
    let parent = settings.parent().ok_or_else(|| "settings path has no parent".to_string())?;
    Ok(parent.join("status-cache"))
}

fn file_name(repo: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(awegit_git::repo_key(repo).as_bytes());
    format!("{:x}.json", hasher.finalize())
}

struct Job {
    dir: PathBuf,
    name: String,
    bytes: Vec<u8>,
}

fn sender() -> &'static Sender<Job> {
    static SENDER: OnceLock<Sender<Job>> = OnceLock::new();
    SENDER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<Job>();
        let _ = std::thread::Builder::new().name("awegit-status-cache".into()).spawn(move || {
            while let Ok(job) = receiver.recv() {
                let _ = write_bytes(&job.dir.join(job.name), &job.bytes);
            }
        });
        sender
    })
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes).map_err(|error| error.to_string())?;
    if std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp, path).map_err(|error| {
            let _ = std::fs::remove_file(&tmp);
            error.to_string()
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use awegit_git::ChangeKind;

    fn fingerprint() -> StatusFingerprint {
        StatusFingerprint {
            head: "ref: refs/heads/main".into(),
            ref_id: "abc".into(),
            index_len: 120,
            index_sum: "ff".into(),
            submodule_sum: "none".into(),
        }
    }

    fn snapshot(count: usize) -> StatusSnapshot {
        let file = |index: usize| FileChange {
            path: format!("file-{index}.txt"),
            kind: ChangeKind::Modified,
            previous_path: None,
        };
        StatusSnapshot {
            revision: 4,
            path: "D:/repo".into(),
            branch: Some("main".into()),
            commit_intent: CommitIntent { head: Some("abc".into()), index: "intent".into() },
            staged: (0..count).map(file).collect(),
            unstaged: vec![FileChange { path: "new.txt".into(), kind: ChangeKind::Untracked, previous_path: None }],
            fingerprint: None,
        }
    }

    #[test]
    fn a_snapshot_roundtrips_without_the_worktree() {
        let dir = std::env::temp_dir().join(format!(
            "awegit-status-cache-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let repo = Path::new("D:/repo");
        let status = snapshot(2);
        let fp = fingerprint();
        write_to(&dir, repo, &status, &fp).unwrap();
        let loaded = load_from(&dir, repo).unwrap();
        assert!(!loaded.overflow);
        assert_eq!(loaded.staged.len(), 2);
        assert_eq!(loaded.unstaged.len(), 1);
        assert_eq!(loaded.fingerprint, fp);
        assert_eq!(loaded.branch.as_deref(), Some("main"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_huge_change_list_is_stored_as_a_stub() {
        let stored = stored_from(&snapshot(MAX_PATHS), &fingerprint());
        assert!(stored.overflow);
        assert!(stored.staged.is_empty());
        assert!(stored.unstaged.is_empty());
        assert_eq!(stored.fingerprint.ref_id, "abc");
    }

    #[test]
    fn an_oversized_cache_file_is_ignored() {
        let dir = std::env::temp_dir().join(format!(
            "awegit-status-cache-big-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Path::new("D:/huge");
        let path = dir.join(file_name(repo));
        std::fs::write(&path, vec![b'x'; (MAX_BYTES as usize) + 1]).unwrap();
        assert!(load_from(&dir, repo).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
