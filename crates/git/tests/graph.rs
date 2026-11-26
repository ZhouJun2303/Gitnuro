mod common;

use std::fs;
use std::sync::{atomic::AtomicBool, Arc};

use awegit_git::{
    blame_file, branch_commits, commit_detail, commit_files, commit_log, commit_tree, commits_from,
    compare_files, locate_ref, repository_refs, show_commit_file, with_cancellation, ChangeKind,
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

#[test]
fn empty_repository_has_no_commits() {
    let repo = repo();
    let log = commit_log(&repo.path, 20).unwrap();
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn cancelled_history_is_not_an_empty_repository() {
    let repo = repo();
    let token = Arc::new(AtomicBool::new(true));
    let error = with_cancellation(token, || commit_log(&repo.path, 20)).unwrap_err();
    assert!(error.to_string().contains("awegit:cancelled:"), "{error}");
}

#[test]
fn log_lists_commits_branches_and_tags() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "first"]);
    repo.git(&["branch", "topic"]);

    fs::rename(repo.path.join("a.txt"), repo.path.join("c.txt")).unwrap();
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-m", "rename"]);
    repo.git(&["tag", "v1"]);

    let log = commit_log(&repo.path, 20).unwrap();
    assert_eq!(log.len(), 2, "{log:?}");
    assert!(log[0].refs.iter().any(|name| name == "main"), "{:?}", log[0].refs);
    assert!(log[0].refs.iter().any(|name| name == "v1"), "{:?}", log[0].refs);
    assert!(log.iter().any(|commit| commit.refs.iter().any(|name| name == "topic")), "{log:?}");
    assert!(log.iter().all(|commit| commit.lane < 8));

    let files = commit_files(&repo.path, "HEAD").unwrap();
    let renamed = files.iter().find(|file| file.path == "c.txt").expect("renamed file");
    assert_eq!(renamed.kind, ChangeKind::Renamed);
    assert_eq!(renamed.previous_path.as_deref(), Some("a.txt"));

    let blame = blame_file(&repo.path, "c.txt").unwrap();
    assert!(blame.iter().any(|line| line.text.contains("one")), "{blame:?}");
    assert!(log.iter().all(|commit| commit.at > 0), "{log:?}");

    let detail = commit_detail(&repo.path, "HEAD").unwrap();
    assert_eq!(detail.author, "Test");
    assert_eq!(detail.committer, "Test");
    assert_eq!(detail.committer_email, "test@example.com");
    assert!(detail.author_at > 0, "{detail:?}");
    assert!(detail.body.is_empty(), "{detail:?}");
    assert_eq!(detail.summary, "rename");

    let refs = repository_refs(&repo.path).unwrap();
    assert!(refs.branches.iter().any(|branch| branch.name == "main" && branch.current));
    assert!(refs.branches.iter().any(|branch| branch.name == "topic" && !branch.current));
    assert!(refs.tags.iter().any(|tag| tag.name == "v1"));
    assert_eq!(refs.worktrees.len(), 1, "{:?}", refs.worktrees);
}

#[test]
fn repository_refs_notice_a_loose_branch_value_change() {
    let repo = repo();
    repo.git(&["commit", "--allow-empty", "-m", "first"]);
    let first = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    repo.git(&["commit", "--allow-empty", "-m", "second"]);
    let before = repository_refs(&repo.path).unwrap();
    assert_eq!(before.head.as_deref(), Some(repo.output(&["rev-parse", "HEAD"]).trim()));

    repo.git(&["update-ref", "refs/heads/main", &first]);
    let after = repository_refs(&repo.path).unwrap();
    assert_eq!(after.head.as_deref(), Some(first.as_str()));
    assert!(after.labels.get(&first).is_some_and(|labels| labels.iter().any(|label| label == "main")));
}

#[test]
fn remote_inventory_preserves_names_and_local_urls_with_spaces() {
    let repo = repo();
    repo.git(&["commit", "--allow-empty", "-m", "base"]);
    repo.git(&["remote", "add", "team/shared", "../origin copy"]);
    repo.git(&["update-ref", "refs/remotes/team/shared/main", "HEAD"]);
    repo.git(&["symbolic-ref", "refs/remotes/team/shared/HEAD", "refs/remotes/team/shared/main"]);
    repo.git(&["branch", "team/shared/main"]);
    let refs = repository_refs(&repo.path).unwrap();
    let remote = refs.remotes.iter().find(|remote| remote.name == "team/shared").unwrap();
    assert_eq!(remote.url.as_deref(), Some("../origin copy"));
    assert_eq!(remote.branches, ["main"]);
    assert_eq!(remote.head.as_deref(), Some("main"));
    let labels = &refs.labels[refs.head.as_ref().unwrap()];
    assert!(labels.iter().any(|name| name == "team/shared/main"));
    assert!(!labels.iter().any(|name| name == "team/shared/HEAD"));
}

