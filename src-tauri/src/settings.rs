//! Local settings. This file is created on save and is not the old DataStore.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// On-disk shape of `settings.json`. Bump this when a migration has to rewrite fields.
pub const SETTINGS_SCHEMA: u32 = 1;

fn schema_current() -> u32 {
    SETTINGS_SCHEMA
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "schema_current")]
    pub schema: u32,
    /// `recovered-backup` when this load restored `settings.json.bak`. Not written to disk.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub recovery_notice: String,
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
    /// Language of AI commit messages: `zh` or `en`. Chinese is the default.
    #[serde(default = "ai_language_default")]
    pub ai_language: String,
    #[serde(default = "ai_max")]
    pub ai_max_chars: u32,
    #[serde(default)]
    pub ai_prompt: String,
    /// `short` or `long`. Empty and anything else mean the short prompt.
    #[serde(default = "prompt_style")]
    pub ai_prompt_style: String,
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
    /// How many days of operation failures the command log keeps.
    #[serde(default = "error_days")]
    pub error_log_days: u32,
    /// Completed operation graphs are retained locally for this many days.
    #[serde(default = "flow_days")]
    pub flow_history_days: u32,
    /// Automatically show the read-only operation observer.
    #[serde(default = "default_true")]
    pub flow_auto_open: bool,
    /// Close the operation observer after all conflicts and follow-up steps finish.
    #[serde(default)]
    pub flow_auto_close: bool,
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
    /// Ordered external tools. List order is the priority; a tool with no
    /// extensions is the fallback for every file.
    #[serde(default)]
    pub diff_tools: Vec<ToolRecord>,
    #[serde(default)]
    pub merge_tools: Vec<ToolRecord>,
    /// Editors that open a file, optionally at a line.
    #[serde(default)]
    pub editor_tools: Vec<ToolRecord>,
    #[serde(default)]
    pub ignore_space: bool,
    #[serde(default)]
    pub vertical_tabs: bool,
    /// `repository<TAB>branch` pairs pinned in the sidebar.
    #[serde(default)]
    pub pinned_refs: Vec<String>,
    /// Source folders, each with an alias in `group_names`. Choosing one opens the folder picker there.
    #[serde(default)]
    pub source_directories: Vec<String>,
    /// `folder<TAB>label` aliases, such as a source folder named 工作 or 个人.
    #[serde(default)]
    pub group_names: Vec<String>,
    #[serde(default)]
    pub show_whitespace: bool,
    /// `tree`, `list`, or `combined`. Empty follows `tree_files`.
    #[serde(default)]
    pub file_layout: String,
    /// When true, a work-tree `.fakeignore` hides matching changes from the lists.
    /// Patterns use `.gitignore` syntax. Git itself is unchanged.
    #[serde(default = "default_true")]
    pub fake_ignore: bool,
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
    /// Fetch dialog starts with every remote selected.
    #[serde(default)]
    pub fetch_all_remotes: bool,
    /// Push dialog starts with every tag included.
    #[serde(default)]
    pub push_all_tags: bool,
    /// New branches are checked out after they are created.
    #[serde(default = "default_true")]
    pub checkout_after_create: bool,
    /// Stash dialog includes untracked files.
    #[serde(default)]
    pub stash_include_untracked: bool,
    #[serde(default)]
    pub diff_word_wrap: bool,
    /// Unchanged lines kept around each hunk. Zero shows only the changes.
    #[serde(default = "diff_context_default")]
    pub diff_context: u32,
    /// Seconds between local status refreshes. Zero leaves that timer off.
    #[serde(default)]
    pub status_interval: u32,
    #[serde(default = "default_true")]
    pub show_worktrees: bool,
    /// How many commits the history list loads. Load more can still ask for a later page.
    #[serde(default = "max_commits_default")]
    pub max_commits: u32,
    #[serde(default)]
    pub rebase_update_refs: bool,
    /// Applying a stash drops it afterwards. Pop already drops the stash.
    #[serde(default)]
    pub apply_stash_delete: bool,
    #[serde(default)]
    pub push_tag_on_create: bool,
    #[serde(default)]
    pub workspace_show_title: bool,
    /// How deep a source folder is walked when looking for repositories.
    #[serde(default = "scan_depth_default")]
    pub scan_depth: u32,
    /// Last commit-message type chosen in the composer.
    #[serde(default = "commit_type_default")]
    pub commit_type: String,
    /// Types offered by the commit composer. Empty keeps the built-in list.
    #[serde(default)]
    pub commit_types: Vec<CommitTypeRecord>,
    /// Update banner stays hidden for this version until a newer one is published.
    #[serde(default)]
    pub dismissed_update: String,
    /// Install an update without showing the installer window.
    #[serde(default = "default_true")]
    pub silent_install: bool,
    /// `pending` asks a new install to open the feature guide once. Missing on an
    /// existing file means the install predates the guide, so it stays `seen`.
    #[serde(default = "features_intro_seen")]
    pub features_intro: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitTypeRecord {
    /// Lowercase name before the colon, such as `feat`.
    #[serde(default)]
    pub name: String,
    /// `#rrggbb` swatch used by the picker.
    #[serde(default)]
    pub color: String,
    /// Custom description shown in the picker. Empty keeps the built-in text.
    #[serde(default)]
    pub label: String,
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
    /// The workspace tabs already include every member repository.
    #[serde(default)]
    pub tabs_seeded: bool,
    /// Count-based width kept in settings. The tab strip fills from the open repositories when its size or count is dirty.
    #[serde(default)]
    pub tab_width: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    /// Arguments after the path. Diff tools use `{left}` `{right}` `{file}`;
    /// merge tools use `{ours}` `{theirs}` `{base}` `{working}` `{file}`;
    /// editors use `{file}` and `{line}`.
    #[serde(default)]
    pub args: String,
    /// Lowercase suffixes without the dot. Empty, or `*`, matches every file.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Used for files that match no explicit extension rule.
    #[serde(default)]
    pub fallback: bool,
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
    "apple".into()
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
fn ai_language_default() -> String {
    "zh".into()
}
fn prompt_style() -> String {
    "short".into()
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
fn flow_days() -> u32 { 3 }
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
fn error_days() -> u32 {
    7
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
fn diff_context_default() -> u32 {
    3
}
fn max_commits_default() -> u32 {
    500
}
fn scan_depth_default() -> u32 {
    5
}
fn commit_type_default() -> String {
    "feat".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: schema_current(),
            recovery_notice: String::new(),
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
            ai_language: ai_language_default(),
            ai_max_chars: ai_max(),
            ai_prompt: String::new(),
            ai_prompt_style: prompt_style(),
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
            error_log_days: error_days(),
            flow_history_days: flow_days(),
            flow_auto_open: true,
            flow_auto_close: false,
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
            diff_tools: Vec::new(),
            merge_tools: Vec::new(),
            editor_tools: Vec::new(),
            ignore_space: false,
            vertical_tabs: false,
            pinned_refs: Vec::new(),
            source_directories: Vec::new(),
            group_names: Vec::new(),
            show_whitespace: false,
            file_layout: String::new(),
            fake_ignore: true,
            tab_labels: Vec::new(),
            tab_colors: Vec::new(),
            accounts: Vec::new(),
            bitbucket_token: String::new(),
            azure_token: String::new(),
            azure_org: String::new(),
            oauth_client_id: String::new(),
            fetch_all_remotes: false,
            push_all_tags: false,
            checkout_after_create: true,
            stash_include_untracked: false,
            diff_word_wrap: false,
            diff_context: diff_context_default(),
            status_interval: 0,
            show_worktrees: true,
            max_commits: max_commits_default(),
            rebase_update_refs: false,
            apply_stash_delete: false,
            push_tag_on_create: false,
            workspace_show_title: false,
            scan_depth: scan_depth_default(),
            commit_type: commit_type_default(),
            commit_types: Vec::new(),
            dismissed_update: String::new(),
            silent_install: true,
            features_intro: features_intro_seen(),
        }
    }
}

fn features_intro_seen() -> String {
    "seen".into()
}

pub fn path() -> Result<PathBuf, String> {
    if let Ok(custom) = std::env::var("AWEGIT_SETTINGS") {
        let custom = custom.trim();
        if !custom.is_empty() {
            return Ok(PathBuf::from(custom));
        }
    }
    let base = if cfg!(windows) {
        std::env::var("APPDATA").map_err(|error| error.to_string())?
    } else if cfg!(target_os = "macos") {
        format!(
            "{}/Library/Application Support",
            std::env::var("HOME").map_err(|error| error.to_string())?
        )
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .unwrap_or_else(|_| format!("{}/.config", std::env::var("HOME").unwrap_or_default()))
    };
    Ok(PathBuf::from(base).join("AweGit").join("settings.json"))
}

pub fn load() -> Result<Settings, String> {
    load_at(&path()?)
}

pub fn save(settings: &Settings) -> Result<(), String> {
    save_at(&path()?, settings)
}

enum ReadIssue {
    Corrupt,
    Newer(u64),
    Io(String),
}

struct StoredFile {
    settings: Settings,
    migrated: bool,
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let name = path.file_name().map(|name| {
        let mut owned = name.to_os_string();
        owned.push(suffix);
        owned
    });
    match name {
        Some(name) => path.with_file_name(name),
        None => path.to_path_buf(),
    }
}

/// A temp name only this process can own, so two app instances never truncate each other's write.
fn temp_sibling(path: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    sibling(path, &format!(".tmp-{}-{stamp}", std::process::id()))
}

fn remove_on_error(tmp: &Path, error: std::io::Error) -> String {
    let _ = std::fs::remove_file(tmp);
    error.to_string()
}

fn read_stored(path: &Path) -> Result<StoredFile, ReadIssue> {
    let text = std::fs::read_to_string(path).map_err(|error| ReadIssue::Io(error.to_string()))?;
    if text.trim().is_empty() {
        return Err(ReadIssue::Corrupt);
    }
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| ReadIssue::Corrupt)?;
    if !value.is_object() {
        return Err(ReadIssue::Corrupt);
    }
    let schema = value.get("schema").and_then(|item| item.as_u64()).unwrap_or(0);
    if schema > SETTINGS_SCHEMA as u64 {
        return Err(ReadIssue::Newer(schema));
    }
    let mut settings: Settings = serde_json::from_value(value).map_err(|_| ReadIssue::Corrupt)?;
    let migrated = (schema as u32) < SETTINGS_SCHEMA;
    settings.schema = SETTINGS_SCHEMA;
    Ok(StoredFile { settings, migrated })
}

