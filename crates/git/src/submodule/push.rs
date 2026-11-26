use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::cli::{run, run_graph, run_output};
use crate::Error;

use super::{
    diagnostic, display_url, module_config, resolve_url, usable_checkout, valid_oid, Probe,
    RemoteRefs, RemoteState,
};

struct PushUpdate {
    id: String,
    destination: String,
    matching: bool,
}

/// Inspect committed gitlinks using the destination's current refs. The
/// checkout's branch, index and remote-tracking refs can all differ from
/// what this push will publish.
pub(crate) fn check_push(repo: &Path, args: &[String]) -> Result<(), Error> {
    let opened = crate::open_repo(repo)?;
    let config = opened.config_snapshot();
    let branch = opened
        .head()
        .ok()
        .and_then(|head| head.referent_name().map(|name| name.shorten().to_string()));
    let branch_config = |key| {
        branch
            .as_ref()
            .and_then(|branch| config.string(format!("branch.{branch}.{key}")))
            .map(|value| value.to_string())
    };
    let upstream_remote = branch_config("remote");
    let mut positionals = args.iter().skip(1).filter(|arg| !arg.starts_with('-'));
    let explicit = positionals.next();
    let remote_names = opened.remote_names();
    let default_remote = upstream_remote.unwrap_or_else(|| {
        if remote_names.len() == 1 {
            if let Some(name) = remote_names.first() {
                return name.to_string();
            }
        }
        "origin".into()
    });
    let remote = explicit
        .cloned()
        .or_else(|| branch_config("pushRemote"))
        .or_else(|| {
            config
                .string("remote.pushDefault")
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| default_remote.clone());
    if remote == "." {
        return Ok(());
    }
    let mut specs: Vec<String> = positionals.cloned().collect();
    let mirror = config.boolean(format!("remote.{remote}.mirror")) == Some(true);
    if specs.is_empty() && !mirror {
        if let Some(configured) = config.strings(format!("remote.{remote}.push")) {
            specs.extend(configured.into_iter().map(|value| value.to_string()));
        } else {
            let pushing_upstream = remote == default_remote;
            let merge = branch_config("merge");
            let mode = config.string("push.default").map(|mode| mode.to_string());
            if mode.as_deref() == Some("matching") {
                specs.push(":".into());
            } else if let Some(branch) = &branch {
                let source = format!("refs/heads/{branch}");
                let destination = match mode.as_deref() {
                    Some("nothing") => None,
                    Some("current") => Some(source.clone()),
                    Some("upstream" | "tracking") if pushing_upstream => merge,
                    Some("upstream" | "tracking") => None,
                    _ if pushing_upstream
                        && merge.as_deref() != Some(source.as_str())
                        && (merge.is_some()
                            || config.boolean("push.autoSetupRemote") != Some(true)) =>
                    {
                        None
                    }
                    _ => Some(source.clone()),
                };
                if let Some(destination) = destination {
                    specs.push(format!("{source}:{destination}"));
                }
            }
        }
    }
    if mirror {
        specs.push("refs/*:refs/*".into());
    }
    if args.iter().any(|arg| arg == "--tags") {
        specs.push("refs/tags/*:refs/tags/*".into());
    }
    if specs.is_empty() {
        return Ok(());
    }

    let mut updates = Vec::new();
    for spec in specs {
        let spec = spec.trim_start_matches('+');
        if spec == ":" {
            add_matching_refs(
                repo,
                "refs/heads/",
                "refs/heads/*",
                "refs/heads/*",
                true,
                &mut updates,
            )?;
        } else if let Some((source, destination)) = spec.split_once(':') {
            if source.contains('*') {
                add_matching_refs(repo, source, source, destination, false, &mut updates)?;
            } else if !source.is_empty() {
                add_update(repo, source, destination, &mut updates)?;
            }
        } else {
            add_update(repo, spec, "", &mut updates)?;
        }
    }
    if updates.is_empty() {
        return Ok(());
    }
    let mut source_ids: Vec<_> = updates.iter().map(|update| update.id.as_str()).collect();
    source_ids.sort_unstable();
    source_ids.dedup();
    // Most pushes come from a repository that has never used submodules.
    // Look through the source history so a gitlink removed at the tip still
    // gets checked if the push introduces the commits that referenced it.
    if !repo.join(".gitmodules").exists()
        && !opened.index_or_empty().ok().is_some_and(|index| {
            index
                .entries()
                .iter()
                .any(|entry| entry.mode == gix::index::entry::Mode::COMMIT)
        })
    {
        let mut input = source_ids.join("\n");
        input.push('\n');
        let history = run_graph(
            repo,
            &[
                "log",
                "--stdin",
                "--full-history",
                "-1",
                "--format=%H",
                "--",
                ".gitmodules",
            ],
            Some(input.as_bytes()),
        )?;
        if history.status.success() && history.stdout.is_empty() {
            return Ok(());
        }
    }
    let urls = match run_output(repo, &["remote", "get-url", "--push", "--all", &remote])? {
        output if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string)
            .collect(),
        _ if explicit.is_some() => vec![remote.clone()],
        _ => return Ok(()),
    };
    for url in urls {
        let live = RemoteRefs::read(repo, &url, false).map_err(|error| Error::Git(format!(
            "Cannot verify the submodule commits before pushing to {remote} ({url}). Check the superproject remote connection and permissions. / 无法在推送到 {remote}（{url}）前校验子模块提交，请检查主仓库远端连接与权限。\n{error}",
            url = display_url(&url), error = diagnostic(&error.to_string()),
        )))?;
        let updates: Vec<_> = updates
            .iter()
            .filter(|update| {
                let previous = live
                    .refs
                    .get(&format!("{}^{{}}", update.destination))
                    .or_else(|| live.refs.get(&update.destination));
                (!update.matching || previous.is_some()) && previous != Some(&update.id)
            })
            .collect();
        if updates.is_empty() {
            continue;
        }
        check_outgoing(repo, &remote, &url, &live, &updates)?;
    }
    Ok(())
}

