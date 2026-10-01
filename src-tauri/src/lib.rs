mod ai;
mod forge;
mod perf;
mod settings;
mod updates;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{Emitter, Manager};

static GIT_WRITE: Mutex<()> = Mutex::new(());
static WATCH_TICK: AtomicU64 = AtomicU64::new(0);

struct RepoWatch(Mutex<Option<RecommendedWatcher>>);

fn git_cmd(dir: &Path) -> Option<PathBuf> {
    let cmd = dir.join("git").join("cmd");
    if cmd.join("git.exe").is_file() || cmd.join("git").is_file() {
        Some(cmd)
    } else {
        None
    }
}

fn prefer_bundled_git() {
    let Ok(exe) = std::env::current_exe() else { return };
    let Some(parent) = exe.parent() else { return };
    if let Some(cmd) = git_cmd(parent).or_else(|| git_cmd(&parent.join("resources"))) {
        awegit_git::use_bundled_git(&cmd);
    }
}

fn repo_from(path: Option<String>) -> Result<PathBuf, String> {
    let start = match path {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => std::env::current_dir().map_err(|error| error.to_string())?,
    };
    Ok(awegit_git::discover(&start).unwrap_or(start))
}

fn read_status(repo: &Path) -> Result<awegit_git::StatusSnapshot, String> {
    awegit_git::status(repo).map_err(|error| error.to_string())
}

fn compose_proxy(settings: &settings::Settings) -> String {
    let host = settings.proxy_host.trim();
    if settings.proxy_enabled && !host.is_empty() {
        let scheme = if settings.proxy_type.eq_ignore_ascii_case("socks") {
            "socks5"
        } else {
            "http"
        };
        let port = if settings.proxy_port > 0 {
            format!(":{}", settings.proxy_port)
        } else {
            String::new()
        };
        let auth = if settings.proxy_user.trim().is_empty() {
            String::new()
        } else {
            format!(
                "{}:{}@",
                encode_userinfo(settings.proxy_user.trim()),
                encode_userinfo(&settings.proxy_password)
            )
        };
        return format!("{scheme}://{auth}{host}{port}");
    }
    let proxy = settings.proxy.trim();
    if proxy.is_empty() {
        return String::new();
    }
    if settings.proxy_user.trim().is_empty() {
        return proxy.to_string();
    }
    let user = encode_userinfo(settings.proxy_user.trim());
    let password = encode_userinfo(&settings.proxy_password);
    if let Some((scheme, rest)) = proxy.split_once("://") {
        format!("{scheme}://{user}:{password}@{rest}")
    } else {
        format!("http://{user}:{password}@{proxy}")
    }
}

