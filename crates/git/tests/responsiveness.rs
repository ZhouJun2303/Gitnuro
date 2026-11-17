mod common;

use awegit_git::{blame_file, blame_file_range, blame_source, commit_log_page, file_history_page, perform, reflog_page, repository_refs, search_log_page, Mutation, RepositoryScan};
use std::fs;

fn repo() -> common::TempRepo {
    let repo = common::TempRepo::new();
    repo.git(&["init", "-b", "main"]);
    repo.git(&["config", "user.name", "Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "core.hooksPath", ".git/no-hooks"]);
    repo
}

#[test]
fn history_search_file_and_reflog_pages_append_without_repeating_the_first_page() {
    let repo = repo();
    for index in 0..7 {
        fs::write(repo.path.join("file.txt"), format!("line {index}\n")).unwrap();
        repo.git(&["add", "file.txt"]);
        repo.git(&["commit", "-m", &format!("test: page {index}")]);
    }
    let log = commit_log_page(&repo.path, 20, true, &[], false, 0).unwrap();
    let ids = |rows: Vec<awegit_git::CommitRow>| rows.into_iter().map(|row| row.id).collect::<Vec<_>>();
    let expected = ids(log);
    for kind in ["log", "file", "search"] {
        let mut actual = Vec::new();
        for offset in [0, 3, 6] {
            let page = match kind {
                "log" => commit_log_page(&repo.path, 3, true, &[], false, offset),
                "file" => file_history_page(&repo.path, "file.txt", 3, offset),
                _ => search_log_page(&repo.path, "message", "branch", "page", 3, false, offset),
            }.unwrap();
            actual.extend(ids(page));
        }
        assert_eq!(actual, expected, "{kind}");
    }
    let all = reflog_page(&repo.path, 20, 0).unwrap();
    let pages: Vec<_> = [0, 3, 6].into_iter().flat_map(|offset| reflog_page(&repo.path, 3, offset).unwrap()).collect();
    assert_eq!(pages, all);
}

#[test]
fn ref_labels_peel_annotated_tags_without_losing_local_or_remote_names() {
    let repo = repo();
    repo.git(&["commit", "--allow-empty", "-m", "test: labels"]);
    repo.git(&["branch", "topic"]);
    repo.git(&["tag", "-a", "v1", "-m", "annotated"]);
    repo.git(&["update-ref", "refs/remotes/origin/topic", "HEAD"]);
    let id = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    let labels = repository_refs(&repo.path).unwrap().labels;
    assert!(labels[&id].iter().any(|label| label == "main"));
    assert!(labels[&id].iter().any(|label| label == "topic"));
    assert!(labels[&id].iter().any(|label| label == "v1"));
    assert!(labels[&id].iter().any(|label| label == "origin/topic"));
}

#[test]
fn stash_id_stays_bound_when_another_stash_changes_its_number() {
    let repo = repo();
    fs::write(repo.path.join("file.txt"), "base\n").unwrap();
    repo.git(&["add", "."]); repo.git(&["commit", "-m", "test: base"]);
    fs::write(repo.path.join("file.txt"), "first\n").unwrap();
    repo.git(&["stash", "push", "-m", "first"]);
    let id = repository_refs(&repo.path).unwrap().stashes[0].id.clone();
    fs::write(repo.path.join("file.txt"), "second\n").unwrap();
    repo.git(&["stash", "push", "-m", "second"]);
    perform(&repo.path, Mutation::StashPop { name: id }).unwrap();
    assert_eq!(fs::read_to_string(repo.path.join("file.txt")).unwrap(), "first\n");
    let stashes = repository_refs(&repo.path).unwrap().stashes;
    assert_eq!(stashes.len(), 1); assert!(stashes[0].summary.contains("second"));
}

#[test]
fn blame_reads_only_the_requested_lines_and_source_version_tracks_external_edits() {
    let repo = repo();
    fs::write(repo.path.join("file.txt"), (1..=450).map(|n| format!("line {n}\n")).collect::<String>()).unwrap();
    repo.git(&["add", "."]); repo.git(&["commit", "-m", "test: blame"]);
    let before = blame_source(&repo.path, "file.txt").unwrap();
    let whole = blame_file(&repo.path, "file.txt").unwrap();
    let visible = blame_file_range(&repo.path, "file.txt", Some((201, 400))).unwrap();
    assert_eq!(visible, whole[200..400]);
    fs::write(repo.path.join("file.txt"), "external edit\n").unwrap();
    assert_ne!(before.version, blame_source(&repo.path, "file.txt").unwrap().version);
}

#[test]
fn a_single_source_directory_yields_multiple_bounded_scan_pages() {
    let root = common::TempRepo::new();
    for index in 0..45 { fs::create_dir_all(root.path.join(format!("repo-{index}" )).join(".git")).unwrap(); }
    fs::create_dir_all(root.path.join("node_modules/ignored/.git")).unwrap();
    let mut scan = RepositoryScan::new(&root.path, 3).unwrap();
    let mut paths = Vec::new(); let mut pages = 0;
    loop {
        let (page, more) = scan.next_page(); pages += 1;
        assert!(page.paths.len() <= 16); assert!(!page.truncated);
        paths.extend(page.paths);
        if !more { break; }
    }
    paths.sort(); paths.dedup();
    assert_eq!(paths.len(), 45); assert!(pages >= 3);
}
