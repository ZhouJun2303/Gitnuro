//! Match a remote to a saved account, and talk to the forge without embedding a browser.
//!
//! Tokens stay in the account records the caller already loaded from the vault.
//! A connectivity probe never follows a redirect, and a failure string never keeps the token.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::settings::{Account, Settings};

static LOGIN_PID: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteParts {
    pub protocol: String,
    pub host: String,
    pub username: String,
}

#[derive(Debug, Clone)]
pub struct RemoteTarget {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub id: String,
    pub label: String,
    pub host: String,
    pub token_url: String,
    pub browser: bool,
    pub probe: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub added: Vec<Account>,
    pub profiles: Vec<Account>,
    pub clear_legacy: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResult {
    pub ok: bool,
    pub needs_token: bool,
    pub token_url: String,
    pub account: Option<Account>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountProbe {
    pub code: String,
    pub login: String,
    pub name: String,
    pub email: String,
    pub avatar_url: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMatch {
    pub remote: String,
    pub url: String,
    pub protocol: String,
    pub host: String,
    pub account_id: String,
    pub label: String,
    pub ssh: bool,
    pub reason: String,
}

pub fn lab_probe() -> AccountProbe {
    AccountProbe {
        code: "ok".into(),
        login: "lab".into(),
        name: "Lab User".into(),
        email: "lab@example.com".into(),
        avatar_url: String::new(),
        message: String::new(),
    }
}

pub fn lab_login() -> LoginResult {
    LoginResult {
        ok: false,
        needs_token: true,
        token_url: "https://example.com/lab/token".into(),
        account: None,
        message: "needs-token".into(),
    }
}

pub fn lab_remote() -> RemoteMatch {
    RemoteMatch {
        remote: "origin".into(),
        url: "https://github.com/lab/example.git".into(),
        protocol: "https".into(),
        host: "github.com".into(),
        account_id: "lab-github".into(),
        label: "Lab User".into(),
        ssh: false,
        reason: "default".into(),
    }
}

/// Drop guard that clears HTTPS passwords when a git write finishes.
pub struct HttpsSession;

impl Drop for HttpsSession {
    fn drop(&mut self) {
        awegit_git::set_https_credentials(Vec::new());
    }
}

/// Install the accounts for this write, including an empty list for a local commit.
pub fn install_https(repo: &Path, request: &awegit_git::Mutation) -> HttpsSession {
    awegit_git::set_https_credentials(Vec::new());
    if let Ok(settings) = crate::settings::load() {
        let creds = credentials_for(&settings, &targets_for(repo, request), &overrides_for(repo, request));
        awegit_git::set_https_credentials(creds);
    }
    HttpsSession
}

pub fn services() -> Vec<ServiceInfo> {
    let gcm = gcm_program().is_some();
    let github_browser = find_program("gh").is_some() || gcm;
    vec![
        service("github", "GitHub", "github.com", &github_token_url("github.com"), github_browser, "api"),
        service("gitlab", "GitLab", "gitlab.com", "https://gitlab.com/-/user_settings/personal_access_tokens", gcm, "api"),
        service("bitbucket", "Bitbucket", "bitbucket.org", "https://bitbucket.org/account/settings/app-passwords/", gcm, "api"),
        service("azure", "Azure DevOps", "dev.azure.com", "https://dev.azure.com/", gcm, "api"),
        service("gitee", "Gitee", "gitee.com", "https://gitee.com/profile/personal_access_tokens", false, "api"),
        service("atomgit", "AtomGit", "atomgit.com", "https://atomgit.com/", false, "git"),
        service("gitcode", "GitCode", "gitcode.com", "https://gitcode.com/", false, "git"),
        service("coding", "Coding", "", "https://coding.net/", false, "git"),
        service("codeup", "Codeup", "codeup.aliyun.com", "https://account.aliyun.com/", false, "git"),
        service("cnb", "CNB", "cnb.cool", "https://cnb.cool/", false, "git"),
        service("gitea", "Codeberg", "codeberg.org", "https://codeberg.org/user/settings/applications", false, "api"),
        service("sourcehut", "SourceHut", "git.sr.ht", "https://meta.sr.ht/oauth2", false, "git"),
        service("generic", "其他主机", "", "", false, "git"),
    ]
}

fn service(id: &str, label: &str, host: &str, token_url: &str, browser: bool, probe: &str) -> ServiceInfo {
    ServiceInfo {
        id: id.into(),
        label: label.into(),
        host: host.into(),
        token_url: token_url.into(),
        browser,
        probe: probe.into(),
    }
}

pub fn token_page(forge: &str, host: &str) -> String {
    let host = if host.trim().is_empty() { default_host(forge).to_string() } else { host_key(host) };
    match forge {
        "github" if host == "github.com" || host.is_empty() => github_token_url("github.com"),
        "github" if host_safe(&host) => format!("https://{host}/settings/tokens/new?scopes=repo,read:org,read:user,user:email&description=AweGit"),
        "gitlab" if host_safe(&host) => format!("https://{host}/-/user_settings/personal_access_tokens"),
        "bitbucket" => "https://bitbucket.org/account/settings/app-passwords/".into(),
        "azure" => "https://dev.azure.com/".into(),
        "gitee" => "https://gitee.com/profile/personal_access_tokens".into(),
        "gitea" if host_safe(&host) => format!("https://{host}/user/settings/applications"),
        "atomgit" => "https://atomgit.com/".into(),
        "gitcode" => "https://gitcode.com/".into(),
        "coding" => "https://coding.net/".into(),
        "codeup" => "https://account.aliyun.com/".into(),
        "cnb" => "https://cnb.cool/".into(),
        "sourcehut" => "https://meta.sr.ht/oauth2".into(),
        _ if host_safe(&host) => format!("https://{host}/"),
        _ => String::new(),
    }
}

fn github_token_url(host: &str) -> String {
    format!("https://{host}/settings/tokens/new?scopes=repo,read:org,read:user,user:email&description=AweGit")
}

pub fn parse_remote(url: &str) -> RemoteParts {
    let url = url.trim();
    if let Some(rest) = url.strip_prefix("ssh://") {
        return parse_authority("ssh", rest);
    }
    if let Some((scheme, rest)) = url.split_once("://") {
        let protocol = if scheme.eq_ignore_ascii_case("https") {
            "https"
        } else if scheme.eq_ignore_ascii_case("http") {
            "http"
        } else if scheme.eq_ignore_ascii_case("ssh") {
            "ssh"
        } else {
            "other"
        };
        return parse_authority(protocol, rest);
    }
    if let Some((user, rest)) = url.split_once('@') {
        if let Some((host, _)) = rest.split_once(':') {
            if !host.contains('/') {
                return RemoteParts {
                    protocol: "ssh".into(),
                    host: host_only(host),
                    username: ignore_git(user),
                };
            }
        }
    }
    RemoteParts { protocol: "other".into(), host: String::new(), username: String::new() }
}

fn parse_authority(protocol: &str, rest: &str) -> RemoteParts {
    let (user, hostpart) = if let Some((user, host)) = rest.split_once('@') {
        (ignore_git(user), host)
    } else {
        (String::new(), rest)
    };
    let host = hostpart.split(['/', '?']).next().unwrap_or("");
    RemoteParts { protocol: protocol.into(), host: host_only(host), username: user }
}

fn ignore_git(user: &str) -> String {
    let user = user.split(':').next().unwrap_or("").trim();
    if user.is_empty() || user.eq_ignore_ascii_case("git") {
        String::new()
    } else {
        user.to_string()
    }
}

fn host_only(host: &str) -> String {
    let host = host.trim().trim_matches(['[', ']']);
    let host = if let Some((name, port)) = host.rsplit_once(':') {
        if port.chars().all(|ch| ch.is_ascii_digit()) { name } else { host }
    } else {
        host
    };
    host.trim_end_matches('.').to_string()
}

pub fn host_key(host: &str) -> String {
    let host = host_only(host).to_ascii_lowercase();
    match host.as_str() {
        "www.github.com" | "ssh.github.com" | "gist.github.com" => "github.com".into(),
        "www.gitlab.com" => "gitlab.com".into(),
        "www.bitbucket.org" => "bitbucket.org".into(),
        other => other.to_string(),
    }
}

pub fn default_host(forge: &str) -> &'static str {
    match forge {
        "gitlab" => "gitlab.com",
        "bitbucket" => "bitbucket.org",
        "azure" => "dev.azure.com",
        "gitee" => "gitee.com",
        "atomgit" => "atomgit.com",
        "gitcode" => "gitcode.com",
        "codeup" => "codeup.aliyun.com",
        "cnb" => "cnb.cool",
        "gitea" => "codeberg.org",
        "sourcehut" => "git.sr.ht",
        "github" => "github.com",
        _ => "",
    }
}

pub fn forge_of_host(host: &str) -> &'static str {
    let host = host_key(host);
    match host.as_str() {
        "github.com" => "github",
        "gitlab.com" => "gitlab",
        "bitbucket.org" => "bitbucket",
        "dev.azure.com" => "azure",
        "gitee.com" => "gitee",
        "atomgit.com" => "atomgit",
        "gitcode.com" | "gitcode.net" => "gitcode",
        "codeup.aliyun.com" => "codeup",
        "cnb.cool" => "cnb",
        "codeberg.org" | "gitea.com" => "gitea",
        "git.sr.ht" => "sourcehut",
        _ if host.ends_with(".visualstudio.com") => "azure",
        _ if host.ends_with(".coding.net") => "coding",
        _ => "generic",
    }
}

pub fn host_safe(host: &str) -> bool {
    !host.is_empty()
        && host.len() < 200
        && !host.starts_with(['-', '.'])
        && !host.ends_with('.')
        && !host.contains("..")
        && host.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
}

pub fn account_host_key(account: &Account) -> String {
    let raw = account.host.trim();
    if raw.is_empty() {
        default_host(&account.forge).to_string()
    } else {
        host_key(raw)
    }
}

pub fn hosts_match(account: &Account, remote_host: &str) -> bool {
    let remote = host_key(remote_host);
    if !host_safe(&remote) {
        return false;
    }
    let own = account_host_key(account);
    if !own.is_empty() && own == remote {
        return true;
    }
    if account.forge == "azure" {
        let public = account.host.trim().is_empty() || host_key(&account.host) == "dev.azure.com";
        if public && (remote == "dev.azure.com" || remote.ends_with(".visualstudio.com")) {
            return true;
        }
    }
    false
}

pub fn explain(accounts: &[Account], remote: &str, url: &str, override_id: Option<&str>) -> RemoteMatch {
    let parts = parse_remote(url);
    let ssh = parts.protocol == "ssh";
    let candidates: Vec<&Account> = accounts.iter().filter(|item| hosts_match(item, &parts.host)).collect();
    let picked = override_id
        .and_then(|id| accounts.iter().find(|item| item.id == id && hosts_match(item, &parts.host)))
        .map(|item| (item, "override"))
        .or_else(|| {
            if parts.username.is_empty() {
                None
            } else {
                candidates.iter().copied().find(|item| item.login.eq_ignore_ascii_case(&parts.username)).map(|item| (item, "login"))
            }
        })
        .or_else(|| candidates.iter().copied().find(|item| item.default_for_host).map(|item| (item, "default")))
        .or_else(|| (candidates.len() == 1).then(|| (candidates[0], "only")));
    match picked {
        Some((account, reason)) => RemoteMatch {
            remote: remote.into(),
            url: url.into(),
            protocol: parts.protocol,
            host: host_key(&parts.host),
            account_id: account.id.clone(),
            label: display_label(account),
            ssh,
            reason: reason.into(),
        },
        None => RemoteMatch {
            remote: remote.into(),
            url: url.into(),
            protocol: parts.protocol,
            host: host_key(&parts.host),
            account_id: String::new(),
            label: String::new(),
            ssh,
            reason: "none".into(),
        },
    }
}

fn display_label(account: &Account) -> String {
    let name = if account.name.trim().is_empty() { account.label.trim() } else { account.name.trim() };
    let login = account.login.trim();
    if !name.is_empty() && !login.is_empty() && name != login {
        format!("{name} @{login}")
    } else if !login.is_empty() {
        format!("@{login}")
    } else if !name.is_empty() {
        name.to_string()
    } else {
        account.forge.clone()
    }
}

pub fn explain_remotes(settings: &Settings, remotes: &[(String, String)], overrides: &[(String, String)]) -> Vec<RemoteMatch> {
    let accounts = with_legacy(settings);
    remotes
        .iter()
        .map(|(name, url)| {
            let override_id = overrides.iter().find(|(remote, _)| remote == name).map(|(_, id)| id.as_str());
            explain(&accounts, name, url, override_id)
        })
        .collect()
}

pub fn git_username(account: &Account) -> String {
    let login = account.login.trim();
    if !login.is_empty() && !login.eq_ignore_ascii_case("git") {
        return login.to_string();
    }
    match account.forge.as_str() {
        "gitlab" | "gitea" | "gitee" => "oauth2".into(),
        "bitbucket" => "x-token-auth".into(),
        "azure" => "pat".into(),
        _ => "x-access-token".into(),
    }
}

pub fn credentials_for(settings: &Settings, targets: &[RemoteTarget], overrides: &[(String, String)]) -> Vec<awegit_git::HttpsCredential> {
    let accounts = with_legacy(settings);
    let mut creds = Vec::new();
    for target in targets {
        let parts = parse_remote(&target.url);
        if parts.protocol != "https" && parts.protocol != "http" {
            continue;
        }
        if !host_safe(&parts.host) {
            continue;
        }
        let host = host_key(&parts.host);
        let override_id = overrides.iter().find(|(name, _)| name == &target.name).map(|(_, id)| id.as_str());
        let chosen = explain(&accounts, &target.name, &target.url, override_id);
        for account in accounts.iter().filter(|item| hosts_match(item, &host) && !item.token.trim().is_empty()) {
            let username = git_username(account);
            let secret = account.token.trim();
            if username.contains(['\n', '\r', '\0', '\'']) || secret.contains(['\n', '\r', '\0']) {
                continue;
            }
            if creds.iter().any(|item: &awegit_git::HttpsCredential| item.host == host && item.username == username && item.secret == secret) {
                continue;
            }
            creds.push(awegit_git::HttpsCredential {
                host: host.clone(),
                username,
                secret: secret.to_string(),
                preferred: chosen.account_id == account.id,
            });
        }
    }
    creds.sort_by(|left, right| left.host.cmp(&right.host).then(right.preferred.cmp(&left.preferred)));
    creds
}

pub fn token_for(settings: &Settings, forge: &str, host: &str) -> String {
    let host = if host.trim().is_empty() { default_host(forge).to_string() } else { host_key(host) };
    let accounts = with_legacy(settings);
    let matched: Vec<&Account> = accounts
        .iter()
        .filter(|item| item.forge == forge && hosts_match(item, &host) && !item.token.trim().is_empty())
        .collect();
    if let Some(item) = matched.iter().copied().find(|item| item.default_for_host) {
        return item.token.clone();
    }
    if matched.len() == 1 {
        return matched[0].token.clone();
    }
    String::new()
}

pub fn fold_legacy(settings: &Settings) -> Vec<Account> {
    let mut added = Vec::new();
    push_legacy(&mut added, &settings.accounts, "legacy-github", "github", "github.com", &settings.github_token);
    let gitlab_host = if settings.gitlab_host.trim().is_empty() { "gitlab.com" } else { settings.gitlab_host.trim() };
    push_legacy(&mut added, &settings.accounts, "legacy-gitlab", "gitlab", gitlab_host, &settings.gitlab_token);
    push_legacy(&mut added, &settings.accounts, "legacy-bitbucket", "bitbucket", "bitbucket.org", &settings.bitbucket_token);
    push_legacy(&mut added, &settings.accounts, "legacy-azure", "azure", "dev.azure.com", &settings.azure_token);
    added
}

fn push_legacy(added: &mut Vec<Account>, existing: &[Account], id: &str, forge: &str, host: &str, token: &str) {
    let token = token.trim();
    if token.is_empty() || token.contains(['\n', '\r', '\0']) {
        return;
    }
    if existing.iter().chain(added.iter()).any(|item| item.id == id) {
        return;
    }
    if existing.iter().chain(added.iter()).any(|item| item.forge == forge && hosts_match(item, host) && !item.token.trim().is_empty()) {
        return;
    }
    let mut account = blank_account(id, forge, host, token);
    account.default_for_host = true;
    added.push(account);
}

fn with_legacy(settings: &Settings) -> Vec<Account> {
    let mut accounts = settings.accounts.clone();
    accounts.extend(fold_legacy(settings));
    accounts
}

fn blank_account(id: &str, forge: &str, host: &str, token: &str) -> Account {
    Account {
        id: id.into(),
        forge: forge.into(),
        label: String::new(),
        token: token.into(),
        host: host.into(),
        client_id: String::new(),
        login: String::new(),
        name: String::new(),
        email: String::new(),
        avatar_url: String::new(),
        default_for_host: false,
    }
}

pub fn scrub_secret(text: &str, secret: &str) -> String {
    let secret = secret.trim();
    let cleaned = if secret.len() >= 4 { text.replace(secret, "••••") } else { text.to_string() };
    let cleaned = cleaned.replace(['\n', '\r'], " ");
    cleaned.chars().take(240).collect()
}

pub fn test_account(account: &Account, probe_url: &str) -> AccountProbe {
    let token = account.token.trim();
    if token.is_empty() {
        return probe_result("missing", "", "", "", "", "");
    }
    if let Some(api) = api_request(account) {
        return probe_api(account, &api);
    }
    probe_git(account, probe_url)
}

fn probe_result(code: &str, login: &str, name: &str, email: &str, avatar: &str, message: &str) -> AccountProbe {
    AccountProbe {
        code: code.into(),
        login: login.into(),
        name: name.into(),
        email: email.into(),
        avatar_url: avatar.into(),
        message: message.into(),
    }
}

struct ApiRequest {
    url: String,
    kind: ApiKind,
}

enum ApiKind {
    Bearer,
    PrivateToken,
    Token,
    Basic,
}

fn api_request(account: &Account) -> Option<ApiRequest> {
    let host = account_host_key(account);
    if !host_safe(&host) {
        return None;
    }
    let (url, kind) = match account.forge.as_str() {
        "github" if host == "github.com" => ("https://api.github.com/user".into(), ApiKind::Bearer),
        "github" => (format!("https://{host}/api/v3/user"), ApiKind::Bearer),
        "gitlab" => (format!("https://{host}/api/v4/user"), ApiKind::PrivateToken),
        "bitbucket" if host == "bitbucket.org" => ("https://api.bitbucket.org/2.0/user".into(), ApiKind::Bearer),
        "azure" if host == "dev.azure.com" || host.ends_with(".visualstudio.com") => {
            ("https://app.vssps.visualstudio.com/_apis/profile/profiles/me?api-version=7.1".into(), ApiKind::Basic)
        }
        "gitee" if host == "gitee.com" => ("https://gitee.com/api/v5/user".into(), ApiKind::Token),
        "gitea" => (format!("https://{host}/api/v1/user"), ApiKind::Token),
        _ => return None,
    };
    Some(ApiRequest { url, kind })
}

fn probe_api(account: &Account, api: &ApiRequest) -> AccountProbe {
    let token = account.token.trim();
    let auth = match api.kind {
        ApiKind::Bearer => format!("Bearer {token}"),
        ApiKind::Token => format!("token {token}"),
        ApiKind::Basic => format!("Basic {}", crate::forge::basic_token(token)),
        ApiKind::PrivateToken => token.to_string(),
    };
    let headers: Vec<(&str, String)> = match api.kind {
        ApiKind::PrivateToken => vec![("PRIVATE-TOKEN", auth)],
        _ => vec![("Authorization", auth), ("Accept", "application/json".into())],
    };
    let pairs: Vec<(&str, &str)> = headers.iter().map(|(name, value)| (*name, value.as_str())).collect();
    let response = crate::http::send_once("GET", &api.url, &pairs, None);
    match response {
        Ok(body) if (200..300).contains(&body.status) => profile_from(account, &body.body),
        Ok(body) if body.status == 401 || body.status == 403 => failed_probe(account, "unauthorized", &format!("HTTP {}", body.status)),
        Ok(body) if (300..400).contains(&body.status) => failed_probe(account, "host", &format!("HTTP {}", body.status)),
        Ok(body) => failed_probe(account, "network", &format!("HTTP {}", body.status)),
        Err(error) if error.contains("awegit:auth:") => failed_probe(account, "unauthorized", &error),
        Err(error) => failed_probe(account, "network", &error),
    }
}

fn profile_from(account: &Account, body: &str) -> AccountProbe {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return failed_probe(account, "network", "profile");
    };
    let (login, name, email, avatar) = match account.forge.as_str() {
        "gitlab" => (
            json_str(&value, &["username"]),
            json_str(&value, &["name"]),
            json_str(&value, &["email"]),
            json_str(&value, &["avatar_url"]),
        ),
        "bitbucket" => (
            json_str(&value, &["username", "nickname"]),
            json_str(&value, &["display_name"]),
            String::new(),
            value.pointer("/links/avatar/href").and_then(|item| item.as_str()).unwrap_or("").to_string(),
        ),
        "azure" => (String::new(), json_str(&value, &["displayName"]), json_str(&value, &["emailAddress"]), String::new()),
        "gitea" => (
            json_str(&value, &["login", "username"]),
            json_str(&value, &["full_name", "name"]),
            json_str(&value, &["email"]),
            json_str(&value, &["avatar_url"]),
        ),
        _ => (
            json_str(&value, &["login", "username"]),
            json_str(&value, &["name"]),
            json_str(&value, &["email"]),
            json_str(&value, &["avatar_url"]),
        ),
    };
    let token = account.token.trim();
    probe_result(
        "ok",
        &scrub_secret(&login, token),
        &scrub_secret(&name, token),
        &scrub_secret(&email, token),
        &scrub_secret(&avatar, token),
        "",
    )
}

