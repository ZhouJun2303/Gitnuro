mod common;

use std::fs;

use awegit_git::{
    commit, discard_hunk, discard_line, file_diff, in_progress, perform, stage_all, stage_hunk, stage_line, status, ChangeKind, CommitRequest, InProgress, Mutation,
};

fn repo() -> common::TempRepo {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.git(&["config", "core.hooksPath", ".git/no-hooks"]);
    repo.git(&["config", "user.name", "Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    repo
}

fn commit_file(repo: &common::TempRepo, name: &str, body: &str, message: &str) {
    fs::write(repo.path.join(name), body).unwrap();
    repo.git(&["add", "--", name]);
    repo.git(&["commit", "-m", message]);
}

#[test]
fn mutation_payloads_use_camel_case_fields() {
    let push: Mutation = serde_json::from_value(serde_json::json!({
        "action": "push", "remote": "origin", "setUpstream": true, "tags": true,
        "forceWithLease": true, "force": false
    }))
    .unwrap();
    assert!(matches!(push, Mutation::Push { set_upstream: true, force_with_lease: true, tags: true, .. }));

    let stash: Mutation = serde_json::from_value(serde_json::json!({
        "action": "stash", "message": "m", "includeUntracked": true
    }))
    .unwrap();
    assert!(matches!(stash, Mutation::Stash { include_untracked: true, .. }));

    let paths: Mutation = serde_json::from_value(serde_json::json!({
        "action": "stashPaths", "files": ["a"], "includeUntracked": true
    }))
    .unwrap();
    assert!(matches!(paths, Mutation::StashPaths { include_untracked: true, .. }));

    let merge: Mutation = serde_json::from_value(serde_json::json!({
        "action": "merge", "name": "t", "squash": false, "noFf": true,
        "autostash": false, "summary": "", "description": ""
    }))
    .unwrap();
    assert!(matches!(merge, Mutation::Merge { no_ff: true, .. }));

    let rebase: Mutation = serde_json::from_value(serde_json::json!({
        "action": "rebaseInteractive", "onto": "HEAD~1", "drop": [], "steps": [],
        "autostash": false, "updateRefs": true
    }))
    .unwrap();
    assert!(matches!(rebase, Mutation::RebaseInteractive { update_refs: true, .. }));

    let push_ref: Mutation = serde_json::from_value(serde_json::json!({
        "action": "pushRef", "remote": "origin", "branch": "a", "remoteBranch": "b",
        "setUpstream": true, "forceWithLease": true, "force": false, "tags": false
    }))
    .unwrap();
    assert!(matches!(
        push_ref,
        Mutation::PushRef { remote_branch, set_upstream: true, force_with_lease: true, .. } if remote_branch == "b"
    ));

    let stage: Mutation = serde_json::from_value(serde_json::json!({
        "action": "stageHunk", "file": "a", "index": 1, "ignoreSpace": true
    }))
    .unwrap();
    assert!(matches!(stage, Mutation::StageHunk { ignore_space: true, .. }));

    let discard: Mutation = serde_json::from_value(serde_json::json!({
        "action": "discardHunk", "file": "a", "index": 1, "ignoreSpace": true
    }))
    .unwrap();
    assert!(matches!(discard, Mutation::DiscardHunk { ignore_space: true, .. }));
}

#[test]
fn line_stage_preserves_crlf_and_a_missing_trailing_newline() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), b"a\r\nb\r\nc").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("a.txt"), b"a\r\nB\r\nc").unwrap();

    let diff = file_diff(&repo.path, "a.txt", false).unwrap();
    let line = diff.lines.iter().find(|line| line.text == "+B").expect("added line");
    stage_line(&repo.path, "a.txt", "B", true, line.stage_at.expect("stage index"), false).unwrap();

    assert_eq!(repo.output(&["show", ":a.txt"]), "a\r\nB\r\nb\r\nc");
    let cached = repo.output(&["diff", "--cached"]);
    assert!(cached.contains("+B"), "{cached}");
}

#[test]
fn removing_the_unterminated_last_line_keeps_no_trailing_newline() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), b"a\nb").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("a.txt"), b"a\n").unwrap();

    let diff = file_diff(&repo.path, "a.txt", false).unwrap();
    let line = diff.lines.iter().find(|line| line.text == "-b").expect("removed line");
    stage_line(&repo.path, "a.txt", "b", false, line.stage_at.expect("stage index"), false).unwrap();
    assert_eq!(repo.output(&["show", ":a.txt"]), "a");
}

#[test]
fn line_stage_refuses_non_utf8_files() {
    let repo = repo();
    let base = [0xff, b'\n', 0xfe, b'\n'];
    fs::write(repo.path.join("bin.txt"), base).unwrap();
    repo.git(&["add", "bin.txt"]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("bin.txt"), [0xff, b'\n', 0xfe, b'\n', b'Z', b'\n']).unwrap();

    let diff = file_diff(&repo.path, "bin.txt", false).unwrap();
    let line = diff.lines.iter().find(|line| line.text == "+Z").expect("added line");
    let error = stage_line(&repo.path, "bin.txt", "Z", true, line.stage_at.expect("stage index"), false)
        .unwrap_err()
        .to_string();
    assert!(error.contains("not valid UTF-8"), "{error}");
}

