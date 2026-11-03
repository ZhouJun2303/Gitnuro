//! Looks at the same update document the current AweGit client uses.

use serde::Serialize;
use std::cmp::Ordering;

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
    /// True when `download_url` is an installer this machine can open.
    pub installable: bool,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn check() -> Result<Option<UpdateNotice>, String> {
    let text = http_get(VERSION_CHECK_URL)?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let app_version = value.get("appVersion").and_then(|item| item.as_str()).unwrap_or("").trim().to_string();
    if !newer_than(&app_version, current_version()) {
        return Ok(None);
    }
    let (download_url, installable) = platform_download(&value);
    Ok(Some(UpdateNotice {
        app_version,
        app_code: value.get("appCode").and_then(|item| item.as_i64()).unwrap_or(0),
        download_url,
        notes: value.get("notes").and_then(|item| item.as_str()).unwrap_or("").trim().to_string(),
        installable,
    }))
}

pub fn download_and_open(url: &str) -> Result<(), String> {
    let name = installer_name(url)?;
    let dir = std::env::temp_dir().join("awegit-update");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let dest = dir.join(name);
    let status = std::process::Command::new("curl")
        .args(["--silent", "--show-error", "--fail", "--location", "--max-time", "180", "--output"])
        .arg(&dest)
        .arg(url)
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        let _ = std::fs::remove_file(&dest);
        return Err("update download failed".into());
    }
    std::process::Command::new(&dest).spawn().map_err(|error| error.to_string())?;
    Ok(())
}

fn platform_download(value: &serde_json::Value) -> (String, bool) {
    let downloads = value.get("downloads");
    let windows = text_at(downloads, "windows");
    let macos = text_at(downloads, "macos");
    let legacy = value.get("downloadUrl").and_then(|item| item.as_str()).unwrap_or("").trim().to_string();
    if cfg!(windows) {
        let url = if windows.is_empty() { legacy } else { windows };
        let installable = installer_name(&url).is_ok();
        return (url, installable);
    }
    if cfg!(target_os = "macos") && !macos.is_empty() {
        let installable = installer_name(&macos).is_ok();
        return (macos, installable);
    }
    let page = if legacy.starts_with("https://github.com/ZhouJun2303/Gitnuro/releases") && installer_name(&legacy).is_err() {
        legacy
    } else {
        RELEASES_PAGE.to_string()
    };
    (page, false)
}

fn text_at(value: Option<&serde_json::Value>, key: &str) -> String {
    value.and_then(|item| item.get(key)).and_then(|item| item.as_str()).unwrap_or("").trim().to_string()
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
    let output = std::process::Command::new("curl")
        .args(["--silent", "--show-error", "--fail", "--max-time", "20", url])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() { "update check failed".into() } else { stderr });
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
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
    use super::{installer_name, newer_than};

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
}
