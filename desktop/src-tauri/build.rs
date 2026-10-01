fn main() {
    let attributes = tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "workspace_status",
            "stage_all",
            "unstage_all",
            "stage_path",
            "unstage_path",
            "commit_changes",
            "file_diff",
            "commit_log",
            "repository_refs",
            "commit_files",
            "show_commit_file",
            "in_progress",
            "mutate",
            "file_history",
            "blame_file",
            "load_settings",
            "save_settings",
            "watch_repository",
            "open_terminal",
            "suggest_commit_message",
            "file_preview",
            "cancel_operation",
            "set_passphrase",
            "approve_credential",
            "check_for_update",
        ]),
    );
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
