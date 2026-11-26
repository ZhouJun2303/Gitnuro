mod push;

pub(crate) use push::check_push;

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cli::{run, run_graph, run_output, run_stdin};
use crate::{Error, SubmoduleRow};

#[derive(Default)]
pub(crate) struct ModuleConfig {
    pub branch: String,
    pub url: String,
}

pub(crate) fn module_config(
    repo: &Path,
    source: &[&str],
) -> Result<BTreeMap<String, ModuleConfig>, Error> {
    let mut args = vec!["config", "-z", "--no-includes"];
    args.extend_from_slice(source);
    args.extend(["--get-regexp", r"^submodule\..*\.(path|url|branch)$"]);
    let output = run_output(repo, &args)?;
    if !matches!(output.status.code(), Some(0 | 1)) {
        return Err(Error::Git(diagnostic(&String::from_utf8_lossy(
            &output.stderr,
        ))));
    }
    let mut entries = BTreeMap::<String, (String, ModuleConfig)>::new();
    for item in output.stdout.split(|byte| *byte == 0) {
        let item = String::from_utf8_lossy(item);
        let Some((key, value)) = item.split_once('\n') else {
            continue;
        };
        let Some((name, key)) = key
            .strip_prefix("submodule.")
            .and_then(|key| key.rsplit_once('.'))
        else {
            continue;
        };
        let (path, module) = entries.entry(name.to_string()).or_default();
        match key {
            "path" => *path = value.to_string(),
            "url" => module.url = value.to_string(),
            "branch" => module.branch = value.to_string(),
            _ => {}
        }
    }
    Ok(entries
        .into_values()
        .filter(|(path, _)| !path.is_empty())
        .collect())
}

