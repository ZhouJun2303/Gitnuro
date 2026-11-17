//! Optional Google Fonts. Font files stay in the user's cache, outside the bundle.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::http;

const MAX_BYTES: u64 = 32 * 1024 * 1024;
const MAX_METADATA: u64 = 64 * 1024;
const MAX_REGISTRY: u64 = 256 * 1024;
static DOWNLOAD: Mutex<()> = Mutex::new(());
static PROGRESS: Mutex<Option<FontProgress>> = Mutex::new(None);

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontProgress {
    request_id: String,
    id: String,
    phase: &'static str,
    received: u64,
    total: u64,
    percent: Option<u8>,
}

pub fn progress(request_id: &str) -> Option<FontProgress> {
    PROGRESS.lock().ok()?.as_ref().filter(|value| value.request_id == request_id).cloned()
}

fn begin_progress(request_id: &str, id: &str) -> Result<(), String> {
    if request_id.is_empty() || request_id.len() > 120 { return Err(http::coded("font", "invalid font download request")); }
    if let Ok(mut value) = PROGRESS.lock() {
        *value = Some(FontProgress { request_id: request_id.into(), id: id.into(), phase: "resolving", received: 0, total: 0, percent: None });
    }
    Ok(())
}

fn report_progress(phase: &'static str, bytes: Option<(u64, u64)>) {
    if let Ok(mut value) = PROGRESS.lock() {
        if let Some(value) = value.as_mut() {
            value.phase = phase;
            if let Some((received, total)) = bytes {
                value.received = received;
                value.total = total;
                value.percent = (total > 0).then(|| (received.saturating_mul(100) / total).min(100) as u8);
            }
            if phase == "ready" { value.percent = Some(100); }
        }
    }
}

#[derive(serde::Deserialize)]
struct FontSource {
    id: String,
    family: String,
    filename: Option<String>,
}

fn catalog() -> &'static [FontSource] {
    static CATALOG: OnceLock<Vec<FontSource>> = OnceLock::new();
    CATALOG.get_or_init(|| serde_json::from_str(include_str!("../../src/lib/font-catalog.json")).expect("bundled font catalog"))
}

fn source(id: &str) -> Option<&'static FontSource> {
    catalog().iter().find(|font| font.id == id)
}

fn field<'a>(record: &'a str, name: &str) -> Option<&'a str> {
    record.lines().find_map(|line| line.trim().strip_prefix(name).map(|value| value.trim().trim_matches('"')))
}

fn metadata_filename(metadata: &str) -> Option<String> {
    metadata.split("fonts {").skip(1).filter_map(|record| {
        let record = record.split('}').next()?;
        if field(record, "style:")? != "normal" { return None; }
        let file = field(record, "filename:")?;
        if !file.ends_with(".ttf") || file.len() > 160 || file.contains("..")
            || !file.bytes().all(|ch| ch.is_ascii_alphanumeric() || b"-_.,[]".contains(&ch)) { return None; }
        let variable = file.contains('[');
        (variable || field(record, "weight:") == Some("400")).then_some((variable, file.to_string()))
    }).max_by_key(|(variable, _)| *variable).map(|(_, file)| file)
}

fn read_metadata(url: &str) -> Result<String, String> {
    use std::io::Read;
    let response = http::download(url, 30)?;
    let mut metadata = String::new();
    response.reader.take(MAX_METADATA + 1).read_to_string(&mut metadata).map_err(|error| http::coded("network", &error.to_string()))?;
    if metadata.len() as u64 > MAX_METADATA { return Err(http::coded("font", "font metadata is too large")); }
    Ok(metadata)
}

fn file_url(folder: &str, family: &str, file: &str) -> String {
    format!("https://raw.githubusercontent.com/google/fonts/main/{folder}/{family}/{}", file.replace('[', "%5B").replace(']', "%5D"))
}

