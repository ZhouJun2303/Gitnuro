mod common;

use std::fs;

use awegit_git::{stage_all, status, status_with, unstage_fakeignored, ChangeKind, FileChange};

#[test]
fn native_ignore_checks_match_git_and_reload_rules_and_the_index() {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.quotePath", "false"]);
    fs::create_dir_all(repo.path.join("nested")).unwrap();
    fs::create_dir_all(repo.path.join("cache")).unwrap();
    fs::write(repo.path.join(".gitignore"), "*.tmp\ncache/\n!keep.tmp\n").unwrap();
    fs::write(repo.path.join("nested/.gitignore"), "!keep.tmp\nonly.log\n").unwrap();
    fs::write(repo.path.join(".git/info/exclude"), "private.txt\n").unwrap();
    fs::write(repo.path.join("global-ignore"), "global.txt\n").unwrap();
    repo.git(&["config", "core.excludesFile", &repo.path.join("global-ignore").display().to_string()]);
    let paths: Vec<String> = ["tracked.tmp", "drop.tmp", "keep.tmp", "nested/drop.tmp", "nested/keep.tmp", "nested/only.log", "cache/hidden.txt", "private.txt", "global.txt", "正常.txt"]
        .into_iter().map(str::to_string).collect();
    for path in &paths { fs::write(repo.path.join(path), "content\n").unwrap(); }
    repo.git(&["add", "-f", "tracked.tmp"]);
    let verify = || {
        let mut args = vec!["check-ignore", "--"];
        args.extend(paths.iter().map(String::as_str));
        let output = repo.raw(&args);
        assert!(matches!(output.status.code(), Some(0 | 1)));
        let expected: Vec<String> = String::from_utf8_lossy(&output.stdout).lines().map(str::to_string).collect();
        assert_eq!(awegit_git::ignored_paths(&repo.path, &paths), expected);
    };
    verify();
    fs::write(repo.path.join("nested/.gitignore"), "!drop.tmp\n!only.log\n").unwrap();
    verify();
    repo.git(&["add", "-f", "drop.tmp"]);
    verify();
    fs::write(repo.path.join("global-ignore"), "!global.txt\n").unwrap();
    fs::write(repo.path.join(".git/info/exclude"), "!private.txt\n").unwrap();
    fs::write(repo.path.join(".gitignore"), "cache/\n!keep.tmp\n").unwrap();
    verify();
}

#[test]
fn native_ignore_checks_respect_case_insensitive_tracked_paths() {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.ignorecase", "true"]);
    fs::write(repo.path.join(".gitignore"), "*.tmp\n").unwrap();
    fs::write(repo.path.join("tracked.tmp"), "tracked\n").unwrap();
    repo.git(&["add", "-f", "tracked.tmp"]);
    let paths = vec!["TRACKED.TMP".into(), "DROP.TMP".into()];
    assert_eq!(awegit_git::ignored_paths(&repo.path, &paths), vec!["DROP.TMP"]);
}

#[test]
fn linked_worktree_ignore_checks_use_the_shared_exclude_file() {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    fs::write(repo.path.join("note"), "base\n").unwrap();
    repo.git(&["add", "note"]);
    repo.git(&["commit", "-m", "base"]);
    let worktree = repo.path.join("linked");
    repo.git(&["worktree", "add", "-b", "linked", &worktree.display().to_string()]);
    fs::write(repo.path.join(".git/info/exclude"), "shared-private.txt\n").unwrap();
    fs::write(worktree.join("shared-private.txt"), "local\n").unwrap();
    let paths = vec!["shared-private.txt".into(), "note".into()];
    assert_eq!(awegit_git::ignored_paths(&worktree, &paths), vec!["shared-private.txt"]);
}

#[test]
fn missing_path_and_plain_directory_are_not_repositories() {
    let repo = common::TempRepo::new();
    let missing = repo.path.join("gone");
    let missing_error = status(&missing).unwrap_err().to_string();
    assert!(
        missing_error.ends_with(": missing"),
        "{missing_error}"
    );
    assert!(missing_error.contains(&missing.display().to_string()));

    let plain = repo.path.join("plain");
    fs::create_dir(&plain).unwrap();
    let plain_error = status(&plain).unwrap_err().to_string();
    assert!(plain_error.ends_with(": missing"), "{plain_error}");
}

#[test]
fn status_splits_staged_and_unstaged_changes() {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "status.showUntrackedFiles", "all"]);
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "initial"]);

    fs::write(repo.path.join("a.txt"), "two\n").unwrap();
    fs::write(repo.path.join("b.txt"), "new\n").unwrap();

    let before_add = status(&repo.path).unwrap();
    assert_eq!(before_add.branch.as_deref(), Some("main"));
    assert!(before_add.staged.is_empty(), "{:?}", before_add.staged);
    assert_has(&before_add.unstaged, "a.txt", ChangeKind::Modified);
    assert_has(&before_add.unstaged, "b.txt", ChangeKind::Untracked);

    repo.git(&["add", "b.txt"]);
    let after_add = status(&repo.path).unwrap();
    assert_has(&after_add.staged, "b.txt", ChangeKind::Added);
    assert_has(&after_add.unstaged, "a.txt", ChangeKind::Modified);
    assert!(!after_add.unstaged.iter().any(|change| change.path == "b.txt"));
}