fn json_str(value: &serde_json::Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(|item| item.as_str()) {
            if !text.trim().is_empty() {
                return text.trim().to_string();
            }
        }
    }
    String::new()
}

fn failed_probe(account: &Account, code: &str, message: &str) -> AccountProbe {
    probe_result(code, "", "", "", "", &scrub_secret(message, account.token.trim()))
}

fn probe_git(account: &Account, probe_url: &str) -> AccountProbe {
    let url = probe_url.trim();
    if url.is_empty() {
        return failed_probe(account, "need-repo", "");
    }
    if !probe_allowed(account, url) {
        return failed_probe(account, "host", "");
    }
    let username = git_username(account);
    let host = parse_remote(url).host;
    awegit_git::set_https_credentials(vec![awegit_git::HttpsCredential {
        host: host_key(&host),
        username,
        secret: account.token.trim().to_string(),
        preferred: true,
    }]);
    let result = awegit_git::ls_remote_head(url);
    awegit_git::set_https_credentials(Vec::new());
    match result {
        Ok(()) => probe_result("ok", account.login.trim(), account.name.trim(), account.email.trim(), account.avatar_url.trim(), ""),
        Err(error) => {
            let text = error.to_string();
            let lower = text.to_ascii_lowercase();
            let code = if lower.contains("authentication") || lower.contains("401") || lower.contains("403") {
                "unauthorized"
            } else if lower.contains("could not resolve") || lower.contains("unable to access") {
                "network"
            } else {
                "git"
            };
            failed_probe(account, code, &text)
        }
    }
}