fn load_at(path: &Path) -> Result<Settings, String> {
    if !path.exists() {
        let mut fresh = Settings::default();
        fresh.features_intro = "pending".into();
        crate::secrets::fill(&mut fresh)?;
        return Ok(fresh);
    }
    match read_stored(path) {
        Ok(stored) => {
            let mut settings = stored.settings;
            let imported = crate::secrets::import_plaintext(&settings)?;
            crate::secrets::fill(&mut settings)?;
            let mut changed = stored.migrated || imported;
            changed |= repair_imported_tabs(&mut settings);
            changed |= assign_tab_widths(&mut settings);
            if changed {
                save_at(path, &settings)?;
            }
            Ok(settings)
        }
        Err(ReadIssue::Corrupt) => recover_at(path),
        Err(ReadIssue::Newer(schema)) => Err(format!("设置文件由更新的版本写入（schema {schema}），这次没有覆盖。")),
        Err(ReadIssue::Io(message)) => Err(message),
    }
}

fn recover_at(path: &Path) -> Result<Settings, String> {
    let bak = sibling(path, ".bak");
    let Ok(stored) = read_stored(&bak) else {
        return Err("设置文件已损坏，没有可恢复的备份。".into());
    };
    let mut settings = stored.settings;
    settings.schema = SETTINGS_SCHEMA;
    let _imported = crate::secrets::import_plaintext(&settings)?;
    crate::secrets::fill(&mut settings)?;
    save_at(path, &settings)?;
    settings.recovery_notice = "recovered-backup".into();
    Ok(settings)
}

