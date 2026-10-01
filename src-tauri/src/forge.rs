//! GitHub notifications and GitHub or GitLab pull requests for the current remote.
//!
//! Tokens stay in the local settings file. They are sent as headers and are not written into logs.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub id: String,
    pub title: String,
    pub url: String,
    pub unread: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub title: String,
    pub author: String,
    pub branch: String,
    pub url: String,
}

pub fn notifications(github_token: &str) -> Result<Vec<Notice>, String> {
    if github_token.trim().is_empty() {
        return Ok(Vec::new());
    }
    let text = http_get(
        "https://api.github.com/notifications?all=false&per_page=20",
        &[("Authorization", &format!("Bearer {}", github_token.trim())), ("Accept", "application/vnd.github+json")],
    )?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    Ok(items
        .iter()
        .filter_map(|item| {
            let title = item.pointer("/subject/title").and_then(|value| value.as_str()).unwrap_or("").to_string();
            if title.is_empty() {
                return None;
            }
            let url = item
                .pointer("/subject/url")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .replace("api.github.com/repos", "github.com")
                .replace("/pulls/", "/pull/");
            Some(Notice {
                id: json_id(item.get("id")),
                title,
                url,
                unread: item.get("unread").and_then(|value| value.as_bool()).unwrap_or(true),
            })
        })
        .collect())
}

pub fn pulls(remote_url: &str, github_token: &str, gitlab_token: &str, gitlab_host: &str) -> Result<Vec<PullRequest>, String> {
    let Some((host, project)) = remote_project(remote_url) else {
        return Ok(Vec::new());
    };
    if host.contains("github.com") {
        if github_token.trim().is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("https://api.github.com/repos/{project}/pulls?state=open&per_page=20");
        let text = http_get(&url, &[("Authorization", &format!("Bearer {}", github_token.trim())), ("Accept", "application/vnd.github+json")])?;
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
        let Some(items) = value.as_array() else {
            return Ok(Vec::new());
        };
        return Ok(items
            .iter()
            .map(|item| PullRequest {
                title: item.get("title").and_then(|value| value.as_str()).unwrap_or("").to_string(),
                author: item.pointer("/user/login").and_then(|value| value.as_str()).unwrap_or("").to_string(),
                branch: item.pointer("/head/ref").and_then(|value| value.as_str()).unwrap_or("").to_string(),
                url: item.get("html_url").and_then(|value| value.as_str()).unwrap_or("").to_string(),
            })
            .filter(|item| !item.title.is_empty())
            .collect());
    }
    if gitlab_token.trim().is_empty() {
        return Ok(Vec::new());
    }
    let api_host = if gitlab_host.trim().is_empty() { host } else { gitlab_host.trim().to_string() };
    let encoded = project.replace('/', "%2F");
    let url = format!("https://{api_host}/api/v4/projects/{encoded}/merge_requests?state=opened&per_page=20");
    let text = http_get(&url, &[("PRIVATE-TOKEN", gitlab_token.trim())])?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    Ok(items
        .iter()
        .map(|item| PullRequest {
            title: item.get("title").and_then(|value| value.as_str()).unwrap_or("").to_string(),
            author: item.pointer("/author/username").and_then(|value| value.as_str()).unwrap_or("").to_string(),
            branch: item.get("source_branch").and_then(|value| value.as_str()).unwrap_or("").to_string(),
            url: item.get("web_url").and_then(|value| value.as_str()).unwrap_or("").to_string(),
        })
        .filter(|item| !item.title.is_empty())
        .collect())
}

fn json_id(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(item) => item.as_str().map(str::to_string).or_else(|| item.as_i64().map(|number| number.to_string())).unwrap_or_default(),
        None => String::new(),
    }
}

fn remote_project(url: &str) -> Option<(String, String)> {
    let url = url.trim().trim_end_matches(".git");
    if let Some(rest) = url.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        let path = path.trim_matches('/');
        if host.is_empty() || path.is_empty() {
            return None;
        }
        return Some((host.to_string(), path.to_string()));
    }
    let without = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let (host, path) = without.split_once('/')?;
    let path = path.trim_matches('/');
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some((host.to_string(), path.to_string()))
}

