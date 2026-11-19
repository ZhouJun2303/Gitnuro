//! High-performance Lite workspace support.
//!
//! The Lite workspace keeps the UI responsive while an external coding agent
//! works in the repository. The agent is deliberately limited to editing and
//! conflict resolution; Git commit and push remain AweGit operations.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::tools::{resolve_terminal_cli_configured, TerminalCli};

static PROMPT_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    pub run_id: String,
    pub phase: String,
    pub text: String,
    pub percent: u8,
    pub state: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentReport {
    pub ok: bool,
    pub output: String,
    pub code: Option<i32>,
}

fn emit(app: &AppHandle, event: AgentEvent) {
    let _ = app.emit("lite-agent", event);
}

/// Commands pass the prompt through a file or stdin instead of an argument.
/// npm installs Codex and Claude as `.cmd` shims, and a batch file cannot carry
/// a multi-line argument.
fn prompt_file(run_id: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let sequence = PROMPT_SEQ.fetch_add(1, Ordering::Relaxed);
    let tag: String = run_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
        .take(40)
        .collect();
    std::env::temp_dir().join(format!("awegit-lite-{}-{stamp}-{sequence}-{tag}.txt", std::process::id()))
}

fn flatten(prompt: &str) -> String {
    prompt.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn command_for(tool: &Path, cli: TerminalCli, prompt_file: &Path, prompt: &str) -> Command {
    #[cfg(windows)]
    let mut command = {
        let script = tool
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("cmd") || value.eq_ignore_ascii_case("bat"))
            .unwrap_or(false);
        if script {
            let shell = std::env::var_os("SystemRoot")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
                .join("System32")
                .join("cmd.exe");
            let mut command = Command::new(shell);
            command.args(["/D", "/S", "/C"]).arg(tool);
            command
        } else {
            Command::new(tool)
        }
    };

    #[cfg(not(windows))]
    let mut command = Command::new(tool);
    match cli {
        // Codex reads the batch prompt from stdin when the prompt argument is "-".
        TerminalCli::Codex => {
            command.args(["exec", "--approve-for-me", "-s", "workspace-write", "-"]);
        }
        // Claude Code's print mode also falls back to stdin when no prompt is given.
        TerminalCli::Claude => {
            command.args(["-p", "--dangerously-skip-permissions"]);
        }
        TerminalCli::Grok => {
            command.arg("--prompt-file").arg(prompt_file).arg("--always-approve");
        }
        // Cursor Agent has no prompt-file option, so keep its argument on one line.
        TerminalCli::CursorAgent => {
            command.args(["-p", "-f", "--trust"]).arg(flatten(prompt));
        }
    }
    command
}

fn read_pipe<R: Read + Send + 'static>(
    reader: R,
    app: AppHandle,
    run_id: String,
    output: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let Ok(line) = line else { break };
            if line.is_empty() {
                continue;
            }
            // A single huge line must not reach the webview as one event.
            let line = if line.len() > 4_000 {
                format!("{}…", line.chars().take(4_000).collect::<String>())
            } else {
                line
            };
            if let Ok(mut text) = output.lock() {
                if text.len() < 64 * 1024 {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&line);
                }
            }
            emit(
                &app,
                AgentEvent {
                    run_id: run_id.clone(),
                    phase: "agent".into(),
                    text: line,
                    percent: 55,
                    state: "running".into(),
                },
            );
        }
    })
}

fn execute(
    app: &AppHandle,
    repo: &Path,
    cli: TerminalCli,
    tool: &Path,
    prompt_file_path: &Path,
    prompt: &str,
    run_id: &str,
) -> Result<AgentReport, String> {
    let mut command = command_for(tool, cli, prompt_file_path, prompt);
    command
        .current_dir(repo)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match cli {
        TerminalCli::Codex | TerminalCli::Claude => {
            let input = File::open(prompt_file_path).map_err(|error| error.to_string())?;
            command.stdin(Stdio::from(input));
        }
        TerminalCli::Grok | TerminalCli::CursorAgent => {
            command.stdin(Stdio::null());
        }
    }
    awegit_git::hide_console(&mut command);
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let output = Arc::new(Mutex::new(String::new()));
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(read_pipe(stdout, app.clone(), run_id.to_string(), output.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(read_pipe(stderr, app.clone(), run_id.to_string(), output.clone()));
    }
    let status = child.wait().map_err(|error| error.to_string())?;
    for reader in readers {
        let _ = reader.join();
    }
    let text = output.lock().map(|value| value.clone()).unwrap_or_default();
    let ok = status.success();
    emit(
        app,
        AgentEvent {
            run_id: run_id.to_string(),
            phase: "agent".into(),
            text: if ok { "CLI finished".into() } else { "CLI failed".into() },
            percent: if ok { 70 } else { 55 },
            state: if ok { "completed" } else { "failed" }.into(),
        },
    );
    Ok(AgentReport { ok, output: text, code: status.code() })
}

pub fn run(
    app: AppHandle,
    repo: PathBuf,
    cli: TerminalCli,
    prompt: String,
    run_id: String,
    configured: Option<String>,
) -> Result<AgentReport, String> {
    let tool = resolve_terminal_cli_configured(cli, configured.as_deref())
        .ok_or_else(|| "terminal-cli-missing".to_string())?;
    emit(
        &app,
        AgentEvent {
            run_id: run_id.clone(),
            phase: "agent".into(),
            text: format!("Starting {}", cli.name()),
            percent: 45,
            state: "starting".into(),
        },
    );

    let file = prompt_file(&run_id);
    fs::write(&file, &prompt).map_err(|error| error.to_string())?;
    let result = execute(&app, &repo, cli, &tool, &file, &prompt, &run_id);
    let _ = fs::remove_file(&file);
    result
}

#[cfg(test)]
mod tests {
    use super::{command_for, flatten, TerminalCli};
    use std::path::Path;

    fn args(cli: TerminalCli, prompt: &str) -> Vec<String> {
        command_for(Path::new("codex"), cli, Path::new("C:\\Temp\\prompt.txt"), prompt)
            .get_args()
            .map(|value| value.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn codex_reads_the_prompt_from_stdin() {
        assert_eq!(args(TerminalCli::Codex, "resolve\nconflicts"), ["exec", "--approve-for-me", "-s", "workspace-write", "-"]);
    }

    #[test]
    fn claude_reads_the_prompt_from_stdin() {
        assert_eq!(args(TerminalCli::Claude, "resolve\nconflicts"), ["-p", "--dangerously-skip-permissions"]);
    }

    #[test]
    fn grok_uses_the_prompt_file() {
        assert_eq!(args(TerminalCli::Grok, "resolve\nconflicts"), ["--prompt-file", "C:\\Temp\\prompt.txt", "--always-approve"]);
    }

    #[test]
    fn cursor_agent_gets_a_single_line_prompt() {
        assert_eq!(args(TerminalCli::CursorAgent, "resolve\n  conflicts"), ["-p", "-f", "--trust", "resolve conflicts"]);
    }

    #[test]
    fn flattened_prompt_keeps_words() {
        assert_eq!(flatten("a\n\nb\tc "), "a b c");
    }
}
