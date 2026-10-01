use std::path::{Path, PathBuf};
use std::sync::Mutex;

static GIT_WRITE: Mutex<()> = Mutex::new(());

fn repo_from(path: Option<String>) -> Result<PathBuf, String> {
    let start = match path {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir().map_err(|error| error.to_string())?,
    };
    Ok(awegit_git::discover(&start).unwrap_or(start))
}

fn read_status(repo: &Path) -> Result<awegit_git::StatusSnapshot, String> {
    awegit_git::status(repo).map_err(|error| error.to_string())
}

fn write_then_status(repo: &Path, write: impl FnOnce(&Path) -> Result<(), awegit_git::Error>) -> Result<awegit_git::StatusSnapshot, String> {
    let _guard = GIT_WRITE.lock().map_err(|_| "a Git write was interrupted".to_string())?;
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            workspace_status,
            stage_all,
            unstage_all,
            stage_path,
            unstage_path,
            commit_changes,
            file_diff
        ])
        .run(tauri::generate_context!())
        .expect("error while running AweGit");
}
