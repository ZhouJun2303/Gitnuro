//! Name the program holding a file when Git cannot replace it in the work tree.
//!
//! Windows reports a locked file as `unable to unlink old '...': Invalid argument`
//! or `...: Permission denied`. Git only has the path; the Restart Manager knows
//! which processes hold it, so the error can tell the user what to close.

use std::path::Path;

/// Git for Windows calls GIT_ASK_YESNO only after its short internal filesystem
/// retries fail. Keep a shared, bounded retry budget for this command and all
/// its children, while the normal process group still supports cancellation.
#[cfg(windows)]
pub(crate) struct FileRetry(std::path::PathBuf);

#[cfg(windows)]
impl FileRetry {
    pub(crate) fn configure(command: &mut std::process::Command) -> Option<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let powershell = std::path::PathBuf::from(std::env::var_os("SystemRoot")?)
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
        let seq = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("awegit-file-retry-{}-{stamp}-{seq}", std::process::id()));
        std::fs::create_dir(&dir).ok()?;
        let guard = Self(dir);
        let script = guard.0.join("retry.sh");
        // Atomic mkdir tickets also bound parallel submodule children. Paths
        // from Git are never interpreted as shell code or used for deletion.
        std::fs::write(&script, r#"#!/bin/sh
case "$1" in
  "Unlink of file "*|"Deletion of directory "*|"Rename from "*) ;;
  *) exit 1 ;;
esac
for attempt in 1 2 3 4 5 6; do
  if mkdir "$0.retry-$attempt" 2>/dev/null; then
    if command -v sleep >/dev/null 2>&1; then
      sleep 0.5 || exit 1
    else
      "$AWEGIT_RETRY_POWERSHELL" -NoLogo -NoProfile -NonInteractive -WindowStyle Hidden -Command 'Start-Sleep -Milliseconds 500' || exit 1
    fi
    exit 0
  fi
done
exit 1
"#).ok()?;
        command.env("GIT_ASK_YESNO", script);
        // MinGit ships sh and mkdir, but omits sleep. Use the Windows timer in
        // that case; do not depend on another Git installation being on PATH.
        command.env("AWEGIT_RETRY_POWERSHELL", powershell);
        Some(guard)
    }
}

