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
    let limit = if settings.ai_max_chars == 0 {
        12_000
    } else {
        settings.ai_max_chars as usize
    };
    limit_diff(&mut diff, limit);
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
    let base = settings
        .ai_base_url
        .trim()
        .trim_end_matches('/')
        .to_string();
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
    let prompt = compose_prompt(
        style,
        settings.ai_prompt.trim(),
        &language,
        &files,
        &branch,
        &recent,
        &diff,
    );
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
    if let Some(message) = value
        .pointer("/error/message")
        .and_then(|item| item.as_str())
    {
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
    let base = if base.trim().is_empty() {
        "HEAD~1"
    } else {
        base.trim()
    };
    if base.contains(['\n', '\r', '\0']) || base.starts_with('-') {
        return Err("base revision is missing".into());
    }
    let output = awegit_git::range_diff(repo, base).map_err(|error| error.to_string())?;
    if output.trim().is_empty() {
        return Err("That range has no diff.".into());
    }
    let limit = if settings.ai_max_chars == 0 {
        12_000
    } else {
        settings.ai_max_chars as usize
    };
    let mut diff = output;
    limit_diff(&mut diff, limit);
    let key = ai_key(settings)?;
    let base_url = settings
        .ai_base_url
        .trim()
        .trim_end_matches('/')
        .to_string();
    let model = if settings.ai_model.trim().is_empty() {
        "grok-4.7".to_string()
    } else {
        settings.ai_model.trim().to_string()
    };
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
    let url = if base_url.contains("api.x.ai") {
        format!("{base_url}/responses")
    } else {
        format!("{base_url}/chat/completions")
    };
    let payload = serde_json::to_string(&body).map_err(|error| error.to_string())?;
    let text = post_json(&url, &key, &payload)?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| text.clone())?;
    if let Some(message) = value
        .pointer("/error/message")
        .and_then(|item| item.as_str())
    {
        return Err(message.to_string());
    }
    let answer = extract_text(&value);
    if answer.trim().is_empty() {
        return Err("The model returned an empty review.".into());
    }
    Ok(answer)
}

fn ai_key(settings: &Settings) -> Result<String, String> {
    let key = resolve_key(&settings.ai_api_key);
    if key.is_empty() {
        return Err("Add an API key in Preferences, or set XAI_API_KEY or OPENAI_API_KEY.".into());
    }
    Ok(key)
}

fn resolve_key(given: &str) -> String {
    let given = given.trim();
    if !given.is_empty() {
        return given.to_string();
    }
    std::env::var("XAI_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Read model ids from an OpenAI-compatible models endpoint. An empty key is allowed for local servers.
pub fn list_models(base: &str, key: &str) -> Result<Vec<String>, String> {
    let url = models_url(base)?;
    let (status, body) = http_get(&url, &resolve_key(key))?;
    let value = parse_endpoint(status, &body)?;
    Ok(model_ids(&value))
}

fn models_url(base: &str) -> Result<String, String> {
    let mut base = base.trim().trim_end_matches('/').to_string();
    for suffix in ["/chat/completions", "/responses", "/models"] {
        if let Some(stripped) = base.strip_suffix(suffix) {
            base = stripped.trim_end_matches('/').to_string();
            break;
        }
    }
    if base.is_empty() {
        return Err("Set an API base URL in Preferences.".into());
    }
    let lower = base.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://"))
        || base.chars().any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err("The API base URL must start with http:// or https://.".into());
    }
    Ok(format!("{base}/models"))
}

fn model_ids(value: &serde_json::Value) -> Vec<String> {
    let mut ids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let list = value
        .get("data")
        .or_else(|| value.get("models"))
        .filter(|item| item.is_array())
        .or_else(|| value.as_array().map(|_| value));
    let Some(items) = list.and_then(|item| item.as_array()) else {
        return ids;
    };
    for item in items {
        let id = item.as_str().or_else(|| {
            item.get("id")
                .and_then(|value| value.as_str())
                .or_else(|| item.get("name").and_then(|value| value.as_str()))
        });
        if let Some(id) = id {
            let id = id.trim();
            if !id.is_empty() && seen.insert(id.to_string()) {
                ids.push(id.to_string());
            }
        }
        if ids.len() == 400 {
            break;
        }
    }
    ids
}

fn parse_endpoint(status: u16, body: &str) -> Result<serde_json::Value, String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| {
        let sample: String = body.trim().chars().take(180).collect();
        if status >= 400 {
            format!("The endpoint returned HTTP {status}. {sample}")
        } else if sample.is_empty() {
            "The endpoint returned an empty response.".into()
        } else {
            format!("The endpoint did not return JSON. {sample}")
        }
    })?;
    if status >= 400 || value.get("error").is_some() {
        let message = value
            .pointer("/error/message")
            .and_then(|item| item.as_str())
            .or_else(|| value.get("error").and_then(|item| item.as_str()))
            .unwrap_or("the endpoint request failed");
        return Err(if status >= 400 {
            format!("HTTP {status}: {message}")
        } else {
            message.to_string()
        });
    }
    Ok(value)
}

