mod common;

use std::fs;

use awegit_git::prompt_facts;

fn repo() -> common::TempRepo {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "core.hooksPath", ".git/no-hooks"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo
}

#[test]
fn prompt_only_describes_staged_index_content() {
    let repo = repo();
    fs::write(repo.path.join("staged.txt"), "baseline\n").unwrap();
    fs::write(repo.path.join("unstaged.txt"), "baseline\n").unwrap();
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "chore: initial"]);
    fs::write(repo.path.join("staged.txt"), "STAGED_CONTENT\n").unwrap();
    repo.git(&["add", "staged.txt"]);
    fs::write(repo.path.join("staged.txt"), "LATER_UNSTAGED_CONTENT\n").unwrap();
    fs::write(repo.path.join("unstaged.txt"), "OTHER_UNSTAGED_CONTENT\n").unwrap();
    fs::write(repo.path.join("untracked.txt"), "UNTRACKED_CONTENT\n").unwrap();
    let index = fs::read(repo.path.join(".git/index")).unwrap();

    for enabled in [false, true] {
        let (files, branch, recent, diff) = prompt_facts(&repo.path, enabled).unwrap();
        assert_eq!(files, "staged.txt");
        assert_eq!(branch, "main");
        assert_eq!(recent, "chore: initial");
        assert!(diff.contains("+STAGED_CONTENT"), "{diff}");
        assert!(!diff.contains("UNSTAGED"), "{diff}");
        assert!(!diff.contains("UNTRACKED"), "{diff}");
    }
    assert_eq!(fs::read(repo.path.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read_to_string(repo.path.join("staged.txt")).unwrap(), "LATER_UNSTAGED_CONTENT\n");
}

#[test]
fn prompt_filters_names_and_diff_with_fakeignore_and_respects_the_toggle() {
    let repo = repo();
    fs::create_dir(repo.path.join("notes")).unwrap();
    for (path, text) in [
        ("src[1].txt", "LITERAL_PATH"),
        ("src1.txt", "HIDDEN_GLOB_NEIGHBOR"),
        ("中文 文件.txt", "VISIBLE_UNICODE_PATH"),
        ("secret.local.txt", "HIDDEN_LOCAL"),
        ("notes/test.txt", "HIDDEN_NOTE"),
        ("notes/keep.txt", "VISIBLE_NEGATION"),
    ] {
        fs::write(repo.path.join(path), format!("{text}\n")).unwrap();
    }
    repo.git(&["add", "."]);
    fs::write(repo.path.join(".fakeignore"), "*.local.txt\nnotes/*\n!notes/keep.txt\nsrc1.txt\n").unwrap();
    let index = fs::read(repo.path.join(".git/index")).unwrap();

    let (files, _, _, diff) = prompt_facts(&repo.path, true).unwrap();
    assert_eq!(files.lines().collect::<Vec<_>>(), ["notes/keep.txt", "src[1].txt", "中文 文件.txt"]);
    for text in ["LITERAL_PATH", "VISIBLE_UNICODE_PATH", "VISIBLE_NEGATION"] {
        assert!(diff.contains(text), "missing {text}: {diff}");
    }
    assert!(!diff.contains("HIDDEN"), "{diff}");
    for path in ["secret.local.txt", "notes/test.txt", "src1.txt"] {
        assert!(!diff.contains(path), "{diff}");
    }

    let (files, _, _, diff) = prompt_facts(&repo.path, false).unwrap();
    for path in ["secret.local.txt", "notes/test.txt", "src1.txt"] {
        assert!(files.lines().any(|line| line == path), "{files}");
        assert!(diff.contains(path), "{diff}");
    }
    assert!(diff.contains("HIDDEN_LOCAL"));
    assert_eq!(fs::read(repo.path.join(".git/index")).unwrap(), index);
}

#[test]
fn prompt_returns_no_changes_when_the_index_is_empty_or_all_staged_files_are_hidden() {
    let repo = repo();
    let (files, _, _, diff) = prompt_facts(&repo.path, true).unwrap();
    assert!(files.is_empty() && diff.is_empty());

    fs::write(repo.path.join("local.txt"), "HIDDEN\n").unwrap();
    repo.git(&["add", "local.txt"]);
    fs::remove_file(repo.path.join("local.txt")).unwrap();
    fs::write(repo.path.join(".fakeignore"), "local.txt\n").unwrap();
    let index = fs::read(repo.path.join(".git/index")).unwrap();
    let (files, _, _, diff) = prompt_facts(&repo.path, true).unwrap();
    assert!(files.is_empty() && diff.is_empty());
    let (files, _, _, diff) = prompt_facts(&repo.path, false).unwrap();
    assert_eq!(files, "local.txt");
    assert!(diff.contains("+HIDDEN"));
    assert!(!repo.path.join("local.txt").exists());
    assert_eq!(fs::read(repo.path.join(".git/index")).unwrap(), index);
}

#[test]
fn prompt_filters_deleted_paths_and_both_sides_of_renames() {
    let repo = repo();
    fs::create_dir(repo.path.join("ignored")).unwrap();
    fs::write(repo.path.join("visible.txt"), "VISIBLE_OLD\n").unwrap();
    fs::write(repo.path.join("ignored/old.txt"), "MOVED_CONTENT\n").unwrap();
    fs::write(repo.path.join("ignored/deleted.txt"), "HIDDEN_DELETION\n").unwrap();
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "chore: initial"]);
    repo.git(&["mv", "visible.txt", "ignored/new.txt"]);
    repo.git(&["mv", "ignored/old.txt", "moved.txt"]);
    repo.git(&["rm", "ignored/deleted.txt"]);
    fs::write(repo.path.join(".fakeignore"), "ignored/\n").unwrap();

    let (files, _, _, diff) = prompt_facts(&repo.path, true).unwrap();
    assert_eq!(files, "moved.txt\nvisible.txt");
    assert!(diff.contains("+MOVED_CONTENT"), "{diff}");
    assert!(diff.contains("-VISIBLE_OLD"), "{diff}");
    assert!(!diff.contains("ignored/"), "{diff}");
    assert!(!diff.contains("HIDDEN_DELETION"), "{diff}");
}

#[test]
fn prompt_filters_large_ignored_diffs_before_the_limit_and_reads_all_path_batches() {
    let repo = repo();
    fs::write(repo.path.join("000-hidden.txt"), "HIDDEN".repeat(200_000)).unwrap();
    for number in 0..40 {
        fs::write(repo.path.join(format!("file-{number:02}.txt")), format!("VISIBLE_{number:02}\n")).unwrap();
    }
    repo.git(&["add", "."]);
    fs::write(repo.path.join(".fakeignore"), "000-hidden.txt\n").unwrap();

    let (files, _, _, diff) = prompt_facts(&repo.path, true).unwrap();
    assert_eq!(files.lines().count(), 40);
    for number in 0..40 {
        assert!(diff.contains(&format!("+VISIBLE_{number:02}")), "missing file {number}");
    }
    assert!(!diff.contains("HIDDEN"));
}

#[test]
fn prompt_does_not_silently_treat_a_git_failure_as_an_empty_index() {
    let repo = common::TempRepo::new();
    assert!(prompt_facts(&repo.path, true).is_err());
}
