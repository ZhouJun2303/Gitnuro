mod common;

use std::fs;
use std::path::Path;

use awegit_git::{
    file_diff, perform, repository_refs, status, submodule_remote_status, Error, Mutation,
};
use common::TempRepo;
use serde_json::Value;

fn repo(bare: bool) -> TempRepo {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".lab/submodule-tests");
    fs::create_dir_all(&root).unwrap();
    let repo = TempRepo::new_in(&root);
    if bare {
        repo.git(&["init", "--bare", "-b", "main"]);
    } else {
        repo.git(&["init", "-b", "main"]);
        for (key, value) in [
            ("core.autocrlf", "false"),
            ("user.name", "Submodule Test"),
            ("user.email", "submodule@example.test"),
            ("commit.gpgsign", "false"),
        ] {
            repo.git(&["config", key, value]);
        }
    }
    repo
}

fn file_url(path: &Path) -> String {
    let text = path.display().to_string().replace('\\', "/");
    let text = text.strip_prefix("//?/").unwrap_or(&text);
    if text.starts_with('/') {
        format!("file://{text}")
    } else {
        format!("file:///{text}")
    }
}

struct Fixture {
    parent: TempRepo,
    child: TempRepo,
    parent_remote: TempRepo,
    child_remote: TempRepo,
    base: String,
}

impl Fixture {
    fn new() -> Self {
        let child_remote = repo(true);
        let child = repo(false);
        child.git(&["remote", "add", "origin", &file_url(&child_remote.path)]);
        fs::write(child.path.join("file.txt"), "base\n").unwrap();
        child.git(&["add", "file.txt"]);
        child.git(&["commit", "-m", "child base"]);
        child.git(&["push", "-u", "origin", "main"]);
        let base = child.output(&["rev-parse", "HEAD"]).trim().into();
        let parent_remote = repo(true);
        let parent = repo(false);
        parent.git(&["remote", "add", "origin", &file_url(&parent_remote.path)]);
        let relative = format!(
            "../{}",
            child_remote.path.file_name().unwrap().to_string_lossy()
        );
        parent.git(&[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-b",
            "main",
            &relative,
            "vendor",
        ]);
        parent.git(&["commit", "-m", "parent base"]);
        parent.git(&["push", "-u", "origin", "main"]);
        Self {
            parent,
            child,
            parent_remote,
            child_remote,
            base,
        }
    }

    fn child_at(&self, args: &[&str]) -> String {
        let mut command = vec!["-C", "vendor"];
        command.extend_from_slice(args);
        self.parent.output(&command).trim().into()
    }

    fn edit_and_commit(&self, text: &str) -> String {
        fs::write(self.parent.path.join("vendor/file.txt"), text).unwrap();
        self.child_at(&["add", "file.txt"]);
        self.child_at(&["commit", "-m", text]);
        self.child_at(&["rev-parse", "HEAD"])
    }

    fn record(&self, id: &str) {
        self.parent.git(&[
            "update-index",
            "--cacheinfo",
            &format!("160000,{id},vendor"),
        ]);
        self.parent.git(&["commit", "-m", "move gitlink"]);
    }

    fn inspect(&self) -> Value {
        let rows = submodule_remote_status(&self.parent.path, Some("vendor")).unwrap();
        let row = serde_json::to_value(&rows[0]).unwrap();
        assert!(row["error"].is_null(), "{row}");
        row
    }

    fn push(&self) -> Result<String, Error> {
        perform(
            &self.parent.path,
            Mutation::Push {
                remote: None,
                set_upstream: false,
                tags: false,
                force: false,
                force_with_lease: false,
                killer_push: true,
                rebase: false,
            },
        )
    }

    fn push_sha(&self, sha: &str) -> Result<String, Error> {
        perform(
            &self.parent.path,
            Mutation::PushRef {
                remote: "origin".into(),
                branch: "main".into(),
                source_commit: Some(sha.into()),
                force_with_lease: false,
                force: false,
                remote_branch: "published".into(),
                set_upstream: false,
                tags: false,
                killer_push: false,
                rebase: false,
            },
        )
    }
}

#[test]
fn submodule_remote_status_tracks_merge_without_moving_pins_or_using_cached_refs() {
    let fixture = Fixture::new();
    fixture.child_at(&["checkout", "-b", "topic"]);
    let topic = fixture.edit_and_commit("topic change\n");
    let before = fixture.inspect();
    assert_eq!(before["recorded"]["state"], "inTarget", "{before}");
    assert_eq!(before["checkout"]["state"], "unreachable", "{before}");

    fixture.record(&topic);
    let head = fixture.parent.output(&["rev-parse", "HEAD"]);
    let error = fixture.push().unwrap_err().to_string();
    assert!(
        error.contains("No advertised remote branch or tag contains"),
        "{error}"
    );
    assert!(
        error.contains("vendor") && error.contains(&topic) && error.contains(head.trim()),
        "{error}"
    );
    assert!(fixture
        .push_sha(head.trim())
        .unwrap_err()
        .to_string()
        .contains(&topic));
    assert_ne!(fixture.parent_remote.output(&["rev-parse", "main"]), head);

    fixture.child_at(&["push", "origin", "topic"]);
    let published = fixture.inspect();
    assert_eq!(
        published["recorded"]["state"], "outsideTarget",
        "{published}"
    );
    fixture.push().unwrap();
    fixture.push_sha(head.trim()).unwrap();
    let copy = repo(false);
    copy.git(&[
        "-c",
        "protocol.file.allow=always",
        "clone",
        "--recurse-submodules",
        &file_url(&fixture.parent_remote.path),
        "clone",
    ]);
    assert_eq!(
        copy.output(&["-C", "clone/vendor", "rev-parse", "HEAD"])
            .trim(),
        topic
    );

    // Merge in a second checkout. The submodule's local remote-tracking refs
    // and object database still know only the separate topic ref.
    fixture.child.git(&["fetch", "origin"]);
    fixture
        .child
        .git(&["merge", "--no-ff", "origin/topic", "-m", "merge PR"]);
    fixture.child.git(&["push", "origin", "main", ":topic"]);
    let after = fixture.inspect();
    assert_eq!(after["recorded"]["state"], "inTarget", "{after}");
    assert_eq!(
        fixture.child_at(&["rev-parse", "origin/main"]),
        fixture.base
    );
    assert_eq!(fixture.child_at(&["rev-parse", "HEAD"]), topic);
    assert_eq!(fixture.parent.output(&["rev-parse", "HEAD"]), head);

    fixture.parent.git(&[
        "config",
        "-f",
        ".gitmodules",
        "submodule.vendor.branch",
        "missing-target",
    ]);
    assert_eq!(fixture.inspect()["recorded"]["state"], "targetMissing");
    fixture.parent.git(&[
        "config",
        "-f",
        ".gitmodules",
        "submodule.vendor.url",
        "./missing-remote",
    ]);
    let rows = submodule_remote_status(&fixture.parent.path, Some("vendor")).unwrap();
    let inaccessible = serde_json::to_value(&rows[0]).unwrap();
    assert!(inaccessible["recorded"].is_null());
    assert!(inaccessible["error"]
        .as_str()
        .unwrap()
        .contains("missing-remote"));
}

