//! Session log and timing for commands, `git` processes, and main-thread stalls.
//!
//! Full lines go to `<settings dir>/logs/awegit-debug.log` or `awegit-release.log`.
//! Warnings, errors, slow calls, session boundaries, and the exit summary also go to
//! `awegit-issue-debug.log` or `awegit-issue.log`, which stays small enough to attach
//! to a report. Git traces keep command names and flags. A real failure also keeps a
//! short stderr tail. Tokens, URL userinfo, and secret assignments are removed.

use std::collections::{BTreeMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const SLOW_MS: u64 = 200;
const STALL_MS: u64 = 150;
const MAX_BYTES: u64 = 8 * 1024 * 1024;
const ISSUE_MAX: u64 = 192 * 1024;
const ISSUE_KEEP: usize = 144 * 1024;

static START: OnceLock<Instant> = OnceLock::new();
static MAIN: OnceLock<ThreadId> = OnceLock::new();
static FILE: Mutex<Option<File>> = Mutex::new(None);
static ISSUE: Mutex<Option<File>> = Mutex::new(None);
static STATS: Mutex<BTreeMap<String, Stat>> = Mutex::new(BTreeMap::new());
static ON_MAIN: Mutex<Option<(String, Instant)>> = Mutex::new(None);
static STALLS: AtomicU64 = AtomicU64::new(0);
static STALL_MAX: AtomicU64 = AtomicU64::new(0);
static STALL_TOTAL: AtomicU64 = AtomicU64::new(0);
static ACTIVITY: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stat {
    pub name: String,
    pub count: u64,
    pub total_ms: u64,
    pub max_ms: u64,
    pub last_ms: u64,
    pub slow: u64,
    pub on_main: u64,
    pub failed: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub uptime_ms: u64,
    pub build: &'static str,
    pub log_file: String,
    pub issue_file: String,
    pub stalls: u64,
    pub stall_max_ms: u64,
    pub stall_total_ms: u64,
    pub items: Vec<Stat>,
}

#[derive(Debug, Deserialize)]
pub struct ClientEntry {
    pub level: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub ms: Option<f64>,
    #[serde(default)]
    pub detail: Option<String>,
}

pub fn dir() -> PathBuf {
    crate::settings::path()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("logs")))
        .unwrap_or_else(|| std::env::temp_dir().join("AweGit").join("logs"))
}

pub fn file() -> PathBuf {
    dir().join(perf_log_name(cfg!(debug_assertions)))
}

pub fn issue_file() -> PathBuf {
    dir().join(issue_log_name(cfg!(debug_assertions)))
}

fn perf_log_name(debug: bool) -> &'static str {
    if debug { "awegit-debug.log" } else { "awegit-release.log" }
}

fn issue_log_name(debug: bool) -> &'static str {
    if debug { "awegit-issue-debug.log" } else { "awegit-issue.log" }
}

fn rotated_log() -> PathBuf {
    dir().join(if cfg!(debug_assertions) { "awegit-debug.1.log" } else { "awegit-release.1.log" })
}

fn build() -> &'static str {
    if cfg!(debug_assertions) { "debug" } else { "release" }
}

/// Call once from the main thread before the Tauri builder runs.
pub fn init() {
    let _ = START.set(Instant::now());
    let _ = MAIN.set(std::thread::current().id());
    let dir = dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = file();
    if std::fs::metadata(&path).map(|meta| meta.len() > MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&path, rotated_log());
    }
    if let Ok(handle) = OpenOptions::new().create(true).append(true).open(&path) {
        if let Ok(mut slot) = FILE.lock() {
            *slot = Some(handle);
        }
    }
    if std::fs::metadata(issue_file()).map(|meta| meta.len() > ISSUE_MAX).unwrap_or(false) {
        shrink_issue_file();
    }
    if let Ok(handle) = OpenOptions::new().create(true).append(true).open(issue_file()) {
        if let Ok(mut slot) = ISSUE.lock() {
            *slot = Some(handle);
        }
    }
    let exe = std::env::current_exe().map(|path| path.display().to_string()).unwrap_or_default();
    write("INFO", "app", &format!(
        "==== AweGit {} {} pid={} os={} arch={} exe={exe}",
        env!("CARGO_PKG_VERSION"),
        build(),
        std::process::id(),
        std::env::consts::OS,
        std::env::consts::ARCH,
    ));
    if cfg!(debug_assertions) {
        write("WARN", "app", "debug build: gitoxide and serde run unoptimized, expect slower status and log reads");
    }
}

