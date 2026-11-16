//! Conservative fetch-first pulls. Unusual pull policies stay with native Git.

use std::path::Path;

/// Small metadata snapshot for deciding whether the UI needs refs/history again.
/// It deliberately contains no worktree status and is never logged or serialized.
#[derive(PartialEq, Eq)]
pub struct RefreshState {
    head: String,
    refs: Vec<(String, String)>,
    config: String,
    modules: Option<Vec<u8>>,
    metadata: Vec<(String, Option<Vec<u8>>)>,
}

pub fn refresh_state(path: &Path) -> Option<RefreshState> {
    let repo = crate::open_repo(path).ok()?;
    let head = repo.head().ok()?;
    let head = format!("{:?}:{}", head.referent_name(), repo.head_id().ok()?);
    let mut refs = Vec::new();
    for reference in repo.references().ok()?.all().ok()? {
        let reference = reference.ok()?;
        refs.push((reference.name().as_bstr().to_string(), format!("{:?}", reference.target())));
    }
    refs.sort();
    let modules = match std::fs::read(path.join(".gitmodules")) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return None,
    };
    // Submodule HEADs and external worktrees can change without moving parent refs.
    if modules.is_some() { return None; }
    let read = |file: &Path| -> Option<Option<Vec<u8>>> {
        match std::fs::read(file) {
            Ok(bytes) => Some(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(None),
            Err(_) => None,
        }
    };
    let mut metadata = Vec::new();
    for name in ["awegit", "gitnuro"] { metadata.push((name.into(), read(&repo.git_dir().join(name))?)); }
    match std::fs::read_dir(repo.common_dir().join("worktrees")) {
        Ok(entries) => for entry in entries {
            let entry = entry.ok()?;
            for name in ["HEAD", "gitdir", "commondir", "locked", "config.worktree"] {
                metadata.push((format!("{}/{name}", entry.file_name().to_string_lossy()), read(&entry.path().join(name))?));
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(_) => return None,
    }
    metadata.sort_by(|left, right| left.0.cmp(&right.0));
    Some(RefreshState { head, refs, config: repo.config_snapshot().to_string(), modules, metadata })
}

pub(crate) struct Prepared {
    pub head: String,
    branch: String,
    pub remote: String,
    pub remote_branch: String,
    config: String,
}

pub(crate) fn prepare(path: &Path, explicit: Option<(&str, &str)>) -> Option<Prepared> {
    let repo = crate::open_repo(path).ok()?;
    // Native pull also coordinates recursive submodule updates and shallow history.
    if repo.git_dir() != repo.common_dir() || repo.is_shallow() { return None; }
    let index = repo.index_or_empty().ok()?;
    if index.entries().iter().any(|entry| entry.mode == gix::index::entry::Mode::COMMIT) { return None; }
    let head_ref = repo.head().ok()?;
    let branch = head_ref.referent_name()?.as_bstr().to_string();
    let name = branch.strip_prefix("refs/heads/")?;
    let head = repo.head_id().ok()?.to_string();
    let config = repo.config_snapshot();
    // An implicit fetch-all may apply another remote's refspec to local HEAD.
    // Native pull coordinates the corresponding worktree update under protection.
    if config.boolean("fetch.all").unwrap_or(false) { return None; }
    let remote = config.string(format!("branch.{name}.remote"))?.to_string();
    if remote == "." || crate::cli::check_name(&remote).is_err() { return None; }
    let merges = config.strings(format!("branch.{name}.merge"))?;
    if merges.len() != 1 { return None; }
    let merge = merges[0].to_string();
    let remote_branch = merge.strip_prefix("refs/heads/")?.to_string();
    crate::cli::check_name(&remote_branch).ok()?;
    if explicit.is_some_and(|(given_remote, given_branch)| given_remote != remote || given_branch != remote_branch) { return None; }
    let specs = config.strings(format!("remote.{remote}.fetch"))?;
    let expected = format!("+refs/heads/*:refs/remotes/{remote}/*");
    if specs.len() != 1 || specs[0].as_ref() != gix::bstr::BStr::new(expected.as_bytes()) { return None; }
    let mut tracking = repo.find_reference(format!("refs/remotes/{remote}/{remote_branch}").as_str()).ok()?;
    if tracking.peel_to_id_in_place().ok()?.to_string() != head { return None; }
    // Do not bypass signature checks, no-ff merges, strategies, or recursive policies.
    for key in ["pull.ff", "merge.ff"] {
        if config.string(key).is_some() && config.boolean(key) != Some(true)
            && !(key == "pull.ff" && config.string(key)?.as_ref() == gix::bstr::BStr::new(b"only")) { return None; }
    }
    if config.boolean("submodule.recurse").unwrap_or(false) { return None; }
    for key in [format!("branch.{name}.mergeOptions"), "pull.twohead".into(), "pull.octopus".into(), "merge.verifySignatures".into()] {
        if config.string(key).is_some() { return None; }
    }
    if path.join(".gitmodules").exists() || repo.git_dir().join("info/grafts").exists() { return None; }
    for reference in repo.references().ok()?.all().ok()? {
        if reference.ok()?.name().as_bstr().starts_with(b"refs/replace/") { return None; }
    }
    Some(Prepared { head, branch, remote, remote_branch, config: config.to_string() })
}

impl Prepared {
    pub fn target(&self, path: &Path) -> Option<String> {
        let repo = crate::open_repo(path).ok()?;
        if repo.config_snapshot().to_string() != self.config { return None; }
        let head = repo.head().ok()?;
        if head.referent_name()?.as_bstr().to_string() != self.branch || repo.head_id().ok()?.to_string() != self.head { return None; }
        // Only this fetch's single merge candidate counts; stale tracking refs do not.
        let text = std::fs::read_to_string(repo.git_dir().join("FETCH_HEAD")).ok()?;
        let targets: Vec<_> = text.lines().filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let oid = fields.next()?;
            (fields.next()? == "").then_some(oid)
        }).collect();
        if targets.len() != 1 { return None; }
        let oid = gix::hash::ObjectId::from_hex(targets[0].as_bytes()).ok()?;
        repo.find_object(oid).ok()?.try_into_commit().ok()?;
        Some(oid.to_string())
    }
}
