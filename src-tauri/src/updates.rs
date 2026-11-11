//! Looks at the same update document the current AweGit client uses.

use serde::Serialize;
use sha2::Digest;
use std::cmp::Ordering;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

/// Set by `cancel_update`; the download loop checks it between chunks.
static CANCEL: AtomicBool = AtomicBool::new(false);

const PROGRESS_BYTES_STEP: u64 = 1_048_576;

const VERSION_CHECK_URL: &str = "https://raw.githubusercontent.com/ZhouJun2303/Gitnuro/release/latest.json";
const RELEASE_PREFIX: &str = "https://github.com/ZhouJun2303/Gitnuro/releases/download/";
const RELEASES_PAGE: &str = "https://github.com/ZhouJun2303/Gitnuro/releases/latest";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNotice {
    pub app_version: String,
    pub app_code: i64,
    pub download_url: String,
    pub notes: String,
    /// True when `download_url` is an installer this machine can open and `sha256` is present.
    pub installable: bool,
    /// Lowercase SHA-256 of the installer. Empty when the manifest has no hash.
    pub sha256: String,
    /// The release page a person can open in a browser.
    pub release_url: String,
    /// Installer file name for the current system. Empty when the release has no installer.
    pub file_name: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    /// `connecting`, `downloading`, `verifying`, or `done`.
    pub phase: &'static str,
    pub percent: Option<u8>,
    pub received: u64,
    pub total: u64,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The GitHub releases page the update document points people at.
pub fn release_page() -> &'static str {
    RELEASES_PAGE
}

pub fn check() -> Result<Option<UpdateNotice>, String> {
    let text = http_get(VERSION_CHECK_URL)?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let app_version = value.get("appVersion").and_then(|item| item.as_str()).unwrap_or("").trim().to_string();
    if !newer_than(&app_version, current_version()) {
        return Ok(None);
    }
    let (download_url, sha256, installable) = platform_download(&value);
    let file_name = if installable {
        installer_name(&download_url).unwrap_or_default()
    } else {
        String::new()
    };
    Ok(Some(UpdateNotice {
        app_version,
        app_code: value.get("appCode").and_then(|item| item.as_i64()).unwrap_or(0),
        download_url,
        notes: value.get("notes").and_then(|item| item.as_str()).unwrap_or("").trim().to_string(),
        installable,
        sha256,
        release_url: RELEASES_PAGE.to_string(),
        file_name,
    }))
}

pub fn cancel() {
    CANCEL.store(true, AtomicOrdering::SeqCst);
}

pub fn download_and_open(url: &str, sha256: Option<&str>, silent: bool, mut progress: impl FnMut(UpdateProgress)) -> Result<(), String> {
    let expected = normalize_sha256(sha256).ok_or_else(|| crate::http::coded("update", "update hash is missing"))?;
    let name = installer_name(url)?;
    if !installer_kind(&name) {
        return Err(crate::http::coded("update", "update is not an installer for this system"));
    }
    CANCEL.store(false, AtomicOrdering::SeqCst);
    let dir = std::env::temp_dir().join("awegit-update");
    std::fs::create_dir_all(&dir).map_err(|error| crate::http::coded("update", &error.to_string()))?;
    let dest = dir.join(&name);
    let partial = dir.join(format!("{name}.partial"));
    progress(UpdateProgress { phase: "connecting", percent: None, received: 0, total: 0 });
    let response = match crate::http::agent(600, true)?.get(url).call() {
        Ok(response) => response,
        Err(error) => return Err(crate::http::coded("network", &error.to_string())),
    };
    let total = response.header("content-length").and_then(|value| value.parse::<u64>().ok()).unwrap_or(0);
    progress(UpdateProgress { phase: "downloading", percent: percent_of(0, total), received: 0, total });
    let mut reader = response.into_reader();
    let mut file = match std::fs::File::create(&partial) {
        Ok(file) => file,
        Err(error) => return Err(crate::http::coded("update", &error.to_string())),
    };
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut got = 0u64;
    let mut reported: Option<u8> = None;
    let mut emitted = 0u64;
    loop {
        if CANCEL.load(AtomicOrdering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&partial);
            return Err(crate::http::coded("cancelled", "update download cancelled"));
        }
        let read = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) => {
                drop(file);
                let _ = std::fs::remove_file(&partial);
                return Err(crate::http::coded("network", &error.to_string()));
            }
        };
        hasher.update(&buf[..read]);
        if let Err(error) = file.write_all(&buf[..read]) {
            drop(file);
            let _ = std::fs::remove_file(&partial);
            return Err(crate::http::coded("update", &error.to_string()));
        }
        got += read as u64;
        let percent = percent_of(got, total);
        if percent != reported || got - emitted >= PROGRESS_BYTES_STEP {
            reported = percent;
            emitted = got;
            progress(UpdateProgress { phase: "downloading", percent, received: got, total });
        }
    }
    drop(file);
    if got == 0 {
        let _ = std::fs::remove_file(&partial);
        return Err(crate::http::coded("network", "update download failed"));
    }
    progress(UpdateProgress { phase: "verifying", percent: Some(100), received: got, total: total.max(got) });
    if CANCEL.load(AtomicOrdering::Relaxed) {
        let _ = std::fs::remove_file(&partial);
        return Err(crate::http::coded("cancelled", "update download cancelled"));
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected {
        let _ = std::fs::remove_file(&partial);
        return Err(crate::http::coded("update", "update hash does not match"));
    }
    replace_download(&partial, &dest)?;
    progress(UpdateProgress { phase: "done", percent: Some(100), received: got, total: total.max(got) });
    open_installer(&dest, silent)
}

