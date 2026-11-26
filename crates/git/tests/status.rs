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
fn scoped_status_is_literal_and_keeps_unrelated_directories_out() {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    for dir in ["selected [1]", "selected 1", "unrelated"] { fs::create_dir(repo.path.join(dir)).unwrap(); }
    for path in ["selected [1]/tracked.txt", "selected 1/tracked.txt", "unrelated/tracked.txt"] { fs::write(repo.path.join(path), "base\n").unwrap(); }
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "base"]);
    for path in ["selected [1]/tracked.txt", "selected 1/tracked.txt", "unrelated/tracked.txt"] { fs::write(repo.path.join(path), "working\n").unwrap(); }
    fs::write(repo.path.join("selected [1]/loose.txt"), "new\n").unwrap();
    repo.git(&["--literal-pathspecs", "add", "--", "selected [1]/tracked.txt"]);
    let scoped = awegit_git::status_paths_with(&repo.path, false, &["selected [1]".into()]).unwrap();
    assert_eq!(scoped.staged.len(), 1);
    assert_has(&scoped.staged, "selected [1]/tracked.txt", ChangeKind::Modified);
    assert_eq!(scoped.unstaged.len(), 1);
    assert_has(&scoped.unstaged, "selected [1]/loose.txt", ChangeKind::Untracked);
    fs::write(repo.path.join(".fakeignore"), "loose.txt\n").unwrap();
    assert!(awegit_git::status_paths_with(&repo.path, true, &["selected [1]".into()]).unwrap().unstaged.is_empty());
    let single = awegit_git::status_paths_with(&repo.path, true, &["selected [1]/tracked.txt".into()]).unwrap();
    assert_eq!(single.staged.len(), 1);
    assert!(single.unstaged.is_empty());
    let empty = awegit_git::status_paths_with(&repo.path, false, &[]).unwrap();
    assert!(empty.staged.is_empty() && empty.unstaged.is_empty());
    assert!(awegit_git::status_paths_with(&repo.path, false, &["../outside".into()]).is_err());
}

#[test]
fn status_reads_a_submodule_with_windows_incompatible_tree_paths() {
    let source = common::TempRepo::new();
    source.git(&["init", "-b", "main"]);
    fs::write(source.path.join("base.txt"), "base\n").unwrap();
    source.git(&["add", "base.txt"]);
    source.git(&["commit", "-m", "submodule base"]);

    let repo = common::TempRepo::new();
    let submodule = "third_party/child [1]";
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.ignorecase", "true"]);
    repo.git(&[
        "-c", "protocol.file.allow=always", "submodule", "add", "--",
        source.path.to_str().unwrap(), submodule,
    ]);
    let in_submodule = |args: &[&str]| {
        let mut command = vec!["-C", submodule];
        command.extend_from_slice(args);
        repo.output(&command).trim().to_string()
    };
    in_submodule(&["config", "gitoxide.core.protectWindows", "true"]);

    // Put a foreign filename only in the commit tree, without checking it out.
    let blob = in_submodule(&["rev-parse", "HEAD:base.txt"]);
    let blob = gix::ObjectId::from_hex(blob.as_bytes()).unwrap();
    let mut tree = b"100644 base.txt\0".to_vec();
    tree.extend_from_slice(blob.as_slice());
    tree.extend_from_slice(b"100644 fixture?.txt\0");
    tree.extend_from_slice(blob.as_slice());
    let tree_file = repo.path.join(".git/foreign-tree");
    fs::write(&tree_file, tree).unwrap();
    let tree = in_submodule(&["hash-object", "-t", "tree", "-w", tree_file.to_str().unwrap()]);
    let head = in_submodule(&["commit-tree", &tree, "-p", "HEAD", "-m", "foreign filename"]);
    in_submodule(&["update-ref", "HEAD", &head]);

    fs::write(repo.path.join("old name.txt"), "base\n").unwrap();
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "add submodule"]);
    repo.git(&["mv", "old name.txt", "新 name.txt"]);
    fs::write(repo.path.join("新 name.txt"), "working\n").unwrap();
    fs::write(repo.path.join(".fakeignore"), "*.log\n").unwrap();
    fs::write(repo.path.join("local.log"), "ignored\n").unwrap();

    let index = repo.path.join(".git/index");
    let submodule_index = repo.path.join(".git/modules").join(submodule).join("index");
    let index_before = fs::read(&index).unwrap();
    let submodule_index_before = fs::read(&submodule_index).unwrap();
    let snapshot = status_with(&repo.path, true).unwrap();
    assert_eq!(snapshot.branch.as_deref(), Some("main"));
    assert_eq!(snapshot.staged, vec![FileChange {
        path: "新 name.txt".into(),
        kind: ChangeKind::Renamed,
        previous_path: Some("old name.txt".into()),
    }]);
    assert_has(&snapshot.unstaged, "新 name.txt", ChangeKind::Modified);
    assert_has(&snapshot.unstaged, submodule, ChangeKind::Modified);
    assert_has(&snapshot.unstaged, ".fakeignore", ChangeKind::Untracked);
    assert_eq!(snapshot.unstaged.len(), 3);

    let scoped = awegit_git::status_paths_with(&repo.path, true, &[submodule.into()]).unwrap();
    assert!(scoped.staged.is_empty());
    assert_eq!(scoped.unstaged.len(), 1);
    assert_has(&scoped.unstaged, submodule, ChangeKind::Modified);
    let different_case = awegit_git::status_paths_with(
        &repo.path, true, &[submodule.to_ascii_uppercase()],
    ).unwrap();
    assert_eq!(different_case, scoped);

    let mut wide_scope: Vec<_> = (0..256)
        .map(|number| format!("unrelated/{number}-{}", "x".repeat(128)))
        .collect();
    wide_scope.extend([submodule.into(), "old name.txt".into()]);
    let wide = awegit_git::status_paths_with(&repo.path, true, &wide_scope).unwrap();
    assert_eq!(wide.unstaged, scoped.unstaged);
    assert_eq!(wide.staged.len(), 1);
    assert_has(&wide.staged, "old name.txt", ChangeKind::Deleted);

    let child = status(repo.path.join(submodule)).unwrap();
    assert_has(&child.staged, "fixture?.txt", ChangeKind::Deleted);
    assert_eq!(fs::read(&index).unwrap(), index_before);
    assert_eq!(fs::read(&submodule_index).unwrap(), submodule_index_before);
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
