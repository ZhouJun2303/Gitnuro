#[tauri::command]
fn workspace_status(path: Option<String>) -> Result<awegit_git::StatusSnapshot, String> {
    let start = match path {
        Some(path) => std::path::PathBuf::from(path),
        None => std::env::current_dir().map_err(|error| error.to_string())?,
    };
    let repo = awegit_git::discover(&start).unwrap_or(start);
    awegit_git::status(repo).map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![workspace_status])
        .run(tauri::generate_context!())
        .expect("error while running AweGit");
}
