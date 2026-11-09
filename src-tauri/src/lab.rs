//! HTTP entry for the browser lab. Git commands call the same functions as the desktop.
//! Repository paths have to stay inside the lab directory.

use std::cell::Cell;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

thread_local! {
    static GUARD: Cell<bool> = const { Cell::new(false) };
}

struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        GUARD.with(|flag| flag.set(false));
    }
}

pub(crate) fn lab_guarded() -> bool {
    GUARD.with(|flag| flag.get())
}

fn enter() -> Guard {
    GUARD.with(|flag| flag.set(true));
    Guard
}

fn isolate_lab_secrets() {
    if std::env::var_os("AWEGIT_SECRET_DIR").is_some() {
        return;
    }
    let Ok(root) = lab_root() else { return };
    let dir = root.join("secrets");
    let _ = std::fs::create_dir_all(&dir);
    unsafe { std::env::set_var("AWEGIT_SECRET_DIR", &dir) };
}

fn lab_root() -> Result<PathBuf, String> {
    if let Ok(root) = std::env::var("AWEGIT_LAB") {
        let root = root.trim();
        if !root.is_empty() {
            return Ok(PathBuf::from(root));
        }
    }
    let settings = std::env::var("AWEGIT_SETTINGS").map_err(|_| "AWEGIT_SETTINGS is not set".to_string())?;
    Path::new(settings.trim())
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "AWEGIT_SETTINGS has no parent".to_string())
}

fn normalize(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut out = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub(crate) fn ensure_lab_path(path: &Path) -> Result<(), String> {
    if !lab_guarded() {
        return Ok(());
    }
    let root = normalize(&lab_root()?);
    let candidate = normalize(path);
    let root_text = folder_key(&root);
    let path_text = folder_key(&candidate);
    if path_text == root_text || path_text.starts_with(&(root_text.clone() + "\\")) {
        return Ok(());
    }
    Err(format!("path is outside the lab: {}", path.display()))
}

fn folder_key(path: &Path) -> String {
    let text = path.to_string_lossy().replace('/', "\\");
    let text = text.trim_end_matches('\\').to_string();
    if cfg!(windows) { text.to_ascii_lowercase() } else { text }
}

fn guard_value(value: &Value, nested: bool) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    for (key, item) in object {
        let repo_key = matches!(key.as_str(), "root" | "destination" | "repo") || (nested && key == "path");
        if repo_key {
            if let Some(text) = item.as_str() {
                if !text.trim().is_empty() {
                    ensure_lab_path(Path::new(text))?;
                }
            }
        }
        if key == "request" {
            guard_value(item, true)?;
        }
    }
    Ok(())
}

fn camel(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for ch in name.chars() {
        if ch == '_' {
            upper = true;
            continue;
        }
        if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn req<T: DeserializeOwned>(args: &Value, name: &str) -> Result<T, String> {
    let camel_name = camel(name);
    let value = args.get(name).or_else(|| args.get(&camel_name)).cloned().unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|error| format!("{name}: {error}"))
}

fn done<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

fn done_result<T: Serialize>(value: Result<T, String>) -> Result<Value, String> {
    done(value?)
}

fn layout() -> Result<Value, String> {
    let root = lab_root()?;
    let repos = root.join("repos");
    let text = |path: PathBuf| path.display().to_string();
    let mut recent = Vec::new();
    let shelf = repos.join("shelf");
    if shelf.is_dir() {
        let mut names = std::fs::read_dir(&shelf)
            .map_err(|error| error.to_string())?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        names.sort();
        for path in names {
            if path.join(".git").exists() {
                recent.push(text(path));
            }
        }
    }
    if recent.is_empty() {
        for name in ["full", "ahead", "conflict", "even"] {
            let path = repos.join(name);
            if path.join(".git").exists() {
                recent.push(text(path));
            }
        }
    }
    Ok(json!({
        "root": text(root.clone()),
        "repos": text(repos.clone()),
        "recentList": recent.join("\n"),
        "remote": text(root.join("remote.git")),
        "remotePush": text(root.join("remote-push.git")),
        "full": text(repos.join("full")),
        "conflict": text(repos.join("conflict")),
        "merge": text(repos.join("merge")),
        "rebase": text(repos.join("rebase")),
        "cherry": text(repos.join("cherry")),
        "revert": text(repos.join("revert")),
        "detached": text(repos.join("detached")),
        "unborn": text(repos.join("unborn")),
        "bisect": text(repos.join("bisect")),
        "ahead": text(repos.join("ahead")),
        "behind": text(repos.join("behind")),
        "even": text(repos.join("even")),
        "pushbox": text(repos.join("pushbox")),
    }))
}

fn save_revision_lab(path: Option<String>, rev: String, file: String) -> Result<(), String> {
    let repo = super::repo_from(path)?;
    let bytes = awegit_git::revision_bytes(&repo, &rev, &file).map_err(|error| error.to_string())?;
    let name = file.rsplit(['/', '\\']).next().filter(|item| !item.is_empty()).unwrap_or("file");
    let dir = lab_root()?.join("saved");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    std::fs::write(dir.join(name), bytes).map_err(|error| error.to_string())
}

fn read_lab_ssh() -> Result<String, String> {
    let path = lab_root()?.join("ssh-config");
    if !path.exists() {
        return Ok("Host lab\n  User git\n".to_string());
    }
    std::fs::read_to_string(path).map_err(|error| error.to_string())
}

fn write_lab_ssh(text: String) -> Result<(), String> {
    if text.contains('\0') {
        return Err("ssh config contains a null".into());
    }
    let path = lab_root()?.join("ssh-config");
    std::fs::write(path, text).map_err(|error| error.to_string())
}

fn restore_repo(name: String) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("lab restore needs a repository name".into());
    }
    let root = lab_root()?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "AWEGIT_LAB has no parent".to_string())?;
    let script = root.join("scripts").join("lab-seed.mjs");
    if !script.is_file() {
        return Err(format!("missing {}", script.display()));
    }
    let status = std::process::Command::new("node")
        .arg(&script)
        .arg(&name)
        .current_dir(&root)
        .env("AWEGIT_LAB", lab_root()?)
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!("lab restore {name} failed"));
    }
    Ok(())
}