fn add_update(
    repo: &Path,
    source: &str,
    destination: &str,
    updates: &mut Vec<PushUpdate>,
) -> Result<(), Error> {
    let revision = format!("{source}^{{commit}}");
    let output = run_output(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &revision,
        ],
    )?;
    let id = String::from_utf8_lossy(&output.stdout);
    let id = id.trim_end();
    // Tags to blobs and trees cannot carry gitlinks.
    if !output.status.success() || !valid_oid(id) {
        return Ok(());
    }
    let target = if destination.starts_with("refs/") {
        destination.into()
    } else {
        let output = run_output(
            repo,
            &[
                "rev-parse",
                "--symbolic-full-name",
                "--verify",
                "--end-of-options",
                source,
            ],
        )?;
        let reference = String::from_utf8_lossy(&output.stdout);
        let reference = reference.trim_end();
        if destination.is_empty() {
            if !reference.starts_with("refs/") {
                return Ok(());
            }
            reference.into()
        } else if reference.starts_with("refs/tags/") {
            format!("refs/tags/{destination}")
        } else {
            format!("refs/heads/{destination}")
        }
    };
    updates.push(PushUpdate {
        id: id.into(),
        destination: target,
        matching: false,
    });
    Ok(())
}

fn add_matching_refs(
    repo: &Path,
    prefix: &str,
    source: &str,
    target: &str,
    matching: bool,
    updates: &mut Vec<PushUpdate>,
) -> Result<(), Error> {
    let Some((start, end)) = source.split_once('*') else {
        return Ok(());
    };
    let output = run(
        repo,
        &[
            "for-each-ref",
            "--format=%(objectname) %(objecttype) %(*objectname) %(*objecttype) %(refname)",
            prefix,
        ],
    )?;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<_> = line.splitn(5, ' ').collect();
        let [id, kind, peeled, peeled_kind, name] = parts.as_slice() else {
            continue;
        };
        let Some(middle) = name
            .strip_prefix(start)
            .and_then(|name| name.strip_suffix(end))
        else {
            continue;
        };
        let id = match (*kind, *peeled_kind) {
            ("commit", _) => *id,
            (_, "commit") => *peeled,
            ("tag", _) => {
                add_update(repo, name, &target.replace('*', middle), updates)?;
                continue;
            }
            _ => continue,
        };
        updates.push(PushUpdate {
            id: id.into(),
            destination: target.replace('*', middle),
            matching,
        });
    }
    Ok(())
}

