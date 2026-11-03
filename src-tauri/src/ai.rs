//! Commit-message suggestion. The key stays in the local settings file, `XAI_API_KEY`, or `OPENAI_API_KEY`.

use serde::Serialize;

use crate::settings::Settings;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub summary: String,
    pub description: String,
}

pub fn suggest(repo: &std::path::Path, settings: &Settings) -> Result<Suggestion, String> {
    if !settings.ai_enabled {
        return Err("AI commit messages are turned off.".into());
    }
    let (files, branch, recent) = awegit_git::prompt_facts(repo);
    if files.trim().is_empty() {
        return Err("Stage changes before asking for a message.".into());
    }
    let mut diff = awegit_git::staged_diff(repo).map_err(|error| error.to_string())?;
    if diff.trim().is_empty() {
        return Err("Stage changes before asking for a message.".into());
    }
    let limit = if settings.ai_max_chars == 0 { 12_000 } else { settings.ai_max_chars as usize };
    if diff.len() > limit {
        let mut end = limit;
        while end > 0 && !diff.is_char_boundary(end) {
            end -= 1;
        }
        diff.truncate(end);
        diff.push_str("\n\n[diff truncated]");
    }
    let key = if settings.ai_api_key.trim().is_empty() {
        std::env::var("XAI_API_KEY")
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .unwrap_or_default()
    } else {
        settings.ai_api_key.trim().to_string()
    };
    if key.is_empty() {
        return Err("Add an API key in Preferences, or set XAI_API_KEY or OPENAI_API_KEY.".into());
    }
    let base = settings.ai_base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err("Set an API base URL in Preferences.".into());
    }
    let model = if settings.ai_model.trim().is_empty() {
        "grok-4.7".to_string()
    } else {
        settings.ai_model.trim().to_string()
    };
    let language = if settings.ai_language.trim().is_empty() {
        "English".to_string()
    } else {
        settings.ai_language.trim().to_string()
    };
    let style = if settings.ai_prompt_style.trim().eq_ignore_ascii_case("long") {
        "long"
    } else {
        "short"
    };
    let prompt = compose_prompt(style, settings.ai_prompt.trim(), &language, &files, &branch, &recent, &diff);
    let xai = base.contains("api.x.ai");
    let (url, mut body) = if xai {
        let body = serde_json::json!({ "model": model, "input": prompt });
        (format!("{base}/responses"), body)
    } else {
        let body = serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": "You write git commit messages. The first line must match type: description. type is feat, fix, docs, art, style, refactor, test, chore, perf, or revert. No scope. Reply with the message only." },
                { "role": "user", "content": prompt }
            ]
        });
        (format!("{base}/chat/completions"), body)
    };
    if settings.ai_temperature > 0.0 {
        body["temperature"] = serde_json::json!(settings.ai_temperature);
    }
    let payload = serde_json::to_string(&body).map_err(|error| error.to_string())?;
    let text = post_json(&url, &key, &payload)?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| text.clone())?;
    if let Some(message) = value.pointer("/error/message").and_then(|item| item.as_str()) {
        return Err(message.to_string());
    }
    let answer = extract_text(&value);
    if answer.trim().is_empty() {
        return Err("The model returned an empty message.".into());
    }
    parse_suggestion(&answer)
}

/// Review the diff of `HEAD` against `base` with the same model as commit messages.
pub fn review(repo: &std::path::Path, settings: &Settings, base: &str) -> Result<String, String> {
    if !settings.ai_enabled {
        return Err("AI commit messages are turned off.".into());
    }
    let base = if base.trim().is_empty() { "HEAD~1" } else { base.trim() };
    if base.contains(['\n', '\r', '\0']) || base.starts_with('-') {
        return Err("base revision is missing".into());
    }
    let output = awegit_git::range_diff(repo, base).map_err(|error| error.to_string())?;
    if output.trim().is_empty() {
        return Err("That range has no diff.".into());
    }
    let limit = if settings.ai_max_chars == 0 { 12_000 } else { settings.ai_max_chars as usize };
    let diff = if output.len() > limit { output[..limit].to_string() } else { output };
    let key = ai_key(settings)?;
    let base_url = settings.ai_base_url.trim().trim_end_matches('/').to_string();
    let model = if settings.ai_model.trim().is_empty() { "grok-4.7".to_string() } else { settings.ai_model.trim().to_string() };
    let prompt = format!("Review this git diff. List real bugs, risky changes, and missing tests. Be concise.\n\n{diff}");
    let body = if base_url.contains("api.x.ai") {
        serde_json::json!({ "model": model, "input": prompt })
    } else {
        serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": "You review git diffs. Mention concrete risks only." },
                { "role": "user", "content": prompt }
            ]
        })
    };
    let url = if base_url.contains("api.x.ai") { format!("{base_url}/responses") } else { format!("{base_url}/chat/completions") };
    let payload = serde_json::to_string(&body).map_err(|error| error.to_string())?;
    let text = post_json(&url, &key, &payload)?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| text.clone())?;
    if let Some(message) = value.pointer("/error/message").and_then(|item| item.as_str()) {
        return Err(message.to_string());
    }
    let answer = extract_text(&value);
    if answer.trim().is_empty() {
        return Err("The model returned an empty review.".into());
    }
    Ok(answer)
}