fn encode_userinfo(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn apply_session() {
    let values = settings::load().unwrap_or_default();
    awegit_git::configure(awegit_git::Session {
        proxy: compose_proxy(&values),
        author_name: values.author_name.trim().to_string(),
        author_email: values.author_email.trim().to_string(),
        ssl_verify: values.ssl_verify,
        ssl_ca: values.ssl_ca_file.trim().to_string(),
        sign_commits: values.sign_commits,
        ask_user: String::new(),
    });
}

fn write_then_status(repo: &Path, write: impl FnOnce(&Path) -> Result<(), awegit_git::Error>) -> Result<awegit_git::StatusSnapshot, String> {
    let _guard = GIT_WRITE.lock().map_err(|_| "a Git write was interrupted".to_string())?;
    apply_session();
    write(repo).map_err(|error| error.to_string())?;
    read_status(repo)
}

#[tauri::command]
fn workspace_status(path: Option<String>) -> Result<awegit_git::StatusSnapshot, String> {
    read_status(&repo_from(path)?)
}

#[tauri::command]
fn stage_all(path: Option<String>) -> Result<awegit_git::StatusSnapshot, String> {
    let repo = repo_from(path)?;
    write_then_status(&repo, awegit_git::stage_all)
}

#[tauri::command]
fn unstage_all(path: Option<String>) -> Result<awegit_git::StatusSnapshot, String> {
    let repo = repo_from(path)?;
    write_then_status(&repo, awegit_git::unstage_all)
}

#[tauri::command]
fn stage_path(path: Option<String>, file: String) -> Result<awegit_git::StatusSnapshot, String> {
    let repo = repo_from(path)?;
    write_then_status(&repo, |repo| awegit_git::stage_paths(repo, &[file]))
}

#[tauri::command]
fn unstage_path(path: Option<String>, file: String) -> Result<awegit_git::StatusSnapshot, String> {
    let repo = repo_from(path)?;
    write_then_status(&repo, |repo| awegit_git::unstage_paths(repo, &[file]))
}

#[tauri::command]
fn commit_changes(
    path: Option<String>,
    summary: String,
    description: String,
    amend: bool,
    sign_off: bool,
    skip_hooks: Option<bool>,
) -> Result<awegit_git::StatusSnapshot, String> {
    let repo = repo_from(path)?;
    let sign_off_format = if sign_off {
        let local = awegit_git::repo_sign_off_format(&repo);
        if local.trim().is_empty() {
            settings::load().map(|values| values.sign_off_format).unwrap_or_default()
        } else {
            local
        }
    } else {
        String::new()
    };
    write_then_status(&repo, |repo| {
        awegit_git::commit(
            repo,
            awegit_git::CommitRequest {
                summary,
                description,
                amend,
                sign_off,
                sign_off_format,
                skip_hooks: skip_hooks.unwrap_or(false),
            },
        )
    })
}

#[tauri::command]
fn file_diff(path: Option<String>, file: String, staged: bool, unified: Option<u32>, ignore_space: Option<bool>) -> Result<awegit_git::FileDiff, String> {
    let repo = repo_from(path)?;
    awegit_git::file_diff_with(&repo, &file, staged, unified.unwrap_or(3), ignore_space.unwrap_or(false)).map_err(|error| error.to_string())
}

#[tauri::command]
fn commit_log(
    path: Option<String>,
    limit: Option<usize>,
    all: Option<bool>,
    order: Option<String>,
    revs: Option<Vec<String>>,
) -> Result<Vec<awegit_git::CommitRow>, String> {
    let repo = repo_from(path)?;
    let limit = limit.unwrap_or(500);
    let topo = order.as_deref() == Some("topo");
    if let Some(revs) = revs.as_ref().filter(|items| !items.is_empty()) {
        return awegit_git::commits_from(&repo, limit, revs, topo).map_err(|error| error.to_string());
    }
    let rows = if all.unwrap_or(true) {
        awegit_git::commit_log_sorted(&repo, limit, topo)
    } else {
        awegit_git::branch_commits_sorted(&repo, limit, topo)
    };
    rows.map_err(|error| error.to_string())
}

#[tauri::command]
fn commit_detail(path: Option<String>, id: String) -> Result<awegit_git::CommitDetail, String> {
    let repo = repo_from(path)?;
    awegit_git::commit_detail(&repo, &id).map_err(|error| error.to_string())
}

#[tauri::command]
fn repository_refs(path: Option<String>) -> Result<awegit_git::RefSnapshot, String> {
    let repo = repo_from(path)?;
    awegit_git::repository_refs(&repo).map_err(|error| error.to_string())
}

#[tauri::command]
fn commit_files(path: Option<String>, id: String) -> Result<Vec<awegit_git::FileChange>, String> {
    let repo = repo_from(path)?;
    awegit_git::commit_files(&repo, &id).map_err(|error| error.to_string())
}

#[tauri::command]
fn show_commit_file(path: Option<String>, id: String, file: String, unified: Option<u32>) -> Result<awegit_git::FileDiff, String> {
    let repo = repo_from(path)?;
    awegit_git::show_commit_file_with(&repo, &id, &file, unified.unwrap_or(3)).map_err(|error| error.to_string())
}

#[tauri::command]
fn in_progress(path: Option<String>) -> Result<Option<awegit_git::InProgress>, String> {
    let repo = repo_from(path)?;
    awegit_git::in_progress(&repo).map_err(|error| error.to_string())
}

#[tauri::command]
fn file_history(path: Option<String>, file: String, limit: Option<usize>) -> Result<Vec<awegit_git::CommitRow>, String> {
    let repo = repo_from(path)?;
    awegit_git::file_history(&repo, &file, limit.unwrap_or(200)).map_err(|error| error.to_string())
}

#[tauri::command]
fn blame_file(path: Option<String>, file: String) -> Result<Vec<awegit_git::BlameLine>, String> {
    let repo = repo_from(path)?;
    awegit_git::blame_file(&repo, &file).map_err(|error| error.to_string())
}

#[derive(serde::Serialize)]
struct MutationReport {
    #[serde(flatten)]
    status: awegit_git::StatusSnapshot,
    #[serde(skip_serializing_if = "String::is_empty")]
    output: String,
}

#[tauri::command]
fn mutate(path: Option<String>, request: awegit_git::Mutation) -> Result<MutationReport, String> {
    let repo = repo_from(path)?;
    let _guard = GIT_WRITE.lock().map_err(|_| "a Git write was interrupted".to_string())?;
    apply_session();
    record_action(&request);
    let next = match &request {
        awegit_git::Mutation::Clone { destination, .. } | awegit_git::Mutation::Init { destination } => {
            PathBuf::from(destination)
        }
        _ => repo.clone(),
    };
    let output = awegit_git::perform(&repo, request).map_err(|error| error.to_string())?;
    Ok(MutationReport {
        status: read_status(&next)?,
        output,
    })
}

fn record_action(request: &awegit_git::Mutation) {
    let Ok(values) = settings::load() else { return };
    let directory = values.log_directory.trim();
    if directory.is_empty() {
        return;
    }
    let label = action_label(request);
    let detail = action_detail(request);
    let _ = std::fs::create_dir_all(directory);
    let path = PathBuf::from(directory).join("awegit.log");
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let line = if detail.is_empty() {
        format!("{unix}\t{label}\n")
    } else {
        format!("{unix}\t{label}\t{detail}\n")
    };
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = file.write_all(line.as_bytes());
    }
}

