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
        ]),
    );
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