fn uptime() -> Duration {
    START.get().map(|start| start.elapsed()).unwrap_or_default()
}

fn on_main_thread() -> bool {
    MAIN.get().is_some_and(|id| *id == std::thread::current().id())
}

pub fn write(level: &str, target: &str, message: &str) {
    let message = redact(message);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("{now} +{:>9.3}s {level:<5} {target:<6} {message}\n", uptime().as_secs_f64());
    if cfg!(debug_assertions) {
        eprint!("{line}");
    }
    if matches!(target, "git" | "cmd" | "stall") {
        if let Ok(mut activity) = ACTIVITY.lock() {
            activity.push_back(format!("{level} {target} {message}"));
            while activity.len() > 80 {
                activity.pop_front();
            }
        }
    }
    if let Ok(mut slot) = FILE.lock() {
        if let Some(handle) = slot.as_mut() {
            let _ = handle.write_all(line.as_bytes());
        }
    }
    if routes_to_issue(level, target, &message) {
        write_issue(&line);
    }
}

pub fn mark(message: &str) {
    write("INFO", "app", message);
}

fn record(name: &str, ms: u64, main: bool, ok: bool) {
    let Ok(mut stats) = STATS.lock() else { return };
    let stat = stats.entry(name.to_string()).or_insert_with(|| Stat { name: name.to_string(), ..Stat::default() });
    stat.count += 1;
    stat.total_ms += ms;
    stat.max_ms = stat.max_ms.max(ms);
    stat.last_ms = ms;
    if ms >= SLOW_MS {
        stat.slow += 1;
    }
    if main {
        stat.on_main += 1;
    }
    if !ok {
        stat.failed += 1;
    }
}

/// Times one Tauri command until dropped.
pub struct Span {
    name: String,
    start: Instant,
    main: bool,
}

pub fn command(name: &str) -> Span {
    let main = on_main_thread();
    if main {
        if let Ok(mut slot) = ON_MAIN.lock() {
            *slot = Some((name.to_string(), Instant::now()));
        }
    }
    Span { name: name.to_string(), start: Instant::now(), main }
}

impl Drop for Span {
    fn drop(&mut self) {
        let ms = self.start.elapsed().as_millis() as u64;
        if self.main {
            if let Ok(mut slot) = ON_MAIN.lock() {
                *slot = None;
            }
        }
        record(&format!("cmd {}", self.name), ms, self.main, true);
        let level = if ms >= SLOW_MS { "SLOW" } else { "INFO" };
        let thread = if self.main { "main" } else { "worker" };
        write(level, "cmd", &format!("{} {ms}ms thread={thread}", self.name));
    }
}

/// Hook for `awegit_git::set_trace`. Paths, messages, and option values are dropped.
/// `note` is a short stderr tail for a failure the caller will surface.
pub fn git(args: &[&str], elapsed: Duration, ok: bool, note: &str) {
    let ms = elapsed.as_millis() as u64;
    let mut sub = "";
    let mut flags = Vec::new();
    let mut skip = false;
    for arg in args {
        if skip {
            skip = false;
            continue;
        }
        if *arg == "-c" || *arg == "-C" {
            skip = true;
            continue;
        }
        if sub.is_empty() {
            if !arg.starts_with('-') {
                sub = arg;
            }
            continue;
        }
        if arg.starts_with('-') {
            flags.push(arg.split('=').next().unwrap_or(arg));
        }
    }
    let name = if sub.is_empty() { "git".to_string() } else { format!("git {sub}") };
    record(&name, ms, on_main_thread(), ok);
    let level = if !ok { "WARN" } else if ms >= SLOW_MS { "SLOW" } else { "INFO" };
    let status = if ok { "" } else { " failed" };
    let detail = flatten_note(note);
    let extra = if detail.is_empty() { String::new() } else { format!(" {detail}") };
    write(level, "git", &format!("{sub} {} {ms}ms{status}{extra}", flags.join(" ")));
}