fn action_label(request: &awegit_git::Mutation) -> &'static str {
    match request {
        awegit_git::Mutation::Fetch { .. } => "fetch",
        awegit_git::Mutation::Pull { .. } => "pull",
        awegit_git::Mutation::Push { .. } => "push",
        awegit_git::Mutation::Stash { .. } => "stash",
        awegit_git::Mutation::StashPop { .. } => "stashPop",
        awegit_git::Mutation::StashApply { .. } => "stashApply",
        awegit_git::Mutation::StashDrop { .. } => "stashDrop",
        awegit_git::Mutation::StashRename { .. } => "stashRename",
        awegit_git::Mutation::StashBranch { .. } => "stashBranch",
        awegit_git::Mutation::StashPatch { .. } => "stashPatch",
        awegit_git::Mutation::StashPaths { .. } => "stashPaths",
        awegit_git::Mutation::Checkout { .. } => "checkout",
        awegit_git::Mutation::CreateBranch { .. } => "createBranch",
        awegit_git::Mutation::DeleteBranch { .. } => "deleteBranch",
        awegit_git::Mutation::Merge { .. } => "merge",
        awegit_git::Mutation::Rebase { .. } => "rebase",
        awegit_git::Mutation::RebaseInteractive { .. } => "rebaseInteractive",
        awegit_git::Mutation::Abort => "abort",
        awegit_git::Mutation::Continue { .. } => "continue",
        awegit_git::Mutation::Skip => "skip",
        awegit_git::Mutation::Reset { .. } => "reset",
        awegit_git::Mutation::CherryPick { .. } => "cherryPick",
        awegit_git::Mutation::Revert { .. } => "revert",
        awegit_git::Mutation::Reword { .. } => "reword",
        awegit_git::Mutation::Tag { .. } => "tag",
        awegit_git::Mutation::DeleteTag { .. } => "deleteTag",
        awegit_git::Mutation::DeleteRemoteTag { .. } => "deleteRemoteTag",
        awegit_git::Mutation::RestoreFile { .. } => "restoreFile",
        awegit_git::Mutation::Bisect { .. } => "bisect",
        awegit_git::Mutation::WriteGitignore { .. } => "writeGitignore",
        awegit_git::Mutation::WriteMailmap { .. } => "writeMailmap",
        awegit_git::Mutation::StageHunk { .. } => "stageHunk",
        awegit_git::Mutation::StageLine { .. } => "stageLine",
        awegit_git::Mutation::StagePaths { .. } => "stagePaths",
        awegit_git::Mutation::DiscardHunk { .. } => "discardHunk",
        awegit_git::Mutation::DiscardLine { .. } => "discardLine",
        awegit_git::Mutation::Resolve { .. } => "resolve",
        awegit_git::Mutation::RenameBranch { .. } => "renameBranch",
        awegit_git::Mutation::SetUpstream { .. } => "setUpstream",
        awegit_git::Mutation::DeleteRemoteBranch { .. } => "deleteRemoteBranch",
        awegit_git::Mutation::CheckoutRemote { .. } => "checkoutRemote",
        awegit_git::Mutation::PushRef { .. } => "pushRef",
        awegit_git::Mutation::PullRef { .. } => "pullRef",
        awegit_git::Mutation::Discard { .. } => "discard",
        awegit_git::Mutation::Delete { .. } => "delete",
        awegit_git::Mutation::ApplyPatch { .. } => "applyPatch",
        awegit_git::Mutation::AddRemote { .. } => "addRemote",
        awegit_git::Mutation::RemoveRemote { .. } => "removeRemote",
        awegit_git::Mutation::SetRemoteUrl { .. } => "setRemoteUrl",
        awegit_git::Mutation::AddWorktree { .. } => "addWorktree",
        awegit_git::Mutation::RemoveWorktree { .. } => "removeWorktree",
        awegit_git::Mutation::SubmoduleUpdate => "submoduleUpdate",
        awegit_git::Mutation::SubmoduleAdd { .. } => "submoduleAdd",
        awegit_git::Mutation::SubmoduleInit { .. } => "submoduleInit",
        awegit_git::Mutation::SubmoduleSync { .. } => "submoduleSync",
        awegit_git::Mutation::SubmoduleRemove { .. } => "submoduleRemove",
        awegit_git::Mutation::GitFlowInit { .. } => "gitFlowInit",
        awegit_git::Mutation::GitFlowStart { .. } => "gitFlowStart",
        awegit_git::Mutation::GitFlowFinish { .. } => "gitFlowFinish",
        awegit_git::Mutation::Squash { .. } => "squash",
        awegit_git::Mutation::Clone { .. } => "clone",
        awegit_git::Mutation::Init { .. } => "init",
        awegit_git::Mutation::LfsPull => "lfsPull",
        awegit_git::Mutation::LfsPush => "lfsPush",
        awegit_git::Mutation::LfsLock { .. } => "lfsLock",
        awegit_git::Mutation::LfsUnlock { .. } => "lfsUnlock",
        awegit_git::Mutation::SetHidden { .. } => "setHidden",
        awegit_git::Mutation::SetExpanded { .. } => "setExpanded",
        awegit_git::Mutation::SetSignOff { .. } => "setSignOff",
        awegit_git::Mutation::Custom { .. } => "custom",
    }
}

