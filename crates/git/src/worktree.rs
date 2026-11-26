//! Git's worktree registry is shared, while HEAD, index and working files are per checkout.

use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cli::{check_name, check_rev, repo_key, run, run_env, run_output, with_read};
use crate::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRow {
    pub path: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub detached: bool,
    pub main: bool,
    pub bare: bool,
    pub missing: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
}

#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AddMode {
    #[default]
    Auto,
    Existing,
    New,
    Detached,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeDetails {
    pub branch: Option<String>,
    pub changes: Vec<WorktreeChange>,
    pub change_count: usize,
    pub has_submodules: bool,
    pub protects_commit: bool,
    pub stamp: String,
}

#[derive(Serialize)]
pub struct WorktreeChange {
    pub code: String,
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeComparison {
    pub head: String,
    pub base: String,
    pub merge_base: Option<String>,
    pub ahead: u64,
    pub behind: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreePrunePreview {
    pub output: String,
    pub protected_paths: Vec<String>,
    pub stamp: String,
}

pub fn common_dir(repo: &Path) -> Result<PathBuf, Error> {
    Ok(crate::open_repo(repo)?.common_dir().to_path_buf())
}

pub fn is_linked(repo: &Path) -> Result<bool, Error> {
    Ok(crate::open_repo(repo)?
        .worktree()
        .is_some_and(|worktree| !worktree.is_main()))
}

/// Index writes and per-worktree reflogs leave the registry stamp unchanged.
pub(crate) fn registry_stamp(common: &Path) -> u64 {
    let mut stamp = DefaultHasher::new();
    stamp_head(common, common, &mut stamp);
    for dir in registry_directories(common) {
        dir.file_name().hash(&mut stamp);
        stamp_head(common, &dir, &mut stamp);
        fs::read(dir.join("locked")).ok().hash(&mut stamp);
        let gitfile = fs::read_to_string(dir.join("gitdir")).ok();
        gitfile.hash(&mut stamp);
        if let Some(gitfile) = gitfile {
            dir.join(gitfile.trim_end_matches(['\r', '\n']))
                .is_file()
                .hash(&mut stamp);
        }
    }
    stamp.finish()
}

fn registry_directories(common: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<_> = fs::read_dir(common.join("worktrees"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort_unstable();
    dirs
}

fn stamp_head(common: &Path, git: &Path, stamp: &mut DefaultHasher) {
    let head = fs::read_to_string(git.join("HEAD")).ok();
    head.hash(stamp);
    if let Some(reference) = head
        .as_deref()
        .and_then(|head| head.trim().strip_prefix("ref: refs/heads/"))
    {
        let reference = format!("refs/heads/{reference}");
        if !reference
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
            && !reference.contains('\\')
        {
            crate::fingerprint::loose_ref(common, &reference)
                .unwrap_or_else(|| crate::fingerprint::packed_stat(common))
                .hash(stamp);
        }
    }
}

pub fn list(repo: &Path) -> Result<Vec<WorktreeRow>, Error> {
    let output = with_read(|| {
        run(
            repo,
            &["worktree", "list", "--porcelain", "-z", "--expire=now"],
        )
    })?;
    Ok(parse_list(&String::from_utf8_lossy(&output.stdout)))
}

pub(crate) fn detached_heads(repo: &Path) -> Result<Vec<String>, Error> {
    let common = common_dir(repo)?;
    let linked = registry_directories(&common);
    let mut heads = Vec::new();
    for git_dir in std::iter::once(common).chain(linked) {
        let Ok(head) = fs::read_to_string(git_dir.join("HEAD")) else {
            continue;
        };
        let head = head.trim();
        if matches!(head.len(), 40 | 64) && head.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            heads.push(head.to_string());
        }
    }
    heads.sort_unstable();
    heads.dedup();
    Ok(heads)
}

fn parse_list(output: &str) -> Vec<WorktreeRow> {
    let mut rows: Vec<WorktreeRow> = Vec::new();
    for field in output.split('\0').filter(|field| !field.is_empty()) {
        let (key, value) = field.split_once(' ').unwrap_or((field, ""));
        if key == "worktree" {
            rows.push(WorktreeRow {
                path: value.to_string(),
                main: rows.is_empty(),
                ..Default::default()
            });
            continue;
        }
        let Some(row) = rows.last_mut() else { continue };
        match key {
            "HEAD" if value.bytes().any(|byte| byte != b'0') => row.head = Some(value.to_string()),
            "branch" => row.branch = value.strip_prefix("refs/heads/").map(str::to_string),
            "detached" => row.detached = true,
            "bare" => row.bare = true,
            "locked" => row.locked = Some(value.to_string()),
            "prunable" => row.prunable = Some(value.to_string()),
            _ => {}
        }
    }
    for row in &mut rows {
        row.missing =
            !row.bare && (row.prunable.is_some() || !Path::new(&row.path).join(".git").exists());
    }
    rows
}

pub fn destination(repo: &Path, path: &str) -> Result<PathBuf, Error> {
    let target = Path::new(path);
    if path.trim().is_empty()
        || path.contains('\0')
        || (!target.is_absolute()
            && (target.has_root()
                || matches!(target.components().next(), Some(Component::Prefix(_)))))
    {
        return Err(Error::Path(path.to_string()));
    }
    Ok(repo.join(target))
}

pub fn display_path(path: &Path) -> String {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path = resolved.to_string_lossy();
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{unc}");
    }
    path.strip_prefix(r"\\?\").unwrap_or(&path).to_string()
}

fn same_path(left: &Path, right: &Path) -> bool {
    repo_key(left) == repo_key(right)
        || (left.exists()
            && right.exists()
            && repo_key(Path::new(&display_path(left)))
                == repo_key(Path::new(&display_path(right))))
}

fn registered<'a>(
    repo: &Path,
    rows: &'a [WorktreeRow],
    path: &str,
) -> Result<&'a WorktreeRow, Error> {
    let target = destination(repo, path)?;
    rows.iter()
        .find(|row| same_path(&target, Path::new(&row.path)))
        .ok_or_else(|| {
            Error::Git(format!(
                "Worktree is no longer registered / 工作树已不在列表中: {path}"
            ))
        })
}

pub(crate) fn validate_merge(
    repo: &Path,
    path: &str,
    expected_head: &str,
    into: &str,
) -> Result<(), Error> {
    let rows = list(repo)?;
    let target = registered(repo, &rows, ".")?;
    if !target.main || target.bare || target.missing || target.head.is_none() {
        return Err(Error::Git(
            "Merge requires a main worktree with an existing branch / 合并需要主工作树检出有提交的分支".into(),
        ));
    }
    let source = registered(repo, &rows, path)?;
    if source.main || source.bare || source.missing {
        return Err(Error::Git(format!(
            "Source worktree is unavailable / 源工作树不可用: {path}"
        )));
    }
    let root = Path::new(&source.path);
    let checkout = crate::open_repo(root)?;
    if !same_path(checkout.common_dir(), &common_dir(repo)?) {
        return Err(Error::Git(format!(
            "Directory belongs to another repository / 目录属于其他仓库: {path}"
        )));
    }
    if crate::in_progress(root)?.is_some() {
        return Err(Error::Git(
            "Finish the Git operation in the source worktree first / 请先结束源工作树中正在进行的 Git 操作".into(),
        ));
    }
    let changes = run_env(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=normal",
            "--ignore-submodules=none",
        ],
        &[("GIT_OPTIONAL_LOCKS", "0")],
    )?;
    if !changes.stdout.is_empty() {
        return Err(Error::Git(
            "Commit the source worktree changes before merging / 请先提交源工作树的改动，再合并".into(),
        ));
    }
    if source.head.as_deref() != Some(expected_head)
        || checkout.head_id().ok().map(|id| id.to_string()).as_deref() != Some(expected_head)
    {
        return Err(Error::Git(
            "Source worktree HEAD changed. Refresh and try again / 源工作树的 HEAD 已变化，请刷新后重试".into(),
        ));
    }
    if target.branch.as_deref() != Some(into)
        || crate::current_branch(repo)?.as_deref() != Some(into)
    {
        return Err(Error::Git(
            "The main worktree branch changed. Refresh and try again / 主工作树的分支已变化，请刷新后重试".into(),
        ));
    }
    Ok(())
}