pub fn client(entries: Vec<ClientEntry>) {
    for entry in entries.into_iter().take(200) {
        if entry.kind == "invoke" && entry.level == "error" {
            // The command span has already recorded the call as success. The rejection
            // arrives from the webview, which is what the user actually saw.
            note_failed(&format!("cmd {}", entry.name));
        }
        let level = match entry.level.as_str() {
            "error" => "ERROR",
            "warn" => "WARN",
            "slow" => "SLOW",
            _ => "INFO",
        };
        let ms = entry.ms.map(|ms| format!(" {ms:.0}ms")).unwrap_or_default();
        let detail = entry
            .detail
            .map(|text| format!(" {}", text.replace(['\n', '\r', '\t', '\0'], " ").chars().take(400).collect::<String>()))
            .unwrap_or_default();
        write(level, "web", &format!("{} {}{ms}{detail}", entry.kind, entry.name));
    }
}

/// Pings the main thread so a blocked event loop shows up in the log with the command that held it.
pub fn watch_main_thread(app: tauri::AppHandle) {
    static PENDING: AtomicBool = AtomicBool::new(false);
    std::thread::Builder::new()
        .name("awegit-watchdog".into())
        .spawn(move || {
            let mut sent = Instant::now();
            let mut warned = 0u64;
            loop {
                std::thread::sleep(Duration::from_millis(100));
                if PENDING.load(Ordering::Acquire) {
                    let waited = sent.elapsed().as_millis() as u64;
                    let step = waited / 1000;
                    if step > warned {
                        warned = step;
                        let holder = ON_MAIN
                            .lock()
                            .ok()
                            .and_then(|slot| slot.as_ref().map(|(name, start)| format!("{name} ({}ms)", start.elapsed().as_millis())))
                            .unwrap_or_else(|| "webview/event loop".into());
                        write("WARN", "stall", &format!("main thread still blocked {waited}ms, running: {holder}"));
                    }
                    continue;
                }
                std::thread::sleep(Duration::from_millis(150));
                PENDING.store(true, Ordering::Release);
                warned = 0;
                sent = Instant::now();
                let started = sent;
                let posted = app.run_on_main_thread(move || {
                    let ms = started.elapsed().as_millis() as u64;
                    PENDING.store(false, Ordering::Release);
                    if ms >= STALL_MS {
                        STALLS.fetch_add(1, Ordering::Relaxed);
                        STALL_TOTAL.fetch_add(ms, Ordering::Relaxed);
                        STALL_MAX.fetch_max(ms, Ordering::Relaxed);
                        write("WARN", "stall", &format!("main thread was blocked {ms}ms"));
                    }
                });
                if posted.is_err() {
                    break;
                }
            }
        })
        .ok();
}

pub fn activity() -> Vec<String> {
    ACTIVITY.lock().map(|items| items.iter().cloned().collect()).unwrap_or_default()
}

pub fn snapshot() -> Snapshot {
    let mut items: Vec<Stat> = STATS.lock().map(|stats| stats.values().cloned().collect()).unwrap_or_default();
    items.sort_by(|a, b| b.total_ms.cmp(&a.total_ms));
    Snapshot {
        uptime_ms: uptime().as_millis() as u64,
        build: build(),
        log_file: file().display().to_string(),
        issue_file: issue_file().display().to_string(),
        stalls: STALLS.load(Ordering::Relaxed),
        stall_max_ms: STALL_MAX.load(Ordering::Relaxed),
        stall_total_ms: STALL_TOTAL.load(Ordering::Relaxed),
        items,
    }
}

/// Writes the slowest entries so a session ends with a readable summary.
pub fn summary() {
    let snap = snapshot();
    write("INFO", "app", &format!(
        "==== exit after {:.1}s, main-thread stalls={} max={}ms total={}ms",
        snap.uptime_ms as f64 / 1000.0,
        snap.stalls,
        snap.stall_max_ms,
        snap.stall_total_ms
    ));
    for stat in snap.items.iter().take(15) {
        write("INFO", "sum", &format!(
            "{} count={} total={}ms avg={}ms max={}ms slow={} main={} failed={}",
            stat.name,
            stat.count,
            stat.total_ms,
            stat.total_ms / stat.count.max(1),
            stat.max_ms,
            stat.slow,
            stat.on_main,
            stat.failed
        ));
    }
}