fn probe_allowed(account: &Account, url: &str) -> bool {
    let url = url.trim();
    let token = account.token.trim();
    if !url.starts_with("https://") || url.contains(['\n', '\r', '\0', ' ']) {
        return false;
    }
    if token.len() >= 4 && url.contains(token) {
        return false;
    }
    let rest = url.trim_start_matches("https://");
    if rest.contains('@') {
        return false;
    }
    let parts = parse_remote(url);
    parts.protocol == "https" && host_safe(&parts.host) && hosts_match(account, &parts.host)
}

pub fn sync_report(settings: &Settings) -> SyncReport {
    let folded = fold_legacy(settings);
    let clear_legacy = !folded.is_empty();
    let mut added = folded;
    let mut known = settings.accounts.clone();
    known.extend(added.iter().cloned());
    let pending: Vec<(String, String)> = discover_targets(settings)
        .into_iter()
        .filter(|(host, forge)| {
            !known.iter().any(|item| item.forge == *forge && hosts_match(item, host) && !item.token.trim().is_empty())
        })
        .collect();
    for ((host, forge), found) in pending.iter().zip(probe_targets(&pending, probe_host)) {
        let Some((login, token)) = found else { continue };
        if !accept_secret(&token) {
            continue;
        }
        if known.iter().any(|item| hosts_match(item, host) && item.token.trim() == token) {
            continue;
        }
        if !login.is_empty() && known.iter().any(|item| hosts_match(item, host) && item.login.eq_ignore_ascii_case(&login) && !item.token.trim().is_empty()) {
            continue;
        }
        let mut account = blank_account(&fresh_id(), forge, host, &token);
        account.login = login;
        account.default_for_host = !known.iter().any(|item| hosts_match(item, host) && item.default_for_host);
        apply_profile(&mut account);
        known.push(account.clone());
        added.push(account);
    }
    let mut profiles = Vec::new();
    for account in &settings.accounts {
        if !account.login.trim().is_empty() || account.token.trim().is_empty() {
            continue;
        }
        let mut copy = account.clone();
        if apply_profile(&mut copy) {
            copy.token.clear();
            profiles.push(copy);
        }
    }
    SyncReport { added, profiles, clear_legacy }
}