#[test]
fn submodule_squash_keeps_old_sha_outside_target_and_checks_unpublished_history() {
    let fixture = Fixture::new();
    fixture.child_at(&["checkout", "-b", "topic"]);
    let topic = fixture.edit_and_commit("squash content\n");
    fixture.child_at(&["push", "origin", "topic"]);
    fixture.record(&topic);
    let old_parent = fixture
        .parent
        .output(&["rev-parse", "HEAD"])
        .trim()
        .to_string();
    fixture.child.git(&["fetch", "origin"]);
    fixture.child.git(&["merge", "--squash", "origin/topic"]);
    fixture.child.git(&["commit", "-m", "different squash SHA"]);
    fixture.child.git(&["push", "origin", "main"]);
    assert_eq!(fixture.inspect()["recorded"]["state"], "outsideTarget");
    fixture.child.git(&["push", "origin", ":topic"]);
    let after = fixture.inspect();
    assert_eq!(after["recorded"]["state"], "unreachable", "{after}");

    // The branch tip pins a safe SHA. The earlier unpublished parent commit
    // remains part of the push and must be diagnosed with its own SHA.
    fixture.record(&fixture.base);
    let error = fixture.push().unwrap_err().to_string();
    assert!(
        error.contains(&topic) && error.contains(&old_parent),
        "{error}"
    );
    assert!(
        fixture
            .child_remote
            .code(&["show-ref", "--verify", "refs/heads/topic"])
            != 0
    );
}

#[test]
fn submodule_status_keeps_dirty_counts_separate_and_reports_index_conflict_stages() {
    let fixture = Fixture::new();
    let topic = fixture.edit_and_commit("pointer only\n");
    let dirty = || {
        status(&fixture.parent.path)
            .unwrap()
            .unstaged
            .into_iter()
            .find(|file| file.path == "vendor")
            .unwrap()
            .submodule_dirty
    };
    assert_eq!(dirty(), Some(0));
    fs::write(fixture.parent.path.join("vendor/file.txt"), "staged\n").unwrap();
    fixture.child_at(&["add", "file.txt"]);
    fs::write(fixture.parent.path.join("vendor/file.txt"), "unstaged\n").unwrap();
    assert_eq!(dirty(), Some(1));
    fs::write(fixture.parent.path.join("vendor/untracked.txt"), "new\n").unwrap();
    assert_eq!(dirty(), Some(2));
    let diff = file_diff(&fixture.parent.path, "vendor", false).unwrap();
    assert!(diff
        .lines
        .iter()
        .any(|line| line.text == format!("+Subproject commit {topic}-dirty")));

    fixture.parent.git(&[
        "config",
        "-f",
        ".gitmodules",
        "submodule.vendor.branch",
        ".",
    ]);
    let refs = repository_refs(&fixture.parent.path).unwrap();
    assert!(refs.submodules[0].inherits_branch);
    assert_eq!(refs.submodules[0].configured_branch, "main");
    assert_eq!(refs.submodules[0].recorded, fixture.base);
    fixture.parent.git(&["add", "vendor"]);
    assert_eq!(
        repository_refs(&fixture.parent.path).unwrap().submodules[0].recorded,
        topic
    );
    fixture.parent.git(&["restore", "--staged", "vendor"]);
    assert_eq!(
        repository_refs(&fixture.parent.path).unwrap().submodules[0].recorded,
        fixture.base
    );

    let index = format!(
        "0 {}\tvendor\n160000 {} 1\tvendor\n160000 {} 2\tvendor\n160000 {} 3\tvendor\n",
        "0".repeat(40),
        fixture.base,
        fixture.base,
        topic
    );
    let mut git = std::process::Command::new("git")
        .current_dir(&fixture.parent.path)
        .args(["update-index", "--index-info"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(git.stdin.take().as_mut().unwrap(), index.as_bytes()).unwrap();
    assert!(git.wait().unwrap().success());
    let refs = repository_refs(&fixture.parent.path).unwrap();
    assert!(refs.submodules[0].recorded.is_empty());
    assert_eq!(
        refs.submodules[0].conflict,
        Some([
            Some(fixture.base.clone()),
            Some(fixture.base.clone()),
            Some(topic)
        ])
    );
}
