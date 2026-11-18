use tauri::ipc::Origin;

// Use Tauri's compiled ACL: the browser lab does not enforce desktop permissions.
fn context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

#[test]
fn main_window_can_invoke_registered_commands() {
    let mut context = context();
    let acl = context.runtime_authority_mut();
    let handler = include_str!("lib.rs")
        .split_once("tauri::generate_handler![").unwrap().1
        .split_once("]);").unwrap().0;
    let history_only = ["revision_copy_support", "create_revision_copy", "remove_revision_copy", "request_history_action"];
    let mut missing = Vec::new();
    for command in handler.split(',').map(str::trim).filter(|name| !name.is_empty()) {
        let command = command.rsplit("::").next().unwrap();
        let allowed = acl.resolve_access(command, "main", "main", &Origin::Local).is_some();
        if history_only.contains(&command) {
            assert!(!allowed, "{command} must remain exclusive to file history");
            assert!(acl.resolve_access(command, "history", "history", &Origin::Local).is_some());
        } else if !allowed {
            missing.push(command);
        }
    }
    assert!(missing.is_empty(), "Registered commands missing desktop ACL permissions: {missing:?}");
}

#[test]
fn scoped_status_reads_are_allowed_only_in_repository_windows() {
    let mut context = context();
    let acl = context.runtime_authority_mut();
    for window in ["main", "desk", "merge", "blame", "history", "blob"] {
        assert!(acl.resolve_access("workspace_status_paths", window, window, &Origin::Local).is_some(), "{window}");
    }
    for window in ["flow-test", "unrelated"] {
        assert!(acl.resolve_access("workspace_status_paths", window, window, &Origin::Local).is_none(), "{window}");
    }
    let remote = Origin::Remote { url: "https://example.com".parse().unwrap() };
    assert!(acl.resolve_access("workspace_status_paths", "main", "main", &remote).is_none());
}

#[test]
fn viewer_reads_do_not_grant_task_cancellation_or_lock_removal() {
    let mut context = context();
    let acl = context.runtime_authority_mut();
    for command in ["operation_tasks", "blame_source", "stash_patch", "scan_repositories_page"] {
        for window in ["blame", "history", "blob"] {
            assert!(acl.resolve_access(command, window, window, &Origin::Local).is_some(), "{command} in {window}");
        }
        assert!(acl.resolve_access(command, "flow-test", "flow-test", &Origin::Local).is_none(), "{command} in observer");
    }
    for command in ["cancel_task", "remove_index_lock", "remove_stale_lock"] {
        for window in ["main", "desk", "merge"] {
            assert!(acl.resolve_access(command, window, window, &Origin::Local).is_some(), "{command} in {window}");
        }
        for window in ["blame", "history", "blob", "flow-test"] {
            assert!(acl.resolve_access(command, window, window, &Origin::Local).is_none(), "{command} in {window}");
        }
    }
}