#[cfg(windows)]
impl Drop for FileRetry {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Lines Git prints when a work-tree file cannot be replaced or removed.
const FILE_MARKERS: [&str; 5] = [
    "unable to unlink old ",
    "unable to unlink ",
    "could not unlink ",
    "unable to create file ",
    "unable to write file ",
];

/// Git can report these for locks, read-only files, or insufficient permissions.
/// Only a known holding process confirms that another program has the file open.
const ACCESS_REASONS: [&str; 10] = [
    "invalid argument",
    "permission denied",
    "access is denied",
    "being used by another process",
    "resource busy",
    "operation not permitted",
    "cannot access the file",
    "sharing violation",
    "text file busy",
    "read-only file system",
];

/// Explain file access failures for every Git command, without assuming a file
/// type or application. The caller keeps Git's original error after this advice.
pub(crate) fn explain(repo: &Path, stderr: &str) -> Option<String> {
    let paths = locked_paths(stderr);
    if paths.is_empty() {
        return None;
    }
    let files: Vec<_> = paths.into_iter().map(|path| {
        let apps = holders(&repo.join(&path));
        (path, apps)
    }).collect();
    Some(describe(&files))
}

fn describe(files: &[(String, Vec<String>)]) -> String {
    let mut lines = vec!["Git operation failed: the following files could not be updated.".to_string()];
    let mut unknown_holder = false;
    for (path, apps) in files {
        lines.push(format!("\nFile: {path}"));
        if apps.is_empty() {
            unknown_holder = true;
            lines.push("Cause: Possible file lock, read-only attribute, or insufficient permissions; no holding process was identified.".into());
        } else {
            lines.push("Cause: File is in use (locked by another process).".into());
            lines.push(format!("In use by: {}", apps.join(", ")));
        }
    }
    lines.push("\nHow to release access:".into());
    lines.push("1. Save your work and close the affected files in the programs using them. If a lock remains, fully quit those programs.".into());
    #[cfg(windows)]
    lines.push("2. If a listed program remains running, check its PID in Task Manager > Details. End only that process, after saving your work.".into());
    #[cfg(not(windows))]
    lines.push("2. If a listed program remains running, check its PID in Activity Monitor or your system process monitor. Quit only that process, after saving your work.".into());
    if unknown_holder {
        #[cfg(windows)]
        lines.push("3. To find an unidentified holder, open Resource Monitor > CPU > Associated Handles and search for the file name. Also check the file's read-only attribute and your write permissions.".into());
        #[cfg(not(windows))]
        lines.push("3. To find an unidentified holder, use lsof with the file path or your system's open-file inspector. Also check the file's read-only attribute and your write permissions.".into());
    }
    let last = if unknown_holder { 4 } else { 3 };
    lines.push(format!("{last}. After releasing access, refresh the repository status. If a Git operation is still in progress, continue that operation; otherwise retry the failed action."));
    lines.join("\n")
}

fn locked_paths(stderr: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for line in stderr.lines() {
        let lower = line.to_ascii_lowercase();
        for marker in FILE_MARKERS {
            let Some(start) = lower.find(marker) else { continue };
            let rest = &line[start + marker.len()..];
            // Split at the error, not an apostrophe or a colon inside the path.
            // Git quotes unlink paths, but normally leaves create paths unquoted.
            let Some((path, reason)) = rest.rsplit_once(": ") else { continue };
            if !access_reason(reason) {
                break;
            }
            let path = path.trim();
            let path = path.strip_prefix('\'').and_then(|value| value.strip_suffix('\''))
                .or_else(|| path.strip_prefix('"').and_then(|value| value.strip_suffix('"')))
                .unwrap_or(path);
            if !path.is_empty() && !found.iter().any(|item| item == path) {
                found.push(path.to_string());
            }
            break;
        }
    }
    found
}

fn access_reason(reason: &str) -> bool {
    let lower = reason.to_ascii_lowercase();
    ACCESS_REASONS.iter().any(|candidate| lower.contains(candidate))
}

#[cfg(windows)]
fn holders(path: &Path) -> Vec<String> {
    windows_rm::holders(path)
}

#[cfg(not(windows))]
fn holders(_path: &Path) -> Vec<String> {
    Vec::new()
}

#[cfg(windows)]
mod windows_rm {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    const ERROR_MORE_DATA: u32 = 234;
    const APP_NAME: usize = 256;
    const SERVICE_NAME: usize = 64;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct UniqueProcess {
        process_id: u32,
        start_time: [u32; 2],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ProcessInfo {
        process: UniqueProcess,
        app_name: [u16; APP_NAME],
        service_name: [u16; SERVICE_NAME],
        application_type: i32,
        app_status: u32,
        session_id: u32,
        restartable: i32,
    }

    #[link(name = "rstrtmgr")]
    extern "system" {
        fn RmStartSession(handle: *mut u32, flags: u32, key: *mut u16) -> u32;
        fn RmRegisterResources(
            handle: u32,
            file_count: u32,
            file_names: *const *const u16,
            app_count: u32,
            app_ids: *const UniqueProcess,
            service_count: u32,
            service_names: *const *const u16,
        ) -> u32;
        fn RmGetList(handle: u32, needed: *mut u32, count: *mut u32, infos: *mut ProcessInfo, reasons: *mut u32) -> u32;
        fn RmEndSession(handle: u32) -> u32;
    }

    pub(super) fn holders(path: &Path) -> Vec<String> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        // CCH_RM_SESSION_KEY is 32 characters, plus the terminator.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let mut key: Vec<u16> = format!("{nanos:032x}").encode_utf16().collect();
        key.push(0);
        let mut handle = 0u32;
        if unsafe { RmStartSession(&mut handle, 0, key.as_mut_ptr()) } != 0 {
            return Vec::new();
        }
        let mut apps = Vec::new();
        let files = [wide.as_ptr()];
        let registered = unsafe {
            RmRegisterResources(handle, 1, files.as_ptr(), 0, std::ptr::null(), 0, std::ptr::null())
        };
        if registered == 0 {
            let mut needed = 0u32;
            let mut count = 0u32;
            let mut reasons = 0u32;
            let mut result = unsafe {
                RmGetList(handle, &mut needed, &mut count, std::ptr::null_mut(), &mut reasons)
            };
            if (result == 0 || result == ERROR_MORE_DATA) && needed > 0 {
                let mut infos: Vec<ProcessInfo> = (0..needed).map(|_| unsafe { std::mem::zeroed() }).collect();
                count = needed;
                result = unsafe { RmGetList(handle, &mut needed, &mut count, infos.as_mut_ptr(), &mut reasons) };
                if result == 0 {
                    for info in infos.iter().take(count as usize) {
                        let pid = info.process.process_id;
                        if pid == 0 {
                            continue;
                        }
                        let name = wide_text(&info.app_name);
                        let label = if name.is_empty() { format!("process {pid}") } else { format!("{name} (PID {pid})") };
                        if !apps.contains(&label) {
                            apps.push(label);
                        }
                    }
                }
            }
        }
        unsafe { RmEndSession(handle) };
        apps
    }

