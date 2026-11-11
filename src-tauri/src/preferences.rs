//! Field updates for the preferences exposed beside their corresponding UI.
use serde::Deserialize;
use crate::settings::Settings;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreferencePatch {
    pub diff_style: Option<String>,
    pub lines_height: Option<String>,
    pub diff_font_size: Option<u32>,
    pub diff_context: Option<u32>,
    pub show_entire_file: Option<bool>,
    pub diff_word_wrap: Option<bool>,
    pub show_whitespace: Option<bool>,
    pub ignore_space: Option<bool>,
    pub show_diff_marks: Option<bool>,
    pub disable_syntax_highlight: Option<bool>,
    pub file_layout: Option<String>,
    pub tree_files: Option<bool>,
    pub swap_panes: Option<bool>,
    pub fake_ignore: Option<bool>,
    pub commit_sort: Option<String>,
    pub max_commits: Option<u32>,
    pub date_relative: Option<bool>,
    pub date24h: Option<bool>,
    pub compact_branch_labels: Option<bool>,
    pub gravatar: Option<bool>,
    pub highlight_issues: Option<bool>,
    pub push_on_commit: Option<bool>,
    pub sign_commits: Option<bool>,
    pub spell_checking: Option<String>,
    pub ai_prompt_style: Option<String>,
    pub ai_language: Option<String>,
    pub fetch_automatically: Option<bool>,
    pub fetch_all_remotes: Option<bool>,
    pub fetch_prune: Option<bool>,
    pub fetch_tags: Option<bool>,
    pub pull_rebase: Option<bool>,
    pub push_all_tags: Option<bool>,
    pub show_worktrees: Option<bool>,
    pub vertical_tabs: Option<bool>,
    pub tab_indicator: Option<bool>,
    pub flow_auto_open: Option<bool>,
    pub flow_auto_close: Option<bool>,
    pub flow_history_days: Option<u32>,
    pub error_log_days: Option<u32>,
}