fn replace_download(from: &std::path::Path, to: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let ok = unsafe { MoveFileExW(from_wide.as_ptr(), to_wide.as_ptr(), 1 | 8) };
        if ok == 0 {
            let _ = std::fs::remove_file(from);
            return Err(crate::http::coded("update", &std::io::Error::last_os_error().to_string()));
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = std::fs::remove_file(to);
        std::fs::rename(from, to).map_err(|error| crate::http::coded("update", &error.to_string()))
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
}

/// Launches the downloaded installer. `silent` skips the installer window on
/// Windows; other systems keep their normal installer flow.
fn open_installer(dest: &std::path::Path, silent: bool) -> Result<(), String> {
    let name = dest.file_name().and_then(|name| name.to_str()).unwrap_or("");
    let lower = name.to_ascii_lowercase();
    #[cfg(windows)]
    let result = if lower.ends_with(".msi") {
        let mut command = std::process::Command::new("msiexec");
        command.arg("/i").arg(dest);
        if silent {
            command.args(silent_install_flags(&lower));
        }
        command.spawn()
    } else {
        let mut command = std::process::Command::new(dest);
        if silent {
            command.args(silent_install_flags(&lower));
        }
        command.spawn()
    };
    #[cfg(target_os = "macos")]
    let result = {
        let _ = silent;
        std::process::Command::new("open").arg(dest).spawn()
    };
    #[cfg(not(any(windows, target_os = "macos")))]
    let result = {
        let _ = silent;
        std::process::Command::new("xdg-open").arg(dest).spawn()
    };
    result.map(|_| ()).map_err(|error| crate::http::coded("update", &error.to_string()))
}

/// Tauri's NSIS installer accepts `/S` (silent), `/UPDATE` (skip the reinstall
/// choice), and `/R` (reopen the app afterwards). MSI uses its quiet switches.
#[cfg(windows)]
fn silent_install_flags(lower_name: &str) -> &'static [&'static str] {
    if lower_name.ends_with(".msi") {
        &["/quiet", "/norestart"]
    } else {
        &["/S", "/UPDATE", "/R"]
    }
}

fn percent_of(got: u64, total: u64) -> Option<u8> {
    if total == 0 {
        return None;
    }
    Some(((got.saturating_mul(100)) / total).min(100) as u8)
}

fn platform_download(value: &serde_json::Value) -> (String, String, bool) {
    let (windows, windows_sha) = asset_at(value, "windows");
    let (macos, macos_sha) = asset_at(value, "macos");
    let legacy = value.get("downloadUrl").and_then(|item| item.as_str()).unwrap_or("").trim().to_string();
    if cfg!(windows) {
        let (url, sha) = if windows.is_empty() { (legacy, String::new()) } else { (windows, windows_sha) };
        return (url.clone(), sha.clone(), installable(&url, &sha));
    }
    if cfg!(target_os = "macos") && !macos.is_empty() {
        return (macos.clone(), macos_sha.clone(), installable(&macos, &macos_sha));
    }
    let page = if legacy.starts_with("https://github.com/ZhouJun2303/Gitnuro/releases") && installer_name(&legacy).is_err() {
        legacy
    } else {
        RELEASES_PAGE.to_string()
    };
    (page, String::new(), false)
}

fn asset_at(value: &serde_json::Value, key: &str) -> (String, String) {
    let Some(item) = value.get("downloads").and_then(|item| item.get(key)) else {
        return (String::new(), String::new());
    };
    match item {
        serde_json::Value::String(text) => (text.trim().to_string(), String::new()),
        serde_json::Value::Object(_) => {
            let url = item.get("url").and_then(|entry| entry.as_str()).unwrap_or("").trim().to_string();
            let sha = item.get("sha256").and_then(|entry| entry.as_str()).unwrap_or("").trim().to_string();
            (url, sha)
        }
        _ => (String::new(), String::new()),
    }
}

fn installable(url: &str, sha256: &str) -> bool {
    let Ok(name) = installer_name(url) else { return false };
    installer_kind(&name) && normalize_sha256(Some(sha256)).is_some()
}

fn installer_kind(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if cfg!(windows) {
        return lower.ends_with(".exe") || lower.ends_with(".msi");
    }
    if cfg!(target_os = "macos") {
        return lower.ends_with(".dmg") || lower.ends_with(".pkg") || lower.ends_with(".zip");
    }
    false
}