/// Names, paths, and verbs only. Passwords, command text, and patch bodies stay out.
fn action_detail(request: &awegit_git::Mutation) -> String {
    use awegit_git::Mutation;
    let text = match request {
        Mutation::Fetch { remote, prune, tags } => join([opt(remote), flag("prune", *prune), flag("tags", *tags)]),
        Mutation::Pull { rebase, autostash } => join([flag("rebase", *rebase), flag("autostash", *autostash)]),
        Mutation::Push { remote, set_upstream, tags, force_with_lease, force } => {
            join([opt(remote), flag("setUpstream", *set_upstream), flag("tags", *tags), flag("forceWithLease", *force_with_lease), flag("force", *force)])
        }
        Mutation::Stash { message, include_untracked } => join([bytes("message", message), flag("untracked", *include_untracked)]),
        Mutation::StashPop { name } => scrub(name),
        Mutation::StashApply { name } | Mutation::StashDrop { name } | Mutation::StashPatch { name } => scrub(name),
        Mutation::StashRename { name, message } => join([scrub(name), bytes("message", message)]),
        Mutation::StashBranch { name, branch } => join([scrub(name), scrub(branch)]),
        Mutation::StashPaths { files, message, include_untracked } => {
            join([files.iter().cloned().map(|file| scrub(&file)).collect::<Vec<_>>().join(","), bytes("message", message), flag("untracked", *include_untracked)])
        }
        Mutation::Checkout { name } => scrub(name),
        Mutation::DeleteBranch { name, force } => join([scrub(name), flag("force", *force)]),
        Mutation::CreateBranch { name, start } => join([scrub(name), opt(start)]),
        Mutation::Merge { name, squash, no_ff, autostash } => {
            join([scrub(name), flag("squash", *squash), flag("noFf", *no_ff), flag("autostash", *autostash)])
        }
        Mutation::Rebase { onto, autostash } => join([scrub(onto), flag("autostash", *autostash)]),
        Mutation::RebaseInteractive { onto, drop, steps, autostash, update_refs } => {
            join([scrub(onto), format!("drop={}", drop.len()), format!("steps={}", steps.len()), flag("autostash", *autostash), flag("updateRefs", *update_refs)])
        }
        Mutation::Reset { rev, mode } => join([scrub(rev), format!("{mode:?}")]),
        Mutation::CherryPick { rev, record } => join([scrub(rev), flag("record", *record)]),
        Mutation::Revert { rev } => scrub(rev),
        Mutation::Reword { rev, summary } => join([scrub(rev), bytes("summary", summary)]),
        Mutation::Tag { name, rev, message } => join([scrub(name), scrub(rev), bytes("message", message)]),
        Mutation::DeleteTag { name } | Mutation::RemoveRemote { name } => scrub(name),
        Mutation::DeleteRemoteTag { remote, name } => join([scrub(remote), scrub(name)]),
        Mutation::RestoreFile { rev, file } => join([scrub(rev), scrub(file)]),
        Mutation::Bisect { verb, rev } => join([scrub(verb), scrub(rev)]),
        Mutation::WriteGitignore { text } | Mutation::WriteMailmap { text } => format!("bytes={}", text.len()),
        Mutation::StageHunk { file, index, unstage } => join([scrub(file), format!("hunk={index}"), flag("unstage", *unstage)]),
        Mutation::StageLine { file, unstage, .. } => join([scrub(file), flag("unstage", *unstage)]),
        Mutation::StagePaths { files, unstage } => join([files.iter().cloned().map(|file| scrub(&file)).collect::<Vec<_>>().join(","), flag("unstage", *unstage)]),
        Mutation::DiscardHunk { file, index } => join([scrub(file), format!("hunk={index}")]),
        Mutation::DiscardLine { file, .. } | Mutation::Discard { file } | Mutation::Delete { file } => scrub(file),
        Mutation::Resolve { file, side, text } => join([scrub(file), scrub(side), format!("bytes={}", text.len())]),
        Mutation::RenameBranch { name, to } => join([scrub(name), scrub(to)]),
        Mutation::SetUpstream { branch, upstream } => join([scrub(branch), scrub(upstream)]),
        Mutation::DeleteRemoteBranch { remote, branch }
        | Mutation::CheckoutRemote { remote, branch }
        | Mutation::PushRef { remote, branch, .. }
        | Mutation::PullRef { remote, branch, .. } => join([scrub(remote), scrub(branch)]),
        Mutation::ApplyPatch { patch } => format!("bytes={}", patch.len()),
        Mutation::AddRemote { name, url } | Mutation::SetRemoteUrl { name, url } => join([scrub(name), public_url(url)]),
        Mutation::AddWorktree { path, branch } => join([scrub(path), scrub(branch)]),
        Mutation::RemoveWorktree { path } => scrub(path),
        Mutation::SubmoduleAdd { url, path } => join([scrub(path), public_url(url)]),
        Mutation::SubmoduleInit { path } | Mutation::SubmoduleSync { path } | Mutation::SubmoduleRemove { path } => scrub(path),
        Mutation::GitFlowStart { name, kind } | Mutation::GitFlowFinish { name, kind } => join([scrub(name), scrub(kind)]),
        Mutation::GitFlowInit { master, develop, feature, release, hotfix, support } => {
            join([scrub(master), scrub(develop), scrub(feature), scrub(release), scrub(hotfix), scrub(support)])
        }
        Mutation::Squash { from, to, summary } => join([scrub(from), scrub(to), bytes("summary", summary)]),
        Mutation::Clone { url, destination } => join([public_url(url), scrub(destination)]),
        Mutation::Init { destination } => scrub(destination),
        Mutation::SetHidden { names } | Mutation::SetExpanded { names } => names.iter().cloned().map(|name| scrub(&name)).collect::<Vec<_>>().join(","),
        Mutation::SetSignOff { enabled, .. } => flag("enabled", *enabled),
        Mutation::Continue { message } => bytes("message", message),
        Mutation::Custom { .. } => String::new(),
        Mutation::LfsLock { path } | Mutation::LfsUnlock { path } => scrub(path),
        Mutation::Abort | Mutation::Skip | Mutation::SubmoduleUpdate | Mutation::LfsPull | Mutation::LfsPush => String::new(),
    };
    text.replace(['\n', '\r', '\t', '\0'], " ")
}