/// Text for the About dialog. Version, build, log paths, and the issue-log tail.
pub fn diagnostics() -> String {
    flush_logs();
    let snap = snapshot();
    let mut slow = String::new();
    for stat in snap.items.iter().filter(|stat| stat.slow > 0 || stat.failed > 0 || stat.max_ms >= SLOW_MS).take(12) {
        slow.push_str(&format!(
            "{} count={} max={}ms slow={} failed={}\n",
            stat.name, stat.count, stat.max_ms, stat.slow, stat.failed
        ));
    }
    if slow.is_empty() {
        slow.push_str("none\n");
    }
    let tail = read_tail(&issue_file(), 32 * 1024);
    let body = if tail.trim().is_empty() { "(no warnings yet)\n".to_string() } else { tail };
    let exe = std::env::current_exe().map(|path| path.display().to_string()).unwrap_or_default();
    format!(
        "AweGit {} {}\n{}\nexe {exe}\nlog {}\nissue {}\nuptime {:.1}s\nstalls {} max {}ms total {}ms\n\nslow:\n{slow}\n---- issue log ----\n{body}",
        env!("CARGO_PKG_VERSION"),
        build(),
        os_detail(),
        snap.log_file,
        snap.issue_file,
        snap.uptime_ms as f64 / 1000.0,
        snap.stalls,
        snap.stall_max_ms,
        snap.stall_total_ms,
    )
}

fn note_failed(name: &str) {
    let Ok(mut stats) = STATS.lock() else { return };
    let stat = stats.entry(name.to_string()).or_insert_with(|| Stat { name: name.to_string(), ..Stat::default() });
    stat.failed += 1;
    if stat.count == 0 {
        stat.count = 1;
    }
}

fn routes_to_issue(level: &str, target: &str, message: &str) -> bool {
    matches!(level, "WARN" | "ERROR" | "SLOW") || target == "sum" || message.starts_with("====")
}

fn write_issue(line: &str) {
    let Ok(mut slot) = ISSUE.lock() else { return };
    let Some(handle) = slot.as_mut() else { return };
    let _ = handle.write_all(line.as_bytes());
    let _ = handle.flush();
    let oversized = handle.metadata().map(|meta| meta.len() > ISSUE_MAX).unwrap_or(false);
    if !oversized {
        return;
    }
    *slot = None;
    shrink_issue_file();
    *slot = OpenOptions::new().create(true).append(true).open(issue_file()).ok();
}

fn shrink_issue_file() {
    let path = issue_file();
    let Ok(text) = std::fs::read(&path) else { return };
    let kept = issue_tail(&text, ISSUE_KEEP);
    if kept.len() == text.len() {
        return;
    }
    let _ = std::fs::write(&path, kept);
}

fn issue_tail(text: &[u8], keep: usize) -> &[u8] {
    if text.len() <= keep {
        return text;
    }
    let start = text.len() - keep;
    let offset = text[start..].iter().position(|byte| *byte == b'\n').map(|index| index + 1).unwrap_or(0);
    &text[start + offset..]
}

fn flush_logs() {
    for slot in [&FILE, &ISSUE] {
        if let Ok(mut guard) = slot.lock() {
            if let Some(handle) = guard.as_mut() {
                let _ = handle.flush();
            }
        }
    }
}

fn read_tail(path: &Path, max: usize) -> String {
    let Ok(text) = std::fs::read(path) else { return String::new() };
    if text.len() <= max {
        return String::from_utf8_lossy(&text).into_owned();
    }
    let start = text.len() - max;
    let aligned = text[start..].iter().position(|byte| *byte == b'\n').map(|index| start + index + 1).unwrap_or(start);
    String::from_utf8_lossy(&text[aligned..]).into_owned()
}

fn flatten_note(note: &str) -> String {
    let mut flat = String::new();
    for ch in note.chars() {
        if ch.is_control() {
            if !flat.ends_with(' ') {
                flat.push(' ');
            }
            continue;
        }
        flat.push(ch);
    }
    let trimmed = flat.trim();
    let count = trimmed.chars().count();
    if count <= 400 {
        return trimmed.to_string();
    }
    trimmed.chars().skip(count - 400).collect()
}

