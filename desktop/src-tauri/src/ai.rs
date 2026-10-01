//! Commit-message suggestion. The key stays in the local settings file or `XAI_API_KEY`.

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
    let mut diff = awegit_git::staged_diff(repo).map_err(|error| error.to_string())?;
    if diff.trim().is_empty() {
        return Err("Stage changes before asking for a message.".into());
    }
    let limit = if settings.ai_max_chars == 0 { 12_000 } else { settings.ai_max_chars as usize };
    if diff.len() > limit {
        diff.truncate(limit);
    }
    let key = if settings.ai_api_key.trim().is_empty() {
        std::env::var("XAI_API_KEY")
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .unwrap_or_default()
    } else {
        settings.ai_api_key.trim().to_string()
    };
    if key.is_empty() {
        return Err("Add an API key in Preferences, or set XAI_API_KEY.".into());
    }
    let base = settings.ai_base_url.trim().trim_end_matches('/').to_string();
    let model = if settings.ai_model.trim().is_empty() {
        "grok-4.7".to_string()
    } else {
        settings.ai_model.trim().to_string()
    };
    let (files, branch, recent) = awegit_git::prompt_facts(repo);
    let language = if settings.ai_language.trim().is_empty() {
        "English".to_string()
    } else {
        settings.ai_language.trim().to_string()
    };
    let template = if settings.ai_prompt.trim().is_empty() {
        "Write a git commit message in {language}. The first line is the subject and must be at most 72 characters. Then a blank line and a short body. Branch: {branch}. Files: {files}. Recent commits: {recent_commits}. Use only this staged diff:\n\n{diff}".to_string()
    } else {
        settings.ai_prompt.clone()
    };
    let prompt = template
        .replace("{diff}", &diff)
        .replace("${diff}", &diff)
        .replace("{files}", &files)
        .replace("${files}", &files)
        .replace("{branch}", &branch)
        .replace("${branch}", &branch)
        .replace("{recent_commits}", &recent)
        .replace("${recent_commits}", &recent)
        .replace("{language}", &language)
        .replace("${language}", &language);
    let xai = base.contains("api.x.ai");
    let (url, mut body) = if xai {
        let body = serde_json::json!({ "model": model, "input": prompt });
        (format!("{base}/responses"), body)
    } else {
        let body = serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": "You write git commit messages. Reply with a subject of at most 72 characters, a blank line, and an optional body." },
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
    Ok(split_message(&answer))
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

fn split_message(text: &str) -> Suggestion {
    let trimmed = text.trim().trim_matches('"');
    let mut lines = trimmed.lines();
    let summary = lines.next().unwrap_or("").trim().to_string();
    let description = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    Suggestion { summary, description }
}