fn check_outgoing(
    repo: &Path,
    remote: &str,
    url: &str,
    live: &RemoteRefs,
    updates: &[&PushUpdate],
) -> Result<(), Error> {
    let opened = crate::open_repo(repo)?;
    let mut modules = HashMap::new();
    let mut selected = BTreeMap::<(String, String, String), (String, String)>::new();
    for update in updates {
        crate::check_cancelled()?;
        let mut input = format!("{}\n", update.id);
        for (_, id) in live.tips() {
            let known_commit = gix::hash::ObjectId::from_hex(id.as_bytes())
                .ok()
                .and_then(|id| opened.find_object(id).ok())
                .is_some_and(|object| object.kind == gix::object::Kind::Commit);
            if known_commit {
                input.push_str(&format!("^{id}\n"));
            }
        }
        let commits = run_graph(repo, &["rev-list", "--stdin"], Some(input.as_bytes()))?;
        if !commits.status.success() {
            return Err(Error::Git(diagnostic(&String::from_utf8_lossy(
                &commits.stderr,
            ))));
        }
        let mut paths = gitlink_changes(repo, &commits.stdout)?;
        // Even a commit already on another remote ref can be newly published
        // to this destination branch. Always verify the tip's complete tree.
        paths.insert(update.id.clone(), None);
        for (commit, changed) in paths {
            let mut links = committed_links(repo, &commit)?;
            if let Some(changed) = &changed {
                links.retain(|path, _| changed.contains(path));
            }
            if links.is_empty() {
                continue;
            }
            let tree_config = run_output(
                repo,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("{commit}:.gitmodules"),
                ],
            )?;
            let blob = String::from_utf8_lossy(&tree_config.stdout)
                .trim_end()
                .to_string();
            if !modules.contains_key(&blob) {
                let config = if tree_config.status.success() {
                    module_config(repo, &["--blob", &blob]).map_err(|error| Error::Git(format!(
                        ".gitmodules at superproject commit {commit} cannot be parsed. / 无法解析主仓库提交 {commit} 中的 .gitmodules。\n{error}"
                    )))?
                } else {
                    BTreeMap::new()
                };
                modules.insert(blob.clone(), config);
            }
            for (path, id) in links {
                let module = modules.get(&blob).and_then(|config| config.get(&path));
                let raw_url = module.map_or("", |module| module.url.as_str());
                let child_url = resolve_url(repo, url, raw_url).map_err(|error| Error::Git(format!(
                    "Submodule {path} at {id}, superproject commit {commit}, destination {remote}/{}. / 子模块 {path} 固定在 {id}，主仓库提交 {commit}，推送目标 {remote}/{}。\n{error}",
                    update.destination, update.destination,
                )))?;
                let key = (child_url, path, id);
                selected
                    .entry(key)
                    .or_insert_with(|| (commit.clone(), update.destination.clone()));
            }
        }
    }
    let mut previous_url = String::new();
    let mut probe = None;
    let mut failures = Vec::new();
    for ((child_url, path, id), (commit, target)) in &selected {
        crate::check_cancelled()?;
        let describe = || {
            format!(
            "{path}\n  Submodule commit / 子模块提交 {id}\n  Superproject commit / 主仓库提交 {commit}\n  Push destination / 推送目标 {remote}/{target}\n  Submodule remote / 子模块远端 {}",
            display_url(child_url),
        )
        };
        if previous_url != *child_url {
            let checkout = usable_checkout(repo, path);
            // Keep the cwd at the superproject for local URLs. Absolute URLs
            // may use the child checkout's object store for graph inspection.
            probe = Some(
                Probe::open(repo, child_url, checkout)
                    .map_err(|error| diagnostic(&error.to_string())),
            );
            previous_url.clone_from(child_url);
        }
        let status = match probe.as_mut().expect("probe exists after resolving a URL") {
            Ok(probe) => probe
                .inspect(id, "")
                .map_err(|error| diagnostic(&error.to_string())),
            Err(error) => Err(error.clone()),
        };
        let error = match status {
            Ok(status) if status.state == RemoteState::Unreachable => "No advertised remote branch or tag contains this commit. Push this exact submodule commit to its remote, or change the gitlink to a published commit and update the listed unpublished superproject commit. / 远端分支和标签均未保留这个提交。请把此子模块提交推到对应远端，或改用已发布的子模块提交并修正上面列出的主仓库提交。".into(),
            Ok(_) => continue,
            Err(error) => format!("Could not verify the submodule remote. Check the URL, credentials and network, then retry. / 无法校验子模块远端。请检查 URL、访问权限和网络后重试。\n{error}"),
        };
        failures.push(format!("{}\n  {error}", describe()));
    }
    if failures.is_empty() {
        return Ok(());
    }
    Err(Error::Git(format!(
        "Push stopped while checking {} submodule pin(s). / {} 个子模块指针未通过校验，主仓库推送已停止。\n\n{}",
        failures.len(), failures.len(), failures.join("\n\n"),
    )))
}

