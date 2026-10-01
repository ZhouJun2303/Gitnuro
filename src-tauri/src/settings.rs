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
    #[serde(default = "split_style")]
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
    #[serde(default)]
    pub show_diff_marks: bool,
    #[serde(default = "diff_font")]
    pub diff_font_size: u32,
    #[serde(default)]
    pub disable_syntax_highlight: bool,
    /// `date` or `topo`.
    #[serde(default = "date_sort")]
    pub commit_sort: String,
    #[serde(default)]
    pub fetch_automatically: bool,
    #[serde(default)]
    pub fetch_tags: bool,
    #[serde(default = "default_true")]
    pub tab_indicator: bool,
    #[serde(default)]
    pub update_submodules_on_checkout: bool,
    /// Character used in place of spaces when creating a branch. Empty keeps the space.
    #[serde(default = "dash")]
    pub branch_space: String,
    #[serde(default)]
    pub push_on_commit: bool,
    #[serde(default)]
    pub compact_branch_labels: bool,
    #[serde(default = "low_limit")]
    pub message_low: u32,
    #[serde(default = "high_limit")]
    pub message_high: u32,
    /// `disable` or `enable`.
    #[serde(default = "spell_off")]
    pub spell_checking: String,
    #[serde(default = "guide_col")]
    pub page_guide: u32,
    #[serde(default = "default_true")]
    pub highlight_issues: bool,
    /// `default` or `custom`.
    #[serde(default = "shell_default")]
    pub shell_kind: String,
    #[serde(default)]
    pub shell_path: String,
    #[serde(default)]
    pub shell_args: String,
    #[serde(default)]
    pub diff_tool_name: String,
    #[serde(default)]
    pub diff_tool_path: String,
    #[serde(default)]
    pub diff_tool_args: String,
    #[serde(default)]
    pub merge_tool_name: String,
    #[serde(default)]
    pub merge_tool_path: String,
    #[serde(default)]
    pub merge_tool_args: String,
    #[serde(default)]
    pub ignore_space: bool,
    #[serde(default)]
    pub vertical_tabs: bool,
    /// `repository<TAB>branch` pairs pinned in the sidebar.
    #[serde(default)]
    pub pinned_refs: Vec<String>,
    /// Extra folders scanned by the repository manager.
    #[serde(default)]
    pub source_directories: Vec<String>,
    /// `folder<TAB>label` names for repository-manager groups.
    #[serde(default)]
    pub group_names: Vec<String>,
    #[serde(default)]
    pub show_whitespace: bool,
    /// `tree`, `list`, or `combined`. Empty follows `tree_files`.
    #[serde(default)]
    pub file_layout: String,
    /// `path<TAB>label` names shown on tabs.
    #[serde(default)]
    pub tab_labels: Vec<String>,
    /// `path<TAB>#rrggbb` colors shown on tabs.
    #[serde(default)]
    pub tab_colors: Vec<String>,
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub bitbucket_token: String,
    #[serde(default)]
    pub azure_token: String,
    #[serde(default)]
    pub azure_org: String,
    /// OAuth client id for a device-code login. Empty keeps token paste.
    #[serde(default)]
    pub oauth_client_id: String,
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
    /// One label per line. Each label replaces `${label}` and the first also replaces `${input}`.
    #[serde(default)]
    pub prompt: String,
    /// When false, the command is offered only in `repo`.
    #[serde(default = "default_true")]
    pub shared: bool,
    #[serde(default)]
    pub repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    /// `github`, `gitlab`, `bitbucket`, or `azure`.
    pub forge: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub client_id: String,
}

fn theme_default() -> String {
    "system".into()
}
fn default_true() -> bool {
    true
}
fn split_style() -> String {
    "split".into()
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
fn diff_font() -> u32 {
    13
}
fn date_sort() -> String {
    "date".into()
}
fn dash() -> String {
    "-".into()
}
fn low_limit() -> u32 {
    50
}
fn high_limit() -> u32 {
    70
}
fn spell_off() -> String {
    "disable".into()
}
fn guide_col() -> u32 {
    72
}
fn shell_default() -> String {
    "default".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: theme_default(),
            pull_rebase: false,
            fetch_prune: true,
            diff_style: split_style(),
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
            show_diff_marks: false,
            diff_font_size: diff_font(),
            disable_syntax_highlight: false,
            commit_sort: date_sort(),
            fetch_automatically: false,
            fetch_tags: false,
            tab_indicator: true,
            update_submodules_on_checkout: false,
            branch_space: dash(),
            push_on_commit: false,
            compact_branch_labels: false,
            message_low: low_limit(),
            message_high: high_limit(),
            spell_checking: spell_off(),
            page_guide: guide_col(),
            highlight_issues: true,
            shell_kind: shell_default(),
            shell_path: String::new(),
            shell_args: String::new(),
            diff_tool_name: String::new(),
            diff_tool_path: String::new(),
            diff_tool_args: String::new(),
            merge_tool_name: String::new(),
            merge_tool_path: String::new(),
            merge_tool_args: String::new(),
            ignore_space: false,
            vertical_tabs: false,
            pinned_refs: Vec::new(),
            source_directories: Vec::new(),
            group_names: Vec::new(),
            show_whitespace: false,
            file_layout: String::new(),
            tab_labels: Vec::new(),
            tab_colors: Vec::new(),
            accounts: Vec::new(),
            bitbucket_token: String::new(),
            azure_token: String::new(),
            azure_org: String::new(),
            oauth_client_id: String::new(),
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
