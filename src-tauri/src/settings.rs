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
    /// Pull stashes local edits and puts them back. Separate from merge autostash.
    #[serde(default)]
    pub pull_stash: bool,
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
    pub rebase_autostash: bool,
    #[serde(default)]
    pub rebase_update_refs: bool,
    /// Applying a stash drops it afterwards. Pop already drops the stash.
    #[serde(default)]
    pub apply_stash_delete: bool,
    #[serde(default)]
    pub push_tag_on_create: bool,
    /// A dirty checkout stashes, switches, then restores. A conflict keeps the stash notice.
    #[serde(default)]
    pub checkout_stash: bool,
    #[serde(default)]
    pub workspace_show_title: bool,
    /// How deep a source folder is walked when looking for repositories.
    #[serde(default = "scan_depth_default")]
    pub scan_depth: u32,
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
fn diff_context_default() -> u32 {
    3
}
fn max_commits_default() -> u32 {
    500
}
fn scan_depth_default() -> u32 {
    5
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
            fake_ignore: true,
            tab_labels: Vec::new(),
            tab_colors: Vec::new(),
            accounts: Vec::new(),
            bitbucket_token: String::new(),
            azure_token: String::new(),
            azure_org: String::new(),
            oauth_client_id: String::new(),
            fetch_all_remotes: false,
            pull_stash: false,
            push_all_tags: false,
            checkout_after_create: true,
            stash_include_untracked: false,
            diff_word_wrap: false,
            diff_context: diff_context_default(),
            status_interval: 0,
            show_worktrees: true,
            max_commits: max_commits_default(),
            rebase_autostash: false,
            rebase_update_refs: false,
            apply_stash_delete: false,
            push_tag_on_create: false,
            checkout_stash: false,
            workspace_show_title: false,
            scan_depth: scan_depth_default(),
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
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
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
pub fn apply_fork(settings: &mut Settings, fork: &serde_json::Value, text: &str) -> (u32, u32, u32) {
    let mut preferences = 0u32;
    take_bool(fork, "Pull_Rebase", &mut settings.pull_rebase, &mut preferences);
    take_bool(fork, "FetchRemotesAutomatically", &mut settings.fetch_automatically, &mut preferences);
    take_bool(fork, "FetchAllTags", &mut settings.fetch_tags, &mut preferences);
    take_bool(fork, "PushAutomaticallyOnCommit", &mut settings.push_on_commit, &mut preferences);
    take_bool(fork, "UpdateSubmodulesOnCheckout", &mut settings.update_submodules_on_checkout, &mut preferences);
    take_bool(fork, "DiffIgnoreWhitespaces", &mut settings.ignore_space, &mut preferences);
    take_bool(fork, "DiffShowHiddenSymbols", &mut settings.show_whitespace, &mut preferences);
    take_bool(fork, "DiffShowChangeMarks", &mut settings.show_diff_marks, &mut preferences);
    take_bool(fork, "DiffShowEntireFile", &mut settings.show_entire_file, &mut preferences);
    take_bool(fork, "DiffWordWrap", &mut settings.diff_word_wrap, &mut preferences);
    take_bool(fork, "CompactBranchLabels", &mut settings.compact_branch_labels, &mut preferences);
    take_bool(fork, "DisableSyntaxHighlighting", &mut settings.disable_syntax_highlight, &mut preferences);
    take_bool(fork, "Fetch_FetchAllRemotes", &mut settings.fetch_all_remotes, &mut preferences);
    take_bool(fork, "Pull_StashAndReapply", &mut settings.pull_stash, &mut preferences);
    take_bool(fork, "Push_PushAllTags", &mut settings.push_all_tags, &mut preferences);
    take_bool(fork, "CreateBranch_Checkout", &mut settings.checkout_after_create, &mut preferences);
    take_bool(fork, "SaveStash_StageNewFiles", &mut settings.stash_include_untracked, &mut preferences);
    take_bool(fork, "ApplyStash_DeleteAfterApply", &mut settings.apply_stash_delete, &mut preferences);
    take_bool(fork, "CreateTag_Push", &mut settings.push_tag_on_create, &mut preferences);
    take_bool(fork, "Checkout_StashAndReapply", &mut settings.checkout_stash, &mut preferences);
    take_bool(fork, "ShowWorktrees", &mut settings.show_worktrees, &mut preferences);
    take_bool(fork, "RebaseAutostash", &mut settings.rebase_autostash, &mut preferences);
    take_bool(fork, "RebaseUpdateRefs", &mut settings.rebase_update_refs, &mut preferences);
    take_u32(fork, "CodeEditorFontSize", &mut settings.diff_font_size, &mut preferences, 1);
    take_u32(fork, "CommitSubjectLowLimit", &mut settings.message_low, &mut preferences, 1);
    take_u32(fork, "CommitSubjectHighLimit", &mut settings.message_high, &mut preferences, 1);
    take_u32(fork, "PageGuideLinePosition", &mut settings.page_guide, &mut preferences, 1);
    take_u32(fork, "DiffContextSize", &mut settings.diff_context, &mut preferences, 0);
    take_u32(fork, "AutomaticStatusUpdateInterval", &mut settings.status_interval, &mut preferences, 0);
    take_u32(fork, "MaxCommitCount", &mut settings.max_commits, &mut preferences, 1);
    if let Some(value) = fork.get("ReferenceSpaceCharacterReplacement").and_then(serde_json::Value::as_str) {
        settings.branch_space = value.to_string();
        preferences += 1;
    }
    if let Some(mode) = fork.get("CommitSpellCheckingMode").and_then(serde_json::Value::as_i64) {
        settings.spell_checking = if mode == 0 { "disable" } else { "enable" }.into();
        preferences += 1;
    }
    if fork.get("FollowSystemTheme").and_then(serde_json::Value::as_bool) == Some(true) {
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
    if let Some(mode) = ["HistoryDiffLayoutMode", "CommitDiffLayoutMode", "RevisionDiffLayoutMode"]
        .into_iter()
        .find_map(|key| fork.get(key).and_then(serde_json::Value::as_i64))
    {
        settings.diff_style = if mode == 0 { "unified" } else { "split" }.into();
        preferences += 1;
    }
    if let Some(depth) = fork.pointer("/RepositoryManager/ScanDepth").and_then(serde_json::Value::as_u64) {
        if depth > 0 {
            settings.scan_depth = depth.min(u32::MAX as u64) as u32;
            preferences += 1;
        }
    }
    if let Some(show) = fork.pointer("/Workspaces/ShowInTitle").and_then(serde_json::Value::as_bool) {
        settings.workspace_show_title = show;
        preferences += 1;
    }
    apply_tools(settings, text, &mut preferences);
    let repositories = merge_repositories(settings, fork);
    let workspaces = merge_workspaces(settings, fork);
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
    let Some(number) = value.as_u64().or_else(|| value.as_f64().map(|item| item as u64)) else { return };
    if number < minimum {
        return;
    }
    *dest = number.min(u32::MAX as u64) as u32;
    *count += 1;
}

fn apply_tools(settings: &mut Settings, text: &str, count: &mut u32) {
    let tools = crate::tools::tools_in_fork_text(text);
    if let Some(tool) = tools.merge.iter().find(|item| item.primary).or(tools.merge.first()) {
        settings.merge_tool_name = tool.name.clone();
        settings.merge_tool_path = tool.path.clone();
        settings.merge_tool_args = tool.args.clone();
        settings.merge_tool = compose_tool(&tool.path, &tool.args);
        *count += 1;
    }
    if let Some(tool) = tools.diff.iter().find(|item| item.primary).or(tools.diff.first()) {
        settings.diff_tool_name = tool.name.clone();
        settings.diff_tool_path = tool.path.clone();
        settings.diff_tool_args = tool.args.clone();
        settings.diff_tool = compose_tool(&tool.path, &tool.args);
        *count += 1;
    }
}

fn compose_tool(path: &str, args: &str) -> String {
    let file = path.trim();
    let rest = args.trim();
    if file.is_empty() {
        return rest.to_string();
    }
    let quoted = if file.contains(' ') { format!("\"{file}\"") } else { file.to_string() };
    if rest.is_empty() { quoted } else { format!("{quoted} {rest}") }
}

fn merge_repositories(settings: &mut Settings, fork: &serde_json::Value) -> u32 {
    let Some(rows) = fork.pointer("/RepositoryManager/Repositories").and_then(serde_json::Value::as_array) else {
        return 0;
    };
    let mut ordered: Vec<(String, String)> = rows
        .iter()
        .filter_map(|item| {
            let path = item.get("Path").and_then(serde_json::Value::as_str)?.trim().to_string();
            if path.is_empty() {
                return None;
            }
            let time = item.get("LastAccessTime").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
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
    let Some(rows) = fork.pointer("/Workspaces/All").and_then(serde_json::Value::as_array) else {
        return 0;
    };
    let mut count = 0u32;
    for item in rows {
        let Some(name) = item.get("Name").and_then(serde_json::Value::as_str).map(str::trim).filter(|name| !name.is_empty()) else {
            continue;
        };
        let repos = string_list(item.get("Repositories"));
        let active = item.get("ActiveRepository").and_then(serde_json::Value::as_str).unwrap_or("").trim().to_string();
        if let Some(existing) = settings.workspaces.iter_mut().find(|workspace| workspace.name == name) {
            existing.repositories = merge_paths(&repos, &existing.repositories);
            if existing.open_tabs.is_empty() && !active.is_empty() {
                existing.open_tabs = vec![active];
                existing.selected_tab = 0;
            }
        } else {
            let id = fresh_workspace_id(settings, name);
            let open_tabs = if active.is_empty() { Vec::new() } else { vec![active] };
            settings.workspaces.push(WorkspaceRecord {
                id,
                name: name.to_string(),
                repositories: repos,
                open_tabs,
                selected_tab: 0,
            });
        }
        count += 1;
    }
    if let Some(active_name) = fork.pointer("/Workspaces/ActiveWorkspace").and_then(serde_json::Value::as_str) {
        if let Some(workspace) = settings.workspaces.iter().find(|item| item.name == active_name) {
            settings.current_workspace = workspace.id.clone();
        }
    }
    count
}

fn string_list(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .map(|rows| rows.iter().filter_map(|item| item.as_str().map(str::trim).filter(|text| !text.is_empty()).map(str::to_string)).collect())
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
    let norm = |value: &str| value.trim().trim_end_matches(['\\', '/']).replace('/', "\\").to_ascii_lowercase();
    !left.trim().is_empty() && norm(left) == norm(right)
}

fn fresh_workspace_id(settings: &Settings, name: &str) -> String {
    let base = format!("fork-{}", slug(name));
    if !settings.workspaces.iter().any(|workspace| workspace.id == base) {
        return base;
    }
    let mut index = 2u32;
    loop {
        let id = format!("{base}-{index}");
        if !settings.workspaces.iter().any(|workspace| workspace.id == id) {
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
    if out.is_empty() { "ws".into() } else { out }
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
        assert_eq!(settings.workspaces[0].open_tabs, vec!["D:/work/old".to_string()]);
        assert!(settings.workspaces[0].repositories.iter().any(|path| path.ends_with("old")));
        assert!(settings.workspaces[0].repositories.iter().any(|path| path.ends_with("two")));
        assert_eq!(settings.workspaces[1].open_tabs, vec!["D:/work/one".to_string()]);
        assert_eq!(settings.current_workspace, "ws-1");
        assert!(settings.recent.iter().any(|path| path.ends_with("one")));
        assert!(settings.recent.iter().any(|path| path == "D:/keep"));
        assert!(settings.source_directories.iter().any(|path| same_path(path, "D:/work")));
        let again = apply_fork(&mut settings, &value, &text);
        assert_eq!(again.2, 2);
        assert_eq!(settings.workspaces.len(), 2);
    }

    #[test]
    fn follow_system_theme_wins() {
        let mut settings = Settings::default();
        apply_fork(&mut settings, &serde_json::json!({"FollowSystemTheme": true, "Theme": 1}), "{}");
        assert_eq!(settings.theme, "system");
    }
}
