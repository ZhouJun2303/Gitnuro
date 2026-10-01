//! Looks at the same update document the current AweGit client uses.

use serde::Serialize;

const VERSION_CHECK_URL: &str = "https://raw.githubusercontent.com/ZhouJun2303/AweGit/main/latest.json";
const APP_VERSION_CODE: i64 = 25;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNotice {
    pub app_version: String,
    pub app_code: i64,
    pub download_url: String,
}

pub fn check() -> Result<Option<UpdateNotice>, String> {
    let text = http_get(VERSION_CHECK_URL)?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let app_code = value.get("appCode").and_then(|item| item.as_i64()).unwrap_or(0);
    if app_code <= APP_VERSION_CODE {
        return Ok(None);
    }
    Ok(Some(UpdateNotice {
        app_version: value.get("appVersion").and_then(|item| item.as_str()).unwrap_or("").to_string(),
        app_code,
        download_url: value.get("downloadUrl").and_then(|item| item.as_str()).unwrap_or("").to_string(),
    }))
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