/// Copy what a sync found into the settings that are about to be written.
///
/// A sync can overlap another one, and both may report the same credential under
/// different ids. Match on the token or login as well as the id so the list never
/// gains a second copy.
pub fn merge_report(settings: &mut Settings, report: &SyncReport) {
    for added in &report.added {
        let token = added.token.trim();
        let duplicate = settings.accounts.iter().any(|item| {
            if item.id == added.id {
                return true;
            }
            if !hosts_match(item, &added.host) || item.token.trim().is_empty() {
                return false;
            }
            item.token.trim() == token
                || (!added.login.trim().is_empty() && item.login.eq_ignore_ascii_case(added.login.trim()))
        });
        if duplicate {
            continue;
        }
        settings.accounts.push(added.clone());
    }
    for profile in &report.profiles {
        let Some(account) = settings.accounts.iter_mut().find(|item| item.id == profile.id) else {
            continue;
        };
        if !profile.login.trim().is_empty() {
            account.login = profile.login.clone();
        }
        if !profile.name.trim().is_empty() {
            account.name = profile.name.clone();
        }
        if !profile.email.trim().is_empty() {
            account.email = profile.email.clone();
        }
        if !profile.avatar_url.trim().is_empty() {
            account.avatar_url = profile.avatar_url.clone();
        }
    }
    if report.clear_legacy {
        settings.github_token.clear();
        settings.gitlab_token.clear();
        settings.bitbucket_token.clear();
        settings.azure_token.clear();
    }
}

