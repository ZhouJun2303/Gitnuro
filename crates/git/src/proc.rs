//! Stop a git child and the processes it starts.
//!
//! Windows assigns the child to a job object before the process runs, so cancel
//! reaches grandchildren. Unix starts the child in its own process group. Cancel
//! waits until that process has exited. The stored job or process group is what
//! gets signaled, so a later command that reused the process id is left alone.

use std::io::{Error, ErrorKind};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// Kill scope captured when a command is spawned.
pub(crate) struct Group {
    #[cfg(windows)]
    job: isize,
    #[cfg(unix)]
    owns_group: bool,
}

impl Group {
    /// Stop this command and wait until its process has exited.
    pub(crate) fn kill_and_wait(&self, pid: u32) {
        #[cfg(windows)]
        if self.job != 0 {
            unsafe { TerminateJobObject(self.job as *mut std::ffi::c_void, 1) };
        }
        kill_pid_tree(pid, cfg_owns_group(self));
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.job != 0 {
            unsafe { CloseHandle(self.job as *mut std::ffi::c_void) };
            self.job = 0;
        }
    }
}

#[cfg(windows)]
fn cfg_owns_group(_group: &Group) -> bool {
    false
}

#[cfg(unix)]
fn cfg_owns_group(group: &Group) -> bool {
    group.owns_group
}

/// Spawn `command` so a later cancel can stop its children as well.
///
/// The returned [`Group`] owns the job object. Dropping it closes that job.
pub(crate) fn spawn_prepared(command: &mut Command) -> Result<(Child, Group), Error> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        let child = command.spawn()?;
        let handle = child.as_raw_handle();
        let job = assign_job(handle);
        if !resume(handle) {
            unsafe { TerminateProcess(handle, 1) };
            if job != 0 {
                unsafe { CloseHandle(job as *mut std::ffi::c_void) };
            }
            return Err(Error::new(ErrorKind::Other, "could not resume process"));
        }
        return Ok((child, Group { job }));
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
        let child = command.spawn()?;
        Ok((child, Group { owns_group: true }))
    }
}

/// Stop an untracked process. A process id we did not spawn is signaled directly,
/// never as a process group, so an unrelated group with the same number stays up.
pub(crate) fn kill_pid(pid: u32) {
    kill_pid_tree(pid, false);
}

fn kill_pid_tree(pid: u32, owns_group: bool) {
    if pid == 0 || pid == std::process::id() {
        return;
    }
    #[cfg(windows)]
    {
        let _ = owns_group;
        kill_tree_windows(pid);
    }
    #[cfg(unix)]
    {
        signal_unix(pid, owns_group);
        let _ = wait_exited(pid, Duration::from_secs(5));
    }
}

#[cfg(windows)]
fn kill_tree_windows(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut tried_taskkill = false;
    while Instant::now() < deadline {
        let descendants = descendant_pids(pid);
        if !process_alive(pid) && descendants.is_empty() {
            return;
        }
        for child in descendants.iter().rev() {
            terminate_pid(*child);
        }
        terminate_pid(pid);
        if wait_dead(pid, Duration::from_millis(200)) && descendant_pids(pid).is_empty() {
            return;
        }
        if !tried_taskkill {
            tried_taskkill = true;
            taskkill_tree(pid);
        } else {
            std::thread::sleep(Duration::from_millis(40));
        }
    }
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const CREATE_SUSPENDED: u32 = 0x0000_0004;
#[cfg(windows)]
const PROCESS_TERMINATE: u32 = 0x0001;
#[cfg(windows)]
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
#[cfg(windows)]
const SYNCHRONIZE: u32 = 0x0010_0000;
#[cfg(windows)]
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
#[cfg(windows)]
const STILL_ACTIVE: u32 = 259;
#[cfg(windows)]
const ERROR_ACCESS_DENIED: u32 = 5;
#[cfg(windows)]
const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;

#[cfg(windows)]
#[repr(C)]
struct JobObjectBasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[cfg(windows)]
#[repr(C)]
struct IoCounters {
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}

#[cfg(windows)]
#[repr(C)]
struct JobObjectExtendedLimitInformation {
    basic: JobObjectBasicLimitInformation,
    io_info: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[cfg(all(windows, target_pointer_width = "64"))]
const _: () = assert!(std::mem::size_of::<JobObjectBasicLimitInformation>() == 64);
#[cfg(all(windows, target_pointer_width = "64"))]
const _: () = assert!(std::mem::size_of::<IoCounters>() == 48);
#[cfg(all(windows, target_pointer_width = "64"))]
const _: () = assert!(std::mem::size_of::<JobObjectExtendedLimitInformation>() == 144);

#[cfg(windows)]
#[repr(C)]
struct ProcessEntry32W {
    size: u32,
    cnt_usage: u32,
    process_id: u32,
    default_heap_id: usize,
    module_id: u32,
    threads: u32,
    parent_process_id: u32,
    pri_class_base: i32,
    flags: u32,
    exe_file: [u16; 260],
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn CreateJobObjectW(attrs: *const u8, name: *const u16) -> *mut std::ffi::c_void;
    fn SetInformationJobObject(job: *mut std::ffi::c_void, class: i32, info: *const std::ffi::c_void, len: u32) -> i32;
    fn AssignProcessToJobObject(job: *mut std::ffi::c_void, process: *mut std::ffi::c_void) -> i32;
    fn TerminateJobObject(job: *mut std::ffi::c_void, exit_code: u32) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
    fn TerminateProcess(process: *mut std::ffi::c_void, exit_code: u32) -> i32;
    fn WaitForSingleObject(handle: *mut std::ffi::c_void, millis: u32) -> u32;
    fn GetExitCodeProcess(process: *mut std::ffi::c_void, code: *mut u32) -> i32;
    fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    fn GetLastError() -> u32;
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut std::ffi::c_void;
    fn Process32FirstW(snapshot: *mut std::ffi::c_void, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: *mut std::ffi::c_void, entry: *mut ProcessEntry32W) -> i32;
}

#[cfg(windows)]
#[link(name = "ntdll")]
extern "system" {
    fn NtResumeProcess(process: *mut std::ffi::c_void) -> i32;
}

#[cfg(windows)]
fn assign_job(process: *mut std::ffi::c_void) -> isize {
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return 0;
        }
        let mut info: JobObjectExtendedLimitInformation = std::mem::zeroed();
        info.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = SetInformationJobObject(
            job,
            9,
            &info as *const JobObjectExtendedLimitInformation as *const std::ffi::c_void,
            std::mem::size_of::<JobObjectExtendedLimitInformation>() as u32,
        );
        if configured == 0 || AssignProcessToJobObject(job, process) == 0 {
            CloseHandle(job);
            return 0;
        }
        job as isize
    }
}