fn join(parts: impl IntoIterator<Item = String>) -> String {
    parts.into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
}

fn scrub(value: &str) -> String {
    value.replace(['\n', '\r', '\t', '\0'], " ")
}

fn opt(value: &Option<String>) -> String {
    value.as_deref().map(scrub).unwrap_or_default()
}

fn flag(name: &str, on: bool) -> String {
    if on { name.to_string() } else { String::new() }
}

fn bytes(name: &str, value: &str) -> String {
    if value.is_empty() { String::new() } else { format!("{name}Bytes={}", value.len()) }
}

fn public_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return scrub(url.split_once('@').map(|(_, host)| host).unwrap_or(url));
    };
    let host = rest.split_once('@').map(|(_, host)| host).unwrap_or(rest);
    scrub(&format!("{scheme}://{host}"))
}

#[tauri::command]
fn load_settings() -> Result<settings::Settings, String> {
    settings::load()
}

#[tauri::command]
fn save_settings(values: settings::Settings) -> Result<settings::Settings, String> {
    settings::save(&values)?;
    Ok(values)
}

#[tauri::command]
fn watch_repository(app: tauri::AppHandle, state: tauri::State<'_, RepoWatch>, path: Option<String>) -> Result<(), String> {
    let repo = repo_from(path)?;
    let mut slot = state.0.lock().map_err(|_| "watcher lock".to_string())?;
    let app_handle = app.clone();
    let mut watcher = notify::recommended_watcher(move |result: Result<Event, notify::Error>| {
        let Ok(event) = result else { return };
        if !event.paths.is_empty() && event.paths.iter().all(|path| ignored_path(path)) {
            return;
        }
        let ticket = WATCH_TICK.fetch_add(1, Ordering::Relaxed) + 1;
        let app_handle = app_handle.clone();
        let sample = event.paths.iter().find(|path| !ignored_path(path)).map(|path| path.display().to_string()).unwrap_or_default();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            if WATCH_TICK.load(Ordering::Relaxed) == ticket {
                perf::write("INFO", "watch", &format!("repo-changed {sample}"));
                let _ = app_handle.emit("repo-changed", ());
            }
        });
    })
    .map_err(|error| error.to_string())?;
    watcher.watch(&repo, RecursiveMode::Recursive).map_err(|error| error.to_string())?;
    *slot = Some(watcher);
    Ok(())
}

fn ignored_path(path: &Path) -> bool {
    let text = path.to_string_lossy().replace('\\', "/");
    text.contains("/.git/objects/")
        || text.contains("/.git/modules/")
        || text.contains("/target/")
        || text.contains("/node_modules/")
        || text.contains("/.svelte-kit/")
        || text.ends_with(".lock")
}

#[tauri::command]
fn open_terminal(path: Option<String>) -> Result<(), String> {
    let repo = repo_from(path)?;
    let loaded = settings::load().ok();
    if let Some(values) = loaded.as_ref() {
        if values.shell_kind != "default" && !values.shell_path.trim().is_empty() {
            let mut command = std::process::Command::new(&values.shell_path);
            if !values.shell_args.trim().is_empty() {
                command.args(split_command_args(&values.shell_args));
            }
            command.current_dir(&repo).spawn().map_err(|error| error.to_string())?;
            return Ok(());
        }
    }
    let configured = loaded.map(|settings| settings.terminal).unwrap_or_default();
    let mut command = std::process::Command::new("cmd");
    if configured.trim().is_empty() {
        command.args(["/C", "start", "cmd"]);
    } else {
        command.arg("/C").arg(configured);
    }
    command.current_dir(&repo).spawn().map_err(|error| error.to_string())?;
    Ok(())
}