pub(crate) fn add(
    repo: &Path,
    path: &str,
    branch: &str,
    mode: AddMode,
    start: Option<&str>,
) -> Result<(), Error> {
    let target = destination(repo, path)?;
    let target = target
        .to_str()
        .ok_or_else(|| Error::Path(path.to_string()))?;
    let branch = if matches!(mode, AddMode::Detached) {
        ""
    } else {
        check_name(branch)?
    };
    let mode = if matches!(mode, AddMode::Auto) {
        if branch_exists(repo, branch)? {
            AddMode::Existing
        } else {
            AddMode::New
        }
    } else {
        mode
    };
    let start = start.map(check_rev).transpose()?.unwrap_or("HEAD");
    let mut args = vec!["worktree", "add"];
    match mode {
        AddMode::Existing => {
            if !branch_exists(repo, branch)? {
                return Err(Error::Git(format!(
                    "Local branch not found / 找不到本地分支: {branch}"
                )));
            }
            // The short local branch name keeps Git's attached checkout semantics.
            args.extend(["--", target, branch]);
        }
        AddMode::New => {
            if start.starts_with("refs/remotes/") {
                args.push("--track");
            }
            args.extend(["-b", branch, "--", target, start]);
        }
        AddMode::Detached => args.extend(["--detach", "--", target, start]),
        AddMode::Auto => unreachable!(),
    }
    run(repo, &args).map(|_| ())
}