#[test]
fn fakeignore_hides_the_same_paths_git_would_and_keeps_them_out_of_the_index() {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "user.name", "Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);

    fs::create_dir_all(repo.path.join("notes")).unwrap();
    fs::create_dir_all(repo.path.join("logs")).unwrap();
    fs::create_dir_all(repo.path.join("src")).unwrap();
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();
    fs::write(repo.path.join("notes/local.txt"), "local\n").unwrap();
    fs::write(repo.path.join("notes/keep.txt"), "keep\n").unwrap();
    fs::write(repo.path.join("README.md"), "readme\n").unwrap();
    fs::write(repo.path.join("src/README.md"), "nested\n").unwrap();
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "initial"]);

    fs::write(repo.path.join("a.txt"), "two\n").unwrap();
    fs::write(repo.path.join("notes/local.txt"), "changed\n").unwrap();
    fs::write(repo.path.join("notes/keep.txt"), "changed\n").unwrap();
    fs::write(repo.path.join("README.md"), "changed\n").unwrap();
    fs::write(repo.path.join("src/README.md"), "changed\n").unwrap();
    fs::write(repo.path.join("other.log"), "noise\n").unwrap();
    fs::write(repo.path.join("keep.log"), "keep\n").unwrap();
    fs::write(repo.path.join("logs/drop.txt"), "drop\n").unwrap();
    fs::write(repo.path.join("logs/keep.txt"), "keep\n").unwrap();
    fs::write(repo.path.join("Noise.LOG"), "case\n").unwrap();
    fs::write(
        repo.path.join(".fakeignore"),
        "\
# local test edits
notes/
*.log
!keep.log
/README.md
logs/*.txt
!logs/keep.txt
",
    )
    .unwrap();

    let paths = [
        "a.txt",
        "notes/local.txt",
        "notes/keep.txt",
        "README.md",
        "src/README.md",
        "other.log",
        "keep.log",
        "logs/drop.txt",
        "logs/keep.txt",
        "Noise.LOG",
        ".fakeignore",
    ];
    let full = status(&repo.path).unwrap();
    for path in paths {
        assert!(
            full.staged.iter().chain(full.unstaged.iter()).any(|change| change.path == path),
            "unfiltered status missing {path}: {full:?}"
        );
    }

    let filtered = status_with(&repo.path, true).unwrap();
    let shown: Vec<&str> = filtered
        .staged
        .iter()
        .chain(filtered.unstaged.iter())
        .map(|change| change.path.as_str())
        .collect();
    for path in paths {
        let code = repo.code(&["-c", "core.excludesFile=.fakeignore", "check-ignore", "--no-index", "-q", "--", path]);
        assert!(code == 0 || code == 1, "git check-ignore {path} exited {code}");
        let visible = shown.iter().any(|item| *item == path);
        assert_eq!(visible, code != 0, "{path} git_exit={code} visible={visible} shown={shown:?}");
    }
    assert!(shown.contains(&"a.txt"));
    assert!(shown.contains(&"keep.log"));
    assert!(shown.contains(&"logs/keep.txt"));
    assert!(shown.contains(&"src/README.md"));
    assert!(!shown.contains(&"notes/local.txt"));
    assert!(!shown.contains(&"other.log"));
    assert!(!shown.contains(&"README.md"));

    stage_all(&repo.path).unwrap();
    unstage_fakeignored(&repo.path).unwrap();
    let after = status(&repo.path).unwrap();
    assert_has(&after.staged, "a.txt", ChangeKind::Modified);
    assert_has(&after.staged, "src/README.md", ChangeKind::Modified);
    assert_has(&after.staged, "keep.log", ChangeKind::Added);
    assert_has(&after.unstaged, "notes/local.txt", ChangeKind::Modified);
    assert_has(&after.unstaged, "other.log", ChangeKind::Untracked);
    assert_has(&after.unstaged, "README.md", ChangeKind::Modified);
    assert!(!after.staged.iter().any(|change| change.path == "notes/local.txt"));

    fs::write(repo.path.join(".fakeignore"), "# off\n").unwrap();
    let restored = status_with(&repo.path, true).unwrap();
    assert!(
        restored.unstaged.iter().any(|change| change.path == "notes/local.txt"),
        "clearing .fakeignore should show the file again: {restored:?}"
    );
}

#[test]
fn codex_scratch_stays_out_of_status() {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "user.name", "Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "initial"]);
    fs::create_dir_all(repo.path.join(".tmp_artifact")).unwrap();
    fs::write(repo.path.join(".tmp_artifact").join("package.json"), "{}\n").unwrap();
    fs::create_dir_all(repo.path.join(".tmp_artifact2")).unwrap();
    fs::write(repo.path.join(".tmp_artifact2").join("note.txt"), "x\n").unwrap();
    fs::write(repo.path.join("b.txt"), "new\n").unwrap();
    let snapshot = status(&repo.path).unwrap();
    assert!(snapshot.unstaged.iter().any(|change| change.path == "b.txt"), "{snapshot:?}");
    assert!(
        snapshot.unstaged.iter().all(|change| !change.path.contains("tmp_artifact")),
        "{snapshot:?}"
    );
}

fn assert_has(changes: &[FileChange], path: &str, kind: ChangeKind) {
    assert!(
        changes.iter().any(|change| change.path == path && change.kind == kind),
        "missing {path} {kind:?} in {changes:?}"
    );
}