fn font_url(font: &FontSource) -> Result<String, String> {
    let file = if let Some(file) = &font.filename { file.clone() } else {
        // Resolve the current regular face only after the user clicks Download.
        let metadata = read_metadata(&file_url("ofl", &font.family, "METADATA.pb"))?;
        metadata_filename(&metadata).ok_or_else(|| http::coded("font", "no regular font file in the official catalog"))?
    };
    Ok(file_url("ofl", &font.family, &file))
}

fn fetch_font(url: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    report_progress("connecting", None);
    let response = http::download(url, 60)?;
    if response.total > MAX_BYTES { return Err(http::coded("font", "font exceeds 32 MB")); }
    let total = response.total;
    let mut reader = response.reader.take(MAX_BYTES + 1);
    let mut bytes = Vec::new();
    let mut chunk = [0; 64 * 1024];
    report_progress("downloading", Some((0, total)));
    loop {
        let count = reader.read(&mut chunk).map_err(|error| http::coded("network", &error.to_string()))?;
        if count == 0 { break; }
        bytes.extend_from_slice(&chunk[..count]);
        report_progress("downloading", Some((bytes.len() as u64, total)));
    }
    if total > 0 && bytes.len() as u64 != total { return Err(http::coded("network", "incomplete font download")); }
    Ok(bytes)
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct CustomFont {
    id: String,
    name: String,
    bytes: u64,
    variable: bool,
}

fn family_key(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 100 || !name.bytes().all(|ch| ch.is_ascii_alphanumeric() || b" -_+".contains(&ch)) {
        return Err(http::coded("font", "enter the family name shown on Google Fonts, for example Noto Sans SC"));
    }
    let key: String = name.chars().filter(char::is_ascii_alphanumeric).map(|ch| ch.to_ascii_lowercase()).collect();
    if key.is_empty() { return Err(http::coded("font", "font family name is empty")); }
    Ok(key)
}

fn custom_at(dir: &Path) -> Result<Vec<CustomFont>, String> {
    use std::io::Read;
    let path = dir.join("google-fonts.json");
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(http::coded("font", &error.to_string())),
    };
    let mut text = String::new();
    file.take(MAX_REGISTRY + 1).read_to_string(&mut text).map_err(|error| http::coded("font", &error.to_string()))?;
    if text.len() as u64 > MAX_REGISTRY { return Err(http::coded("font", "font registry is too large")); }
    let fonts: Vec<CustomFont> = serde_json::from_str(&text).map_err(|error| http::coded("font", &error.to_string()))?;
    for font in &fonts {
        if font.id != format!("google-{}", family_key(&font.name)?) || font.bytes > MAX_BYTES {
            return Err(http::coded("font", "invalid font registry"));
        }
    }
    Ok(fonts)
}