fn branch_exists(repo: &Path, branch: &str) -> Result<bool, Error> {
    Ok(run_output(
        repo,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )?
    .status
    .success())
}

pub fn details(repo: &Path, path: &str) -> Result<WorktreeDetails, Error> {
    let rows = list(repo)?;
    let row = registered(repo, &rows, path)?;
    removal_details(repo, &rows, row)
}

fn removal_details(
    repo: &Path,
    rows: &[WorktreeRow],
    row: &WorktreeRow,
) -> Result<WorktreeDetails, Error> {
    let root = Path::new(&row.path);
    let (bytes, index, has_submodules) = if row.bare || row.missing {
        (Vec::new(), (0, String::new()), false)
    } else {
        let checkout = crate::open_repo(root)?;
        if !same_path(checkout.common_dir(), &common_dir(repo)?) {
            return Err(Error::Git(format!(
                "Directory belongs to another repository / 目录属于其他仓库: {}",
                row.path
            )));
        }
        let index = checkout
            .index_or_empty()
            .map_err(|error| Error::Git(error.to_string()))?;
        let has_submodules = index
            .entries()
            .iter()
            .any(|entry| entry.mode == gix::index::entry::Mode::COMMIT);
        let bytes = with_read(|| {
            // Optional index writes can notify the watcher and start another review.
            run_env(
                root,
                &[
                    "status",
                    "--porcelain=v1",
                    "-z",
                    "--untracked-files=all",
                    "--ignored=matching",
                    "--ignore-submodules=none",
                ],
                &[("GIT_OPTIONAL_LOCKS", "0")],
            )
        })?
        .stdout;
        (
            bytes,
            crate::fingerprint::index_tail(&checkout.path().join("index")),
            has_submodules,
        )
    };
    let protects_commit = has_unreferenced_head(repo, rows.iter(), row)?;
    let mut stamp = DefaultHasher::new();
    (
        &row.path,
        &row.head,
        &row.branch,
        &row.locked,
        &row.prunable,
        &bytes,
        index,
        has_submodules,
        protects_commit,
    )
        .hash(&mut stamp);
    let mut fields = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());
    let mut changes = Vec::new();
    let mut change_count = 0;
    while let Some(field) = fields.next() {
        let (Some(code), Some(path)) = (field.get(..2), field.get(3..)) else {
            continue;
        };
        let code = String::from_utf8_lossy(code);
        if code.contains(['R', 'C']) {
            fields.next();
        }
        // Git's XY codes alone cannot detect edits to an already dirty file.
        let relative = gix::path::from_bstr(gix::bstr::BStr::new(
            path.strip_suffix(b"/").unwrap_or(path),
        ));
        stamp_contents(root.join(relative), &mut stamp)?;
        change_count += 1;
        if changes.len() < 200 {
            changes.push(WorktreeChange {
                code: code.into_owned(),
                path: String::from_utf8_lossy(path).into_owned(),
            });
        }
    }
    Ok(WorktreeDetails {
        branch: row.branch.clone().filter(|_| row.head.is_some()),
        changes,
        change_count,
        has_submodules,
        protects_commit,
        stamp: stamp.finish().to_string(),
    })
}