#[test]
fn current_history_uses_existing_visible_upstream() {
    let repo = repo();
    repo.git(&["commit", "--allow-empty", "-m", "base"]);
    let scope = || awegit_git::current_branch_log_revisions(&repo.path).unwrap();
    assert_eq!(scope(), ["refs/heads/main"]);
    repo.git(&["remote", "add", "team/shared", "../origin copy"]);
    repo.git(&["update-ref", "refs/remotes/team/shared/main", "HEAD"]);
    repo.git(&["config", "branch.main.remote", "team/shared"]);
    repo.git(&["config", "branch.main.merge", "refs/heads/main"]);
    assert_eq!(scope(), ["refs/heads/main", "refs/remotes/team/shared/main"]);
    awegit_git::write_hidden_refs(&repo.path, &["team/".into()]).unwrap();
    assert_eq!(scope(), ["refs/heads/main"]);
    awegit_git::write_hidden_refs(&repo.path, &[]).unwrap();
    repo.git(&["update-ref", "-d", "refs/remotes/team/shared/main"]);
    assert_eq!(scope(), ["refs/heads/main"]);
    repo.git(&["branch", "local"]);
    repo.git(&["branch", "--set-upstream-to=local"]);
    assert_eq!(scope(), ["refs/heads/main", "refs/heads/local"]);
    repo.git(&["checkout", "--detach"]);
    assert!(scope().is_empty());
}

#[test]
fn repository_search_can_read_other_branches_with_an_unborn_head() {
    let repo = repo();
    repo.git(&["commit", "--allow-empty", "-m", "searchable commit"]);
    repo.git(&["symbolic-ref", "HEAD", "refs/heads/unborn"]);
    let commits = awegit_git::search_log(&repo.path, "message", "repository", "searchable", 20, false).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].summary, "searchable commit");
    assert!(awegit_git::search_log(&repo.path, "message", "branch", "searchable", 20, false).unwrap().is_empty());
}

#[test]
fn branch_log_hides_other_branches() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "first"]);
    repo.git(&["checkout", "-b", "other"]);
    fs::write(repo.path.join("b.txt"), "two\n").unwrap();
    repo.git(&["add", "b.txt"]);
    repo.git(&["commit", "-m", "only-other"]);
    repo.git(&["checkout", "main"]);

    let current = branch_commits(&repo.path, 20).unwrap();
    assert!(current.iter().all(|commit| commit.summary != "only-other"), "{current:?}");
    let all = commit_log(&repo.path, 20).unwrap();
    assert!(all.iter().any(|commit| commit.summary == "only-other"), "{all:?}");
}

#[test]
fn stash_commit_lists_its_changes() {
    let repo = repo();
    fs::write(repo.path.join("a.txt"), "one\n").unwrap();
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "-m", "first"]);
    fs::write(repo.path.join("a.txt"), "two\n").unwrap();
    repo.git(&["stash", "push", "-m", "wip"]);

    let refs = repository_refs(&repo.path).unwrap();
    let stash = refs.stashes.first().expect("stash");
    let files = commit_files(&repo.path, &stash.id).unwrap();
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(files[0].path, "a.txt");
    let diff = awegit_git::show_commit_file(&repo.path, &stash.id, "a.txt").unwrap();
    assert!(diff.lines.iter().any(|line| line.text.contains("two")), "{diff:?}");
}