fn split_status(text: String) -> (u16, String) {
    let Some((body, code)) = text.rsplit_once('\n') else {
        return (0, text);
    };
    let code = code.trim();
    if code.len() == 3 && code.chars().all(|ch| ch.is_ascii_digit()) {
        (code.parse().unwrap_or(0), body.to_string())
    } else {
        (0, text)
    }
}

fn http_get(url: &str, key: &str) -> Result<(u16, String), String> {
    let mut command = std::process::Command::new("curl");
    awegit_git::hide_console(&mut command);
    command.args([
        "-sS",
        "--max-time",
        "30",
        "--proto",
        "=http,https",
        "--proto-redir",
        "=http,https",
        "--max-redirs",
        "3",
        "-L",
        "-H",
        "Accept: application/json",
        "-w",
        "\n%{http_code}",
    ]);
    if !key.is_empty() {
        command
            .arg("-H")
            .arg(format!("Authorization: Bearer {key}"));
    }
    let output = command
        .arg(url)
        .output()
        .map_err(|error| format!("curl is not available: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "the endpoint request failed".into()
        } else {
            stderr
        });
    }
    Ok(split_status(
        String::from_utf8_lossy(&output.stdout).to_string(),
    ))
}

fn post_json(url: &str, key: &str, payload: &str) -> Result<String, String> {
    let mut command = std::process::Command::new("curl");
    awegit_git::hide_console(&mut command);
    let mut child = command
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
        stdin
            .write_all(payload.as_bytes())
            .map_err(|error| error.to_string())?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "the model request failed".into()
        } else {
            stderr
        });
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
        return fill(
            &format!("{convention}\n\n{custom}"),
            diff,
            files,
            branch,
            recent,
            language,
        );
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