pub fn mark_read(github_token: &str, id: &str) -> Result<(), String> {
    let id = id.trim();
    if github_token.trim().is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err("notification id is missing".into());
    }
    let url = format!("https://api.github.com/notifications/threads/{id}");
    http_send("PATCH", &url, &[("Authorization", &format!("Bearer {}", github_token.trim())), ("Accept", "application/vnd.github+json")], None)?;
    Ok(())
}

pub fn create_repo(github_token: &str, name: &str, private_repo: bool) -> Result<String, String> {
    let name = repo_name(name)?;
    if github_token.trim().is_empty() {
        return Err("repository name is missing".into());
    }
    let body = format!(r#"{{"name":"{name}","private":{private_repo}}}"#);
    let text = http_send(
        "POST",
        "https://api.github.com/user/repos",
        &[
            ("Authorization", &format!("Bearer {}", github_token.trim())),
            ("Accept", "application/vnd.github+json"),
            ("Content-Type", "application/json"),
        ],
        Some(body.as_str()),
    )?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    value.get("html_url").and_then(|item| item.as_str()).map(str::to_string).ok_or_else(|| "GitHub did not return a repository".into())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStart {
    pub verification_uri: String,
    pub user_code: String,
}

#[derive(Clone)]
struct PendingDevice {
    forge: String,
    client_id: String,
    host: String,
    device_code: String,
}

static PENDING: std::sync::Mutex<Option<PendingDevice>> = std::sync::Mutex::new(None);

pub fn create_hosted(forge: &str, token: &str, host: &str, name: &str, private_repo: bool, org: &str) -> Result<String, String> {
    let name = repo_name(name)?;
    let token = token.trim();
    if token.is_empty() {
        return Err("token is missing".into());
    }
    match forge {
        "gitlab" => {
            let host = if host.trim().is_empty() { "gitlab.com".to_string() } else { host.trim().to_string() };
            let visibility = if private_repo { "private" } else { "public" };
            let body = format!(r#"{{"name":"{name}","visibility":"{visibility}"}}"#);
            let text = http_send(
                "POST",
                &format!("https://{host}/api/v4/projects"),
                &[("PRIVATE-TOKEN", token), ("Content-Type", "application/json")],
                Some(&body),
            )?;
            json_url(&text, &["web_url"])
        }
        "bitbucket" => {
            let workspace = org.trim();
            if !repo_name(workspace).is_ok() {
                return Err("Bitbucket needs a workspace".into());
            }
            let body = format!(r#"{{"scm":"git","is_private":{private_repo}}}"#);
            let text = http_send(
                "POST",
                &format!("https://api.bitbucket.org/2.0/repositories/{workspace}/{name}"),
                &[("Authorization", &format!("Bearer {token}")), ("Content-Type", "application/json")],
                Some(&body),
            )?;
            let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
            value
                .pointer("/links/html/href")
                .and_then(|item| item.as_str())
                .map(str::to_string)
                .ok_or_else(|| "Bitbucket did not return a repository".into())
        }
        "azure" => {
            let org = org.trim();
            let project = if host.trim().is_empty() { name.as_str() } else { host.trim() };
            if repo_name(org).is_err() || repo_name(project).is_err() {
                return Err("Azure needs an organization and a project".into());
            }
            let body = format!(r#"{{"name":"{name}"}}"#);
            let url = format!("https://dev.azure.com/{org}/{project}/_apis/git/repositories?api-version=7.1");
            let text = http_send(
                "POST",
                &url,
                &[("Authorization", &format!("Basic {}", basic_token(token))), ("Content-Type", "application/json")],
                Some(&body),
            )?;
            json_url(&text, &["webUrl", "remoteUrl"])
        }
        _ => create_repo(token, &name, private_repo),
    }
}

pub fn device_start(forge: &str, client_id: &str, host: &str) -> Result<DeviceStart, String> {
    let client_id = client_id.trim();
    if client_id.is_empty() || client_id.contains(['\n', ' ', '&']) {
        return Err("OAuth client id is missing".into());
    }
    let host = if host.trim().is_empty() {
        if forge == "gitlab" { "gitlab.com" } else { "github.com" }
    } else {
        host.trim()
    };
    let (url, body) = if forge == "gitlab" {
        (format!("https://{host}/oauth/authorize_device"), format!("client_id={client_id}&scope=api"))
    } else {
        ("https://github.com/login/device/code".into(), format!("client_id={client_id}&scope=repo"))
    };
    let text = http_send("POST", &url, &[("Accept", "application/json"), ("Content-Type", "application/x-www-form-urlencoded")], Some(&body))?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let device_code = value.get("device_code").and_then(|item| item.as_str()).unwrap_or("").to_string();
    let user_code = value.get("user_code").and_then(|item| item.as_str()).unwrap_or("").to_string();
    let verification_uri = value
        .get("verification_uri")
        .or_else(|| value.get("verification_url"))
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string();
    if device_code.is_empty() || user_code.is_empty() {
        return Err(text);
    }
    if let Ok(mut slot) = PENDING.lock() {
        *slot = Some(PendingDevice { forge: forge.to_string(), client_id: client_id.to_string(), host: host.to_string(), device_code });
    }
    Ok(DeviceStart { verification_uri, user_code })
}

/// Returns the access token, or `pending` while the user has not finished the browser step.
pub fn device_poll() -> Result<String, String> {
    let pending = PENDING.lock().map_err(|_| "device login was interrupted".to_string())?.clone();
    let Some(pending) = pending else {
        return Err("start a device login first".into());
    };
    let body = format!(
        "client_id={}&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
        pending.client_id, pending.device_code
    );
    let url = if pending.forge == "gitlab" {
        format!("https://{}/oauth/token", pending.host)
    } else {
        "https://github.com/login/oauth/access_token".into()
    };
    let text = http_send("POST", &url, &[("Accept", "application/json"), ("Content-Type", "application/x-www-form-urlencoded")], Some(&body))?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| text.clone())?;
    if let Some(token) = value.get("access_token").and_then(|item| item.as_str()) {
        if let Ok(mut slot) = PENDING.lock() {
            *slot = None;
        }
        return Ok(token.to_string());
    }
    let error = value.get("error").and_then(|item| item.as_str()).unwrap_or("");
    if error == "authorization_pending" || error == "slow_down" {
        return Ok("pending".into());
    }
    Err(if error.is_empty() { text } else { error.to_string() })
}

fn repo_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    let safe = !name.is_empty() && !name.starts_with('-') && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if safe { Ok(name.to_string()) } else { Err("repository name is missing".into()) }
}

fn json_url(text: &str, keys: &[&str]) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    for key in keys {
        if let Some(url) = value.get(*key).and_then(|item| item.as_str()) {
            return Ok(url.to_string());
        }
    }
    Err("the host did not return a repository".into())
}

fn basic_token(token: &str) -> String {
    let raw = format!(":{token}");
    base64_encode(raw.as_bytes())
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0] as u32;
        let second = chunk.get(1).copied().unwrap_or(0) as u32;
        let third = chunk.get(2).copied().unwrap_or(0) as u32;
        let value = (first << 16) | (second << 8) | third;
        out.push(TABLE[((value >> 18) & 63) as usize] as char);
        out.push(TABLE[((value >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { TABLE[((value >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[(value & 63) as usize] as char } else { '=' });
    }
    out
}

fn http_get(url: &str, headers: &[(&str, &str)]) -> Result<String, String> {
    http_send("GET", url, headers, None)
}

fn http_send(method: &str, url: &str, headers: &[(&str, &str)], body: Option<&str>) -> Result<String, String> {
    let mut command = std::process::Command::new("curl");
    command.args(["--silent", "--show-error", "--fail", "--max-time", "20", "--request", method, "--header", "User-Agent: AweGit"]);
    if let Some(body) = body {
        command.arg("--data").arg(body);
    }
    for (name, value) in headers {
        command.arg("--header").arg(format!("{name}: {value}"));
    }
    command.arg(url);
    let output = command.output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err("could not reach the host".into());
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}