/// Look credentials up four hosts at a time. Each host waits on a keyring or a
/// cold credential manager, so the waits overlap instead of adding up.
const PROBE_WIDTH: usize = 4;

fn probe_targets<F>(targets: &[(String, String)], probe: F) -> Vec<Option<(String, String)>>
where
    F: Fn(&str, &str) -> Option<(String, String)> + Sync,
{
    let mut found: Vec<Option<(String, String)>> = vec![None; targets.len()];
    for (chunk_index, chunk) in targets.chunks(PROBE_WIDTH).enumerate() {
        let base = chunk_index * PROBE_WIDTH;
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|(host, forge)| scope.spawn(|| probe(host.as_str(), forge.as_str())))
                .collect();
            for (index, handle) in handles.into_iter().enumerate() {
                found[base + index] = handle.join().unwrap_or(None);
            }
        });
    }
    found
}

fn probe_host(host: &str, forge: &str) -> Option<(String, String)> {
    if forge != "github" {
        return credential_fill(host);
    }
    std::thread::scope(|scope| {
        let gh = scope.spawn(|| gh_token(host));
        let git = scope.spawn(|| credential_fill(host));
        let gh = gh.join().ok().flatten().map(|token| (String::new(), token));
        let git = git.join().ok().flatten();
        gh.or(git)
    })
}