/// Each distinct parent comparison is included for merges. A .gitmodules
/// change can repoint unchanged gitlinks to different repositories.
fn gitlink_changes(
    repo: &Path,
    commits: &[u8],
) -> Result<BTreeMap<String, Option<HashSet<String>>>, Error> {
    let output = run_graph(
        repo,
        &[
            "diff-tree",
            "--stdin",
            "--root",
            "-m",
            "--raw",
            "-r",
            "-z",
            "--no-renames",
            "--no-abbrev",
        ],
        Some(commits),
    )?;
    if !output.status.success() {
        return Err(Error::Git(diagnostic(&String::from_utf8_lossy(
            &output.stderr,
        ))));
    }
    let mut paths = BTreeMap::new();
    let mut records = output.stdout.split(|byte| *byte == 0);
    let mut commit = String::new();
    while let Some(record) = records.next() {
        let record = String::from_utf8_lossy(record);
        if valid_oid(&record) {
            commit = record.into_owned();
        } else if let Some(meta) = record.strip_prefix(':') {
            let Some(path) = records.next() else { break };
            let path = String::from_utf8_lossy(path);
            if commit.is_empty() {
                continue;
            }
            if path == ".gitmodules" {
                paths.insert(commit.clone(), None);
            } else if meta.split_whitespace().nth(1) == Some("160000") {
                if let Some(paths) = paths
                    .entry(commit.clone())
                    .or_insert_with(|| Some(HashSet::new()))
                {
                    paths.insert(path.into_owned());
                }
            }
        }
    }
    Ok(paths)
}

fn committed_links(repo: &Path, commit: &str) -> Result<BTreeMap<String, String>, Error> {
    let output = run_graph(repo, &["ls-tree", "-r", "-z", commit], None)?;
    if !output.status.success() {
        return Err(Error::Git(diagnostic(&String::from_utf8_lossy(
            &output.stderr,
        ))));
    }
    let mut links = BTreeMap::new();
    for item in output.stdout.split(|byte| *byte == 0) {
        let item = String::from_utf8_lossy(item);
        let Some((meta, path)) = item.split_once('\t') else {
            continue;
        };
        if let Some(id) = meta.strip_prefix("160000 commit ") {
            links.insert(path.into(), id.into());
        }
    }
    Ok(links)
}
