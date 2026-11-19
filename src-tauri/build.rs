use std::time::{SystemTime, UNIX_EPOCH};

// Baked into the binary so an installed copy can be told apart from a fresh
// build even when both carry the same version number.
fn build_stamp() -> String {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|time| time.as_secs()).unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { (mp + 3) as u32 } else { (mp - 9) as u32 };
    let year = yoe as i64 + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

fn app_code() -> i64 {
    if let Ok(value) = std::env::var("AWE_APP_CODE") {
        if let Ok(code) = value.trim().parse() {
            return code;
        }
    }
    let Ok(text) = std::fs::read_to_string("../latest.json") else { return 0 };
    let Some((_, rest)) = text.split_once("\"appCode\"") else { return 0 };
    let Some((_, rest)) = rest.split_once(':') else { return 0 };
    let digits: String = rest.trim_start().chars().take_while(|ch| ch.is_ascii_digit()).collect();
    digits.parse().unwrap_or(0)
}

fn main() {
    println!("cargo:rerun-if-changed=../src/lib/font-catalog.json");
    println!("cargo:rustc-env=AWE_BUILD_STAMP={}", build_stamp());
    println!("cargo:rustc-env=AWE_APP_CODE={}", app_code());
    let attributes = tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "workspace_status",
            "workspace_status_paths",
            "stage_all",
            "unstage_all",
            "stage_path",
            "unstage_path",
            "commit_changes",
            "file_diff",
            "commit_log",
            "commit_detail",
            "locate_ref",
            "repository_refs",
            "commit_files",
            "show_commit_file",
            "in_progress",
            "mutate",
            "operation_flow",
            "operation_history",
            "operation_tasks",
            "cancel_task",
            "file_history",
            "blame_file",
            "blame_source",
            "load_settings",
            "font_data",
            "download_font",
            "download_google_font",
            "recognize_google_font",
            "font_download_progress",
            "font_cache_info",
            "open_font_folder",
            "save_settings",
            "save_window_bounds",
            "settings_path",
            "import_fork_settings",
            "external_tools",
            "common_tools",
            "detect_terminal_clis",
            "editor_tools",
            "program_exists",
            "watch_repository",
            "open_terminal",
            "run_lite_agent",
            "clipboard_text",
            "pick_directory",
            "pick_file",
            "suggest_commit_message",
            "test_ai_endpoint",
            "list_ai_models",
            "file_preview",
            "cancel_operation",
            "cancel_reads",
            "remove_index_lock",
            "remove_stale_lock",
            "save_hidden_refs",
            "save_expanded_groups",
            "save_sign_off",
            "set_passphrase",
            "approve_credential",
            "check_for_update",
            "app_version",
            "app_info",
            "update_release_page",
            "download_update",
            "cancel_update",
            "conflict_sides",
            "stash_patch",
            "conflict_view",
            "compare_files",
            "compare_file",
            "set_repo_author",
            "revision_preview",
            "blob_view",
            "revision_copy_support",
            "revision_copy_formats",
            "create_revision_copy",
            "remove_revision_copy",
            "reflog",
            "commit_tree",
            "repository_summary",
            "repository_inspection",
            "rebase_conflicts",
            "lfs_locks",
            "launch_tool",
            "forge_notifications",
            "forge_pulls",
            "log_client",
            "perf_snapshot",
            "diagnostics",
            "open_log_folder",
            "repo_facts",
            "scan_repositories",
            "scan_repositories_page",
            "mark_notification",
            "create_github_repo",
            "generate_ssh_key",
            "search_commits",
            "search_log",
            "cherry_preview",
            "merge_preview",
            "merge_state",
            "merged_branches",
            "pick_history",
            "activity_log",
            "create_hosted_repo",
            "account_services",
            "sync_accounts",
            "test_account",
            "avatar_image",
            "browser_login",
            "cancel_browser_login",
            "remote_matches",
            "save_remote_account",
            "device_start",
            "device_poll",
            "review_branch",
            "read_ssh_config",
            "write_ssh_config",
            "open_editor",
            "open_window",
            "reveal_commit",
            "request_history_action",
            "save_revision",
            "open_file",
            "reveal_path",
            "git_runtime",
        ]),
    );
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