fn fill(
    template: &str,
    diff: &str,
    files: &str,
    branch: &str,
    recent: &str,
    language: &str,
) -> String {
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
    if lines
        .first()
        .is_some_and(|line| line.trim().starts_with("```"))
    {
        lines.remove(0);
        if lines
            .last()
            .is_some_and(|line| line.trim().starts_with("```"))
        {
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
        let sample = lines
            .iter()
            .find(|line| !line.trim().is_empty())
            .copied()
            .unwrap_or("");
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
    Ok(Suggestion {
        summary,
        description,
    })
}

fn limit_diff(diff: &mut String, limit: usize) {
    if diff.len() <= limit {
        return;
    }
    let mut end = limit;
    while end > 0 && !diff.is_char_boundary(end) {
        end -= 1;
    }
    diff.truncate(end);
    diff.push_str("\n\n[diff truncated]");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_prompt_keeps_the_hook_rule_and_staged_diff() {
        let prompt = compose_prompt(
            "short",
            "",
            "中文",
            "src/app.ts",
            "main",
            "chore: old",
            "diff-body",
        );
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
        let prompt = compose_prompt(
            "long",
            "",
            "中文",
            "src/app.ts",
            "main",
            "chore: old",
            "diff-body",
        );
        assert!(prompt.contains("blank line"));
        assert!(prompt.contains("chore: old"));
        assert!(prompt.contains("diff-body"));
        assert!(prompt.contains("refactor 重构"));
    }

    #[test]
    fn custom_template_still_gets_the_hook_rule() {
        let prompt = compose_prompt(
            "short",
            "Only this.\n{diff}",
            "English",
            "a.ts",
            "dev",
            "old",
            "DIFF",
        );
        assert!(prompt.contains("commit-msg hook"));
        assert!(prompt.contains("Only this."));
        assert_eq!(prompt.matches("DIFF").count(), 1);
    }

    #[test]
    fn accepts_the_hook_examples() {
        let value = parse_suggestion("feat: 新增挂机奖励结算逻辑").unwrap();
        assert_eq!(value.summary, "feat: 新增挂机奖励结算逻辑");
        assert!(value.description.is_empty());
        let tight =
            parse_suggestion("fix:keep the composer visible\n\nThe pane collapsed.").unwrap();
        assert_eq!(tight.summary, "fix:keep the composer visible");
        assert_eq!(tight.description, "The pane collapsed.");
    }

    #[test]
    fn models_url_uses_the_models_path() {
        assert_eq!(
            models_url(" https://api.x.ai/v1/ ").unwrap(),
            "https://api.x.ai/v1/models"
        );
        assert_eq!(
            models_url("https://api.openai.com/v1/chat/completions").unwrap(),
            "https://api.openai.com/v1/models"
        );
        assert_eq!(
            models_url("http://127.0.0.1:11434/v1/models").unwrap(),
            "http://127.0.0.1:11434/v1/models"
        );
        assert!(models_url("").is_err());
        assert!(models_url("ftp://files.example/v1").is_err());
        assert!(models_url("https://api.example/v1 extra").is_err());
    }

    #[test]
    fn reads_provider_model_lists() {
        let openai = serde_json::json!({"data":[{"id":"b"},{"id":" a "},{"id":"a"},{"id":""}]});
        assert_eq!(model_ids(&openai), vec!["b".to_string(), "a".to_string()]);
        let ollama = serde_json::json!({"models":[{"name":"llama3"}, "mistral"]});
        assert_eq!(
            model_ids(&ollama),
            vec!["llama3".to_string(), "mistral".to_string()]
        );
        assert!(model_ids(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn endpoint_error_uses_the_message() {
        let error = parse_endpoint(401, r#"{"error":{"message":"bad key"}}"#).unwrap_err();
        assert!(error.contains("401"));
        assert!(error.contains("bad key"));
        let (status, body) = split_status("{\"data\":[]}\n200".into());
        assert_eq!(status, 200);
        assert!(parse_endpoint(status, &body).is_ok());
    }

    #[test]
    fn given_key_is_trimmed_before_the_environment() {
        assert_eq!(resolve_key("  abc  "), "abc");
    }

    fn serve_once(status: &str, body: &str) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let status = status.to_string();
        let body = body.to_string();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
            let mut buf = Vec::new();
            let mut tmp = [0u8; 512];
            while !buf.windows(4).any(|item| item == b"\r\n\r\n") && buf.len() < 8192 {
                match std::io::Read::read(&mut stream, &mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                }
            }
            let header = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = std::io::Write::write_all(&mut stream, header.as_bytes());
            let _ = std::io::Write::write_all(&mut stream, body.as_bytes());
        });
        port
    }

    #[test]
    fn lists_models_from_a_local_endpoint() {
        let port = serve_once("200 OK", r#"{"data":[{"id":"local-a"},{"id":"local-b"}]}"#);
        let models = list_models(&format!("http://127.0.0.1:{port}/v1"), "").unwrap();
        assert_eq!(models, vec!["local-a".to_string(), "local-b".to_string()]);
    }

    #[test]
    fn local_endpoint_reports_an_unauthorized_key() {
        let port = serve_once("401 Unauthorized", r#"{"error":{"message":"bad key"}}"#);
        let error = list_models(&format!("http://127.0.0.1:{port}/v1"), "secret").unwrap_err();
        assert!(error.contains("bad key"), "{error}");
        assert!(!error.contains("secret"), "{error}");
    }

    #[test]
    fn strips_fences_and_rejects_a_freeform_subject() {
        let value =
            parse_suggestion("```\n**refactor: 抽出提交提示词**\n\n避免漏掉钩子规则。\n```")
                .unwrap();
        assert_eq!(value.summary, "refactor: 抽出提交提示词");
        assert!(value.description.contains("钩子"));
        let error = parse_suggestion("updated the button").unwrap_err();
        assert!(error.contains("does not match"));
        assert!(error.contains("updated the button"));
    }

    #[test]
    fn truncates_a_diff_on_a_character_boundary() {
        let mut diff = "中".repeat(5_000);
        limit_diff(&mut diff, 12_001);
        assert!(diff.ends_with("[diff truncated]"));
        assert!(diff.len() <= 12_001 + "\n\n[diff truncated]".len());
        assert!(diff.trim_end_matches("\n\n[diff truncated]").chars().all(|ch| ch == '中'));
    }
}