fn split_command_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in text.chars() {
        match (quote, ch) {
            (None, ' ' | '\t') => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            (None, '"' | '\'') => quote = Some(ch),
            (Some(mark), found) if found == mark => quote = None,
            (_, found) => current.push(found),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

#[tauri::command]
fn pick_directory() -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms | Out-Null
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.ShowNewFolderButton = $true
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $dialog.SelectedPath }
"#;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-STA", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if err.is_empty() { "Could not open the folder picker.".into() } else { err });
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok(if text.is_empty() { None } else { Some(text) });
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("osascript")
            .args(["-e", "try", "-e", "POSIX path of (choose folder)", "-e", "on error number -128", "-e", "\"\"", "-e", "end try"])
            .output()
            .map_err(|error| error.to_string())?;
        let text = String::from_utf8_lossy(&output.stdout).trim().trim_end_matches('/').to_string();
        return Ok(if text.is_empty() { None } else { Some(text) });
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Err("This system has no folder picker.".into())
    }
}

#[tauri::command]
fn file_preview(path: Option<String>, file: String) -> Result<Option<awegit_git::FilePreview>, String> {
    let repo = repo_from(path)?;
    awegit_git::file_preview(&repo, &file).map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_operation() -> Result<(), String> {
    awegit_git::cancel_running();
    Ok(())
}

#[tauri::command]
fn set_passphrase(user: String, secret: String) -> Result<(), String> {
    awegit_git::set_passphrase(user, secret);
    Ok(())
}

#[tauri::command]
fn approve_credential(path: Option<String>, protocol: String, host: String, username: String, password: String) -> Result<(), String> {
    let repo = repo_from(path)?;
    let _guard = GIT_WRITE.lock().map_err(|_| "a Git write was interrupted".to_string())?;
    apply_session();
    awegit_git::approve_credential(&repo, &protocol, &host, &username, &password).map_err(|error| error.to_string())
}

#[tauri::command]
fn check_for_update() -> Result<Option<updates::UpdateNotice>, String> {
    updates::check()
}

#[tauri::command]
fn conflict_sides(path: Option<String>, file: String) -> Result<awegit_git::ConflictSides, String> {
    let repo = repo_from(path)?;
    awegit_git::conflict_sides(&repo, &file).map_err(|error| error.to_string())
}

#[tauri::command]
fn compare_files(path: Option<String>, from: String, to: String) -> Result<Vec<awegit_git::FileChange>, String> {
    let repo = repo_from(path)?;
    awegit_git::compare_files(&repo, &from, &to).map_err(|error| error.to_string())
}

#[tauri::command]
fn compare_file(path: Option<String>, from: String, to: String, file: String, unified: Option<u32>) -> Result<awegit_git::FileDiff, String> {
    let repo = repo_from(path)?;
    awegit_git::compare_file(&repo, &from, &to, &file, unified.unwrap_or(3)).map_err(|error| error.to_string())
}

#[tauri::command]
fn revision_preview(path: Option<String>, rev: String, file: String) -> Result<Option<awegit_git::FilePreview>, String> {
    let repo = repo_from(path)?;
    awegit_git::blob_preview(&repo, &rev, &file).map_err(|error| error.to_string())
}

#[tauri::command]
fn blob_view(path: Option<String>, rev: String, file: String) -> Result<awegit_git::BlobView, String> {
    let repo = repo_from(path)?;
    awegit_git::blob_view(&repo, &rev, &file).map_err(|error| error.to_string())
}

#[tauri::command]
fn reflog(path: Option<String>, limit: Option<usize>) -> Result<Vec<awegit_git::ReflogRow>, String> {
    let repo = repo_from(path)?;
    awegit_git::reflog(&repo, limit.unwrap_or(200)).map_err(|error| error.to_string())
}

#[tauri::command]
fn commit_tree(path: Option<String>, rev: String) -> Result<Vec<String>, String> {
    let repo = repo_from(path)?;
    awegit_git::commit_tree(&repo, &rev).map_err(|error| error.to_string())
}

#[tauri::command]
fn repository_summary(path: String) -> Result<awegit_git::RepoSummary, String> {
    awegit_git::repository_summary(std::path::Path::new(&path)).map_err(|error| error.to_string())
}

#[tauri::command]
fn rebase_conflicts(path: Option<String>, onto: String) -> Result<Vec<String>, String> {
    let repo = repo_from(path)?;
    awegit_git::rebase_conflicts(&repo, &onto).map_err(|error| error.to_string())
}

#[tauri::command]
fn lfs_locks(path: Option<String>) -> Result<LfsLockReport, String> {
    let repo = repo_from(path)?;
    let (available, locks) = awegit_git::lfs_locks(&repo).map_err(|error| error.to_string())?;
    Ok(LfsLockReport { available, locks })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LfsLockReport {
    available: bool,
    locks: Vec<awegit_git::LfsLockRow>,
}

#[tauri::command]
fn launch_tool(path: Option<String>, kind: String, file: String) -> Result<(), String> {
    let repo = repo_from(path)?;
    let values = settings::load()?;
    let result = if kind == "merge" {
        awegit_git::launch_merge_tool(&repo, &values.merge_tool, &file)
    } else {
        awegit_git::launch_diff_tool(&repo, &values.diff_tool, &file, false)
    };
    result.map_err(|error| error.to_string())
}

#[tauri::command]
fn account_token(values: &settings::Settings, forge_name: &str, fallback: &str) -> String {
    if !fallback.trim().is_empty() {
        return fallback.to_string();
    }
    values
        .accounts
        .iter()
        .find(|item| item.forge == forge_name && !item.token.trim().is_empty())
        .map(|item| item.token.clone())
        .unwrap_or_default()
}

#[tauri::command]
fn forge_notifications() -> Result<Vec<forge::Notice>, String> {
    let values = settings::load()?;
    forge::notifications(&account_token(&values, "github", &values.github_token))
}

#[tauri::command]
fn forge_pulls(path: Option<String>) -> Result<Vec<forge::PullRequest>, String> {
    let repo = repo_from(path)?;
    let values = settings::load()?;
    let url = std::process::Command::new("git")
        .current_dir(&repo)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();
    forge::pulls(
        &url,
        &account_token(&values, "github", &values.github_token),
        &account_token(&values, "gitlab", &values.gitlab_token),
        &values.gitlab_host,
    )
}

#[tauri::command]
fn repo_facts(path: Option<String>) -> Result<awegit_git::RepoFacts, String> {
    let repo = repo_from(path)?;
    awegit_git::repo_facts(&repo).map_err(|error| error.to_string())
}

#[tauri::command]
fn scan_repositories(root: String) -> Result<Vec<String>, String> {
    awegit_git::scan_repositories(std::path::Path::new(&root)).map_err(|error| error.to_string())
}

#[tauri::command]
fn mark_notification(id: String) -> Result<(), String> {
    let values = settings::load()?;
    forge::mark_read(&account_token(&values, "github", &values.github_token), &id)
}

#[tauri::command]
fn create_github_repo(name: String, private_repo: bool) -> Result<String, String> {
    let values = settings::load()?;
    forge::create_repo(&account_token(&values, "github", &values.github_token), &name, private_repo)
}

#[tauri::command]
fn generate_ssh_key() -> Result<String, String> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map_err(|_| "home directory is missing".to_string())?;
    let dir = std::path::PathBuf::from(home).join(".ssh");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let key = dir.join("id_ed25519");
    if key.exists() {
        return Err("id_ed25519 already exists".into());
    }
    let output = std::process::Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-f"])
        .arg(&key)
        .args(["-N", "", "-C", "awegit"])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    std::fs::read_to_string(dir.join("id_ed25519.pub")).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_repo_author(path: Option<String>, name: String, email: String) -> Result<(), String> {
    let repo = repo_from(path)?;
    let _guard = GIT_WRITE.lock().map_err(|_| "a Git write was interrupted".to_string())?;
    awegit_git::set_repo_author(&repo, &name, &email).map_err(|error| error.to_string())
}

#[tauri::command]
fn search_commits(path: Option<String>, kind: String, needle: String) -> Result<Vec<awegit_git::SearchHit>, String> {
    let repo = repo_from(path)?;
    awegit_git::search_commits(&repo, &kind, &needle).map_err(|error| error.to_string())
}

#[tauri::command]
fn cherry_preview(path: Option<String>, rev: String) -> Result<String, String> {
    let repo = repo_from(path)?;
    awegit_git::cherry_preview(&repo, &rev).map_err(|error| error.to_string())
}

#[tauri::command]
fn merged_branches(path: Option<String>) -> Result<Vec<String>, String> {
    let repo = repo_from(path)?;
    awegit_git::merged_branches(&repo).map_err(|error| error.to_string())
}

#[tauri::command]
fn pick_history(path: Option<String>, file: String, text: String) -> Result<String, String> {
    let repo = repo_from(path)?;
    awegit_git::pick_history(&repo, &file, &text).map_err(|error| error.to_string())
}

#[tauri::command]
fn activity_log() -> Vec<String> {
    perf::activity()
}

#[tauri::command]
fn create_hosted_repo(forge_name: String, name: String, private_repo: bool, host: String, org: String) -> Result<String, String> {
    let values = settings::load()?;
    let fallback = match forge_name.as_str() {
        "gitlab" => values.gitlab_token.clone(),
        "bitbucket" => values.bitbucket_token.clone(),
        "azure" => values.azure_token.clone(),
        _ => values.github_token.clone(),
    };
    let token = account_token(&values, &forge_name, &fallback);
    forge::create_hosted(&forge_name, &token, &host, &name, private_repo, &org)
}

#[tauri::command]
fn device_start(forge_name: String, client_id: String, host: String) -> Result<forge::DeviceStart, String> {
    forge::device_start(&forge_name, &client_id, &host)
}

#[tauri::command]
fn device_poll() -> Result<String, String> {
    forge::device_poll()
}

#[tauri::command]
fn review_branch(path: Option<String>, base: String) -> Result<String, String> {
    let repo = repo_from(path)?;
    let values = settings::load()?;
    ai::review(&repo, &values, &base)
}

#[tauri::command]
fn read_ssh_config() -> Result<String, String> {
    let path = ssh_config_path()?;
    if !path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(path).map_err(|error| error.to_string())
}

#[tauri::command]
fn write_ssh_config(text: String) -> Result<(), String> {
    if text.contains('\0') {
        return Err("ssh config contains a null".into());
    }
    let path = ssh_config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, text).map_err(|error| error.to_string())
}

fn ssh_config_path() -> Result<std::path::PathBuf, String> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map_err(|_| "home directory is missing".to_string())?;
    Ok(std::path::PathBuf::from(home).join(".ssh").join("config"))
}