#[test]
fn history_paths_round_trip_without_git_quoting() {
    let repo = repo();
    let mut paths = vec!["café.txt", "中文 文件.txt"];
    if cfg!(unix) {
        paths.extend(["tab\tname.txt", "line\nbreak.txt", "quote\"slash\\name.txt"]);
    }
    for path in &paths {
        fs::write(repo.path.join(path), "original content\n").unwrap();
    }
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-m", "unicode paths"]);
    for quote in ["true", "false"] {
        repo.git(&["config", "core.quotePath", quote]);
        let files = commit_files(&repo.path, "HEAD").unwrap();
        let tree = commit_tree(&repo.path, "HEAD").unwrap();
        assert_eq!(files.len(), paths.len(), "{files:?}");
        assert_eq!(tree.len(), paths.len(), "{tree:?}");
        for path in &paths {
            assert!(files.iter().any(|file| file.path == *path), "{files:?}");
            assert!(tree.iter().any(|entry| entry == path), "{tree:?}");
            if path.contains('\n') {
                // Lists preserve the name; commands retain their existing newline-path policy.
                assert!(show_commit_file(&repo.path, "HEAD", path).is_err());
                continue;
            }
            let diff = show_commit_file(&repo.path, "HEAD", path).unwrap();
            assert!(diff.lines.iter().any(|line| line.text.contains("original content")), "{diff:?}");
        }
    }
    repo.git(&["config", "core.quotePath", "true"]);
    let renamed = "重命名 café.txt";
    fs::rename(repo.path.join(paths[1]), repo.path.join(renamed)).unwrap();
    fs::write(repo.path.join(paths[0]), "changed content\n").unwrap();
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-m", "rename unicode path"]);
    for files in [
        commit_files(&repo.path, "HEAD").unwrap(),
        compare_files(&repo.path, "HEAD^", "HEAD").unwrap(),
    ] {
        let file = files.iter().find(|file| file.path == renamed).expect("unquoted rename");
        assert_eq!(file.kind, ChangeKind::Renamed);
        assert_eq!(file.previous_path.as_deref(), Some(paths[1]));
        assert!(files.iter().any(|file| file.path == paths[0] && file.kind == ChangeKind::Modified), "{files:?}");
    }
}

#[test]
fn blame_preserves_metadata_when_a_commit_reappears() {
    let repo = repo();
    fs::write(repo.path.join("blame.txt"), "first\nmiddle\nlast\n").unwrap();
    repo.git(&["add", "blame.txt"]);
    repo.git(&["commit", "--author", "First <first@example.com>", "-m", "first summary"]);
    let first = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    fs::write(repo.path.join("blame.txt"), "first\nsecond author\nlast\n").unwrap();
    repo.git(&["add", "blame.txt"]);
    repo.git(&["commit", "--author", "Second <second@example.com>", "-m", "second summary"]);
    let blame = blame_file(&repo.path, "blame.txt").unwrap();
    assert_eq!(blame.len(), 3, "{blame:?}");
    assert_eq!(blame.iter().map(|line| line.author.as_str()).collect::<Vec<_>>(), ["First", "Second", "First"]);
    assert_eq!(blame.iter().map(|line| line.summary.as_str()).collect::<Vec<_>>(), ["first summary", "second summary", "first summary"]);
    assert_eq!(blame.iter().map(|line| line.line).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(blame[0].id, first);
    assert_eq!(blame[2].id, first);
}

#[test]
fn locate_ref_finds_a_branch_buried_under_later_commits() {
    let repo = repo();
    repo.git(&["commit", "--allow-empty", "-m", "base"]);
    repo.git(&["branch", "buried"]);
    let buried = repo.output(&["rev-parse", "buried"]).trim().to_string();
    for i in 0..40 {
        repo.git(&["commit", "--allow-empty", "-m", &format!("feat: later {i}")]);
    }
    let found = locate_ref(&repo.path, "buried", false).unwrap();
    assert_eq!(found.id, buried);
    assert!(found.commits.iter().any(|commit| commit.id == buried), "{} commits", found.commits.len());
}

#[test]
fn named_revs_skip_unnamed_remotes() {
    let repo = repo();
    repo.git(&["commit", "--allow-empty", "-m", "base"]);
    repo.git(&["checkout", "-b", "fork"]);
    repo.git(&["commit", "--allow-empty", "-m", "feat: only on the fork"]);
    let tip = repo.output(&["rev-parse", "HEAD"]).trim().to_string();
    repo.git(&["update-ref", "refs/remotes/origin/fork", &tip]);
    repo.git(&["checkout", "main"]);
    repo.git(&["branch", "-D", "fork"]);
    let rows = commits_from(&repo.path, 20, &["main".into()], false).unwrap();
    assert!(rows.iter().all(|commit| commit.id != tip), "exact log included the unnamed remote");
    let named = commits_from(&repo.path, 20, &["main".into(), "origin/fork".into()], false).unwrap();
    let found = named.iter().find(|commit| commit.id == tip).expect("named remote tip");
    assert!(found.refs.iter().any(|name| name == "origin/fork"), "{:?}", found.refs);
}
