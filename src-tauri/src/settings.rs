//! Local settings. This file is created on save and is not the old DataStore.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "theme_default")]
    pub theme: String,
    #[serde(default)]
    pub pull_rebase: bool,
    #[serde(default = "default_true")]
    pub fetch_prune: bool,
    #[serde(default = "unified")]
    pub diff_style: String,
    #[serde(default)]
    pub proxy: String,
    #[serde(default = "ai_base")]
    pub ai_base_url: String,
    #[serde(default = "ai_model")]
    pub ai_model: String,
    #[serde(default)]
    pub ai_api_key: String,
    #[serde(default)]
    pub terminal: String,
    #[serde(default)]
    pub swap_panes: bool,
    #[serde(default)]
    pub show_entire_file: bool,
    #[serde(default = "compact")]
    pub lines_height: String,
    #[serde(default = "scale_default")]
    pub ui_scale: u32,
    #[serde(default = "default_true")]
    pub date_relative: bool,
    #[serde(default)]
    pub date_format: String,
    #[serde(default = "default_true")]
    pub date_24h: bool,
    #[serde(default)]
    pub hidden_refs: String,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub author_email: String,
    #[serde(default = "default_true")]
    pub ssl_verify: bool,
    #[serde(default)]
    pub ssl_ca_file: String,
    #[serde(default)]
    pub proxy_user: String,
    #[serde(default)]
    pub proxy_password: String,
    /// When true and `proxy_host` is set, that host is used instead of `proxy`.
    #[serde(default)]
    pub proxy_enabled: bool,
    /// `http` or `socks`. Empty means HTTP.
    #[serde(default)]
    pub proxy_type: String,
    #[serde(default)]
    pub proxy_host: String,
    #[serde(default)]
    pub proxy_port: u32,
    #[serde(default)]
    pub sign_commits: bool,
    #[serde(default)]
    pub merge_no_ff: bool,
    #[serde(default)]
    pub merge_autostash: bool,
    #[serde(default)]
    pub clone_directory: String,
    #[serde(default)]
    pub window_x: i32,
    #[serde(default)]
    pub window_y: i32,
    #[serde(default)]
    pub window_width: u32,
    #[serde(default)]
    pub window_height: u32,
    #[serde(default)]
    pub tree_files: bool,
    #[serde(default)]
    pub gravatar: bool,
    #[serde(default)]
    pub sign_off: bool,
    #[serde(default = "sign_off_format")]
    pub sign_off_format: String,
    #[serde(default = "default_true")]
    pub force_with_lease: bool,
    #[serde(default = "default_true")]
    pub ai_enabled: bool,
    #[serde(default)]
    pub ai_language: String,
    #[serde(default = "ai_max")]
    pub ai_max_chars: u32,
    #[serde(default)]
    pub ai_prompt: String,
    #[serde(default)]
    pub ai_temperature: f32,
    #[serde(default)]
    pub log_directory: String,
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default)]
    pub workspaces: Vec<WorkspaceRecord>,
    #[serde(default)]
    pub current_workspace: String,
    #[serde(default)]
    pub commands: Vec<CommandRecord>,
    #[serde(default)]
    pub flow_master: String,
    #[serde(default = "develop_name")]
    pub flow_develop: String,
    #[serde(default = "feature_prefix")]
    pub flow_feature: String,
    #[serde(default = "release_prefix")]
    pub flow_release: String,
    #[serde(default = "hotfix_prefix")]
    pub flow_hotfix: String,
    #[serde(default = "support_prefix")]
    pub flow_support: String,
    #[serde(default)]
    pub expanded_groups: Vec<String>,
    /// `system`, `zh`, or `en`. Empty is treated as system.
    #[serde(default = "locale_default")]
    pub locale: String,
    #[serde(default)]
    pub diff_tool: String,
    #[serde(default)]
    pub merge_tool: String,
    #[serde(default)]
    pub github_token: String,
    #[serde(default)]
    pub gitlab_token: String,
    #[serde(default)]
    pub gitlab_host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecord {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub repositories: Vec<String>,
    #[serde(default)]
    pub open_tabs: Vec<String>,
    #[serde(default)]
    pub selected_tab: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRecord {
    pub id: String,
    pub name: String,
    /// repository, commit, branch, or file.
    pub target: String,
    pub command: String,
}

fn theme_default() -> String {
    "system".into()
}
fn default_true() -> bool {
    true
}
fn unified() -> String {
    "unified".into()
}
fn ai_base() -> String {
    "https://api.x.ai/v1".into()
}
fn ai_model() -> String {
    "grok-4.7".into()
}
fn compact() -> String {
    "compact".into()
}
fn scale_default() -> u32 {
    13
}
fn sign_off_format() -> String {
    "Signed-off-by: %user <%email>".into()
}
fn ai_max() -> u32 {
    12_000
}
fn develop_name() -> String {
    "develop".into()
}
fn feature_prefix() -> String {
    "feature/".into()
}
fn release_prefix() -> String {
    "release/".into()
}
fn hotfix_prefix() -> String {
    "hotfix/".into()
}
fn support_prefix() -> String {
    "support/".into()
}
fn locale_default() -> String {
    "system".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: theme_default(),
            pull_rebase: false,
            fetch_prune: true,
            diff_style: unified(),
            proxy: String::new(),
            ai_base_url: ai_base(),
            ai_model: ai_model(),
            ai_api_key: String::new(),
            terminal: String::new(),
            swap_panes: false,
            show_entire_file: false,
            lines_height: compact(),
            ui_scale: scale_default(),
            date_relative: true,
            date_format: String::new(),
            date_24h: true,
            hidden_refs: String::new(),
            author_name: String::new(),
            author_email: String::new(),
            ssl_verify: true,
            ssl_ca_file: String::new(),
            proxy_user: String::new(),
            proxy_password: String::new(),
            proxy_enabled: false,
            proxy_type: String::new(),
            proxy_host: String::new(),
            proxy_port: 0,
            sign_commits: false,
            merge_no_ff: false,
            merge_autostash: false,
            clone_directory: String::new(),
            window_x: 0,
            window_y: 0,
            window_width: 0,
            window_height: 0,
            tree_files: false,
            gravatar: false,
            sign_off: false,
            sign_off_format: sign_off_format(),
            force_with_lease: true,
            ai_enabled: true,
            ai_language: String::new(),
            ai_max_chars: ai_max(),
            ai_prompt: String::new(),
            ai_temperature: 0.0,
            log_directory: String::new(),
            recent: Vec::new(),
            workspaces: Vec::new(),
            current_workspace: String::new(),
            commands: Vec::new(),
            flow_master: String::new(),
            flow_develop: develop_name(),
            flow_feature: feature_prefix(),
            flow_release: release_prefix(),
            flow_hotfix: hotfix_prefix(),
            flow_support: support_prefix(),
            expanded_groups: Vec::new(),
            locale: locale_default(),
            diff_tool: String::new(),
            merge_tool: String::new(),
            github_token: String::new(),
            gitlab_token: String::new(),
            gitlab_host: String::new(),
        }
    }
}

pub fn path() -> Result<PathBuf, String> {
    let base = if cfg!(windows) {
        std::env::var("APPDATA").map_err(|error| error.to_string())?
    } else if cfg!(target_os = "macos") {
        format!("{}/Library/Application Support", std::env::var("HOME").map_err(|error| error.to_string())?)
    } else {
        std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            format!("{}/.config", std::env::var("HOME").unwrap_or_default())
        })
    };
    Ok(PathBuf::from(base).join("AweGit").join("settings.json"))
}

pub fn load() -> Result<Settings, String> {
    let path = path()?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

pub fn save(settings: &Settings) -> Result<(), String> {
    let path = path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    std::fs::write(path, text).map_err(|error| error.to_string())
}