#[cfg(windows)]
fn resume(process: *mut std::ffi::c_void) -> bool {
    unsafe { NtResumeProcess(process) >= 0 }
}

#[cfg(windows)]
fn terminate_pid(pid: u32) {
    if pid == 0 || pid == std::process::id() {
        return;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            return;
        }
        TerminateProcess(handle, 1);
        CloseHandle(handle);
    }
}

#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);
        ok != 0 && code == STILL_ACTIVE
    }
}

#[cfg(windows)]
fn wait_dead(pid: u32, timeout: Duration) -> bool {
    unsafe {
        let handle = OpenProcess(SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return GetLastError() != ERROR_ACCESS_DENIED;
        }
        let millis = timeout.as_millis().min(u32::MAX as u128) as u32;
        let result = WaitForSingleObject(handle, millis);
        CloseHandle(handle);
        result == 0
    }
}

#[cfg(windows)]
fn descendant_pids(root: u32) -> Vec<u32> {
    let mut rows = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() || snapshot == (-1isize as *mut std::ffi::c_void) {
            return Vec::new();
        }
        let mut entry: ProcessEntry32W = std::mem::zeroed();
        entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                rows.push((entry.process_id, entry.parent_process_id));
                entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }
    let mut children: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    for (pid, parent) in rows {
        children.entry(parent).or_default().push(pid);
    }
    let mut found = Vec::new();
    let mut pending = vec![root];
    let mut seen = std::collections::HashSet::new();
    seen.insert(root);
    while let Some(pid) = pending.pop() {
        if let Some(kids) = children.get(&pid) {
            for child in kids {
                if *child != 0 && *child != std::process::id() && seen.insert(*child) {
                    pending.push(*child);
                    found.push(*child);
                }
            }
        }
    }
    found
}

#[cfg(windows)]
fn taskkill_tree(pid: u32) {
    let mut command = Command::new("taskkill");
    crate::cli::hide_console(&mut command);
    let _ = command.args(["/F", "/T", "/PID", &pid.to_string()]).output();
}

#[cfg(unix)]
fn signal_unix(pid: u32, owns_group: bool) {
    if pid == 0 || pid == std::process::id() {
        return;
    }
    unsafe {
        if owns_group {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
        libc::kill(pid as i32, libc::SIGKILL);
    }
}

/// Wait until `pid` has exited without consuming the status the parent will read.
#[cfg(unix)]
fn wait_exited(pid: u32, timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let rc = unsafe { libc::waitid(libc::P_PID, pid as libc::id_t, &mut info, libc::WEXITED | libc::WNOWAIT | libc::WNOHANG) };
        if rc == 0 && info.si_pid == pid as i32 {
            return true;
        }
        if rc != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD) {
            return true;
        }
        let probe = unsafe { libc::kill(pid as i32, 0) };
        if probe != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}
