use std::io::{ErrorKind, Read};
use std::path::Path;

use awegit_git::CommitIntent;
use serde_json::Value;
use sha2::{Digest, Sha256};

const CHANGED: &str = "The selected files changed before execution. Refresh and confirm again. / 所选文件在执行前已变化，请刷新后重新确认。";

pub(super) enum Intent {
    Commit { expected: CommitIntent, branch: Option<String> },
    Files { files: Vec<String>, git: CommitIntent, disk: [u8; 32] },
}

impl Intent {
    pub(super) fn capture(command: &str, repo: &Path, args: &Value) -> Result<Option<Self>, String> {
        if command == "commit_changes" {
            let expected = match args.get("__expectedCommit") {
                Some(value) => serde_json::from_value(value.clone()).map_err(|error| error.to_string())?,
                None => awegit_git::commit_intent(repo).map_err(|error| error.to_string())?,
            };
            let branch = match args.get("__branch") {
                Some(value) => serde_json::from_value(value.clone()).map_err(|error| error.to_string())?,
                None => awegit_git::current_branch(repo).map_err(|error| error.to_string())?,
            };
            return Ok(Some(Self::Commit { expected, branch }));
        }
        let request = &args["request"];
        let mut files: Vec<String> = match request["action"].as_str() {
            Some("discard" | "delete" | "restoreFile") => vec![super::arg(request, "file")?],
            Some("discardPaths" | "deletePaths") => super::arg(request, "files")?,
            _ => return Ok(None),
        };
        for file in &mut files {
            awegit_git::check_path(file).map_err(|error| error.to_string())?;
            if cfg!(windows) { *file = file.replace('\\', "/"); }
            if file.split('/').any(|part| part.trim_end_matches(['.', ' ']).eq_ignore_ascii_case(".git")) {
                return Err("A queued worktree operation cannot change Git metadata. / 工作区操作不能修改 Git 元数据。".into());
            }
        }
        files.sort();
        files.dedup();
        let git = awegit_git::worktree_intent(repo, &files).map_err(|error| error.to_string())?;
        let disk = fingerprint(repo, &files)?;
        Ok(Some(Self::Files { files, git, disk }))
    }

    pub(super) fn check(&self, repo: &Path) -> Result<(), String> {
        match self {
            Self::Commit { expected, branch } => {
                if &awegit_git::current_branch(repo).map_err(|error| error.to_string())? != branch {
                    return Err("The selected branch changed before execution. / 当前分支已改变，请核对后重试。".into());
                }
                let actual = awegit_git::commit_intent(repo).map_err(|error| error.to_string())?;
                if actual.head != expected.head {
                    return Err("HEAD changed before the commit. Review it and retry. / HEAD 在提交前已变化，请核对后重试。".into());
                }
                if actual.index != expected.index {
                    return Err("The index changed before the commit. Review staged files and retry. / 暂存区在提交前已变化，请核对后重试。".into());
                }
            }
            Self::Files { files, git, disk } => {
                let actual = awegit_git::worktree_intent(repo, files).map_err(|error| error.to_string())?;
                if &actual != git || &fingerprint(repo, files)? != disk { return Err(CHANGED.into()); }
            }
        }
        Ok(())
    }
}

fn fingerprint(repo: &Path, files: &[String]) -> Result<[u8; 32], String> {
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    for file in files {
        let path = repo.join(file);
        for ancestor in path.ancestors().skip(1).take_while(|ancestor| *ancestor != repo) {
            if std::fs::symlink_metadata(ancestor).is_ok_and(|meta| linked(&meta)) {
                return Err(format!("Cannot verify a path through a symbolic link: {file}"));
            }
        }
        hash_bytes(&mut hash, file.as_bytes());
        hash_path(&path, &mut hash, &mut buffer).map_err(|error| format!("Could not verify {file}: {error}"))?;
    }
    Ok(hash.finalize().into())
}

fn linked(info: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Junctions and mount points are also directory redirects.
        if info.is_dir() && info.file_attributes() & 0x400 != 0 { return true; }
    }
    info.file_type().is_symlink()
}

fn hash_bytes(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn hash_path(path: &Path, hash: &mut Sha256, buffer: &mut [u8]) -> std::io::Result<()> {
    let info = match std::fs::symlink_metadata(path) {
        Ok(info) => info,
        Err(error) if error.kind() == ErrorKind::NotFound => { hash.update([0]); return Ok(()); }
        Err(error) => return Err(error),
    };
    if info.file_type().is_symlink() {
        hash.update([1]);
        hash_bytes(hash, std::fs::read_link(path)?.as_os_str().as_encoded_bytes());
    } else if info.is_dir() {
        if linked(&info) { return Err(std::io::Error::other("cannot verify a redirected directory")); }
        hash.update([2]);
        let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        hash.update((entries.len() as u64).to_le_bytes());
        for entry in entries {
            hash_bytes(hash, entry.file_name().as_encoded_bytes());
            hash_path(&entry.path(), hash, buffer)?;
        }
    } else if info.is_file() {
        hash.update([3]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            hash.update(info.permissions().mode().to_le_bytes());
        }
        hash.update(info.len().to_le_bytes());
        let mut source = std::fs::File::open(path)?;
        let mut size = 0u64;
        loop {
            let read = source.read(buffer)?;
            if read == 0 { break; }
            size += read as u64;
            hash.update(&buffer[..read]);
        }
        if size != info.len() || source.metadata()?.modified()? != info.modified()? {
            return Err(std::io::Error::other("file changed while it was read"));
        }
    } else {
        return Err(std::io::Error::other("unsupported file type"));
    }
    Ok(())
}