fn resolve_family(name: &str, mut metadata: impl FnMut(&str) -> Result<String, String>) -> Result<(String, String, bool), String> {
    let key = family_key(name)?;
    for folder in ["ofl", "apache", "ufl"] {
        let text = match metadata(&file_url(folder, &key, "METADATA.pb")) {
            Ok(text) => text,
            Err(error) if error.contains("HTTP 404") || error.contains("status code 404") => continue,
            Err(error) => return Err(error),
        };
        if text.len() as u64 > MAX_METADATA { return Err(http::coded("font", "font metadata is too large")); }
        let official = field(&text, "name:").ok_or_else(|| http::coded("font", "font name missing from official catalog"))?;
        if family_key(official)? != key { return Err(http::coded("font", "font name does not match the official catalog")); }
        let file = metadata_filename(&text).ok_or_else(|| http::coded("font", "no regular font file in the official catalog"))?;
        return Ok((official.to_string(), file_url(folder, &key, &file), file.contains("wght")));
    }
    Err(http::coded("font", "font family not found; copy the family name from fonts.google.com"))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDownload {
    font: CustomFont,
    data_url: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontMatch {
    id: String,
    name: String,
    bytes: Option<u64>,
    cached: bool,
}

pub fn recognize_google(name: &str) -> Result<FontMatch, String> {
    let id = format!("google-{}", family_key(name)?);
    let dir = cache_dir()?;
    if let Some(font) = custom_at(&dir)?.into_iter().find(|font| font.id == id) {
        if let Some(bytes) = cached_size(&dir.join(format!("{id}.ttf"))) {
            return Ok(FontMatch { id, name: font.name, bytes: Some(bytes), cached: true });
        }
    }
    let (name, _, _) = resolve_family(name, read_metadata)?;
    Ok(FontMatch { id, name, bytes: None, cached: false })
}

pub fn download_google(name: &str, request_id: &str) -> Result<GoogleDownload, String> {
    let _download = DOWNLOAD.lock().map_err(|_| http::coded("font", "font download unavailable"))?;
    begin_progress(request_id, &format!("google-{}", family_key(name)?))?;
    let result = cache_dir().and_then(|dir| download_google_at(&dir, name, read_metadata, fetch_font));
    report_progress(if result.is_ok() { "ready" } else { "error" }, result.as_ref().ok().map(|value| (value.font.bytes, value.font.bytes)));
    result
}

fn download_google_at(dir: &Path, name: &str, metadata: impl FnMut(&str) -> Result<String, String>, fetch: impl FnOnce(&str) -> Result<Vec<u8>, String>) -> Result<GoogleDownload, String> {
    let id = format!("google-{}", family_key(name)?);
    let mut fonts = custom_at(dir)?;
    if let Some(font) = fonts.iter().find(|font| font.id == id) {
        if let Some(data_url) = load_at(dir, &id, false, || unreachable!())? { return Ok(GoogleDownload { font: font.clone(), data_url }); }
    }
    let (name, url, variable) = resolve_family(name, metadata)?;
    let data_url = load_at(dir, &id, true, || fetch(&url))?.ok_or_else(|| http::coded("font", "font download was empty"))?;
    let bytes = std::fs::metadata(dir.join(format!("{id}.ttf"))).map_err(|error| http::coded("font", &error.to_string()))?.len();
    let font = CustomFont { id, name, bytes, variable };
    fonts.retain(|entry| entry.id != font.id);
    fonts.push(font.clone());
    let json = serde_json::to_vec(&fonts).map_err(|error| http::coded("font", &error.to_string()))?;
    if json.len() as u64 > MAX_REGISTRY { return Err(http::coded("font", "font registry is too large")); }
    let partial = dir.join(format!("google-fonts.{}.partial", std::process::id()));
    std::fs::write(&partial, json).and_then(|_| std::fs::rename(&partial, dir.join("google-fonts.json"))).map_err(|error| http::coded("font", &error.to_string()))?;
    Ok(GoogleDownload { font, data_url })
}

pub fn cache_dir() -> Result<PathBuf, String> {
    let settings = crate::settings::path()?;
    Ok(settings.parent().ok_or_else(|| http::coded("font", "font cache unavailable"))?.join("fonts"))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    directory: String,
    fonts: Vec<CachedFont>,
    custom_fonts: Vec<CustomFont>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CachedFont {
    id: String,
    path: String,
    bytes: Option<u64>,
}

pub fn cache_info() -> Result<CacheInfo, String> {
    info_at(&cache_dir()?)
}

fn info_at(dir: &Path) -> Result<CacheInfo, String> {
    let custom_fonts = custom_at(dir)?;
    let fonts = catalog().iter().map(|font| font.id.clone()).chain(custom_fonts.iter().map(|font| font.id.clone())).map(|id| {
        let path = dir.join(format!("{id}.ttf"));
        // Inspect only a fixed-size header. Opening preferences never decodes every cached font.
        let bytes = cached_size(&path);
        CachedFont { id, path: path.to_string_lossy().into_owned(), bytes }
    }).collect();
    Ok(CacheInfo { directory: dir.to_string_lossy().into_owned(), fonts, custom_fonts })
}

fn cached_size(path: &Path) -> Option<u64> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if len > MAX_BYTES { return None; }
    let mut header = [0; 12];
    file.read_exact(&mut header).ok()?;
    valid(&header).then_some(len)
}

pub fn data(id: &str, download: bool, request_id: &str) -> Result<Option<String>, String> {
    let _download = if download { Some(DOWNLOAD.lock().map_err(|_| http::coded("font", "font download unavailable"))?) } else { None };
    if download { begin_progress(request_id, id)?; }
    let result = read_data(id, download);
    if download {
        let bytes = result.as_ref().ok().and_then(|value| value.as_ref()).and_then(|_| cache_dir().ok()).and_then(|dir| cached_size(&dir.join(format!("{id}.ttf"))));
        report_progress(if result.is_ok() { "ready" } else { "error" }, bytes.map(|bytes| (bytes, bytes)));
    }
    result
}

fn read_data(id: &str, download: bool) -> Result<Option<String>, String> {
    let dir = cache_dir()?;
    if let Some(source) = source(id) {
        return load_at(&dir, id, download, || fetch_font(&font_url(source)?));
    }
    let font = custom_at(&dir)?.into_iter().find(|font| font.id == id).ok_or_else(|| http::coded("font", "unknown font"))?;
    load_at(&dir, id, download, || fetch_font(&resolve_family(&font.name, read_metadata)?.1))
}

fn valid(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && bytes.len() as u64 <= MAX_BYTES
        && (bytes.starts_with(&[0, 1, 0, 0]) || bytes.starts_with(b"OTTO"))
}

fn load_at(dir: &Path, id: &str, download: bool, fetch: impl FnOnce() -> Result<Vec<u8>, String>) -> Result<Option<String>, String> {
    let path = dir.join(format!("{id}.ttf"));
    if cached_size(&path).is_none() {
        if !download { return Ok(None); }
        let bytes = fetch()?;
        if !valid(&bytes) { return Err(http::coded("font", "invalid font file or font exceeds 32 MB")); }
        std::fs::create_dir_all(dir).map_err(|error| http::coded("font", &error.to_string()))?;
        let partial = dir.join(format!("{id}.{}.partial", std::process::id()));
        std::fs::write(&partial, &bytes).map_err(|error| http::coded("font", &error.to_string()))?;
        // The complete file is published only after its header and size pass validation.
        if let Err(error) = std::fs::rename(&partial, &path) {
            let _ = std::fs::remove_file(&partial);
            return Err(http::coded("font", &error.to_string()));
        }
    }
    // Send a short cache reference through IPC; the webview reads binary font bytes directly.
    Ok(Some(format!("awegit-font:{id}")))
}

fn cached_bytes_at(dir: &Path, id: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    // Validate before joining a path, including names supplied through the URI protocol.
    if id.len() > 110 || !id.bytes().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == b'-') {
        return Err(http::coded("font", "unknown font"));
    }
    if source(id).is_none() && !custom_at(dir)?.iter().any(|font| font.id == id) {
        return Err(http::coded("font", "unknown font"));
    }
    let file = std::fs::File::open(dir.join(format!("{id}.ttf"))).map_err(|error| http::coded("font", &error.to_string()))?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|error| http::coded("font", &error.to_string()))?;
    if !valid(&bytes) { return Err(http::coded("font", "invalid cached font")); }
    Ok(bytes)
}

