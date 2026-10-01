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

fn http_get(url: &str, headers: &[(&str, &str)]) -> Result<String, String> {
    let mut command = std::process::Command::new("curl");
    command.args(["--silent", "--show-error", "--fail", "--max-time", "20", "--header", "User-Agent: AweGit"]);
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