fn discover_targets(settings: &Settings) -> Vec<(String, String)> {
    let mut targets: Vec<(String, String)> = ["github.com", "gitlab.com", "bitbucket.org", "dev.azure.com", "gitee.com", "codeberg.org", "codeup.aliyun.com"]
        .into_iter()
        .map(|host| (host.to_string(), forge_of_host(host).to_string()))
        .collect();
    for account in &settings.accounts {
        if !account.token.trim().is_empty() {
            continue;
        }
        let host = account_host_key(account);
        if host_safe(&host) {
            targets.push((host, account.forge.clone()));
        }
    }
    let mut seen = Vec::new();
    targets.retain(|(host, forge)| {
        let key = format!("{forge}\n{host}");
        if seen.iter().any(|item| item == &key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
    targets
}

fn apply_profile(account: &mut Account) -> bool {
    let probe = test_account(account, "");
    if probe.code != "ok" {
        return false;
    }
    if !probe.login.is_empty() {
        account.login = probe.login;
    }
    if !probe.name.is_empty() {
        account.name = probe.name;
        if account.label.trim().is_empty() {
            account.label = account.name.clone();
        }
    }
    if !probe.email.is_empty() {
        account.email = probe.email;
    }
    if !probe.avatar_url.is_empty() {
        account.avatar_url = probe.avatar_url;
    }
    true
}

fn accept_secret(token: &str) -> bool {
    let token = token.trim();
    !token.is_empty() && token.len() < 4000 && !token.contains(['\n', '\r', '\0', ' '])
}

pub fn browser_login(settings: &Settings, forge: &str, host: &str) -> LoginResult {
    cancel_browser_login();
    let host = if host.trim().is_empty() { default_host(forge).to_string() } else { host_key(host) };
    if !host_safe(&host) && forge != "generic" {
        return LoginResult { ok: false, needs_token: true, token_url: String::new(), account: None, message: "host".into() };
    }
    let page = token_page(forge, &host);
    if forge == "github" {
        match gh_web_login(&host) {
            LoginAttempt::Token { login, token } => return finish_login(settings, forge, &host, &login, &token, &page),
            LoginAttempt::Cancelled => return cancelled_login(),
            LoginAttempt::Absent => {}
        }
    }
    if matches!(forge, "github" | "gitlab" | "bitbucket" | "azure") {
        match gcm_web_login(forge, &host) {
            LoginAttempt::Token { login, token } => return finish_login(settings, forge, &host, &login, &token, &page),
            LoginAttempt::Cancelled => return cancelled_login(),
            LoginAttempt::Absent => {}
        }
    }
    LoginResult { ok: false, needs_token: true, token_url: page, account: None, message: "needs-token".into() }
}

fn cancelled_login() -> LoginResult {
    LoginResult { ok: false, needs_token: false, token_url: String::new(), account: None, message: "cancelled".into() }
}

enum LoginAttempt {
    Token { login: String, token: String },
    Cancelled,
    Absent,
}

fn finish_login(settings: &Settings, forge: &str, host: &str, login: &str, token: &str, page: &str) -> LoginResult {
    if !accept_secret(token) {
        return LoginResult { ok: false, needs_token: true, token_url: page.into(), account: None, message: "needs-token".into() };
    }
    let mut account = blank_account(&fresh_id(), forge, host, token);
    account.login = login.trim().to_string();
    if let Some(existing) = settings.accounts.iter().find(|item| {
        hosts_match(item, host) && !login.is_empty() && item.login.eq_ignore_ascii_case(login)
    }) {
        account.id = existing.id.clone();
        account.default_for_host = existing.default_for_host;
        account.label = existing.label.clone();
    } else {
        account.default_for_host = !settings.accounts.iter().any(|item| hosts_match(item, host) && item.default_for_host);
    }
    apply_profile(&mut account);
    if account.label.trim().is_empty() {
        account.label = if account.name.trim().is_empty() { account.login.clone() } else { account.name.clone() };
    }
    LoginResult { ok: true, needs_token: false, token_url: page.into(), account: Some(account), message: String::new() }
}

pub fn cancel_browser_login() {
    let pid = LOGIN_PID.swap(0, Ordering::SeqCst);
    if pid > 0 {
        awegit_git::stop_process_tree(pid);
    }
}

fn gh_web_login(host: &str) -> LoginAttempt {
    let Some(gh) = find_program("gh") else { return LoginAttempt::Absent };
    let args = vec![
        "auth".into(),
        "login".into(),
        "--hostname".into(),
        host.into(),
        "--git-protocol".into(),
        "https".into(),
        "--web".into(),
        "--skip-ssh-key".into(),
    ];
    let Ok(captured) = run_captured(&gh, &args, None, &[], Duration::from_secs(300), true) else {
        return LoginAttempt::Absent;
    };
    if captured.cancelled {
        return LoginAttempt::Cancelled;
    }
    if captured.code != 0 {
        return LoginAttempt::Absent;
    }
    gh_token(host)
        .map(|token| LoginAttempt::Token { login: String::new(), token })
        .unwrap_or(LoginAttempt::Absent)
}

fn gh_token(host: &str) -> Option<String> {
    let gh = find_program("gh")?;
    let args = vec!["auth".into(), "token".into(), "--hostname".into(), host.into()];
    let captured = run_captured(
        &gh,
        &args,
        None,
        &[("GIT_TERMINAL_PROMPT", "0"), ("GCM_INTERACTIVE", "never")],
        Duration::from_secs(4),
        false,
    )
    .ok()?;
    if captured.cancelled || captured.code != 0 {
        return None;
    }
    let token = captured.stdout.trim();
    accept_secret(token).then(|| token.to_string())
}

fn gcm_web_login(forge: &str, host: &str) -> LoginAttempt {
    let Some((program, args)) = gcm_login_args(forge, host) else { return LoginAttempt::Absent };
    let Ok(captured) = run_captured(&program, &args, None, &[], Duration::from_secs(300), true) else {
        return LoginAttempt::Absent;
    };
    if captured.cancelled {
        return LoginAttempt::Cancelled;
    }
    if captured.code != 0 {
        return LoginAttempt::Absent;
    }
    credential_fill(host)
        .map(|(login, token)| LoginAttempt::Token { login, token })
        .unwrap_or(LoginAttempt::Absent)
}

fn gcm_login_args(forge: &str, host: &str) -> Option<(String, Vec<String>)> {
    let (program, prefix) = gcm_program()?;
    let top = run_captured(&program, &prefix, None, &[("GIT_TERMINAL_PROMPT", "0"), ("GCM_INTERACTIVE", "never")], Duration::from_secs(4), false).ok()?;
    let help = format!("{}\n{}", top.stdout, top.stderr);
    let names: &[&str] = match forge {
        "github" => &["github"],
        "gitlab" => &["gitlab"],
        "bitbucket" => &["bitbucket"],
        "azure" => &["azure-repos", "azure"],
        _ => return None,
    };
    for name in names {
        if !help_lists(&help, name) {
            continue;
        }
        let mut sub_args = prefix.clone();
        sub_args.push((*name).to_string());
        let sub = run_captured(&program, &sub_args, None, &[("GCM_INTERACTIVE", "never")], Duration::from_secs(4), false).ok()?;
        let sub_help = format!("{}\n{}", sub.stdout, sub.stderr);
        if !help_lists(&sub_help, "login") {
            continue;
        }
        sub_args.push("login".into());
        if host_safe(host) && (forge == "github" || forge == "gitlab") {
            sub_args.push("--hostname".into());
            sub_args.push(host.into());
        }
        return Some((program, sub_args));
    }
    None
}

fn gcm_command() -> Option<(String, Vec<String>)> {
    if let Some(path) = find_program("git-credential-manager") {
        return Some((path, Vec::new()));
    }
    let git = awegit_git::git_program();
    if git.trim().is_empty() {
        return None;
    }
    Some((git, vec!["credential-manager".into()]))
}

fn gcm_program() -> Option<(String, Vec<String>)> {
    static SLOT: std::sync::OnceLock<Option<(String, Vec<String>)>> = std::sync::OnceLock::new();
    SLOT.get_or_init(|| {
        let (program, prefix) = gcm_command()?;
        let help = run_captured(&program, &prefix, None, &[("GIT_TERMINAL_PROMPT", "0"), ("GCM_INTERACTIVE", "never")], Duration::from_secs(4), false).ok()?;
        let text = format!("{}\n{}", help.stdout, help.stderr);
        let lower = text.to_ascii_lowercase();
        if lower.contains("not a git command") || lower.contains("no known browser") {
            return None;
        }
        if !help_lists(&text, "github") && !help_lists(&text, "gitlab") && !help_lists(&text, "bitbucket") {
            return None;
        }
        Some((program, prefix))
    })
    .clone()
}

fn help_lists(text: &str, name: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-').any(|word| word == name)
}

fn credential_fill(host: &str) -> Option<(String, String)> {
    if !host_safe(host) {
        return None;
    }
    let git = awegit_git::git_program();
    let input = format!("protocol=https\nhost={host}\n\n");
    let captured = run_captured(
        &git,
        &["credential".into(), "fill".into()],
        Some(input),
        &[("GIT_TERMINAL_PROMPT", "0"), ("GCM_INTERACTIVE", "never")],
        Duration::from_secs(4),
        false,
    )
    .ok()?;
    if captured.cancelled || captured.code != 0 {
        return None;
    }
    let mut username = String::new();
    let mut password = String::new();
    for line in captured.stdout.lines() {
        if let Some(value) = line.strip_prefix("username=") {
            username = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("password=") {
            password = value.trim().to_string();
        }
    }
    if username.is_empty() || !accept_secret(&password) {
        return None;
    }
    Some((username, password))
}

struct Captured {
    code: i32,
    stdout: String,
    stderr: String,
    cancelled: bool,
}

fn run_captured(
    program: &str,
    args: &[String],
    stdin: Option<String>,
    env: &[(&str, &str)],
    timeout: Duration,
    track: bool,
) -> Result<Captured, String> {
    if program.trim().is_empty() || program.starts_with('-') {
        return Err("program".into());
    }
    let mut command = Command::new(program);
    command.args(args);
    command.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    awegit_git::hide_console(&mut command);
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command.spawn().map_err(|_| "spawn".to_string())?;
    if let Some(text) = stdin {
        if let Some(mut pipe) = child.stdin.take() {
            let _ = pipe.write_all(text.as_bytes());
        }
    }
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out_thread = std::thread::spawn(move || read_pipe(stdout));
    let err_thread = std::thread::spawn(move || read_pipe(stderr));
    let pid = child.id();
    if track {
        LOGIN_PID.store(pid, Ordering::SeqCst);
    }
    let started = Instant::now();
    let status = loop {
        if track && LOGIN_PID.load(Ordering::SeqCst) != pid {
            let _ = child.kill();
            let _ = child.wait();
            let _ = out_thread.join();
            let _ = err_thread.join();
            return Ok(Captured { code: 1, stdout: String::new(), stderr: String::new(), cancelled: true });
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > timeout => {
                let _ = awegit_git::stop_process_tree(pid);
                let _ = child.wait();
                // A helper's own children can keep the pipes open after the direct
                // child dies. Leave the readers detached so a slow keyring cannot
                // hold the whole sync past its timeout.
                drop(out_thread);
                drop(err_thread);
                if track {
                    let _ = LOGIN_PID.compare_exchange(pid, 0, Ordering::SeqCst, Ordering::SeqCst);
                }
                return Err("timeout".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(_) => {
                let _ = out_thread.join();
                let _ = err_thread.join();
                return Err("wait".into());
            }
        }
    };
    if track {
        let _ = LOGIN_PID.compare_exchange(pid, 0, Ordering::SeqCst, Ordering::SeqCst);
    }
    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    Ok(Captured { code: status.code().unwrap_or(1), stdout, stderr, cancelled: false })
}

fn read_pipe(pipe: Option<impl Read>) -> String {
    let mut text = String::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_string(&mut text);
    }
    text
}

fn find_program(name: &str) -> Option<String> {
    let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).map(|folder| folder.join(&file)).find(|candidate| candidate.is_file()).map(|path| path.display().to_string())
}

fn fresh_id() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_nanos()).unwrap_or(0);
    format!("acct-{nanos}")
}

fn targets_for(repo: &Path, request: &awegit_git::Mutation) -> Vec<RemoteTarget> {
    match request {
        awegit_git::Mutation::Fetch { remote: Some(name), .. } | awegit_git::Mutation::Push { remote: Some(name), .. } | awegit_git::Mutation::DeleteRemoteTag { remote: name, .. } => named_remote(repo, name),
        awegit_git::Mutation::Fetch { remote: None, .. }
        | awegit_git::Mutation::Pull { .. }
        | awegit_git::Mutation::Push { remote: None, .. }
        | awegit_git::Mutation::SubmoduleUpdate
        | awegit_git::Mutation::SubmoduleSync { .. }
        | awegit_git::Mutation::LfsPush
        | awegit_git::Mutation::LfsPull => all_remotes(repo),
        awegit_git::Mutation::Clone { url, .. } | awegit_git::Mutation::SubmoduleAdd { url, .. } => {
            vec![RemoteTarget { name: String::new(), url: url.clone() }]
        }
        _ => Vec::new(),
    }
}

fn overrides_for(repo: &Path, request: &awegit_git::Mutation) -> Vec<(String, String)> {
    match request {
        awegit_git::Mutation::Clone { .. } | awegit_git::Mutation::Init { .. } => Vec::new(),
        _ => awegit_git::remote_account_overrides(repo),
    }
}

fn all_remotes(repo: &Path) -> Vec<RemoteTarget> {
    awegit_git::remote_list(repo).into_iter().map(|(name, url)| RemoteTarget { name, url }).collect()
}

fn named_remote(repo: &Path, name: &str) -> Vec<RemoteTarget> {
    all_remotes(repo).into_iter().filter(|item| item.name == name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(id: &str, forge: &str, host: &str, login: &str, token: &str, default_for_host: bool) -> Account {
        let mut item = blank_account(id, forge, host, token);
        item.login = login.into();
        item.name = login.into();
        item.default_for_host = default_for_host;
        item
    }

    fn target(url: &str) -> RemoteTarget {
        RemoteTarget { name: "origin".into(), url: url.into() }
    }

    #[test]
    fn recognizes_common_hosts_and_ssh_forms() {
        assert_eq!(parse_remote("https://github.com/a/b.git").host, "github.com");
        assert_eq!(parse_remote("https://octocat@github.com/a/b.git").username, "octocat");
        assert_eq!(parse_remote("https://git@github.com/a/b.git").username, "");
        assert_eq!(parse_remote("git@github.com:a/b.git").protocol, "ssh");
        assert_eq!(parse_remote("git@github.com:a/b.git").host, "github.com");
        assert_eq!(parse_remote("ssh://git@gitlab.com/group/repo.git").host, "gitlab.com");
        assert_eq!(parse_remote("https://github.com:443/a/b.git").host, "github.com");
        assert_eq!(forge_of_host("gitee.com"), "gitee");
        assert_eq!(forge_of_host("codeberg.org"), "gitea");
        assert_eq!(forge_of_host("dev.azure.com"), "azure");
        assert_eq!(forge_of_host("org.visualstudio.com"), "azure");
        assert_eq!(forge_of_host("team.coding.net"), "coding");
        assert_eq!(forge_of_host("codeup.aliyun.com"), "codeup");
        assert_eq!(parse_remote("git@codeup.aliyun.com:org/repo.git").host, "codeup.aliyun.com");
        assert_eq!(token_page("codeup", "codeup.aliyun.com"), "https://account.aliyun.com/");
        assert_eq!(forge_of_host("github.example.com"), "generic");
        let github = account("gh", "github", "", "me", "t", true);
        assert!(hosts_match(&github, "www.github.com"));
        assert!(hosts_match(&github, "ssh.github.com"));
        assert!(!hosts_match(&github, "github.example.com"));
        let azure = account("az", "azure", "", "me", "t", true);
        assert!(hosts_match(&azure, "dev.azure.com"));
        assert!(hosts_match(&azure, "org.visualstudio.com"));
        let specific = account("az2", "azure", "org.visualstudio.com", "me", "t", true);
        assert!(hosts_match(&specific, "org.visualstudio.com"));
        assert!(!hosts_match(&specific, "other.visualstudio.com"));
    }

    #[test]
    fn match_order_prefers_override_then_login_then_default() {
        let accounts = vec![
            account("def", "github", "github.com", "defuser", "t1", true),
            account("octo", "github", "github.com", "octocat", "t2", false),
            account("gl", "gitlab", "gitlab.com", "gl", "t3", true),
        ];
        let login = explain(&accounts, "origin", "https://octocat@github.com/a/b.git", None);
        assert_eq!(login.reason, "login");
        assert_eq!(login.account_id, "octo");
        let fallback = explain(&accounts, "origin", "https://github.com/a/b.git", None);
        assert_eq!(fallback.reason, "default");
        assert_eq!(fallback.account_id, "def");
        let picked = explain(&accounts, "origin", "https://github.com/a/b.git", Some("octo"));
        assert_eq!(picked.reason, "override");
        assert_eq!(picked.account_id, "octo");
        let crossed = explain(&accounts, "origin", "https://github.com/a/b.git", Some("gl"));
        assert_eq!(crossed.account_id, "def");
        assert_ne!(crossed.reason, "override");
        let only = explain(&[account("one", "github", "", "me", "t", false)], "origin", "git@github.com:a/b.git", None);
        assert_eq!(only.reason, "only");
        assert!(only.ssh);
    }

    #[test]
    fn a_token_is_not_offered_to_another_host() {
        let mut settings = Settings::default();
        settings.accounts.push(account("gh", "github", "github.com", "octo", "SECRET-TOKEN-VALUE", true));
        assert!(credentials_for(&settings, &[target("https://gitlab.com/a/b.git")], &[]).is_empty());
        assert!(credentials_for(&settings, &[target("https://github.example.com/a/b.git")], &[]).is_empty());
        assert!(credentials_for(&settings, &[target("git@github.com:a/b.git")], &[]).is_empty());
        let creds = credentials_for(&settings, &[target("https://github.com/a/b.git")], &[]);
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].host, "github.com");
        assert_eq!(creds[0].secret, "SECRET-TOKEN-VALUE");
        settings.accounts.push(account("other", "github", "github.com", "other", "OTHER-TOKEN", false));
        let both = credentials_for(&settings, &[target("https://github.com/a/b.git")], &[]);
        assert_eq!(both.len(), 2);
        assert!(both.iter().any(|item| item.preferred && item.secret == "SECRET-TOKEN-VALUE"));
        assert!(both.iter().any(|item| !item.preferred && item.secret == "OTHER-TOKEN"));
        let mut unmarked = Settings::default();
        unmarked.accounts.push(account("a", "github", "github.com", "a", "ta", false));
        unmarked.accounts.push(account("b", "github", "github.com", "b", "tb", false));
        let none = credentials_for(&unmarked, &[target("https://github.com/a/b.git")], &[]);
        assert!(none.iter().all(|item| !item.preferred));
    }

    #[test]
    fn legacy_token_is_not_duplicated() {
        let mut settings = Settings::default();
        settings.github_token = "legacy-secret".into();
        settings.accounts.push(account("real", "github", "github.com", "me", "kept", true));
        assert!(fold_legacy(&settings).is_empty());
        let creds = credentials_for(&settings, &[target("https://github.com/a/b.git")], &[]);
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].secret, "kept");
        settings.accounts.clear();
        let folded = fold_legacy(&settings);
        assert_eq!(folded.len(), 1);
        assert_eq!(folded[0].id, "legacy-github");
        assert_eq!(folded[0].token, "legacy-secret");
        let creds = credentials_for(&settings, &[target("https://github.com/a/b.git")], &[]);
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].secret, "legacy-secret");
    }

    #[test]
    fn sync_merge_keeps_one_copy_of_each_credential() {
        let mut settings = Settings::default();
        settings.accounts.push(account("saved", "github", "github.com", "octo", "TOKEN-ONE", true));
        let report = SyncReport {
            added: vec![
                account("fresh", "github", "github.com", "octo", "TOKEN-ONE", false),
                account("new", "gitlab", "gitlab.com", "gl", "TOKEN-TWO", true),
            ],
            profiles: vec![],
            clear_legacy: false,
        };
        merge_report(&mut settings, &report);
        assert_eq!(settings.accounts.len(), 2);
        assert_eq!(settings.accounts[0].id, "saved");
        assert_eq!(settings.accounts[1].id, "new");
    }

    #[test]
    fn sync_merge_fills_profile_fields_and_clears_legacy_tokens() {
        let mut settings = Settings::default();
        settings.github_token = "legacy".into();
        settings.accounts.push(account("kept", "github", "github.com", "", "TOKEN", false));
        let mut profile = settings.accounts[0].clone();
        profile.login = "octo".into();
        profile.name = "Octo Cat".into();
        profile.email = "octo@example.com".into();
        profile.avatar_url = "https://example.com/a.png".into();
        profile.token.clear();
        let report = SyncReport { added: vec![], profiles: vec![profile], clear_legacy: true };
        merge_report(&mut settings, &report);
        assert_eq!(settings.accounts.len(), 1);
        assert_eq!(settings.accounts[0].login, "octo");
        assert_eq!(settings.accounts[0].name, "Octo Cat");
        assert_eq!(settings.accounts[0].email, "octo@example.com");
        assert_eq!(settings.accounts[0].token, "TOKEN");
        assert!(settings.github_token.is_empty());
    }

    #[test]
    fn probe_targets_keep_the_target_order() {
        let targets = vec![
            ("second.example".to_string(), "gitlab".to_string()),
            ("first.example".to_string(), "github".to_string()),
            ("third.example".to_string(), "gitea".to_string()),
        ];
        let found = probe_targets(&targets, |host, forge| Some((forge.to_string(), format!("token-{host}"))));
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].as_ref().unwrap().0, "gitlab");
        assert_eq!(found[1].as_ref().unwrap().1, "token-first.example");
        assert_eq!(found[2].as_ref().unwrap().0, "gitea");
    }

    #[test]
    fn failure_text_does_not_contain_the_token() {
        let token = "super-secret-token-value";
        let cleaned = scrub_secret(&format!("HTTP 401 {token} unauthorized"), token);
        assert!(!cleaned.contains(token));
        assert!(cleaned.contains("••••"));
        let mut item = account("ag", "atomgit", "atomgit.com", "", "", false);
        assert_eq!(test_account(&item, "").code, "missing");
        item.token = token.into();
        let wrong = test_account(&item, "https://evil.example/repo.git");
        assert_eq!(wrong.code, "host");
        assert!(!wrong.message.contains(token));
        let embedded = test_account(&item, &format!("https://atomgit.com/{token}/repo.git"));
        assert_eq!(embedded.code, "host");
        assert!(!embedded.message.contains(token));
        assert!(!embedded.login.contains(token));
        let needs = test_account(&item, "");
        assert_eq!(needs.code, "need-repo");
        assert!(!needs.message.contains(token));
    }
}