fn dispatch(command: &str, args: &Value) -> Result<Value, String> {
    match command {
        "lab_layout" => layout(),
        "lab_restore" => done_result(restore_repo(req(args, "name")?)),
        "workspace_status" => done_result(super::workspace_status(req(args, "path")?)),
        "stage_all" => done_result(super::stage_all(req(args, "path")?)),
        "unstage_all" => done_result(super::unstage_all(req(args, "path")?)),
        "stage_path" => done_result(super::stage_path(req(args, "path")?, req(args, "file")?)),
        "unstage_path" => done_result(super::unstage_path(req(args, "path")?, req(args, "file")?)),
        "commit_changes" => done_result(super::commit_changes(
            req(args, "path")?,
            req(args, "summary")?,
            req(args, "description")?,
            req(args, "amend")?,
            req(args, "sign_off")?,
            req(args, "skip_hooks")?,
        )),
        "file_diff" => done_result(super::file_diff(
            req(args, "path")?,
            req(args, "file")?,
            req(args, "staged")?,
            req(args, "unified")?,
            req(args, "ignore_space")?,
            req(args, "load")?,
        )),
        "commit_log" => done_result(super::commit_log(
            req(args, "path")?,
            req(args, "limit")?,
            req(args, "all")?,
            req(args, "order")?,
            req(args, "revs")?,
        )),
        "commit_detail" => done_result(super::commit_detail(req(args, "path")?, req(args, "id")?)),
        "locate_ref" => done_result(super::locate_ref(req(args, "path")?, req(args, "name")?, req(args, "order")?)),
        "repository_refs" => done_result(super::repository_refs(req(args, "path")?)),
        "commit_files" => done_result(super::commit_files(req(args, "path")?, req(args, "id")?)),
        "show_commit_file" => done_result(super::show_commit_file(
            req(args, "path")?,
            req(args, "id")?,
            req(args, "file")?,
            req(args, "unified")?,
        )),
        "in_progress" => done_result(super::in_progress(req(args, "path")?)),
        "file_history" => done_result(super::file_history(req(args, "path")?, req(args, "file")?, req(args, "limit")?)),
        "blame_file" => done_result(super::blame_file(req(args, "path")?, req(args, "file")?)),
        "mutate" => done_result(super::mutate(req(args, "path")?, req(args, "request")?)),
        "load_settings" => done_result(super::load_settings()),
        "save_settings" => done_result(super::save_settings(req(args, "values")?)),
        "settings_path" => done_result(super::settings_path()),
        "import_fork_settings" => {
            let path: Option<String> = req(args, "path")?;
            if path.as_deref().map(str::trim).unwrap_or("").is_empty() {
                return Err("fork-settings-missing".into());
            }
            done_result(super::import_fork_settings(path))
        }
        "external_tools" => done(super::external_tools()),
        "program_exists" => done(super::program_exists(req(args, "path")?)),
        "watch_repository" => Ok(Value::Null),
        "open_terminal" => Ok(Value::Null),
        "clipboard_text" => done("lab clipboard"),
        "pick_directory" => done(Some(lab_root()?.join("repos").display().to_string())),
        "pick_file" => {
            let file = lab_root()?.join("repos").join("full").join("README.md");
            done(file.exists().then(|| file.display().to_string()))
        }
        "suggest_commit_message" => done(crate::ai::Suggestion {
            summary: "lab: describe the staged change".into(),
            description: "实验室生成的提交说明。".into(),
        }),
        "test_ai_endpoint" => done(1_u32),
        "list_ai_models" => done(vec!["lab-model".to_string()]),
        "file_preview" => done_result(super::file_preview(req(args, "path")?, req(args, "file")?)),
        "cancel_operation" => done_result(super::cancel_operation(req(args, "path")?)),
        "cancel_reads" => Ok(Value::Null),
        "save_hidden_refs" => done_result(super::save_hidden_refs(req(args, "path")?, req(args, "names")?)),
        "save_expanded_groups" => done_result(super::save_expanded_groups(req(args, "path")?, req(args, "names")?)),
        "save_sign_off" => done_result(super::save_sign_off(req(args, "path")?, req(args, "enabled")?, req(args, "format")?)),
        "set_passphrase" => done_result(super::set_passphrase(req(args, "user")?, req(args, "secret")?)),
        "approve_credential" => Ok(Value::Null),
        "check_for_update" => Ok(Value::Null),
        "app_version" => done(super::app_version()),
        "update_release_page" => done(super::updates::release_page()),
        "download_update" => Ok(Value::Null),
        "conflict_sides" => done_result(super::conflict_sides(req(args, "path")?, req(args, "file")?)),
        "conflict_view" => done_result(super::conflict_view(req(args, "path")?, req(args, "file")?)),
        "compare_files" => done_result(super::compare_files(req(args, "path")?, req(args, "from")?, req(args, "to")?)),
        "compare_file" => done_result(super::compare_file(
            req(args, "path")?,
            req(args, "from")?,
            req(args, "to")?,
            req(args, "file")?,
            req(args, "unified")?,
        )),
        "set_repo_author" => done_result(super::set_repo_author(req(args, "path")?, req(args, "name")?, req(args, "email")?)),
        "revision_preview" => done_result(super::revision_preview(req(args, "path")?, req(args, "rev")?, req(args, "file")?)),
        "blob_view" => done_result(super::blob_view(req(args, "path")?, req(args, "rev")?, req(args, "file")?)),
        "reflog" => done_result(super::reflog(req(args, "path")?, req(args, "limit")?)),
        "commit_tree" => done_result(super::commit_tree(req(args, "path")?, req(args, "rev")?)),
        "repository_summary" => {
            let path: String = req(args, "path")?;
            ensure_lab_path(Path::new(&path))?;
            done_result(super::repository_summary(path))
        }
        "rebase_conflicts" => done_result(super::rebase_conflicts(req(args, "path")?, req(args, "onto")?)),
        "lfs_locks" => done_result(super::lfs_locks(req(args, "path")?, req(args, "list")?)),
        "launch_tool" => Ok(Value::Null),
        "forge_notifications" => done(vec![crate::forge::Notice {
            id: "lab-1".into(),
            title: "实验室通知".into(),
            url: "https://example.com/lab/notice".into(),
            unread: true,
        }]),
        "forge_pulls" => done(vec![crate::forge::PullRequest {
            title: "实验室拉取请求".into(),
            author: "lab".into(),
            branch: "topic".into(),
            url: "https://example.com/lab/pull/1".into(),
        }]),
        "repo_facts" => done_result(super::repo_facts(req(args, "path")?, req(args, "full")?)),
        "scan_repositories" => {
            let root: String = req(args, "root")?;
            ensure_lab_path(Path::new(&root))?;
            done_result(super::scan_repositories(root, req(args, "depth")?))
        }
        "mark_notification" => Ok(Value::Null),
        "create_github_repo" => done("https://example.com/lab/hosted.git".to_string()),
        "generate_ssh_key" => done("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILabKey awegit-lab".to_string()),
        "search_commits" => done_result(super::search_commits(req(args, "path")?, req(args, "kind")?, req(args, "needle")?)),
        "search_log" => done_result(super::search_log(
            req(args, "path")?,
            req(args, "field")?,
            req(args, "scope")?,
            req(args, "query")?,
            req(args, "limit")?,
            req(args, "order")?,
        )),
        "cherry_preview" => done_result(super::cherry_preview(req(args, "path")?, req(args, "rev")?)),
        "merge_preview" => done_result(super::merge_preview(req(args, "path")?, req(args, "name")?)),
        "merge_state" => done_result(super::merge_state(req(args, "path")?)),
        "merged_branches" => done_result(super::merged_branches(req(args, "path")?)),
        "pick_history" => done_result(super::pick_history(req(args, "path")?, req(args, "file")?, req(args, "text")?)),
        "activity_log" => done(crate::perf::activity()),
        "create_hosted_repo" => done("https://example.com/lab/hosted.git".to_string()),
        "device_start" => done(crate::forge::DeviceStart {
            verification_uri: "https://example.com/lab/device".into(),
            user_code: "LAB-CODE".into(),
        }),
        "device_poll" => done("pending".to_string()),
        "review_branch" => done("实验室审查：这个分支可以合并。".to_string()),
        "read_ssh_config" => done_result(read_lab_ssh()),
        "write_ssh_config" => done_result(write_lab_ssh(req(args, "text")?)),
        "open_editor" => Ok(Value::Null),
        "open_window" => Ok(Value::Null),
        "reveal_commit" => Ok(Value::Null),
        "save_revision" => done_result(save_revision_lab(req(args, "path")?, req(args, "rev")?, req(args, "file")?)),
        "log_client" => {
            crate::perf::client(req(args, "entries")?);
            Ok(Value::Null)
        }
        "perf_snapshot" => done(crate::perf::snapshot()),
        "diagnostics" => done(crate::perf::diagnostics()),
        "open_log_folder" => Ok(Value::Null),
        "reveal_path" => Ok(Value::Null),
        "git_runtime" => done(serde_json::json!({
            "program": "git",
            "version": "2.56.0",
            "mergeTree": true,
            "lfs": true,
            "minimum": true
        })),
        other => Err(format!("unknown lab command {other}")),
    }
}