fn ai_key(settings: &Settings) -> Result<String, String> {
    let key = if settings.ai_api_key.trim().is_empty() {
        std::env::var("XAI_API_KEY").or_else(|_| std::env::var("OPENAI_API_KEY")).unwrap_or_default()
    } else {
        settings.ai_api_key.trim().to_string()
    };
    if key.is_empty() {
        return Err("Add an API key in Preferences, or set XAI_API_KEY or OPENAI_API_KEY.".into());
    }
    Ok(key)
}

fn post_json(url: &str, key: &str, payload: &str) -> Result<String, String> {
    let mut child = std::process::Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            "60",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-H",
            &format!("Authorization: Bearer {key}"),
            "--data-binary",
            "@-",
            url,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("curl is not available: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(payload.as_bytes()).map_err(|error| error.to_string())?;
    }
    let output = child.wait_with_output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() { "the model request failed".into() } else { stderr });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn extract_text(value: &serde_json::Value) -> String {
    if let Some(items) = value.get("output").and_then(|item| item.as_array()) {
        let mut parts = Vec::new();
        for item in items {
            if let Some(blocks) = item.get("content").and_then(|block| block.as_array()) {
                for block in blocks {
                    if block.get("type").and_then(|kind| kind.as_str()) == Some("output_text") {
                        if let Some(text) = block.get("text").and_then(|text| text.as_str()) {
                            parts.push(text.to_string());
                        }
                    }
                }
            }
        }
        if !parts.is_empty() {
            return parts.join("\n");
        }
    }
    value
        .pointer("/choices/0/message/content")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string()
}

const COMMIT_TYPES: [&str; 10] = [
    "feat", "fix", "docs", "art", "style", "refactor", "test", "chore", "perf", "revert",
];

fn compose_prompt(
    style: &str,
    custom: &str,
    language: &str,
    files: &str,
    branch: &str,
    recent: &str,
    diff: &str,
) -> String {
    let convention = format!(
        "Write in {language}. A commit-msg hook rejects any other first line. \
It must match ^(feat|fix|docs|art|style|refactor|test|chore|perf|revert):[[:space:]]*[^[:space:]].* \
Types: feat 新增功能; fix 修复 bug; docs 仅文档、注释或说明; art 美术资源; style 只改格式、不改逻辑; \
refactor 重构，不是新功能也不是修 bug; test 测试; chore 构建、工具、配置或流程; perf 性能; revert 回滚之前的提交. \
The type is lowercase and is followed immediately by a colon. A space after the colon is optional. \
Do not add a scope, issue id, quotes, or markdown. Example: feat: 新增挂机奖励结算逻辑"
    );
    let custom = custom.trim();
    if custom.contains("{diff}") || custom.contains("${diff}") {
        return fill(&format!("{convention}\n\n{custom}"), diff, files, branch, recent, language);
    }
    let instructions = if style == "long" {
        "Reply with the subject, a blank line, then a short body that says why this change exists. \
Do not list files. The subject is one line of at most 72 characters. \
Describe only the staged diff. If the diff is cut off, do not invent the missing part. \
Use style for formatting only, test when the change is tests, and docs when only comments or docs change."
    } else {
        "Reply with exactly one line: the subject. No body. At most 72 characters. \
Describe only the staged diff. Do not invent files or behavior that are not in the diff."
    };
    let extra = if custom.is_empty() {
        String::new()
    } else {
        format!("\nExtra instructions:\n{custom}")
    };
    let history = if style == "long" {
        format!("\nRecent subjects, for tone only:\n{recent}")
    } else {
        String::new()
    };
    fill(
        &format!(
            "{convention}\n\n{instructions}{extra}\nBranch: {{branch}}\nStaged files:\n{{files}}{history}\n\nStaged diff:\n{{diff}}"
        ),
        diff,
        files,
        branch,
        recent,
        language,
    )
}