pub fn cached_bytes(id: &str) -> Result<Vec<u8>, String> {
    cached_bytes_at(&cache_dir()?, id)
}

pub fn protocol_response(request: tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    let origin = request.headers().get("Origin").and_then(|value| value.to_str().ok()).unwrap_or("");
    let allowed = matches!(origin, "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost")
        || (cfg!(debug_assertions) && matches!(origin, "http://localhost:1420" | "http://127.0.0.1:1420"));
    let mut response = tauri::http::Response::builder()
        .header("Content-Type", "font/ttf")
        .header("X-Content-Type-Options", "nosniff")
        .header("Cache-Control", "no-store");
    if allowed { response = response.header("Access-Control-Allow-Origin", origin); }
    if (!origin.is_empty() && !allowed) || request.method() != tauri::http::Method::GET {
        return response.status(403).body(Vec::new()).unwrap();
    }
    match cached_bytes(request.uri().path().strip_prefix('/').unwrap_or("")) {
        Ok(bytes) => response.body(bytes).unwrap(),
        Err(_) => response.status(404).body(Vec::new()).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires access to the official Google Fonts repository"]
    fn google_font_sources_stream_valid_font_files() {
        use std::io::Read;
        for font in catalog() {
            let id = &font.id;
            let url = font_url(font).unwrap();
            let mut response = http::download(&url, 30).unwrap_or_else(|error| panic!("{id}: {error}"));
            assert!(response.total <= MAX_BYTES, "{id} exceeds the font size limit");
            let mut header = [0; 12];
            response.reader.read_exact(&mut header).unwrap();
            assert!(valid(&header), "{id} is not a font");
            println!("{id}: {} bytes", response.total);
        }
    }

    #[test]
    fn optional_fonts_only_download_on_request_and_reuse_the_cache() {
        let dir = std::env::temp_dir().join(format!("awegit-font-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(load_at(&dir, "inter", false, || panic!("offline load must not fetch")).unwrap(), None);
        let bytes = vec![0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        let first = load_at(&dir, "inter", true, || Ok(bytes.clone())).unwrap();
        assert_eq!(first.as_deref(), Some("awegit-font:inter"));
        assert_eq!(cached_bytes_at(&dir, "inter").unwrap(), bytes);
        for id in ["../settings", "%2e%2e/settings", "google-missing", "inter.ttf", "INTER"] {
            assert!(cached_bytes_at(&dir, id).is_err(), "{id}");
        }
        assert_eq!(load_at(&dir, "inter", true, || panic!("cached fonts must not fetch")).unwrap(), first);
        assert!(load_at(&dir, "bad", true, || Ok(b"<html>not a font</html>".to_vec())).is_err());
        assert!(!dir.join("bad.ttf").exists());
        let info = info_at(&dir).unwrap();
        assert_eq!(info.fonts.len(), catalog().len());
        let inter = info.fonts.iter().find(|font| font.id == "inter").unwrap();
        assert_eq!(inter.bytes, Some(bytes.len() as u64));
        assert_eq!(info.fonts[0].bytes, None);
        assert_eq!(Path::new(&inter.path), dir.join("inter.ttf"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn font_sources_and_sizes_are_bounded() {
        let ids: std::collections::HashSet<_> = catalog().iter().map(|font| &font.id).collect();
        assert_eq!(ids.len(), catalog().len());
        assert!(catalog().len() >= 17);
        assert!(catalog().iter().all(|font| font.family.bytes().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())));
        assert!(source("../settings").is_none());
        assert!(source("https://example.com/font.ttf").is_none());
        assert!(!valid(&[]));
        let mut bytes = vec![0; MAX_BYTES as usize + 1];
        bytes[1] = 1;
        assert!(!valid(&bytes));
    }

    #[test]
    fn official_metadata_selects_regular_fonts_and_rejects_paths() {
        let metadata = r#"name: "Font"
fonts {
  style: "normal"
  weight: 100
  filename: "Font-Thin.ttf"
}
fonts {
  style: "italic"
  weight: 400
  filename: "Font-Italic[wght].ttf"
}
fonts {
  style: "normal"
  weight: 400
  filename: "Font-Regular.ttf"
}
fonts {
  style: "normal"
  weight: 400
  filename: "Font[wdth,wght].ttf"
}"#;
        assert_eq!(metadata_filename(metadata).as_deref(), Some("Font[wdth,wght].ttf"));
        assert_eq!(metadata_filename(&metadata[..metadata.rfind("fonts {").unwrap()]).as_deref(), Some("Font-Regular.ttf"));
        for file in ["../outside.ttf", "https://host/font.ttf", "font.otf", "x\\font.ttf"] {
            assert!(metadata_filename(&format!("fonts {{\nstyle: \"normal\"\nweight: 400\nfilename: \"{file}\"\n}}")).is_none());
        }
    }

    #[test]
    fn google_family_downloads_are_registered_and_reopen_without_network() {
        let dir = std::env::temp_dir().join(format!("awegit-google-font-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bytes = vec![0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        let mut lookups = Vec::new();
        let result = download_google_at(&dir, "Example Sans", |url| {
            lookups.push(url.to_string());
            if url.contains("/ofl/") { return Err(http::coded("network", "HTTP 404")); }
            Ok("name: \"Example Sans\"\nfonts {\nstyle: \"normal\"\nweight: 400\nfilename: \"ExampleSans[wght].ttf\"\n}".into())
        }, |url| {
            assert_eq!(url, "https://raw.githubusercontent.com/google/fonts/main/apache/examplesans/ExampleSans%5Bwght%5D.ttf");
            Ok(bytes.clone())
        }).unwrap();
        assert_eq!(lookups.len(), 2);
        assert_eq!(result.font.id, "google-examplesans");
        assert_eq!(result.font.name, "Example Sans");
        assert!(result.font.variable);
        assert_eq!(result.font.bytes, bytes.len() as u64);
        let reopened = download_google_at(&dir, "Example+Sans", |_| panic!("cached font must not read metadata"), |_| panic!("cached font must not download")).unwrap();
        assert_eq!(reopened.data_url, result.data_url);
        let info = info_at(&dir).unwrap();
        assert_eq!(info.custom_fonts.len(), 1);
        assert!(info.fonts.iter().any(|font| font.id == "google-examplesans" && font.bytes == Some(bytes.len() as u64)));
        assert_eq!(std::fs::read(dir.join("google-examplesans.ttf")).unwrap(), bytes);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn custom_font_names_cannot_select_paths_or_unrelated_hosts() {
        for name in ["", "../settings", "https://example.com/font", "file.ttf", "name\nheader", "-_"] {
            assert!(family_key(name).is_err(), "{name}");
            assert!(resolve_family(name, |_| panic!("invalid input must not reach the network")).is_err());
        }
        assert_eq!(family_key("M PLUS 1p").unwrap(), "mplus1p");
        assert!(resolve_family("Example Sans", |_| Ok("name: \"Other Font\"".into())).is_err());
        assert!(resolve_family("Example Sans", |_| Ok("x".repeat(MAX_METADATA as usize + 1))).is_err());
        assert!(resolve_family("Example Sans", |_| Err(http::coded("network", "HTTP 404"))).unwrap_err().contains("font family not found"));
    }

    #[test]
    fn download_progress_is_bounded_and_isolated_by_request() {
        let _download = DOWNLOAD.lock().unwrap();
        assert!(begin_progress("", "inter").is_err());
        begin_progress("test-first", "inter").unwrap();
        assert!(progress("test-other").is_none());
        report_progress("downloading", Some((1024, 2048)));
        let value = progress("test-first").unwrap();
        assert_eq!(value.percent, Some(50));
        assert_eq!(value.received, 1024);
        report_progress("downloading", Some((4096, 0)));
        assert_eq!(progress("test-first").unwrap().percent, None);
        report_progress("ready", Some((4096, 4096)));
        assert_eq!(progress("test-first").unwrap().percent, Some(100));
        begin_progress("test-second", "jetbrains-mono").unwrap();
        assert!(progress("test-first").is_none());
        report_progress("error", None);
        assert_eq!(progress("test-second").unwrap().phase, "error");
    }

    #[test]
    fn font_protocol_rejects_foreign_origins_and_write_methods() {
        for (method, origin) in [("GET", "https://example.com"), ("POST", "tauri://localhost")] {
            let request = tauri::http::Request::builder().method(method).uri("awegit-font://localhost/inter").header("Origin", origin).body(Vec::new()).unwrap();
            assert_eq!(protocol_response(request).status(), 403);
        }
    }
}