pub(crate) fn config_key(module: Option<&ModuleConfig>, branch: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    module.map(|config| config.url.as_str()).hash(&mut hasher);
    module
        .map(|config| config.branch.as_str())
        .hash(&mut hasher);
    branch.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(crate) fn valid_oid(id: &str) -> bool {
    matches!(id.len(), 40 | 64) && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn diagnostic(text: &str) -> String {
    crate::flow::redact(text.trim())
}

fn display_url(url: &str) -> String {
    diagnostic(url.split(['?', '#']).next().unwrap_or(url))
}

fn relative_url(url: &str) -> bool {
    url.starts_with("./") || url.starts_with("../")
}

/// Git joins a relative submodule URL to the remote repository path, including
/// its last component. For SCP URLs, crossing that component leaves `host:`.
fn resolve_url(repo: &Path, base: &str, url: &str) -> Result<String, Error> {
    if url.is_empty() || url.starts_with('-') || url.contains(['\0', '\r', '\n']) {
        return Err(Error::Git(
            "Invalid or missing submodule URL / 子模块 URL 缺失或无效".into(),
        ));
    }
    if !relative_url(url) {
        return Ok(url.into());
    }
    let mut base = if base.is_empty() || base == "." {
        repo.to_string_lossy().replace('\\', "/")
    } else if base.contains("://") || scp_separator(base).is_some() {
        base.to_string()
    } else {
        repo.join(base).to_string_lossy().replace('\\', "/")
    };
    let root = if let Some((scheme, host)) = base.split_once("://") {
        let authority = host.find('/').unwrap_or(host.len());
        scheme.len() + 3 + authority
    } else if let Some(colon) = scp_separator(&base) {
        colon + 1
    } else {
        base.find('/').map_or(0, |slash| slash + 1)
    };
    let mut tail = url;
    while let Some(rest) = tail.strip_prefix("../").or_else(|| tail.strip_prefix("./")) {
        if tail.starts_with("../") {
            let end = base.trim_end_matches('/').len();
            if end <= root {
                return Err(Error::Git("Relative submodule URL exceeds its remote path / 相对子模块 URL 超出主仓库远端路径".into()));
            }
            let end = base[..end]
                .rfind('/')
                .filter(|slash| *slash >= root)
                .unwrap_or(root);
            base.truncate(end);
        }
        tail = rest;
    }
    if !base.ends_with([':', '/']) {
        base.push('/');
    }
    base.push_str(tail);
    Ok(base)
}

fn scp_separator(value: &str) -> Option<usize> {
    let colon = value.rfind(':')?;
    (colon > 0
        && !value[..colon].contains(['/', '\\'])
        && !(cfg!(windows) && colon == 1 && value.as_bytes()[0].is_ascii_alphabetic()))
    .then_some(colon)
}

fn remote_url(repo: &Path, name: &str) -> Option<String> {
    let output = run(repo, &["remote", "get-url", name]).ok()?;
    Some(
        String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string(),
    )
}

fn default_remote_url(repo: &Path, branch: &str) -> String {
    let remote = crate::open_repo(repo)
        .ok()
        .and_then(|opened| {
            opened
                .config_snapshot()
                .string(format!("branch.{branch}.remote"))
                .map(|remote| remote.to_string())
        })
        .unwrap_or_else(|| "origin".into());
    if remote == "." {
        return ".".into();
    }
    remote_url(repo, &remote).unwrap_or_default()
}

// Only use an actual child checkout as the object database, never the parent
// found by Git when the submodule directory is empty or has been deinitialized.
fn usable_checkout(repo: &Path, path: &str) -> Option<PathBuf> {
    let checkout = repo.join(crate::cli::check_path(path).ok()?);
    let child = gix::open(&checkout).ok()?;
    let workdir = child.workdir()?;
    (crate::repo_key(workdir) == crate::repo_key(&checkout)).then_some(checkout)
}

#[derive(Default)]
struct RemoteRefs {
    head: String,
    refs: BTreeMap<String, String>,
}

const TRANSPORT: &[&str] = &[
    "-c",
    "protocol.allow=never",
    "-c",
    "protocol.ext.allow=never",
    "-c",
    "protocol.file.allow=always",
    "-c",
    "protocol.git.allow=always",
    "-c",
    "protocol.ssh.allow=always",
    "-c",
    "protocol.http.allow=always",
    "-c",
    "protocol.https.allow=always",
];

impl RemoteRefs {
    fn read(repo: &Path, url: &str, submodule: bool) -> Result<Self, Error> {
        let mut args = if submodule {
            TRANSPORT.to_vec()
        } else {
            Vec::new()
        };
        args.extend(["ls-remote", "--symref", "--", url]);
        if submodule {
            args.extend(["HEAD", "refs/heads/*", "refs/tags/*"]);
        }
        let output = run(repo, &args)?;
        let mut remote = Self::default();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Some((id, name)) = line.split_once('\t') else {
                continue;
            };
            if name == "HEAD" {
                if let Some(branch) = id.strip_prefix("ref: refs/heads/") {
                    remote.head = branch.into();
                }
            } else if valid_oid(id) {
                remote.refs.insert(name.into(), id.into());
            }
        }
        Ok(remote)
    }

    fn tips(&self) -> impl Iterator<Item = (&str, &str)> {
        self.refs.iter().filter_map(|(name, id)| {
            if name.ends_with("^{}") {
                return None;
            }
            let id = self.refs.get(&format!("{name}^{{}}")).unwrap_or(id);
            Some((name.as_str(), id.as_str()))
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum RemoteState {
    InTarget,
    OutsideTarget,
    TargetMissing,
    Reachable,
    Unreachable,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteCommit {
    id: String,
    state: RemoteState,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    reference: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleRemoteRow {
    path: String,
    config_key: String,
    target: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    recorded: Option<RemoteCommit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checkout: Option<RemoteCommit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// A scratch object store is only needed when a current advertised tip or pin
// is missing locally. It never changes a checkout, its refs, or FETCH_HEAD.
struct Probe<'a> {
    context: &'a Path,
    url: &'a str,
    remote: RemoteRefs,
    checkout: Option<PathBuf>,
    scratch: Option<Result<tempfile::TempDir, String>>,
}

impl<'a> Probe<'a> {
    fn open(context: &'a Path, url: &'a str, checkout: Option<PathBuf>) -> Result<Self, Error> {
        let remote = RemoteRefs::read(context, url, true)?;
        let checkout = checkout.filter(|path| {
            gix::open(path).is_ok_and(|repo| {
                !repo.is_shallow()
                    && repo
                        .config_snapshot()
                        .string("extensions.partialClone")
                        .is_none()
                    && repo.config_snapshot().boolean("remote.origin.promisor") != Some(true)
            })
        });
        Ok(Self {
            context,
            url,
            remote,
            checkout,
            scratch: None,
        })
    }

    fn inspect(&mut self, id: &str, target: &str) -> Result<RemoteCommit, Error> {
        let branch = format!("refs/heads/{target}");
        let target_id = self.remote.refs.get(&branch);
        let (available, in_target, reference) = if target_id.is_some_and(|tip| tip == id) {
            (true, true, Some(branch))
        } else {
            let local = self.checkout.as_deref().map(|path| {
                commit_reachability(path, id, &self.remote, target_id.map(String::as_str))
            });
            match local {
                Some(Ok(result)) => result,
                _ if self.remote.refs.is_empty() => (false, false, None),
                _ => {
                    let scratch = self
                        .scratch
                        .get_or_insert_with(|| {
                            fetch_graph(
                                self.context,
                                self.url,
                                &self.remote,
                                self.checkout.as_deref(),
                                id,
                            )
                            .map_err(|error| diagnostic(&error.to_string()))
                        })
                        .as_ref()
                        .map_err(|error| Error::Git(error.clone()))?;
                    commit_reachability(
                        scratch.path(),
                        id,
                        &self.remote,
                        target_id.map(String::as_str),
                    )?
                }
            }
        };
        let state = if in_target {
            RemoteState::InTarget
        } else if !available {
            RemoteState::Unreachable
        } else if target.is_empty() {
            RemoteState::Reachable
        } else if target_id.is_none() {
            RemoteState::TargetMissing
        } else {
            RemoteState::OutsideTarget
        };
        Ok(RemoteCommit {
            id: id.into(),
            state,
            reference,
        })
    }
}

fn commit_reachability(
    repo: &Path,
    id: &str,
    refs: &RemoteRefs,
    target: Option<&str>,
) -> Result<(bool, bool, Option<String>), Error> {
    let mut input = format!("{id}\n");
    for (_, tip) in refs.tips() {
        input.push_str(tip);
        input.push('\n');
    }
    let types = run_graph(
        repo,
        &["cat-file", "--batch-check=%(objecttype)"],
        Some(input.as_bytes()),
    )?;
    if !types.status.success() {
        return Err(Error::Git(diagnostic(&String::from_utf8_lossy(
            &types.stderr,
        ))));
    }
    let text = String::from_utf8_lossy(&types.stdout);
    let mut kinds = text.lines();
    let exists = kinds.next() == Some("commit");
    let mut commits = Vec::new();
    for ((name, tip), kind) in refs.tips().zip(kinds) {
        if kind.ends_with(" missing") {
            return Err(Error::Git(format!(
                "Missing advertised tip {name} ({tip}) / 本地缺少远端引用的提交"
            )));
        }
        if kind == "commit" {
            commits.push((name, tip));
        }
    }
    if !exists {
        return Ok((false, false, None));
    }
    if let Some(target) = target {
        let output = graph_output(repo, &["merge-base", "--is-ancestor", id, target])?;
        if output {
            return Ok((true, true, None));
        }
    }
    if let Some((name, _)) = commits.iter().find(|(_, tip)| *tip == id) {
        return Ok((true, false, Some((*name).into())));
    }
    let mut revisions = format!("{id}\n");
    for (_, tip) in commits {
        revisions.push_str(&format!("^{tip}\n"));
    }
    let output = run_graph(
        repo,
        &["rev-list", "--max-count=1", "--stdin"],
        Some(revisions.as_bytes()),
    )?;
    if !output.status.success() {
        return Err(Error::Git(diagnostic(&String::from_utf8_lossy(
            &output.stderr,
        ))));
    }
    Ok((output.stdout.is_empty(), false, None))
}

fn graph_output(repo: &Path, args: &[&str]) -> Result<bool, Error> {
    let output = run_graph(repo, args, None)?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(Error::Git(diagnostic(&String::from_utf8_lossy(
            &output.stderr,
        )))),
    }
}

fn fetch_graph(
    context: &Path,
    url: &str,
    refs: &RemoteRefs,
    checkout: Option<&Path>,
    id: &str,
) -> Result<tempfile::TempDir, Error> {
    let scratch = tempfile::Builder::new()
        .prefix("awegit-submodule-")
        .tempdir()
        .map_err(|error| Error::Git(error.to_string()))?;
    let destination = scratch.path().to_string_lossy();
    let format = if id.len() == 64 {
        "--object-format=sha256"
    } else {
        "--object-format=sha1"
    };
    run(
        context,
        &[
            "init",
            "--bare",
            "--quiet",
            "--template=",
            format,
            &destination,
        ],
    )?;
    if let Some(objects) = checkout
        .and_then(|path| gix::open(path).ok())
        .and_then(|repo| repo.common_dir().join("objects").canonicalize().ok())
    {
        let objects = objects.to_string_lossy().replace('\\', "/");
        let objects = objects.strip_prefix("//?/").unwrap_or(&objects);
        std::fs::write(
            scratch.path().join("objects/info/alternates"),
            format!("{objects}\n"),
        )
        .map_err(|error| Error::Git(error.to_string()))?;
    }
    let mut args = TRANSPORT.to_vec();
    args.extend([
        "--git-dir",
        &destination,
        "fetch",
        "--no-auto-gc",
        "--no-recurse-submodules",
        "--no-tags",
        "--no-write-fetch-head",
        "--filter=tree:0",
        "--stdin",
        "--",
        url,
    ]);
    // Fetch the advertised object ids. A branch moved between the two requests
    // is an indeterminate check if the server has already dropped the old tip.
    let mut tips: Vec<_> = refs
        .refs
        .iter()
        .filter(|(name, _)| !name.ends_with("^{}"))
        .map(|(_, id)| id.as_str())
        .collect();
    tips.sort_unstable();
    tips.dedup();
    run_stdin(context, &args, tips.join("\n").as_bytes())?;
    Ok(scratch)
}

fn inspect_row(
    repo: &Path,
    row: SubmoduleRow,
    config: Option<&ModuleConfig>,
    parent_url: &str,
) -> SubmoduleRemoteRow {
    let mut report = SubmoduleRemoteRow {
        path: row.path,
        config_key: row.config_key,
        target: row.configured_branch,
        url: String::new(),
        recorded: None,
        checkout: None,
        error: row.config_error,
    };
    let mut check = || -> Result<(), Error> {
        if report.error.is_some() {
            return Ok(());
        }
        let url = resolve_url(
            repo,
            parent_url,
            config.map_or("", |config| config.url.as_str()),
        )?;
        report.url = display_url(&url);
        let checkout = usable_checkout(repo, &report.path);
        let context = checkout
            .as_deref()
            .filter(|_| url_has_host_or_root(&url))
            .unwrap_or(repo);
        let mut probe = Probe::open(context, &url, checkout.clone())?;
        if report.target.is_empty() && !row.inherits_branch {
            report.target = probe.remote.head.clone();
        }
        if !row.recorded.is_empty() {
            report.recorded = Some(probe.inspect(&row.recorded, &report.target)?);
        }
        if row.ready && row.id != row.recorded {
            report.checkout = Some(probe.inspect(&row.id, &report.target)?);
        }
        Ok(())
    };
    if let Err(error) = check() {
        report.error = Some(diagnostic(&error.to_string()));
    }
    report
}

fn url_has_host_or_root(url: &str) -> bool {
    Path::new(url).is_absolute() || url.contains("://") || scp_separator(url).is_some()
}

pub fn submodule_remote_status(
    repo: &Path,
    path: Option<&str>,
) -> Result<Vec<SubmoduleRemoteRow>, Error> {
    let (rows, config) = crate::model::submodules_with_config(repo)?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let branch = crate::current_branch(repo)?.unwrap_or_default();
    let parent_url = default_remote_url(repo, &branch);
    let mut reports = Vec::new();
    for row in rows {
        crate::check_cancelled()?;
        if path.is_some_and(|path| row.path != path) {
            continue;
        }
        let config = config.get(&row.path);
        reports.push(inspect_row(repo, row, config, &parent_url));
    }
    Ok(reports)
}

/// Current URLs for account selection. A push can also inspect historical URLs.
pub fn submodule_urls(repo: &Path) -> Vec<String> {
    if !repo.join(".gitmodules").is_file() {
        return Vec::new();
    }
    let Ok(config) = module_config(repo, &["--file", ".gitmodules"]) else {
        return Vec::new();
    };
    let branch = crate::current_branch(repo)
        .ok()
        .flatten()
        .unwrap_or_default();
    let base = config
        .values()
        .any(|module| relative_url(&module.url))
        .then(|| default_remote_url(repo, &branch))
        .unwrap_or_default();
    config
        .values()
        .filter_map(|module| resolve_url(repo, &base, &module.url).ok())
        .collect()
}