fn fill(template: &str, diff: &str, files: &str, branch: &str, recent: &str, language: &str) -> String {
    template
        .replace("{diff}", diff)
        .replace("${diff}", diff)
        .replace("{files}", files)
        .replace("${files}", files)
        .replace("{branch}", branch)
        .replace("${branch}", branch)
        .replace("{recent_commits}", recent)
        .replace("${recent_commits}", recent)
        .replace("{language}", language)
        .replace("${language}", language)
}

fn skip_space(text: &str) -> &str {
    text.trim_start_matches([' ', '\t', '\u{000B}', '\u{000C}'])
}

fn valid_subject(line: &str) -> bool {
    let Some((kind, rest)) = line.split_once(':') else {
        return false;
    };
    COMMIT_TYPES.contains(&kind) && !skip_space(rest).is_empty()
}

fn clean_line(line: &str) -> String {
    let mut line = line.trim();
    if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
        line = rest.trim();
    }
    line.trim_matches(['"', '\'', '`', '*']).trim().to_string()
}

fn cap_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn parse_suggestion(text: &str) -> Result<Suggestion, String> {
    let mut lines: Vec<&str> = text.trim().lines().collect();
    if lines.first().is_some_and(|line| line.trim().starts_with("```")) {
        lines.remove(0);
        if lines.last().is_some_and(|line| line.trim().starts_with("```")) {
            lines.pop();
        }
    }
    let mut subject_at = None;
    for (index, line) in lines.iter().enumerate() {
        if valid_subject(&clean_line(line)) {
            subject_at = Some(index);
            break;
        }
    }
    let Some(index) = subject_at else {
        let sample = lines.iter().find(|line| !line.trim().is_empty()).copied().unwrap_or("");
        return Err(format!(
            "The model reply does not match the commit format: {}",
            cap_chars(sample.trim(), 80)
        ));
    };
    let summary = cap_chars(&clean_line(lines[index]), 200);
    if !valid_subject(&summary) {
        return Err(format!(
            "The model reply does not match the commit format: {}",
            cap_chars(&summary, 80)
        ));
    }
    let description = lines
        .iter()
        .skip(index + 1)
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    Ok(Suggestion { summary, description })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_prompt_keeps_the_hook_rule_and_staged_diff() {
        let prompt = compose_prompt("short", "", "中文", "src/app.ts", "main", "chore: old", "diff-body");
        assert!(prompt.contains("feat|fix|docs|art|style|refactor|test|chore|perf|revert"));
        assert!(prompt.contains("feat: 新增挂机奖励结算逻辑"));
        assert!(prompt.contains("exactly one line"));
        assert!(!prompt.contains("blank line"));
        assert!(prompt.contains("src/app.ts"));
        assert!(prompt.contains("diff-body"));
        assert!(!prompt.contains("chore: old"));
        assert!(!prompt.contains("UNSTAGED"));
    }

    #[test]
    fn long_prompt_asks_for_a_body_and_recent_subjects() {
        let prompt = compose_prompt("long", "", "中文", "src/app.ts", "main", "chore: old", "diff-body");
        assert!(prompt.contains("blank line"));
        assert!(prompt.contains("chore: old"));
        assert!(prompt.contains("diff-body"));
        assert!(prompt.contains("refactor 重构"));
    }

    #[test]
    fn custom_template_still_gets_the_hook_rule() {
        let prompt = compose_prompt("short", "Only this.\n{diff}", "English", "a.ts", "dev", "old", "DIFF");
        assert!(prompt.contains("commit-msg hook"));
        assert!(prompt.contains("Only this."));
        assert_eq!(prompt.matches("DIFF").count(), 1);
    }

    #[test]
    fn accepts_the_hook_examples() {
        let value = parse_suggestion("feat: 新增挂机奖励结算逻辑").unwrap();
        assert_eq!(value.summary, "feat: 新增挂机奖励结算逻辑");
        assert!(value.description.is_empty());
        let tight = parse_suggestion("fix:keep the composer visible\n\nThe pane collapsed.").unwrap();
        assert_eq!(tight.summary, "fix:keep the composer visible");
        assert_eq!(tight.description, "The pane collapsed.");
    }

    #[test]
    fn strips_fences_and_rejects_a_freeform_subject() {
        let value = parse_suggestion("```\n**refactor: 抽出提交提示词**\n\n避免漏掉钩子规则。\n```").unwrap();
        assert_eq!(value.summary, "refactor: 抽出提交提示词");
        assert!(value.description.contains("钩子"));
        let error = parse_suggestion("updated the button").unwrap_err();
        assert!(error.contains("does not match"));
        assert!(error.contains("updated the button"));
    }
}