fn save_at(path: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            let existing = value.get("schema").and_then(|item| item.as_u64()).unwrap_or(0);
            if existing > SETTINGS_SCHEMA as u64 {
                return Err(format!("设置文件由更新的版本写入（schema {existing}），这次没有覆盖。"));
            }
        }
    }
    let mut stored = settings.clone();
    stored.recovery_notice.clear();
    stored.schema = SETTINGS_SCHEMA;
    crate::secrets::store_and_clear(&mut stored)?;
    let text = serde_json::to_string_pretty(&stored).map_err(|error| error.to_string())?;
    let tmp = temp_sibling(path);
    {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|error| remove_on_error(&tmp, error))?;
        file.write_all(text.as_bytes()).map_err(|error| remove_on_error(&tmp, error))?;
        file.sync_all().map_err(|error| remove_on_error(&tmp, error))?;
    }
    if read_stored(path).is_ok() {
        copy_durable(path, &sibling(path, ".bak"))?;
    }
    if let Err(error) = replace_file(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(error);
    }
    sync_parent(path);
    let _ = copy_durable(path, &sibling(path, ".bak"));
    Ok(())
}

fn copy_durable(from: &Path, to: &Path) -> Result<(), String> {
    let tmp = temp_sibling(to);
    std::fs::copy(from, &tmp).map_err(|error| remove_on_error(&tmp, error))?;
    if let Ok(file) = std::fs::OpenOptions::new().write(true).open(&tmp) {
        let _ = file.sync_all();
    }
    if let Err(error) = replace_file(&tmp, to) {
        let _ = std::fs::remove_file(&tmp);
        return Err(error);
    }
    Ok(())
}

fn replace_file(from: &Path, to: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let ok = unsafe { MoveFileExW(from_wide.as_ptr(), to_wide.as_ptr(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) };
        if ok == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(from, to).map_err(|error| error.to_string())
    }
}

