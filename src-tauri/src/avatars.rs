//! Account avatar cache.
//!
//! A forge profile image is downloaded at most once. The bytes sit next to the
//! settings file, and the webview receives them as a data URL, so reopening the
//! accounts page or a remote dialog never waits on the network. A failed fetch
//! falls back to a stale copy instead of leaving the panel blank.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::http;

const MAX_BYTES: u64 = 512 * 1024;
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const MEMORY_LIMIT: usize = 64;
/// MIME type and file extension for every shape the cache keeps.
static KINDS: [(&str, &str); 4] = [
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/webp", "webp"),
    ("image/gif", "gif"),
];

fn dir() -> PathBuf {
    crate::settings::path()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("avatars")))
        .unwrap_or_else(|| std::env::temp_dir().join("AweGit").join("avatars"))
}

fn memory() -> &'static Mutex<HashMap<String, String>> {
    static MEMORY: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    MEMORY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Data URL for an account avatar, served from disk unless it is missing or stale.
pub fn image(url: &str) -> Result<String, String> {
    let url = url.trim();
    if !safe_url(url) {
        return Err(http::coded("avatar", "invalid image url"));
    }
    if let Some(hit) = memory().lock().ok().and_then(|cache| cache.get(url).cloned()) {
        return Ok(hit);
    }
    let loaded = image_at(&dir(), url, |target| {
        let response = http::send_bytes(target, MAX_BYTES)?;
        Ok((response.content_type, response.body))
    })?;
    if let Ok(mut cache) = memory().lock() {
        if cache.len() >= MEMORY_LIMIT {
            cache.clear();
        }
        cache.insert(url.to_string(), loaded.clone());
    }
    Ok(loaded)
}

fn image_at(dir: &Path, url: &str, fetch: impl FnOnce(&str) -> Result<(String, Vec<u8>), String>) -> Result<String, String> {
    let key = key_of(url);
    let cached = find_cached(dir, &key);
    if let Some((path, mime)) = &cached {
        if fresh(path) {
            if let Some(source) = read_image(path, mime) {
                return Ok(source);
            }
        }
    }
    let fallback = cached.as_ref().and_then(|(path, mime)| read_image(path, mime));
    match fetch(url) {
        Ok((mime, body)) => {
            let Some((image_mime, ext)) = kind(&mime, &body) else {
                return fallback.ok_or_else(|| http::coded("avatar", "unsupported image type"));
            };
            // A cache that cannot be written still displays; the next view fetches again.
            let _ = write_cached(dir, &key, ext, &body);
            Ok(data_url(image_mime, &body))
        }
        Err(error) => fallback.ok_or(error),
    }
}

fn read_image(path: &Path, mime: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    (!bytes.is_empty()).then(|| data_url(mime, &bytes))
}

fn fresh(path: &Path) -> bool {
    let Ok(modified) = std::fs::metadata(path).and_then(|meta| meta.modified()) else {
        return false;
    };
    modified.elapsed().map(|age| age < MAX_AGE).unwrap_or(true)
}

fn find_cached(dir: &Path, key: &str) -> Option<(PathBuf, &'static str)> {
    KINDS.iter().find_map(|(mime, ext)| {
        let path = dir.join(format!("{key}.{ext}"));
        path.is_file().then_some((path, *mime))
    })
}

fn write_cached(dir: &Path, key: &str, ext: &str, bytes: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    for (_, other) in KINDS {
        if other != ext {
            let _ = std::fs::remove_file(dir.join(format!("{key}.{other}")));
        }
    }
    let path = dir.join(format!("{key}.{ext}"));
    let temp = dir.join(format!("{key}.{ext}.{}.tmp", std::process::id()));
    std::fs::write(&temp, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&temp, &path).map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        error.to_string()
    })
}

fn kind(mime: &str, bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    match mime {
        "image/png" => Some(("image/png", "png")),
        "image/jpeg" => Some(("image/jpeg", "jpg")),
        "image/webp" => Some(("image/webp", "webp")),
        "image/gif" => Some(("image/gif", "gif")),
        _ => sniff(bytes),
    }
}

fn sniff(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("image/jpeg", "jpg"))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(("image/gif", "gif"))
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else {
        None
    }
}

/// Only anonymous HTTPS images with a public-looking host are fetched, so a
/// profile response cannot point the client at loopback or an intranet name.
fn safe_url(url: &str) -> bool {
    if url.len() < 12 || url.len() > 2048 || url.contains(['\n', '\r', '\0', ' ']) {
        return false;
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority.rsplit_once('@').map(|(_, tail)| tail).unwrap_or(authority);
    let host = host_port.split(':').next().unwrap_or("").trim_end_matches('.');
    if !host.contains('.') || host.ends_with(".local") || host.ends_with(".localhost") {
        return false;
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return false;
    }
    crate::accounts::host_safe(host)
}

fn key_of(url: &str) -> String {
    let mut key = String::with_capacity(64);
    for byte in Sha256::digest(url.as_bytes()) {
        use std::fmt::Write as _;
        let _ = write!(key, "{byte:02x}");
    }
    key
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64(bytes))
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(TABLE[(value >> 18) as usize & 63] as char);
        out.push(TABLE[(value >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(value >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[value as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake";

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("awegit-avatar-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn avatars_only_load_from_public_https_hosts() {
        assert!(safe_url("https://avatars.githubusercontent.com/u/1?v=4"));
        assert!(safe_url("https://gitlab.com/uploads/-/system/user/avatar/1/avatar.png"));
        assert!(!safe_url("http://avatars.githubusercontent.com/u/1"));
        assert!(!safe_url("https://127.0.0.1/avatar.png"));
        assert!(!safe_url("https://localhost/avatar.png"));
        assert!(!safe_url("https://build.local/avatar.png"));
        assert!(!safe_url("https://example.com/a b.png"));
    }

    #[test]
    fn a_fetched_avatar_is_served_from_disk_next_time() {
        let dir = temp_dir("reuse");
        let first = image_at(&dir, "https://avatars.example.com/u/1", |_| {
            Ok(("image/png".into(), PNG.to_vec()))
        })
        .unwrap();
        assert!(first.starts_with("data:image/png;base64,"), "{first}");
        let again = image_at(&dir, "https://avatars.example.com/u/1", |_| {
            panic!("cache was not reused")
        })
        .unwrap();
        assert_eq!(first, again);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stale_avatar_survives_a_failed_refresh() {
        let dir = temp_dir("stale");
        let url = "https://avatars.example.com/u/2";
        image_at(&dir, url, |_| Ok(("image/png".into(), PNG.to_vec()))).unwrap();
        let path = find_cached(&dir, &key_of(url)).unwrap().0;
        let old = SystemTime::now() - MAX_AGE - Duration::from_secs(60);
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_times(std::fs::FileTimes::new().set_modified(old)).unwrap();
        let served = image_at(&dir, url, |_| Err(http::coded("network", "offline"))).unwrap();
        assert!(served.starts_with("data:image/png;base64,"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_non_image_response_is_not_cached() {
        let dir = temp_dir("html");
        let error = image_at(&dir, "https://avatars.example.com/u/3", |_| {
            Ok(("text/html".into(), b"<html>nope</html>".to_vec()))
        })
        .unwrap_err();
        assert!(error.contains("unsupported"), "{error}");
        assert!(!dir.join(format!("{}.png", key_of("https://avatars.example.com/u/3"))).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn base64_matches_the_standard_alphabet() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