fn os_detail() -> String {
    #[cfg(windows)]
    if let Some(label) = windows_label() {
        return label;
    }
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(windows)]
fn windows_label() -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let output = std::process::Command::new("cmd").args(["/C", "ver"]).creation_flags(CREATE_NO_WINDOW).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    Some(format!("{collapsed} {}", std::env::consts::ARCH))
}

/// Remove URL userinfo, `key=secret` assignments, and well-known token prefixes.
pub fn redact(input: &str) -> String {
    if !looks_sensitive(input) {
        return input.to_string();
    }
    redact_prefixed_tokens(&redact_assignments(&redact_url_userinfo(input)))
}

fn looks_sensitive(input: &str) -> bool {
    if input.contains('@') || input.contains("://") {
        return true;
    }
    let lower = input.to_ascii_lowercase();
    ["token", "password", "passwd", "secret", "authorization", "bearer ", "ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_", "glpat-", "glptt-", "xox", "akia"]
        .iter()
        .any(|key| lower.contains(key))
}

const ASSIGN_KEYS: &[&str] = &["access_token", "private_token", "api_key", "authorization", "password", "passwd", "secret", "token"];

fn redact_url_userinfo(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(scheme) = find_sub(bytes, cursor, b"://") {
        let auth = scheme + 3;
        out.push_str(&input[cursor..auth]);
        let mut end = auth;
        let mut at = None;
        while end < bytes.len() {
            let byte = bytes[end];
            if matches!(byte, b'/' | b'?' | b'#' | b' ' | b'\t' | b'\r' | b'\n' | b'"' | b'\'' | b'<' | b'>' | b')' | b']' | b',') {
                break;
            }
            if byte == b'@' {
                at = Some(end);
            }
            end += 1;
        }
        if let Some(at) = at.filter(|at| *at > auth) {
            out.push('*');
            out.push('@');
            out.push_str(&input[at + 1..end]);
        } else {
            out.push_str(&input[auth..end]);
        }
        cursor = end;
    }
    out.push_str(&input[cursor..]);
    out
}

fn redact_assignments(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    while index < bytes.len() {
        if let Some(separator) = assignment_at(&lower, index) {
            let value_start = skip_ws(bytes, separator + 1);
            let value_end = if bytes[separator] == b':' { line_end(bytes, value_start) } else { assignment_value_end(bytes, value_start) };
            out.push_str(&input[index..value_start]);
            if value_end > value_start {
                out.push('*');
            }
            index = value_end;
            continue;
        }
        let ch = input[index..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        index += ch.len_utf8();
    }
    out
}

fn assignment_at(lower: &str, index: usize) -> Option<usize> {
    let bytes = lower.as_bytes();
    if index > 0 && is_ident(bytes[index - 1]) {
        return None;
    }
    for key in ASSIGN_KEYS {
        let key_bytes = key.as_bytes();
        if !bytes[index..].starts_with(key_bytes) {
            continue;
        }
        let after = index + key_bytes.len();
        if after < bytes.len() && is_ident(bytes[after]) {
            continue;
        }
        let mut cursor = after;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor < bytes.len() && (bytes[cursor] == b'=' || bytes[cursor] == b':') {
            return Some(cursor);
        }
    }
    None
}

fn assignment_value_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start;
    if index < bytes.len() && (bytes[index] == b'"' || bytes[index] == b'\'') {
        let quote = bytes[index];
        index += 1;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index < bytes.len() {
            index += 1;
        }
        return index;
    }
    while index < bytes.len() && !matches!(bytes[index], b' ' | b'\t' | b'\r' | b'\n' | b'&' | b'"' | b'\'' | b',' | b'<' | b'>') {
        index += 1;
    }
    index
}

fn line_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' && bytes[index] != b'\r' {
        index += 1;
    }
    index
}

const TOKEN_PREFIXES: &[&[u8]] = &[
    b"github_pat_",
    b"ghp_",
    b"gho_",
    b"ghu_",
    b"ghs_",
    b"ghr_",
    b"glpat-",
    b"glptt-",
    b"xoxb-",
    b"xoxp-",
    b"xoxa-",
    b"sk-",
    b"AKIA",
];