fn sync_parent(path: &Path) {
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

#[cfg(windows)]
const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
#[cfg(windows)]
const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkImport {
    pub settings: Settings,
    pub preferences: u32,
    pub repositories: u32,
    pub workspaces: u32,
}

/// Read a Fork `settings.json`, merge the matching fields, and save AweGit's file.
pub fn import_fork_at(path: &std::path::Path) -> Result<ForkImport, String> {
    if !path.is_file() {
        return Err("fork-settings-missing".into());
    }
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let mut current = load().unwrap_or_else(|_| Settings::default());
    let report = apply_fork(&mut current, &value, &text);
    save(&current)?;
    Ok(ForkImport {
        settings: current,
        preferences: report.0,
        repositories: report.1,
        workspaces: report.2,
    })
}

/// Merge Fork values into `settings`. Secrets, commands, and the window rectangle stay as they are.
pub fn apply_fork(
    settings: &mut Settings,
    fork: &serde_json::Value,
    text: &str,
) -> (u32, u32, u32) {
    let mut preferences = 0u32;
    take_bool(
        fork,
        "Pull_Rebase",
        &mut settings.pull_rebase,
        &mut preferences,
    );
    take_bool(
        fork,
        "FetchRemotesAutomatically",
        &mut settings.fetch_automatically,
        &mut preferences,
    );
    take_bool(
        fork,
        "FetchAllTags",
        &mut settings.fetch_tags,
        &mut preferences,
    );
    take_bool(
        fork,
        "PushAutomaticallyOnCommit",
        &mut settings.push_on_commit,
        &mut preferences,
    );
    take_bool(
        fork,
        "UpdateSubmodulesOnCheckout",
        &mut settings.update_submodules_on_checkout,
        &mut preferences,
    );
    take_bool(
        fork,
        "DiffIgnoreWhitespaces",
        &mut settings.ignore_space,
        &mut preferences,
    );
    take_bool(
        fork,
        "DiffShowHiddenSymbols",
        &mut settings.show_whitespace,
        &mut preferences,
    );
    take_bool(
        fork,
        "DiffShowChangeMarks",
        &mut settings.show_diff_marks,
        &mut preferences,
    );
    take_bool(
        fork,
        "DiffShowEntireFile",
        &mut settings.show_entire_file,
        &mut preferences,
    );
    take_bool(
        fork,
        "DiffWordWrap",
        &mut settings.diff_word_wrap,
        &mut preferences,
    );
    take_bool(
        fork,
        "CompactBranchLabels",
        &mut settings.compact_branch_labels,
        &mut preferences,
    );
    take_bool(
        fork,
        "DisableSyntaxHighlighting",
        &mut settings.disable_syntax_highlight,
        &mut preferences,
    );
    take_bool(
        fork,
        "Fetch_FetchAllRemotes",
        &mut settings.fetch_all_remotes,
        &mut preferences,
    );
    take_bool(
        fork,
        "Push_PushAllTags",
        &mut settings.push_all_tags,
        &mut preferences,
    );
    take_bool(
        fork,
        "CreateBranch_Checkout",
        &mut settings.checkout_after_create,
        &mut preferences,
    );
    take_bool(
        fork,
        "SaveStash_StageNewFiles",
        &mut settings.stash_include_untracked,
        &mut preferences,
    );
    take_bool(
        fork,
        "ApplyStash_DeleteAfterApply",
        &mut settings.apply_stash_delete,
        &mut preferences,
    );
    take_bool(
        fork,
        "CreateTag_Push",
        &mut settings.push_tag_on_create,
        &mut preferences,
    );
    take_bool(
        fork,
        "ShowWorktrees",
        &mut settings.show_worktrees,
        &mut preferences,
    );
    take_bool(
        fork,
        "RebaseUpdateRefs",
        &mut settings.rebase_update_refs,
        &mut preferences,
    );
    take_u32(
        fork,
        "CodeEditorFontSize",
        &mut settings.diff_font_size,
        &mut preferences,
        1,
    );
    take_u32(
        fork,
        "CommitSubjectLowLimit",
        &mut settings.message_low,
        &mut preferences,
        1,
    );
    take_u32(
        fork,
        "CommitSubjectHighLimit",
        &mut settings.message_high,
        &mut preferences,
        1,
    );
    take_u32(
        fork,
        "PageGuideLinePosition",
        &mut settings.page_guide,
        &mut preferences,
        1,
    );
    take_u32(
        fork,
        "DiffContextSize",
        &mut settings.diff_context,
        &mut preferences,
        0,
    );
    take_u32(
        fork,
        "AutomaticStatusUpdateInterval",
        &mut settings.status_interval,
        &mut preferences,
        0,
    );
    take_u32(
        fork,
        "MaxCommitCount",
        &mut settings.max_commits,
        &mut preferences,
        1,
    );
    if let Some(value) = fork
        .get("ReferenceSpaceCharacterReplacement")
        .and_then(serde_json::Value::as_str)
    {
        settings.branch_space = value.to_string();
        preferences += 1;
    }
    if let Some(mode) = fork
        .get("CommitSpellCheckingMode")
        .and_then(serde_json::Value::as_i64)
    {
        settings.spell_checking = if mode == 0 { "disable" } else { "enable" }.into();
        preferences += 1;
    }
    if fork
        .get("FollowSystemTheme")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        settings.theme = "system".into();
        preferences += 1;
    } else if let Some(theme) = fork.get("Theme").and_then(serde_json::Value::as_i64) {
        settings.theme = if theme == 1 { "dark" } else { "light" }.into();
        preferences += 1;
    }
    if let Some(mode) = fork.get("FileListMode").and_then(serde_json::Value::as_i64) {
        match mode {
            1 => {
                settings.file_layout = "tree".into();
                settings.tree_files = true;
            }
            2 => {
                settings.file_layout = "combined".into();
                settings.tree_files = false;
            }
            _ => {
                settings.file_layout = "list".into();
                settings.tree_files = false;
            }
        }
        preferences += 1;
    }
    if let Some(mode) = [
        "HistoryDiffLayoutMode",
        "CommitDiffLayoutMode",
        "RevisionDiffLayoutMode",
    ]
    .into_iter()
    .find_map(|key| fork.get(key).and_then(serde_json::Value::as_i64))
    {
        settings.diff_style = if mode == 0 { "unified" } else { "split" }.into();
        preferences += 1;
    }
    if let Some(depth) = fork
        .pointer("/RepositoryManager/ScanDepth")
        .and_then(serde_json::Value::as_u64)
    {
        if depth > 0 {
            settings.scan_depth = depth.min(u32::MAX as u64) as u32;
            preferences += 1;
        }
    }
    if let Some(show) = fork
        .pointer("/Workspaces/ShowInTitle")
        .and_then(serde_json::Value::as_bool)
    {
        settings.workspace_show_title = show;
        preferences += 1;
    }
    apply_tools(settings, text, &mut preferences);
    let repositories = merge_repositories(settings, fork);
    let workspaces = merge_workspaces(settings, fork);
    assign_tab_widths(settings);
    (preferences, repositories, workspaces)
}

fn take_bool(fork: &serde_json::Value, key: &str, dest: &mut bool, count: &mut u32) {
    if let Some(value) = fork.get(key).and_then(serde_json::Value::as_bool) {
        *dest = value;
        *count += 1;
    }
}

fn take_u32(fork: &serde_json::Value, key: &str, dest: &mut u32, count: &mut u32, minimum: u64) {
    let Some(value) = fork.get(key) else { return };
    let Some(number) = value
        .as_u64()
        .or_else(|| value.as_f64().map(|item| item as u64))
    else {
        return;
    };
    if number < minimum {
        return;
    }
    *dest = number.min(u32::MAX as u64) as u32;
    *count += 1;
}

fn apply_tools(settings: &mut Settings, text: &str, count: &mut u32) {
    let tools = crate::tools::tools_in_fork_text(text);
    if let Some(tool) = tools
        .merge
        .iter()
        .find(|item| item.primary)
        .or(tools.merge.first())
    {
        settings.merge_tool_name = tool.name.clone();
        settings.merge_tool_path = tool.path.clone();
        settings.merge_tool_args = tool.args.clone();
        settings.merge_tool = compose_tool(&tool.path, &tool.args);
        *count += 1;
    }
    if let Some(tool) = tools
        .diff
        .iter()
        .find(|item| item.primary)
        .or(tools.diff.first())
    {
        settings.diff_tool_name = tool.name.clone();
        settings.diff_tool_path = tool.path.clone();
        settings.diff_tool_args = tool.args.clone();
        settings.diff_tool = compose_tool(&tool.path, &tool.args);
        *count += 1;
    }
    append_imported(&mut settings.merge_tools, &tools.merge);
    append_imported(&mut settings.diff_tools, &tools.diff);
}