pub(crate) fn normalize_sha256(text: Option<&str>) -> Option<String> {
    let text = text.unwrap_or("").trim();
    if text.len() == 64 && text.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(text.to_ascii_lowercase())
    } else {
        None
    }
}

fn installer_name(url: &str) -> Result<String, String> {
    if !url.starts_with(RELEASE_PREFIX) || url.contains("..") || url.chars().any(|ch| ch.is_control() || ch == '\\') {
        return Err("update download is not allowed".into());
    }
    let rest = &url[RELEASE_PREFIX.len()..];
    let name = rest.rsplit('/').next().unwrap_or("").split('?').next().unwrap_or("");
    if name.is_empty()
        || name.contains("..")
        || !name.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err("update download is not allowed".into());
    }
    Ok(name.to_string())
}

fn http_get(url: &str) -> Result<String, String> {
    crate::http::agent(20, true)?
        .get(url)
        .call()
        .map_err(|error| crate::http::coded("network", &error.to_string()))?
        .into_string()
        .map_err(|error| crate::http::coded("network", &error.to_string()))
}

/// `remote` is newer than the version compiled into this build.
///
/// Only `major.minor.patch` counts. `2.0` and `2.0-beta03` are not versions.
pub fn newer_than(remote: &str, current: &str) -> bool {
    let Some(remote) = parse_version(remote) else { return false };
    let Some(current) = parse_version(current) else { return false };
    remote.cmp(&current) == Ordering::Greater
}

/// Three numeric parts, such as `1.0.0`.
fn parse_version(text: &str) -> Option<[u64; 3]> {
    let mut parts = text.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some([major, minor, patch])
}

#[cfg(test)]
mod tests {
    use super::{asset_at, installer_name, normalize_sha256, newer_than, percent_of, platform_download};
    #[cfg(windows)]
    use super::silent_install_flags;

    #[test]
    fn three_part_versions_compare_by_number() {
        assert!(newer_than("1.0.1", "1.0.0"));
        assert!(newer_than("1.1.0", "1.0.9"));
        assert!(newer_than("2.0.0", "1.9.9"));
        assert!(!newer_than("1.0.0", "1.0.0"));
        assert!(!newer_than("1.0.0", "1.0.1"));
        assert!(!newer_than("2.0", "1.0.0"));
        assert!(!newer_than("2.0-beta03", "1.0.0"));
        assert!(!newer_than("v1.0.1", "1.0.0"));
    }

    #[test]
    fn installer_url_must_be_a_release_asset() {
        assert!(installer_name("https://github.com/ZhouJun2303/Gitnuro/releases/download/v2.0.1/AweGit_2.0.1_x64-setup.exe").is_ok());
        assert!(installer_name("https://github.com/ZhouJun2303/Gitnuro/releases/latest").is_err());
        assert!(installer_name("https://example.com/AweGit_2.0.1_x64-setup.exe").is_err());
        assert!(installer_name("https://github.com/ZhouJun2303/Gitnuro/releases/download/v2.0.1/../secret.exe").is_err());
    }

    #[test]
    fn a_release_asset_needs_a_sha256_before_it_can_install() {
        let exe = "https://github.com/ZhouJun2303/Gitnuro/releases/download/v2.0.2/AweGit_2.0.2_x64-setup.exe";
        let hash = "ab".repeat(32);
        let plain = serde_json::json!({ "downloads": { "windows": exe, "macos": "" } });
        let (url, sha) = asset_at(&plain, "windows");
        assert_eq!(url, exe);
        assert!(sha.is_empty());
        let hashed = serde_json::json!({ "downloads": { "windows": { "url": exe, "sha256": hash }, "macos": "" } });
        let (url, sha) = asset_at(&hashed, "windows");
        assert_eq!(url, exe);
        assert_eq!(sha, hash);
        let (_, _, installable) = platform_download(&plain);
        assert!(!installable);
        if cfg!(windows) {
            let (_, _, installable) = platform_download(&hashed);
            assert!(installable);
        }
        assert!(normalize_sha256(Some(&hash.to_uppercase())).is_some());
        assert!(normalize_sha256(Some("short")).is_none());
        assert!(normalize_sha256(None).is_none());
    }

    #[test]
    fn download_percent_follows_bytes_received() {
        assert_eq!(percent_of(0, 0), None);
        assert_eq!(percent_of(50, 0), None);
        assert_eq!(percent_of(0, 200), Some(0));
        assert_eq!(percent_of(50, 200), Some(25));
        assert_eq!(percent_of(200, 200), Some(100));
        assert_eq!(percent_of(250, 200), Some(100));
    }

    #[cfg(windows)]
    #[test]
    fn silent_install_flags_match_the_installer_kind() {
        assert_eq!(silent_install_flags("awegit_2.0.6_x64-setup.exe"), &["/S", "/UPDATE", "/R"]);
        assert_eq!(silent_install_flags("awegit_2.0.6_x64_en-us.msi"), &["/quiet", "/norestart"]);
    }
}