#[tauri::command]
fn open_editor(path: Option<String>, file: String, line: u32) -> Result<(), String> {
    let repo = repo_from(path)?;
    let full = repo.join(&file);
    let spec = format!("{}:{line}", full.display());
    for bin in ["cursor", "code"] {
        if std::process::Command::new(bin).arg("-g").arg(&spec).spawn().is_ok() {
            return Ok(());
        }
    }
    std::process::Command::new("notepad").arg(&full).spawn().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_window(app: tauri::AppHandle, view: Option<String>, repo: Option<String>, file: Option<String>) -> Result<(), String> {
    let label = match view.as_deref() {
        Some("blame") => "blame",
        Some("history") => "history",
        _ => "desk",
    };
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_focus();
        return Ok(());
    }
    let mut query = String::new();
    if let Some(view) = view.as_deref() {
        query.push_str(&format!("view={}&", encode_query(view)));
    }
    if let Some(repo) = repo.as_deref() {
        query.push_str(&format!("repo={}&", encode_query(repo)));
    }
    if let Some(file) = file.as_deref() {
        query.push_str(&format!("file={}", encode_query(file)));
    }
    let url = if query.is_empty() { "index.html".to_string() } else { format!("index.html?{query}") };
    tauri::webview::WebviewWindowBuilder::new(&app, label, tauri::WebviewUrl::App(url.into()))
        .title("AweGit")
        .inner_size(1100.0, 720.0)
        .build()
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn encode_query(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[tauri::command]
fn suggest_commit_message(path: Option<String>) -> Result<ai::Suggestion, String> {
    let repo = repo_from(path)?;
    let values = settings::load()?;
    ai::suggest(&repo, &values)
}

#[tauri::command]
fn log_client(entries: Vec<perf::ClientEntry>) {
    perf::client(entries);
}

#[tauri::command]
fn perf_snapshot() -> perf::Snapshot {
    perf::snapshot()
}

#[tauri::command]
fn open_log_folder() -> Result<(), String> {
    let dir = perf::dir();
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let program = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    std::process::Command::new(program).arg(&dir).spawn().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    perf::init();
    awegit_git::set_trace(perf::git);
    prefer_bundled_git();
    let handler: Box<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync> = Box::new(tauri::generate_handler![
            workspace_status,
            stage_all,
            unstage_all,
            stage_path,
            unstage_path,
            commit_changes,
            file_diff,
            commit_log,
            commit_detail,
            repository_refs,
            commit_files,
            show_commit_file,
            in_progress,
            mutate,
            file_history,
            blame_file,
            load_settings,
            save_settings,
            watch_repository,
            open_terminal,
            pick_directory,
            suggest_commit_message,
            file_preview,
            cancel_operation,
            set_passphrase,
            approve_credential,
            check_for_update,
            conflict_sides,
            compare_files,
            compare_file,
            set_repo_author,
            revision_preview,
            blob_view,
            reflog,
            commit_tree,
            repository_summary,
            rebase_conflicts,
            lfs_locks,
            launch_tool,
            forge_notifications,
            forge_pulls,
            repo_facts,
            scan_repositories,
            mark_notification,
            create_github_repo,
            generate_ssh_key,
            search_commits,
            cherry_preview,
            merged_branches,
            pick_history,
            activity_log,
            create_hosted_repo,
            device_start,
            device_poll,
            review_branch,
            read_ssh_config,
            write_ssh_config,
            open_editor,
            open_window,
            log_client,
            perf_snapshot,
            open_log_folder
        ]);
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(RepoWatch(Mutex::new(None)))
        .setup(|app| {
            perf::mark("setup");
            if let Ok(dir) = app.path().resource_dir() {
                if let Some(cmd) = git_cmd(&dir) {
                    awegit_git::use_bundled_git(&cmd);
                }
            }
            perf::watch_main_thread(app.handle().clone());
            Ok(())
        })
        .on_page_load(|_, payload| {
            perf::mark(&format!("page {:?} {}", payload.event(), payload.url()));
        })
        .invoke_handler(move |invoke| {
            let name = invoke.message.command().to_string();
            let _span = (!matches!(name.as_str(), "log_client" | "perf_snapshot")).then(|| perf::command(&name));
            handler(invoke)
        })
        .build(tauri::generate_context!())
        .expect("error while building AweGit")
        .run(|_, event| {
            if let tauri::RunEvent::Exit = event {
                perf::summary();
            }
        });
}