fn stamp_contents(path: PathBuf, stamp: &mut DefaultHasher) -> Result<(), Error> {
    let read_error = |source| Error::Read {
        path: path.display().to_string(),
        source,
    };
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            false.hash(stamp);
            return Ok(());
        }
        Err(error) => return Err(read_error(error)),
    };
    (
        metadata.file_type(),
        metadata.len(),
        metadata.modified().ok(),
    )
        .hash(stamp);
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Ok(());
        }
    }
    if metadata.is_dir() {
        let mut children = fs::read_dir(&path)
            .map_err(&read_error)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(read_error)?;
        children.sort_unstable();
        for child in children {
            child.file_name().hash(stamp);
            stamp_contents(child, stamp)?;
        }
    }
    Ok(())
}

fn has_unreferenced_head<'a>(
    repo: &Path,
    rows: impl Iterator<Item = &'a WorktreeRow>,
    row: &WorktreeRow,
) -> Result<bool, Error> {
    let Some(head) = row.head.as_deref().filter(|_| row.detached) else {
        return Ok(false);
    };
    let refs = run(
        repo,
        &[
            "for-each-ref",
            "--count=1",
            &format!("--contains={head}"),
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ],
    )?;
    if !refs.stdout.is_empty() {
        return Ok(false);
    }
    for other in rows.filter(|other| other.path != row.path) {
        if let Some(tip) = &other.head {
            if run_output(repo, &["merge-base", "--is-ancestor", head, tip])?
                .status
                .success()
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(crate) fn remove(
    repo: &Path,
    path: &str,
    force: bool,
    expected: Option<&str>,
    delete_branch: Option<&str>,
) -> Result<String, Error> {
    let rows = list(repo)?;
    let row = registered(repo, &rows, path)?;
    if row.main || same_path(repo, Path::new(&row.path)) {
        return Err(Error::Git("Switch worktrees before removing the current checkout. The main worktree cannot be removed. / 请先切换工作树，主工作树不能删除。".into()));
    }
    if row.locked.is_some() {
        return Err(Error::Git(
            "Unlock the worktree before removing it. / 请先解锁工作树。".into(),
        ));
    }
    if row.missing && Path::new(&row.path).exists() {
        return Err(Error::Git("The directory exists but its worktree link is unavailable. Repair or prune its registration. / 目录仍存在但链接无效，请修复或清理工作树记录。".into()));
    }
    let details = removal_details(repo, &rows, row)?;
    if details.protects_commit {
        return Err(Error::Git("Save the detached HEAD to a branch before removing this worktree. / 请先将游离 HEAD 保存为分支。".into()));
    }
    let branch_changed = delete_branch.is_some() && delete_branch != details.branch.as_deref();
    let needs_review = force || expected.is_some() || delete_branch.is_some();
    if branch_changed || (needs_review && expected != Some(details.stamp.as_str())) {
        return Err(Error::Git("Review the current worktree contents before removing it. / 请刷新并核对工作树状态后删除。".into()));
    }
    if !force && (details.change_count != 0 || details.has_submodules) {
        return Err(Error::Git(
            "Worktree contains local files or submodules. / 工作树包含本地改动或子模块。".into(),
        ));
    }
    if delete_branch.is_some_and(|branch| {
        rows.iter()
            .any(|other| other.path != row.path && other.branch.as_deref() == Some(branch))
    }) {
        return Err(Error::Git("The branch is checked out in another worktree. Clear the branch deletion checkbox to proceed. / 分支已在其他工作树检出，请取消勾选同时删除分支。".into()));
    }
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.extend(["--", &row.path]);
    run(repo, &args)?;
    let Some(branch) = delete_branch else {
        return Ok(String::new());
    };

    let reference = format!("refs/heads/{branch}");
    let deletion = run(repo, &["rev-parse", "--verify", "--quiet", &reference]).and_then(|output| {
        let tip = String::from_utf8_lossy(&output.stdout);
        if Some(tip.trim()) != row.head.as_deref() {
            return Err(Error::Git("The branch tip changed. / 分支指向已变化。".into()));
        }
        run(repo, &["branch", "-D", "--", branch]).map(|_| ())
    });
    // The checkout is already gone. Refresh its tabs even if the branch could not be deleted.
    Ok(match deletion {
        Ok(()) => String::new(),
        Err(error) => format!("Worktree removed. Could not delete branch {branch}. / 工作树已删除，分支 {branch} 删除失败。\n{error}"),
    })
}

pub(crate) fn set_lock(
    repo: &Path,
    path: &str,
    locked: bool,
    reason: Option<&str>,
) -> Result<(), Error> {
    let rows = list(repo)?;
    let row = registered(repo, &rows, path)?;
    let mut args = vec!["worktree", if locked { "lock" } else { "unlock" }];
    if let Some(reason) = reason.filter(|reason| locked && !reason.trim().is_empty()) {
        args.extend(["--reason", reason]);
    }
    args.extend(["--", &row.path]);
    run(repo, &args).map(|_| ())
}

pub(crate) fn move_to(repo: &Path, path: &str, to: &str) -> Result<(), Error> {
    let rows = list(repo)?;
    let row = registered(repo, &rows, path)?;
    if same_path(repo, Path::new(&row.path)) {
        return Err(Error::Git(
            "Switch worktrees before moving this checkout. / 请切换到另一个工作树后移动。".into(),
        ));
    }
    let target = destination(repo, to)?;
    run(
        repo,
        &[
            "worktree",
            "move",
            "--",
            &row.path,
            &target.to_string_lossy(),
        ],
    )
    .map(|_| ())
}

pub fn move_destination(repo: &Path, path: &str, to: &str) -> Result<PathBuf, Error> {
    let target = destination(repo, to)?;
    if !target.is_dir() {
        return Ok(target);
    }
    let rows = list(repo)?;
    let row = registered(repo, &rows, path)?;
    let name = Path::new(&row.path)
        .file_name()
        .ok_or_else(|| Error::Path(path.to_string()))?;
    Ok(target.join(name))
}

pub(crate) fn repair(repo: &Path, path: Option<&str>) -> Result<(), Error> {
    if let Some(path) = path {
        let target = destination(repo, path)?;
        run(
            repo,
            &["worktree", "repair", "--", &target.to_string_lossy()],
        )
        .map(|_| ())
    } else {
        run(repo, &["worktree", "repair"]).map(|_| ())
    }
}

pub fn prune_preview(repo: &Path) -> Result<WorktreePrunePreview, Error> {
    let output = with_read(|| {
        run(
            repo,
            &[
                "worktree",
                "prune",
                "--dry-run",
                "--verbose",
                "--expire=now",
            ],
        )
    })?;
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .trim()
    .to_string();
    let mut protected_paths = Vec::new();
    if !output.is_empty() {
        let rows = list(repo)?;
        let stale = |row: &WorktreeRow| {
            !row.main && row.locked.is_none() && (row.prunable.is_some() || row.missing)
        };
        for row in rows.iter().filter(|row| stale(row)) {
            if has_unreferenced_head(repo, rows.iter().filter(|other| !stale(other)), row)? {
                protected_paths.push(row.path.clone());
            }
        }
    }
    let mut stamp = DefaultHasher::new();
    (
        &output,
        registry_stamp(&common_dir(repo)?),
        &protected_paths,
    )
        .hash(&mut stamp);
    Ok(WorktreePrunePreview {
        output,
        protected_paths,
        stamp: stamp.finish().to_string(),
    })
}

pub(crate) fn prune(repo: &Path, expected: &str) -> Result<(), Error> {
    let preview = prune_preview(repo)?;
    if preview.output.is_empty() || preview.stamp != expected {
        return Err(Error::Git("Refresh the prune preview and check the listed worktrees. / 请重新预览要清理的工作树。".into()));
    }
    if let Some(path) = preview.protected_paths.first() {
        return Err(Error::Git(format!("Save the detached HEAD to a branch before pruning / 清理前请将游离 HEAD 保存为分支: {path}")));
    }
    run(repo, &["worktree", "prune", "--expire=now"]).map(|_| ())
}

pub fn compare(repo: &Path, head: &str, base: &str) -> Result<WorktreeComparison, Error> {
    let resolve = |rev| {
        let rev = check_rev(rev)?;
        let output = run(
            repo,
            &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
        )?;
        Ok::<_, Error>(String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    with_read(|| {
        let head = resolve(head)?;
        let base = resolve(base)?;
        let output = run(
            repo,
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("{base}...{head}"),
                "--",
            ],
        )?;
        let counts = String::from_utf8_lossy(&output.stdout);
        let (behind, ahead) = counts
            .trim()
            .split_once('\t')
            .ok_or_else(|| Error::Git("Invalid Git revision counts".into()))?;
        let output = run_output(repo, &["merge-base", &base, &head])?;
        let merge_base = output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string());
        Ok(WorktreeComparison {
            head,
            base,
            merge_base,
            ahead: ahead
                .parse()
                .map_err(|_| Error::Git("Invalid Git revision counts".into()))?,
            behind: behind
                .parse()
                .map_err(|_| Error::Git("Invalid Git revision counts".into()))?,
        })
    })
}
