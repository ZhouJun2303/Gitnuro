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
            sign_commits: false,
            merge_no_ff: false,
            merge_autostash: false,
            clone_directory: String::new(),
            window_x: 0,
            window_y: 0,
            window_width: 0,
            window_height: 0,
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
