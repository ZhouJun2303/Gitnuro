mod ai;
mod settings;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::Emitter;

static GIT_WRITE: Mutex<()> = Mutex::new(());
static WATCH_TICK: AtomicU64 = AtomicU64::new(0);

struct RepoWatch(Mutex<Option<RecommendedWatcher>>);

fn prefer_bundled_git() {
    let Ok(exe) = std::env::current_exe() else { return };
    let Some(parent) = exe.parent() else { return };
    let cmd = parent.join("git").join("cmd");
    if cmd.join("git.exe").is_file() {
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

fn apply_proxy() {
    let proxy = settings::load().ok().and_then(|settings| {
        let proxy = settings.proxy.trim().to_string();
        if proxy.is_empty() { None } else { Some(proxy) }
    });
    awegit_git::set_http_proxy(proxy);
}

fn write_then_status(repo: &Path, write: impl FnOnce(&Path) -> Result<(), awegit_git::Error>) -> Result<awegit_git::StatusSnapshot, String> {
    let _guard = GIT_WRITE.lock().map_err(|_| "a Git write was interrupted".to_string())?;
    apply_proxy();
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
) -> Result<awegit_git::StatusSnapshot, String> {
    let repo = repo_from(path)?;
    write_then_status(&repo, |repo| {
        awegit_git::commit(
            repo,
            awegit_git::CommitRequest {
                summary,
                description,
                amend,
                sign_off,
            },
        )
    })
}

#[tauri::command]
fn file_diff(path: Option<String>, file: String, staged: bool) -> Result<awegit_git::FileDiff, String> {
    let repo = repo_from(path)?;
    awegit_git::file_diff(&repo, &file, staged).map_err(|error| error.to_string())
}

#[tauri::command]
fn commit_log(path: Option<String>, limit: Option<usize>) -> Result<Vec<awegit_git::CommitRow>, String> {
    let repo = repo_from(path)?;
    awegit_git::commit_log(&repo, limit.unwrap_or(500)).map_err(|error| error.to_string())
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
fn show_commit_file(path: Option<String>, id: String, file: String) -> Result<awegit_git::FileDiff, String> {
    let repo = repo_from(path)?;
    awegit_git::show_commit_file(&repo, &id, &file).map_err(|error| error.to_string())
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

#[tauri::command]
fn mutate(path: Option<String>, request: awegit_git::Mutation) -> Result<awegit_git::StatusSnapshot, String> {
    let repo = repo_from(path)?;
    let _guard = GIT_WRITE.lock().map_err(|_| "a Git write was interrupted".to_string())?;
    apply_proxy();
    let next = match &request {
        awegit_git::Mutation::Clone { destination, .. } | awegit_git::Mutation::Init { destination } => {
            PathBuf::from(destination)
        }
        _ => repo.clone(),
    };
    awegit_git::perform(&repo, request).map_err(|error| error.to_string())?;
    read_status(&next)
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
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            if WATCH_TICK.load(Ordering::Relaxed) == ticket {
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
    let configured = settings::load().map(|settings| settings.terminal).unwrap_or_default();
    let mut command = std::process::Command::new("cmd");
    if configured.trim().is_empty() {
        command.args(["/C", "start", "cmd"]);
    } else {
        command.arg("/C").arg(configured);
    }
    command.current_dir(&repo).spawn().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn suggest_commit_message(path: Option<String>) -> Result<ai::Suggestion, String> {
    let repo = repo_from(path)?;
    let values = settings::load()?;
    ai::suggest(&repo, &values)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    prefer_bundled_git();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(RepoWatch(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            workspace_status,
            stage_all,
            unstage_all,
            stage_path,
            unstage_path,
            commit_changes,
            file_diff,
            commit_log,
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
            suggest_commit_message
        ])
        .run(tauri::generate_context!())
        .expect("error while running AweGit");
}