fn append_imported(list: &mut Vec<ToolRecord>, imported: &[crate::tools::ExternalTool]) {
    for tool in imported {
        if list.iter().any(|item| same_tool_path(&item.path, &tool.path)) {
            continue;
        }
        let id = format!("tool-{:016x}", path_hash(&tool.path));
        list.push(ToolRecord {
            id,
            name: tool.name.clone(),
            path: tool.path.clone(),
            args: tool.args.clone(),
            extensions: Vec::new(),
            fallback: false,
        });
    }
}

fn path_hash(path: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.trim().replace('/', "\\").to_ascii_lowercase().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn same_tool_path(left: &str, right: &str) -> bool {
    let norm = |value: &str| value.trim().trim_end_matches(['\\', '/']).replace('/', "\\").to_ascii_lowercase();
    !left.trim().is_empty() && norm(left) == norm(right)
}

pub(crate) fn compose_tool(path: &str, args: &str) -> String {
    let file = path.trim();
    let rest = args.trim();
    if file.is_empty() {
        return rest.to_string();
    }
    let quoted = if file.contains(' ') {
        format!("\"{file}\"")
    } else {
        file.to_string()
    };
    if rest.is_empty() {
        quoted
    } else {
        format!("{quoted} {rest}")
    }
}

fn merge_repositories(settings: &mut Settings, fork: &serde_json::Value) -> u32 {
    let Some(rows) = fork
        .pointer("/RepositoryManager/Repositories")
        .and_then(serde_json::Value::as_array)
    else {
        return 0;
    };
    let mut ordered: Vec<(String, String)> = rows
        .iter()
        .filter_map(|item| {
            let path = item
                .get("Path")
                .and_then(serde_json::Value::as_str)?
                .trim()
                .to_string();
            if path.is_empty() {
                return None;
            }
            let time = item
                .get("LastAccessTime")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            Some((time, path))
        })
        .collect();
    ordered.sort_by(|left, right| right.0.cmp(&left.0));
    let imported: Vec<String> = ordered.into_iter().map(|(_, path)| path).collect();
    let count = unique_paths(&imported);
    let roots = string_list(fork.pointer("/RepositoryManager/SourceDirectories"));
    settings.source_directories = merge_paths(&roots, &settings.source_directories);
    settings.recent = merge_paths(&imported, &settings.recent);
    count
}

fn merge_workspaces(settings: &mut Settings, fork: &serde_json::Value) -> u32 {
    let Some(rows) = fork
        .pointer("/Workspaces/All")
        .and_then(serde_json::Value::as_array)
    else {
        return 0;
    };
    let mut count = 0u32;
    for item in rows {
        let Some(name) = item
            .get("Name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        let repos = string_list(item.get("Repositories"));
        let active = item
            .get("ActiveRepository")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if let Some(existing) = settings
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.name == name)
        {
            existing.repositories = merge_paths(&repos, &existing.repositories);
            let (tabs, selected) = open_workspace(&existing.repositories, &active);
            existing.open_tabs = tabs;
            existing.selected_tab = selected;
            existing.tabs_seeded = true;
        } else {
            let id = fresh_workspace_id(settings, name);
            let (open_tabs, selected_tab) = open_workspace(&repos, &active);
            settings.workspaces.push(WorkspaceRecord {
                id,
                name: name.to_string(),
                repositories: repos,
                open_tabs,
                selected_tab,
                tabs_seeded: true,
                tab_width: 0,
            });
        }
        count += 1;
    }
    if let Some(active_name) = fork
        .pointer("/Workspaces/ActiveWorkspace")
        .and_then(serde_json::Value::as_str)
    {
        if let Some(workspace) = settings
            .workspaces
            .iter()
            .find(|item| item.name == active_name)
        {
            settings.current_workspace = workspace.id.clone();
        }
    }
    count
}

/// More repositories share a narrower fixed tab. This is stored, not measured from the window.
fn workspace_tab_width(count: usize) -> u32 {
    match count {
        0 | 1 => 220,
        2 => 200,
        3 | 4 => 168,
        5..=8 => 140,
        9..=12 => 112,
        _ => 96,
    }
}

fn assign_tab_widths(settings: &mut Settings) -> bool {
    let mut changed = false;
    for workspace in &mut settings.workspaces {
        let count = if workspace.repositories.is_empty() {
            workspace.open_tabs.len()
        } else {
            workspace.repositories.len()
        };
        let width = workspace_tab_width(count);
        if workspace.tab_width != width {
            workspace.tab_width = width;
            changed = true;
        }
    }
    changed
}

/// Fork stores every workspace repository as an open tab and one of them as the active tab.
fn open_workspace(repos: &[String], active: &str) -> (Vec<String>, u32) {
    let selected = repos
        .iter()
        .position(|path| same_path(path, active))
        .unwrap_or(0) as u32;
    (repos.to_vec(), selected)
}

/// The first import kept only Fork's active repository in `open_tabs`.
fn repair_imported_tabs(settings: &mut Settings) -> bool {
    let mut changed = false;
    for workspace in &mut settings.workspaces {
        if workspace.tabs_seeded || !workspace.id.starts_with("fork-") {
            continue;
        }
        if workspace.open_tabs.len() < workspace.repositories.len() {
            let active = workspace.open_tabs.first().cloned().unwrap_or_default();
            let (tabs, selected) = open_workspace(&workspace.repositories, &active);
            workspace.open_tabs = tabs;
            workspace.selected_tab = selected;
        }
        workspace.tabs_seeded = true;
        changed = true;
    }
    changed
}

fn string_list(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|item| {
                    item.as_str()
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn unique_paths(paths: &[String]) -> u32 {
    merge_paths(paths, &[]).len() as u32
}

fn merge_paths(primary: &[String], extra: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for path in primary.iter().chain(extra.iter()) {
        let path = path.trim();
        if path.is_empty() || out.iter().any(|item| same_path(item, path)) {
            continue;
        }
        out.push(path.to_string());
    }
    out
}

fn same_path(left: &str, right: &str) -> bool {
    let norm = |value: &str| {
        value
            .trim()
            .trim_end_matches(['\\', '/'])
            .replace('/', "\\")
            .to_ascii_lowercase()
    };
    !left.trim().is_empty() && norm(left) == norm(right)
}

fn fresh_workspace_id(settings: &Settings, name: &str) -> String {
    let base = format!("fork-{}", slug(name));
    if !settings
        .workspaces
        .iter()
        .any(|workspace| workspace.id == base)
    {
        return base;
    }
    let mut index = 2u32;
    loop {
        let id = format!("{base}-{index}");
        if !settings
            .workspaces
            .iter()
            .any(|workspace| workspace.id == id)
        {
            return id;
        }
        index += 1;
    }
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "ws".into()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (serde_json::Value, String) {
        let text = r#"{
          "FollowSystemTheme": false,
          "Theme": 1,
          "Pull_Rebase": true,
          "CommitSpellCheckingMode": 0,
          "ReferenceSpaceCharacterReplacement": "_",
          "CodeEditorFontSize": 15.0,
          "FileListMode": 1,
          "HistoryDiffLayoutMode": 1,
          "AutomaticStatusUpdateInterval": 60,
          "MergeTool": {
            "Type": "Custom",
            "ApplicationPath": "C:/Tools/merge.exe",
            "Arguments": "$LOCAL,$BASE,$REMOTE,$MERGED"
          },
          "ExternalDiffTool": {
            "Type": "Custom",
            "ApplicationPath": "C:/Tools/diff.exe",
            "Arguments": "\"$REMOTE\" \"$LOCAL\""
          },
          "RepositoryManager": {
            "SourceDirectories": ["D:/work"],
            "ScanDepth": 4,
            "Repositories": [
              { "Name": "one", "Path": "D:/work/one", "LastAccessTime": "2026-01-02T00:00:00" },
              { "Name": "two", "Path": "D:/work/two", "LastAccessTime": "2026-01-01T00:00:00" }
            ]
          },
          "Workspaces": {
            "All": [
              {
                "Name": "Blog",
                "Repositories": ["D:/work/one", "D:/work/two"],
                "ActiveRepository": "D:/work/two"
              },
              {
                "Name": "Home",
                "Repositories": ["D:/work/one"],
                "ActiveRepository": "D:/work/one"
              }
            ],
            "ActiveWorkspace": "Blog",
            "ShowInTitle": true
          }
        }"#;
        (serde_json::from_str(text).unwrap(), text.to_string())
    }

    #[test]
    fn missing_commit_type_stays_feat() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.commit_type, "feat");
    }

    #[test]
    fn operation_observer_defaults_on_and_keeps_an_explicit_opt_out() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert!(settings.flow_auto_open);
        assert!(Settings::default().flow_auto_open);
        let settings: Settings = serde_json::from_str(r#"{"flowAutoOpen":false}"#).unwrap();
        let saved = serde_json::to_value(&settings).unwrap();
        assert_eq!(saved["flowAutoOpen"], false);
        assert!(!serde_json::from_value::<Settings>(saved).unwrap().flow_auto_open);
    }

    #[test]
    fn operation_observer_auto_close_defaults_off_and_keeps_an_explicit_choice() {
        let settings: Settings = serde_json::from_str(r#"{"flowAutoOpen":true}"#).unwrap();
        assert!(!settings.flow_auto_close);
        assert!(!Settings::default().flow_auto_close);
        for enabled in [false, true] {
            let settings: Settings = serde_json::from_value(serde_json::json!({
                "flowAutoClose": enabled,
            })).unwrap();
            let saved = serde_json::to_value(&settings).unwrap();
            assert_eq!(saved["flowAutoClose"], enabled);
            assert_eq!(serde_json::from_value::<Settings>(saved).unwrap().flow_auto_close, enabled);
        }
    }

    #[test]
    fn missing_ai_language_defaults_to_chinese() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.ai_language, "zh");
    }

    #[test]
    fn settings_keep_custom_commit_types() {
        let settings: Settings = serde_json::from_str(
            r##"{"commitTypes":[{"name":"ui","color":"#123456","label":"界面"},{"name":"feat"}]}"##,
        )
        .unwrap();
        assert_eq!(settings.commit_types.len(), 2);
        assert_eq!(settings.commit_types[0].name, "ui");
        assert_eq!(settings.commit_types[0].color, "#123456");
        assert_eq!(settings.commit_types[0].label, "界面");
        assert_eq!(settings.commit_types[1].name, "feat");
        assert!(settings.commit_types[1].label.is_empty());
        assert!(Settings::default().commit_types.is_empty());
    }

    #[test]
    fn missing_silent_install_defaults_on_and_keeps_an_explicit_opt_out() {
        assert!(serde_json::from_str::<Settings>("{}").unwrap().silent_install);
        assert!(Settings::default().silent_install);
        let settings: Settings = serde_json::from_str(r#"{"silentInstall":false}"#).unwrap();
        let saved = serde_json::to_value(&settings).unwrap();
        assert_eq!(saved["silentInstall"], false);
        assert!(!serde_json::from_value::<Settings>(saved).unwrap().silent_install);
    }

    #[test]
    fn missing_features_intro_counts_as_seen() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.features_intro, "seen");
    }

    #[test]
    fn maps_fork_settings_without_touching_secrets() {
        let mut settings = Settings::default();
        settings.github_token = "secret".into();
        settings.ai_api_key = "key".into();
        settings.window_width = 1400;
        settings.commands = vec![CommandRecord {
            id: "cmd-1".into(),
            name: "Open".into(),
            target: "repository".into(),
            command: "echo hi".into(),
            prompt: String::new(),
            shared: true,
            repo: String::new(),
        }];
        settings.recent = vec!["D:/keep".into()];
        settings.workspaces = vec![WorkspaceRecord {
            id: "ws-1".into(),
            name: "Blog".into(),
            repositories: vec!["D:/work/old".into()],
            open_tabs: vec!["D:/work/old".into()],
            selected_tab: 0,
            tabs_seeded: false,
            tab_width: 0,
        }];
        let (value, text) = sample();
        let (preferences, repositories, workspaces) = apply_fork(&mut settings, &value, &text);
        assert!(preferences >= 8, "{preferences}");
        assert_eq!(repositories, 2);
        assert_eq!(workspaces, 2);
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.diff_font_size, 15);
        assert_eq!(settings.branch_space, "_");
        assert_eq!(settings.spell_checking, "disable");
        assert_eq!(settings.file_layout, "tree");
        assert!(settings.tree_files);
        assert_eq!(settings.diff_style, "split");
        assert_eq!(settings.status_interval, 60);
        assert_eq!(settings.scan_depth, 4);
        assert!(settings.pull_rebase);
        assert!(settings.workspace_show_title);
        assert_eq!(settings.merge_tool_args, "{ours},{base},{theirs},{working}");
        assert!(settings.diff_tool_args.find("{right}") < settings.diff_tool_args.find("{left}"));
        assert_eq!(settings.github_token, "secret");
        assert_eq!(settings.ai_api_key, "key");
        assert_eq!(settings.window_width, 1400);
        assert_eq!(settings.commands.len(), 1);
        assert_eq!(settings.workspaces.len(), 2);
        assert_eq!(settings.workspaces[0].id, "ws-1");
        assert_eq!(
            settings.workspaces[0].open_tabs,
            vec![
                "D:/work/one".to_string(),
                "D:/work/two".to_string(),
                "D:/work/old".to_string()
            ]
        );
        assert_eq!(settings.workspaces[0].selected_tab, 1);
        assert!(settings.workspaces[0].tabs_seeded);
        assert_eq!(settings.workspaces[0].tab_width, 168);
        assert_eq!(settings.workspaces[1].tab_width, 220);
        assert!(settings.workspaces[0]
            .repositories
            .iter()
            .any(|path| path.ends_with("old")));
        assert!(settings.workspaces[0]
            .repositories
            .iter()
            .any(|path| path.ends_with("two")));
        assert_eq!(
            settings.workspaces[1].open_tabs,
            vec!["D:/work/one".to_string()]
        );
        assert_eq!(settings.current_workspace, "ws-1");
        assert!(settings.recent.iter().any(|path| path.ends_with("one")));
        assert!(settings.recent.iter().any(|path| path == "D:/keep"));
        assert!(settings
            .source_directories
            .iter()
            .any(|path| same_path(path, "D:/work")));
        let again = apply_fork(&mut settings, &value, &text);
        assert_eq!(again.2, 2);
        assert_eq!(settings.workspaces.len(), 2);
        assert_eq!(settings.workspaces[0].open_tabs.len(), 3);
    }

    #[test]
    fn repairs_imported_workspace_that_only_kept_the_active_tab() {
        let mut settings = Settings::default();
        settings.workspaces = vec![
            WorkspaceRecord {
                id: "fork-blog".into(),
                name: "Blog".into(),
                repositories: vec!["D:/a".into(), "D:/b".into()],
                open_tabs: vec!["D:/b".into()],
                selected_tab: 0,
                tabs_seeded: false,
                tab_width: 0,
            },
            WorkspaceRecord {
                id: "ws-1".into(),
                name: "work".into(),
                repositories: vec!["D:/a".into(), "D:/b".into()],
                open_tabs: vec!["D:/a".into()],
                selected_tab: 0,
                tabs_seeded: false,
                tab_width: 0,
            },
        ];
        assert!(repair_imported_tabs(&mut settings));
        assert_eq!(
            settings.workspaces[0].open_tabs,
            vec!["D:/a".to_string(), "D:/b".to_string()]
        );
        assert_eq!(settings.workspaces[0].selected_tab, 1);
        assert!(settings.workspaces[0].tabs_seeded);
        assert_eq!(settings.workspaces[1].open_tabs, vec!["D:/a".to_string()]);
        settings.workspaces[0].open_tabs = vec!["D:/a".into()];
        assert!(!repair_imported_tabs(&mut settings));
        assert_eq!(settings.workspaces[0].open_tabs, vec!["D:/a".to_string()]);
    }

    #[test]
    fn follow_system_theme_wins() {
        let mut settings = Settings::default();
        apply_fork(
            &mut settings,
            &serde_json::json!({"FollowSystemTheme": true, "Theme": 1}),
            "{}",
        );
        assert_eq!(settings.theme, "system");
    }

    struct TempSettings {
        dir: std::path::PathBuf,
        path: std::path::PathBuf,
        previous_secrets: Option<std::ffi::OsString>,
    }

    impl TempSettings {
        fn new() -> Self {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let dir = std::env::temp_dir().join(format!("awegit-settings-{nanos}-{}", std::process::id()));
            let secrets = dir.join("secrets");
            std::fs::create_dir_all(&secrets).unwrap();
            let previous_secrets = std::env::var_os("AWEGIT_SECRET_DIR");
            unsafe { std::env::set_var("AWEGIT_SECRET_DIR", &secrets) };
            let path = dir.join("settings.json");
            Self { dir, path, previous_secrets }
        }
    }

    impl Drop for TempSettings {
        fn drop(&mut self) {
            match self.previous_secrets.take() {
                Some(value) => unsafe { std::env::set_var("AWEGIT_SECRET_DIR", value) },
                None => unsafe { std::env::remove_var("AWEGIT_SECRET_DIR") },
            }
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn sample_settings() -> Settings {
        let mut settings = Settings::default();
        settings.theme = "dark".into();
        settings.github_token = "secret".into();
        settings.commit_type = "fix".into();
        settings
    }

    #[test]
    fn appearance_defaults_to_apple_and_preserves_saved_choices() {
        let dir = TempSettings::new();
        assert_eq!(load_at(&dir.path).unwrap().theme, "apple");
        for theme in ["apple", "light", "dark", "system"] {
            let mut settings = Settings::default();
            settings.theme = theme.into();
            save_at(&dir.path, &settings).unwrap();
            assert_eq!(load_at(&dir.path).unwrap().theme, theme);
        }
    }

    #[test]
    fn settings_interrupted_temp_write_keeps_the_last_good_file() {
        let dir = TempSettings::new();
        let settings = sample_settings();
        save_at(&dir.path, &settings).unwrap();
        std::fs::write(sibling(&dir.path, ".tmp"), b"{").unwrap();
        let loaded = load_at(&dir.path).unwrap();
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.github_token, "secret");
        assert!(loaded.recovery_notice.is_empty());
    }

    #[test]
    fn settings_empty_and_invalid_files_restore_the_backup() {
        let dir = TempSettings::new();
        save_at(&dir.path, &sample_settings()).unwrap();
        assert!(sibling(&dir.path, ".bak").is_file());

        std::fs::write(&dir.path, b"").unwrap();
        let loaded = load_at(&dir.path).unwrap();
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.github_token, "secret");
        assert_eq!(loaded.recovery_notice, "recovered-backup");
        assert!(std::fs::read_to_string(&dir.path).unwrap().contains("dark"));
        assert!(!std::fs::read_to_string(&dir.path).unwrap().contains("recoveryNotice"));

        std::fs::write(&dir.path, b"{nope").unwrap();
        let loaded = load_at(&dir.path).unwrap();
        assert_eq!(loaded.commit_type, "fix");
        assert_eq!(loaded.recovery_notice, "recovered-backup");
    }

    #[test]
    fn settings_old_fields_survive_and_gain_a_schema() {
        let dir = TempSettings::new();
        std::fs::write(&dir.path, r#"{"theme":"dark","githubToken":"abc","commitType":"fix"}"#).unwrap();
        let loaded = load_at(&dir.path).unwrap();
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.github_token, "abc");
        assert_eq!(loaded.commit_type, "fix");
        assert_eq!(loaded.schema, SETTINGS_SCHEMA);
        assert!(loaded.recovery_notice.is_empty());
        let disk = std::fs::read_to_string(&dir.path).unwrap();
        assert!(disk.contains("\"schema\": 1") || disk.contains("\"schema\":1"), "{disk}");
        assert!(!disk.contains("abc"), "{disk}");
        let again = load_at(&dir.path).unwrap();
        assert!(again.recovery_notice.is_empty());
        assert_eq!(again.github_token, "abc");
    }

    #[test]
    fn settings_corrupt_file_without_a_backup_is_left_in_place() {
        let dir = TempSettings::new();
        std::fs::write(&dir.path, b"[]").unwrap();
        let error = load_at(&dir.path).unwrap_err();
        assert!(error.contains("备份"), "{error}");
        assert_eq!(std::fs::read(&dir.path).unwrap(), b"[]");
    }

    #[test]
    fn settings_newer_schema_is_not_overwritten() {
        let dir = TempSettings::new();
        let original = r#"{"schema":99,"theme":"dark","githubToken":"keep"}"#;
        std::fs::write(&dir.path, original).unwrap();
        let error = load_at(&dir.path).unwrap_err();
        assert!(error.contains("99"), "{error}");
        assert_eq!(std::fs::read_to_string(&dir.path).unwrap(), original);
        let mut settings = sample_settings();
        settings.schema = 1;
        let error = save_at(&dir.path, &settings).unwrap_err();
        assert!(error.contains("99"), "{error}");
        assert_eq!(std::fs::read_to_string(&dir.path).unwrap(), original);
    }

    #[test]
    fn settings_save_does_not_persist_the_recovery_notice() {
        let dir = TempSettings::new();
        let mut settings = sample_settings();
        settings.recovery_notice = "recovered-backup".into();
        save_at(&dir.path, &settings).unwrap();
        let text = std::fs::read_to_string(&dir.path).unwrap();
        assert!(!text.contains("recoveryNotice"), "{text}");
        assert!(!text.contains("secret"), "{text}");
        assert_eq!(load_at(&dir.path).unwrap().github_token, "secret");
    }

    #[test]
    fn settings_account_tokens_leave_the_file_and_a_clear_removes_the_vault_entry() {
        let dir = TempSettings::new();
        let mut settings = sample_settings();
        settings.proxy_password = "proxy-secret".into();
        settings.ai_api_key = "ai-secret".into();
        settings.accounts.push(Account {
            id: "gh-1".into(),
            forge: "github".into(),
            label: "me".into(),
            token: "acct-secret".into(),
            host: String::new(),
            client_id: String::new(),
        });
        save_at(&dir.path, &settings).unwrap();
        let disk = std::fs::read_to_string(&dir.path).unwrap();
        assert!(!disk.contains("secret"), "{disk}");
        let loaded = load_at(&dir.path).unwrap();
        assert_eq!(loaded.github_token, "secret");
        assert_eq!(loaded.proxy_password, "proxy-secret");
        assert_eq!(loaded.ai_api_key, "ai-secret");
        assert_eq!(loaded.accounts[0].token, "acct-secret");
        let mut cleared = loaded;
        cleared.github_token.clear();
        cleared.accounts[0].token.clear();
        save_at(&dir.path, &cleared).unwrap();
        let again = load_at(&dir.path).unwrap();
        assert!(again.github_token.is_empty());
        assert!(again.accounts[0].token.is_empty());
        assert_eq!(again.proxy_password, "proxy-secret");
        assert_eq!(again.ai_api_key, "ai-secret");
    }
}
