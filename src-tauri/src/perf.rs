//! Session log and timing for commands, `git` processes, and main-thread stalls.
//!
//! Lines go to `<settings dir>/logs/awegit-perf.log`. Arguments that may carry
//! paths, messages, or secrets are not written; only command names, flags, and times.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const SLOW_MS: u64 = 200;
const STALL_MS: u64 = 150;
const MAX_BYTES: u64 = 8 * 1024 * 1024;

static START: OnceLock<Instant> = OnceLock::new();
static MAIN: OnceLock<ThreadId> = OnceLock::new();
static FILE: Mutex<Option<File>> = Mutex::new(None);
static STATS: Mutex<BTreeMap<String, Stat>> = Mutex::new(BTreeMap::new());
static ON_MAIN: Mutex<Option<(String, Instant)>> = Mutex::new(None);
static STALLS: AtomicU64 = AtomicU64::new(0);
static STALL_MAX: AtomicU64 = AtomicU64::new(0);
static STALL_TOTAL: AtomicU64 = AtomicU64::new(0);

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
    dir().join("awegit-perf.log")
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
        let _ = std::fs::rename(&path, dir.join("awegit-perf.1.log"));
    }
    if let Ok(handle) = OpenOptions::new().create(true).append(true).open(&path) {
        if let Ok(mut slot) = FILE.lock() {
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
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("{now} +{:>9.3}s {level:<5} {target:<6} {message}\n", uptime().as_secs_f64());
    if cfg!(debug_assertions) {
        eprint!("{line}");
    }
    if let Ok(mut slot) = FILE.lock() {
        if let Some(handle) = slot.as_mut() {
            let _ = handle.write_all(line.as_bytes());
        }
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
pub fn git(args: &[&str], elapsed: Duration, ok: bool) {
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
    write(level, "git", &format!("{sub} {} {ms}ms{status}", flags.join(" ")));
}

pub fn client(entries: Vec<ClientEntry>) {
    for entry in entries.into_iter().take(200) {
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

pub fn snapshot() -> Snapshot {
    let mut items: Vec<Stat> = STATS.lock().map(|stats| stats.values().cloned().collect()).unwrap_or_default();
    items.sort_by(|a, b| b.total_ms.cmp(&a.total_ms));
    Snapshot {
        uptime_ms: uptime().as_millis() as u64,
        build: build(),
        log_file: file().display().to_string(),
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