pub fn lab_invoke(command: &str, args: Value) -> Result<Value, String> {
    let _guard = enter();
    guard_value(&args, false)?;
    dispatch(command, &args)
}

pub fn run_lab() {
    isolate_lab_secrets();
    crate::perf::init();
    let listener = TcpListener::bind("127.0.0.1:1429").unwrap_or_else(|error| {
        eprintln!("awegit-lab could not listen on 127.0.0.1:1429: {error}");
        std::process::exit(1);
    });
    eprintln!("awegit-lab listening on 127.0.0.1:1429");
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        if let Err(error) = serve(&mut stream) {
            let body = json!({ "ok": false, "error": error }).to_string();
            write_response(&mut stream, &body);
        }
    }
}

fn serve(stream: &mut TcpStream) -> Result<(), String> {
    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(60))).ok();
    let mut buf = Vec::new();
    let mut tmp = [0_u8; 8192];
    let header_end = loop {
        let read = stream.read(&mut tmp).map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("connection closed".into());
        }
        buf.extend_from_slice(&tmp[..read]);
        if let Some(pos) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > 1024 * 1024 {
            return Err("request header is too large".into());
        }
    };
    let header = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut path = String::from("/");
    let mut content_len = 0_usize;
    for (index, line) in header.split("\r\n").enumerate() {
        if index == 0 {
            path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
        } else if let Some(rest) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            content_len = rest.trim().parse().unwrap_or(0);
        }
    }
    if content_len > 8 * 1024 * 1024 {
        return Err("request body is too large".into());
    }
    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < content_len {
        let read = stream.read(&mut tmp).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..read]);
    }
    body.truncate(content_len);
    let route = path.split('?').next().unwrap_or("/");
    if route == "/health" {
        write_response(stream, r#"{"ok":true}"#);
        return Ok(());
    }
    if route != "/invoke" {
        return Err(format!("unknown lab route {route}"));
    }
    let request: Value = serde_json::from_slice(&body).map_err(|error| error.to_string())?;
    let command = request.get("cmd").and_then(Value::as_str).unwrap_or("").to_string();
    let args = request.get("args").cloned().unwrap_or_else(|| json!({}));
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| lab_invoke(&command, args)));
    let payload = match outcome {
        Ok(Ok(value)) => json!({ "ok": true, "value": value }),
        Ok(Err(error)) => json!({ "ok": false, "error": error }),
        Err(_) => json!({ "ok": false, "error": format!("lab command panicked: {command}") }),
    };
    write_response(stream, &payload.to_string());
    Ok(())
}

fn write_response(stream: &mut TcpStream, body: &str) {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body.as_bytes());
}