#[test]
fn discard_line_preserves_crlf_and_refuses_non_utf8() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), b"a\r\nb\r\nc").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("a.txt"), b"a\r\nc").unwrap();
    let diff = file_diff(&repo.path, "a.txt", false).unwrap();
    let line = diff.lines.iter().find(|line| line.text == "-b").expect("removed line");
    discard_line(&repo.path, "a.txt", "b", false, line.work_at.expect("work index")).unwrap();
    assert_eq!(fs::read(repo.path.join("a.txt")).unwrap(), b"a\r\nb\r\nc");

    let original = [0xff, b'\n', 0xfe, b'\n'];
    fs::write(repo.path.join("bin.txt"), original).unwrap();
    let error = discard_line(&repo.path, "bin.txt", "x", false, 0).unwrap_err().to_string();
    assert!(error.contains("not valid UTF-8"), "{error}");
    assert_eq!(fs::read(repo.path.join("bin.txt")).unwrap(), original);
}

#[test]
fn hunk_operations_follow_the_ignore_space_view() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "a\nb\nc\nd\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "base"]);
    fs::write(repo.path.join("a.txt"), "a\n b\nc\nD\n").unwrap();

    stage_hunk(&repo.path, "a.txt", 0, false, true).unwrap();
    assert_eq!(repo.output(&["show", ":a.txt"]), "a\nb\nc\nD\n");
    let staged = repo.output(&["diff", "--cached"]);
    assert!(staged.contains("+D"), "{staged}");
    assert!(!staged.lines().any(|line| line == "+ b"), "{staged}");
    let unstaged = repo.output(&["diff"]);
    assert!(unstaged.lines().any(|line| line == "+ b"), "{unstaged}");
    assert!(!unstaged.contains("+D"), "{unstaged}");

    repo.git(&["reset", "--hard"]);
    fs::write(repo.path.join("a.txt"), "a\n b\nc\nD\n").unwrap();
    discard_hunk(&repo.path, "a.txt", 0, true).unwrap();
    assert_eq!(fs::read_to_string(repo.path.join("a.txt")).unwrap(), "a\n b\nc\nd\n");
}

#[test]
fn stage_all_leaves_unmerged_paths_alone_and_commit_refuses_them() {
    let repo = repo();
    commit_file(&repo, "a.txt", "base\n", "base");
    repo.git(&["checkout", "-b", "side"]);
    commit_file(&repo, "a.txt", "side\n", "side");
    repo.git(&["checkout", "main"]);
    commit_file(&repo, "a.txt", "main\n", "main");
    assert!(perform(
        &repo.path,
        Mutation::Merge { name: "side".into(), squash: false, no_ff: false, autostash: false, summary: String::new(), description: String::new() },
    )
    .is_err());
    fs::write(repo.path.join("other.txt"), "other\n").unwrap();

    stage_all(&repo.path).unwrap();
    let snapshot = status(&repo.path).unwrap();
    assert!(snapshot.unstaged.iter().any(|file| file.path == "a.txt" && file.kind == ChangeKind::Conflict), "{snapshot:?}");
    assert!(snapshot.staged.iter().any(|file| file.path == "other.txt"), "{snapshot:?}");

    let error = commit(
        &repo.path,
        CommitRequest {
            summary: "test: merge".into(),
            description: String::new(),
            amend: false,
            sign_off: false,
            sign_off_format: String::new(),
            skip_hooks: true,
            convention: false,
        },
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("resolve all conflicts"), "{error}");
    perform(&repo.path, Mutation::Abort).unwrap();
}

#[test]
fn discarding_many_paths_does_not_deadlock() {
    let repo = repo();
    let mut files = Vec::new();
    for index in 0..200 {
        let path = format!("deep/dir-{}/file-{:03}-{}.txt", index % 5, index, "x".repeat(30));
        fs::create_dir_all(repo.path.join("deep").join(format!("dir-{}", index % 5))).unwrap();
        fs::write(repo.path.join(&path), "base\n").unwrap();
        files.push(path);
    }
    repo.git(&["add", "--", "."]);
    repo.git(&["commit", "-m", "many"]);
    for path in &files {
        fs::write(repo.path.join(path), "changed\n").unwrap();
    }

    perform(&repo.path, Mutation::DiscardPaths { files }).unwrap();
    let snapshot = status(&repo.path).unwrap();
    assert!(snapshot.staged.is_empty() && snapshot.unstaged.is_empty(), "{snapshot:?}");
}

#[test]
fn a_conflict_subject_containing_editor_is_not_treated_as_an_editor_failure() {
    let repo = repo();
    commit_file(&repo, "a.txt", "base\n", "base");
    repo.git(&["checkout", "-b", "topic"]);
    commit_file(&repo, "a.txt", "topic\n", "editor topic");
    repo.git(&["checkout", "main"]);
    commit_file(&repo, "a.txt", "main\n", "main");
    let onto = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    repo.git(&["checkout", "topic"]);

    let result = perform(
        &repo.path,
        Mutation::RebaseInteractive { onto, drop: Vec::new(), steps: Vec::new(), autostash: false, update_refs: false },
    );
    assert!(result.is_err(), "rebase should conflict");
    assert_eq!(in_progress(&repo.path).unwrap(), Some(InProgress::Rebase));
    perform(&repo.path, Mutation::Abort).unwrap();
}