fn redact_prefixed_tokens(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    while index < bytes.len() {
        if let Some((prefix_len, total)) = token_at(bytes, index) {
            out.push_str(&input[index..index + prefix_len]);
            out.push('*');
            index += total;
            continue;
        }
        let ch = input[index..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        index += ch.len_utf8();
    }
    out
}

fn token_at(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    if index > 0 && is_ident(bytes[index - 1]) {
        return None;
    }
    for prefix in TOKEN_PREFIXES {
        if !bytes[index..].starts_with(prefix) {
            continue;
        }
        let mut end = index + prefix.len();
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_' || bytes[end] == b'-') {
            end += 1;
        }
        let body = end - (index + prefix.len());
        let min = if *prefix == b"AKIA" { 16 } else { 12 };
        if body >= min {
            return Some((prefix.len(), end - index));
        }
    }
    None
}

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn skip_ws(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn find_sub(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    bytes[from..].windows(needle.len()).position(|window| window == needle).map(|index| from + index)
}

#[cfg(test)]
mod tests {
    use super::{client, issue_log_name, issue_tail, perf_log_name, read_tail, redact, routes_to_issue, snapshot, ClientEntry};

    #[test]
    fn log_names_follow_the_build() {
        assert_eq!(perf_log_name(true), "awegit-debug.log");
        assert_eq!(perf_log_name(false), "awegit-release.log");
        assert_eq!(issue_log_name(true), "awegit-issue-debug.log");
        assert_eq!(issue_log_name(false), "awegit-issue.log");
    }

    #[test]
    fn issue_log_keeps_failures_and_the_summary() {
        assert!(routes_to_issue("WARN", "git", "push failed"));
        assert!(routes_to_issue("ERROR", "web", "invoke mutate"));
        assert!(routes_to_issue("SLOW", "cmd", "file_diff 400ms"));
        assert!(routes_to_issue("INFO", "sum", "git status count=1"));
        assert!(routes_to_issue("INFO", "app", "==== exit after 1.0s"));
        assert!(!routes_to_issue("INFO", "cmd", "workspace_status 4ms thread=worker"));
        assert!(!routes_to_issue("INFO", "app", "setup"));
    }

    #[test]
    fn issue_tail_starts_on_a_line() {
        let text = b"one\ntwo\nthree\nfour\n";
        assert_eq!(issue_tail(text, 12), b"three\nfour\n");
        assert_eq!(issue_tail(text, 100), text);
    }

    #[test]
    fn read_tail_drops_a_partial_first_line() {
        let dir = std::env::temp_dir().join(format!("awegit-log-tail-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("issue.log");
        std::fs::write(&path, "alpha\nbeta\ngamma\n").unwrap();
        let tail = read_tail(&path, 8);
        assert_eq!(tail, "gamma\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn redact_strips_url_userinfo_assignments_and_tokens() {
        assert_eq!(redact("workspace_status 4ms thread=worker"), "workspace_status 4ms thread=worker");
        assert_eq!(
            redact("fatal: unable to access 'https://github.com/a/b.git/': The requested URL returned error: 403"),
            "fatal: unable to access 'https://github.com/a/b.git/': The requested URL returned error: 403"
        );
        assert_eq!(
            redact("remote: Support for password authentication was removed. Please use a personal access token."),
            "remote: Support for password authentication was removed. Please use a personal access token."
        );
        assert_eq!(
            redact("https://user:ghp_0123456789abcdefghij@github.com/a/b.git"),
            "https://*@github.com/a/b.git"
        );
        assert_eq!(redact("https://github.com/a/b.git"), "https://github.com/a/b.git");
        assert_eq!(redact("token=ghp_0123456789abcdefghijklmnop&x=1"), "token=*&x=1");
        assert_eq!(redact("Private-Token: supersecretvalue"), "Private-Token: *");
        assert_eq!(redact("remote rejected ghp_0123456789abcdefghij"), "remote rejected ghp_*");
        assert_eq!(redact("Authorization: Bearer abcdefghijklmnop"), "Authorization: *");
    }

    #[test]
    fn invoke_errors_increment_failed() {
        let name = format!("diag-{}", std::process::id());
        client(vec![ClientEntry {
            level: "error".into(),
            kind: "invoke".into(),
            name: name.clone(),
            ms: Some(3.0),
            detail: Some("push failed".into()),
        }]);
        let snap = snapshot();
        let stat = snap.items.iter().find(|item| item.name == format!("cmd {name}")).expect("stat");
        assert_eq!(stat.failed, 1);
    }
}
