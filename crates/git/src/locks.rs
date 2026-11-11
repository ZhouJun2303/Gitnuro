//! Name the program holding a file when Git cannot replace it in the work tree.
//!
//! Windows reports a locked file as `unable to unlink old '...': Invalid argument`
//! or `...: Permission denied`. Git only has the path; the Restart Manager knows
//! which processes hold it, so the error can tell the user what to close.

use std::path::Path;

/// Lines Git prints when a work-tree file cannot be replaced or removed.
const FILE_MARKERS: [&str; 5] = [
    "unable to unlink old '",
    "unable to unlink '",
    "could not unlink '",
    "unable to create file '",
    "unable to write file '",
];

/// Failures that mean another program holds the file, rather than a bad path.
const LOCK_REASONS: [&str; 8] = [
    "invalid argument",
    "permission denied",
    "access is denied",
    "being used by another process",
    "resource busy",
    "operation not permitted",
    "cannot access the file",
    "sharing violation",
];

/// Turn a Git failure about a locked work-tree file into a message that names
/// the process holding it. Every other failure is left alone.
pub(crate) fn explain(repo: &Path, stderr: &str) -> Option<String> {
    let paths = locked_paths(stderr);
    if paths.is_empty() {
        return None;
    }
    let mut apps: Vec<String> = Vec::new();
    for path in &paths {
        for app in holders(&repo.join(path)) {
            if !apps.contains(&app) {
                apps.push(app);
            }
        }
    }
    let files = list_files(&paths);
    Some(if apps.is_empty() {
        format!("could not update {files}: the file is locked or read-only. Close the program using it or clear the read-only attribute, then try again.")
    } else {
        format!("could not update {files}: in use by {}. Close the program, then try again.", apps.join(", "))
    })
}

fn locked_paths(stderr: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for line in stderr.lines() {
        if !lock_reason(line) {
            continue;
        }
        for marker in FILE_MARKERS {
            let Some(start) = line.find(marker) else { continue };
            let rest = &line[start + marker.len()..];
            let Some(end) = rest.find('\'') else { continue };
            let path = rest[..end].trim();
            if !path.is_empty() && !found.iter().any(|item| item == path) {
                found.push(path.to_string());
            }
            break;
        }
    }
    found
}

fn lock_reason(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    LOCK_REASONS.iter().any(|reason| lower.contains(reason))
}

fn list_files(paths: &[String]) -> String {
    const SHOWN: usize = 3;
    if paths.len() <= SHOWN {
        paths.join(", ")
    } else {
        format!("{}, and {} more", paths[..SHOWN].join(", "), paths.len() - SHOWN)
    }
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
        assert!(message.contains("Close"));
    }

    #[test]
    fn lists_at_most_three_paths() {
        let paths = vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()];
        assert_eq!(list_files(&paths), "a, b, c, and 1 more");
    }

    #[cfg(windows)]
    #[test]
    fn merge_names_the_process_holding_the_file() {
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
        let message = crate::cli::run(&repo, &["merge", "--ff-only", "other"]).unwrap_err().to_string();
        assert!(message.contains("could not update held.txt"), "{message}");
        assert!(message.contains(&format!("PID {}", std::process::id())), "{message}");
        drop(lock);
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