    fn wide_text(buffer: &[u16]) -> String {
        let end = buffer.iter().position(|value| *value == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_path_git_could_not_replace() {
        let stderr = "error: unable to unlink old 'Excel/Z战斗怪物表.xlsx': Invalid argument\n\
                      error: could not apply autostash; your changes are safe in the stash";
        assert_eq!(locked_paths(stderr), vec!["Excel/Z战斗怪物表.xlsx".to_string()]);
    }

    #[test]
    fn reads_a_file_that_could_not_be_created() {
        let stderr = "error: unable to create file 'Assets/UI/Main.prefab': Permission denied";
        assert_eq!(locked_paths(stderr), vec!["Assets/UI/Main.prefab".to_string()]);
    }

    #[test]
    fn ignores_failures_that_are_not_file_locks() {
        assert!(locked_paths("error: pathspec 'nope' did not match any file(s)").is_empty());
        assert!(locked_paths("error: unable to unlink old 'a.txt': No such file or directory").is_empty());
        assert!(locked_paths("error: could not apply autostash; your changes are safe in the stash").is_empty());
    }

    #[test]
    fn explains_the_file_without_a_known_holder() {
        let repo = std::env::temp_dir();
        let message = explain(&repo, "error: unable to unlink old 'locked.xlsx': Invalid argument").unwrap();
        assert!(message.contains("locked.xlsx"));
        assert!(message.contains("Possible file lock"));
        assert!(message.contains("no holding process was identified"));
        assert!(message.contains("How to release access:"));
        assert!(message.contains("write permissions"));
        assert!(!message.contains("Cause: File is in use"));
    }

    #[test]
    fn reads_unquoted_paths_and_apostrophes_without_losing_the_file_name() {
        let stderr = "error: unable to create file docs/team's notes.md: Permission denied\n\
                      error: unable to unlink old 'data/user's records.db': Invalid argument\n\
                      error: unable to write file \"art/source file.psd\": Sharing violation";
        assert_eq!(locked_paths(stderr), vec![
            "docs/team's notes.md".to_string(),
            "data/user's records.db".to_string(),
            "art/source file.psd".to_string(),
        ]);
    }

    #[test]
    fn does_not_mistake_a_file_name_for_a_lock_reason() {
        assert!(locked_paths("error: unable to unlink old 'permission denied.txt': No such file or directory").is_empty());
        assert!(locked_paths("error: unable to create file 'resource busy.txt': No space left on device").is_empty());
        assert!(locked_paths("fatal: Unable to create '.git/index.lock': File exists").is_empty());
    }

    #[test]
    fn deduplicates_paths_across_git_errors() {
        let stderr = "error: unable to unlink old 'data/cache.db': Permission denied\n\
                      error: unable to create file data/cache.db: Permission denied";
        assert_eq!(locked_paths(stderr), vec!["data/cache.db".to_string()]);
    }

    #[test]
    fn confirmed_locks_include_the_cause_process_and_recovery_steps() {
        let message = describe(&[("art/source.psd".into(), vec!["Image Editor (PID 123)".into()])]);
        assert!(message.contains("Cause: File is in use (locked by another process)."), "{message}");
        assert!(message.contains("In use by: Image Editor (PID 123)"), "{message}");
        assert!(message.contains("Save your work"), "{message}");
        assert!(message.contains("fully quit"), "{message}");
        assert!(message.contains("check its PID"), "{message}");
        assert!(message.contains("Git operation is still in progress, continue"), "{message}");
        assert!(!message.contains("Possible file lock"), "{message}");
    }

    #[test]
    fn keeps_each_file_with_its_own_holder_including_unidentified_files() {
        let message = describe(&[
            ("file-a.db".into(), vec!["Database (PID 11)".into()]),
            ("file-b.psd".into(), vec!["Image Editor (PID 22)".into()]),
            ("file-c.pdf".into(), vec![]),
            ("file-d.txt".into(), vec!["Editor (PID 33)".into()]),
        ]);
        let first = message.split("File: file-b.psd").next().unwrap();
        assert!(first.contains("Database (PID 11)"));
        assert!(!first.contains("Image Editor"));
        let unknown = message.split("File: file-c.pdf").nth(1).unwrap().split("File: file-d.txt").next().unwrap();
        assert!(unknown.contains("no holding process was identified"));
        assert!(!unknown.contains("Cause: File is in use"));
        assert!(message.contains("File: file-d.txt"));
        assert!(message.contains("write permissions"));
    }

    #[cfg(windows)]
    #[test]
    fn writes_name_the_process_and_explain_how_to_release_the_file() {
        let repo = std::env::temp_dir().join(format!("awegit-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            crate::cli::run(&repo, args).unwrap_or_else(|error| panic!("git {}: {error}", args.join(" ")));
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::create_dir_all(repo.join("nohooks")).unwrap();
        git(&["config", "core.hooksPath", "nohooks"]);
        git(&["config", "user.name", "Test"]);
        git(&["config", "user.email", "test@example.com"]);
        std::fs::write(repo.join("held.txt"), "one").unwrap();
        git(&["add", "held.txt"]);
        git(&["commit", "-qm", "one"]);
        git(&["checkout", "-qb", "other"]);
        std::fs::write(repo.join("held.txt"), "two").unwrap();
        git(&["commit", "-qam", "two"]);
        git(&["checkout", "-q", "main"]);
        // Let the index settle: a racily clean entry is re-read, and the locked
        // file cannot be read, which hides the real unlink failure.
        std::thread::sleep(std::time::Duration::from_millis(2000));
        git(&["update-index", "--refresh"]);

        let lock = test_lock::FileLock::exclusive(&repo.join("held.txt"));
        for args in [&["merge", "--ff-only", "other"][..], &["checkout", "other"][..]] {
            let message = crate::cli::run(&repo, args).unwrap_err().to_string();
            assert!(message.contains("File: held.txt"), "{message}");
            assert!(message.contains("Cause: File is in use (locked by another process)."), "{message}");
            assert!(message.contains(&format!("PID {}", std::process::id())), "{message}");
            assert!(message.contains("Task Manager > Details"), "{message}");
            assert!(message.contains("unable to unlink"), "{message}");
        }
        drop(lock);
        // Checkout can exit 0 and leave HEAD on `other` while the file stays behind.
        // `reset --hard main` would rewind that branch. Switch back, then prove the
        // same merge updates the file once nothing is holding it.
        git(&["checkout", "-f", "main"]);
        git(&["merge", "--ff-only", "other"]);
        assert_eq!(std::fs::read_to_string(repo.join("held.txt")).unwrap(), "two");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[cfg(windows)]
    mod test_lock {
        use std::os::windows::ffi::OsStrExt;
        use std::path::Path;

        const GENERIC_READ: u32 = 0x8000_0000;
        const OPEN_EXISTING: u32 = 3;
        const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;

        #[link(name = "kernel32")]
        extern "system" {
            fn CreateFileW(
                name: *const u16,
                access: u32,
                share: u32,
                security: *const std::ffi::c_void,
                creation: u32,
                flags: u32,
                template: *mut std::ffi::c_void,
            ) -> *mut std::ffi::c_void;
            fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        }

        pub(super) struct FileLock(*mut std::ffi::c_void);

        impl FileLock {
            pub(super) fn exclusive(path: &Path) -> Self {
                let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
                let handle = unsafe {
                    CreateFileW(wide.as_ptr(), GENERIC_READ, 0, std::ptr::null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut())
                };
                assert!(handle != (-1isize as *mut std::ffi::c_void), "could not open the test file");
                Self(handle)
            }
        }

        impl Drop for FileLock {
            fn drop(&mut self) {
                unsafe { CloseHandle(self.0) };
            }
        }
    }
}