impl PreferencePatch {
    pub fn apply(self, values: &mut Settings) -> Result<(), String> {
        if self.diff_style.as_deref().is_some_and(|value| !["split", "unified"].contains(&value)) { return Err("Invalid diff_style".into()); }
        if self.lines_height.as_deref().is_some_and(|value| !["compact", "spaced"].contains(&value)) { return Err("Invalid lines_height".into()); }
        if self.file_layout.as_deref().is_some_and(|value| !["tree", "list", "combined"].contains(&value)) { return Err("Invalid file_layout".into()); }
        if self.commit_sort.as_deref().is_some_and(|value| !["topo", "date"].contains(&value)) { return Err("Invalid commit_sort".into()); }
        if self.spell_checking.as_deref().is_some_and(|value| !["enable", "disable"].contains(&value)) { return Err("Invalid spell_checking".into()); }
        if self.ai_prompt_style.as_deref().is_some_and(|value| !["short", "long"].contains(&value)) { return Err("Invalid ai_prompt_style".into()); }
        if self.ai_language.as_deref().is_some_and(|value| !["zh", "en"].contains(&value)) { return Err("Invalid ai_language".into()); }
        if let Some(ref layout) = self.file_layout { values.tree_files = layout == "tree"; }
        else if let Some(tree) = self.tree_files { values.file_layout = if tree { "tree" } else { "list" }.into(); }
        if let Some(value) = self.diff_style { values.diff_style = value; }
        if let Some(value) = self.lines_height { values.lines_height = value; }
        if let Some(value) = self.diff_font_size { values.diff_font_size = value.clamp(8, 32); }
        if let Some(value) = self.diff_context { values.diff_context = value.clamp(0, 100); }
        if let Some(value) = self.show_entire_file { values.show_entire_file = value; }
        if let Some(value) = self.diff_word_wrap { values.diff_word_wrap = value; }
        if let Some(value) = self.show_whitespace { values.show_whitespace = value; }
        if let Some(value) = self.ignore_space { values.ignore_space = value; }
        if let Some(value) = self.show_diff_marks { values.show_diff_marks = value; }
        if let Some(value) = self.disable_syntax_highlight { values.disable_syntax_highlight = value; }
        if let Some(value) = self.file_layout { values.file_layout = value; }
        if let Some(value) = self.tree_files { values.tree_files = value; }
        if let Some(value) = self.swap_panes { values.swap_panes = value; }
        if let Some(value) = self.fake_ignore { values.fake_ignore = value; }
        if let Some(value) = self.commit_sort { values.commit_sort = value; }
        if let Some(value) = self.max_commits { values.max_commits = value.clamp(50, 100000); }
        if let Some(value) = self.date_relative { values.date_relative = value; }
        if let Some(value) = self.date24h { values.date_24h = value; }
        if let Some(value) = self.compact_branch_labels { values.compact_branch_labels = value; }
        if let Some(value) = self.gravatar { values.gravatar = value; }
        if let Some(value) = self.highlight_issues { values.highlight_issues = value; }
        if let Some(value) = self.push_on_commit { values.push_on_commit = value; }
        if let Some(value) = self.sign_commits { values.sign_commits = value; }
        if let Some(value) = self.spell_checking { values.spell_checking = value; }
        if let Some(value) = self.ai_prompt_style { values.ai_prompt_style = value; }
        if let Some(value) = self.ai_language { values.ai_language = value; }
        if let Some(value) = self.fetch_automatically { values.fetch_automatically = value; }
        if let Some(value) = self.fetch_all_remotes { values.fetch_all_remotes = value; }
        if let Some(value) = self.fetch_prune { values.fetch_prune = value; }
        if let Some(value) = self.fetch_tags { values.fetch_tags = value; }
        if let Some(value) = self.pull_rebase { values.pull_rebase = value; }
        if let Some(value) = self.push_all_tags { values.push_all_tags = value; }
        if let Some(value) = self.show_worktrees { values.show_worktrees = value; }
        if let Some(value) = self.vertical_tabs { values.vertical_tabs = value; }
        if let Some(value) = self.tab_indicator { values.tab_indicator = value; }
        if let Some(value) = self.flow_auto_open { values.flow_auto_open = value; }
        if let Some(value) = self.flow_auto_close { values.flow_auto_close = value; }
        if let Some(value) = self.flow_history_days { values.flow_history_days = value.clamp(1, 90); }
        if let Some(value) = self.error_log_days { values.error_log_days = value.clamp(1, 90); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_patches_merge_without_replacing_other_windows_or_credentials() {
        let mut settings = Settings::default();
        settings.ai_api_key = "retained-in-memory".into();
        settings.recent = vec!["D:/example".into()];
        let first: PreferencePatch = serde_json::from_str(r#"{"diffWordWrap":true,"fileLayout":"list"}"#).unwrap();
        let second: PreferencePatch = serde_json::from_str(r#"{"flowAutoClose":true,"diffFontSize":100}"#).unwrap();
        first.apply(&mut settings).unwrap();
        second.apply(&mut settings).unwrap();
        assert!(settings.diff_word_wrap);
        assert!(!settings.tree_files);
        assert!(settings.flow_auto_close);
        assert_eq!(settings.diff_font_size, 32);
        assert_eq!(settings.ai_api_key, "retained-in-memory");
        assert_eq!(settings.recent, vec!["D:/example"]);
    }

    #[test]
    fn preference_patches_reject_unsupported_fields_and_invalid_choices() {
        assert!(serde_json::from_str::<PreferencePatch>(r#"{"aiApiKey":"unexpected"}"#).is_err());
        assert!(serde_json::from_str::<PreferencePatch>(r#"{"diffWordWrap":"yes"}"#).is_err());
        let mut settings = Settings::default();
        let invalid: PreferencePatch = serde_json::from_str(r#"{"diffStyle":"invalid","diffFontSize":20}"#).unwrap();
        assert!(invalid.apply(&mut settings).is_err());
        assert_eq!(settings.diff_font_size, Settings::default().diff_font_size);
    }
}
