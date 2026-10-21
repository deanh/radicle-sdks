//! Non-interactive local project workflows. The crate (not `rad init` CLI)
//! performs the work; upstream init/checkout invoke the Git executable.
mod values;
pub(crate) use values::ProjectDoc;
use values::ProjectLocation;

use pyo3::prelude::*;
use radicle::git;
use radicle::identity::{RepoId, Visibility};
use std::path::PathBuf;

use crate::{error, profile::PySigner, repository::PyStorage};

fn rid(value: &str) -> PyResult<RepoId> {
    value
        .parse()
        .map_err(|e| error::value_err(format!("invalid RepoId: {e}")))
}

/// Discover Git from an explicit path and inspect one named fetch remote.
/// No profile, storage access, subprocess, identity verification, or writes.
#[pyfunction]
#[pyo3(signature = (path, *, remote_name="rad"))]
pub fn project_at(path: PathBuf, remote_name: &str) -> PyResult<Option<ProjectLocation>> {
    if !git::raw::Remote::is_valid_name(remote_name) {
        return Err(error::value_err(
            "remote_name must be a valid Git remote name",
        ));
    }
    std::fs::metadata(&path).map_err(error::git_err)?;
    let repo = match git::raw::Repository::discover(path) {
        Ok(repo) => repo,
        Err(e) if e.code() == git::raw::ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(error::git_err(e)),
    };
    let remote = match repo.find_remote(remote_name) {
        Ok(remote) => remote,
        Err(e) if e.code() == git::raw::ErrorCode::NotFound => {
            // libgit2 also reports NotFound for an existing remote section
            // lacking a fetch URL. Do not disguise broken config as absence.
            let config = repo.config().map_err(error::git_err)?;
            let mut entries = config.entries(None).map_err(error::git_err)?;
            let prefix = format!("remote.{remote_name}.");
            while let Some(entry) = entries.next() {
                if entry
                    .map_err(error::git_err)?
                    .name_bytes()
                    .starts_with(prefix.as_bytes())
                {
                    return Err(error::git_err(e));
                }
            }
            return Ok(None);
        }
        Err(e) => return Err(error::git_err(e)),
    };
    let url: git::Url = remote
        .url()
        .map_err(error::git_err)?
        .parse()
        .map_err(error::git_err)?;
    Ok(Some(ProjectLocation {
        rid: url.repo.to_string(),
        remote_name: remote_name.to_owned(),
        namespace: url.namespace.map(|key| key.to_string()),
    }))
}

#[pyfunction]
#[pyo3(signature = (storage, signer, path, *, name, description, default_branch, visibility="public"))]
pub fn init_project(
    storage: &PyStorage,
    signer: &PySigner,
    path: PathBuf,
    name: &str,
    description: &str,
    default_branch: &str,
    visibility: &str,
) -> PyResult<String> {
    let name = name
        .try_into()
        .map_err(|e| error::value_err(format!("invalid project name: {e}")))?;
    // A short heads name is required; validating before touching storage also
    // gives deterministic errors for an empty or malformed branch name.
    if default_branch.is_empty()
        || !git::raw::Reference::is_valid_name(&format!("refs/heads/{default_branch}"))
        || default_branch.starts_with("refs/")
    {
        return Err(error::value_err(
            "default_branch must be a valid short branch name",
        ));
    }
    let branch = git::fmt::RefStr::try_from_str(default_branch)
        .map_err(|e| error::value_err(format!("invalid default branch: {e}")))?
        .to_ref_string();
    let visibility = match visibility {
        "public" => Visibility::Public,
        "private" => Visibility::private([]),
        _ => return Err(error::value_err("visibility must be 'public' or 'private'")),
    };
    let repo = git::raw::Repository::open(&path).map_err(error::git_err)?;
    // Upstream init cleanup deletes `rad` on failure, even if it predates
    // this call. Match the CLI's preflight to preserve existing configuration
    // and remote-tracking refs before any upstream side effects.
    match repo.find_remote(&radicle::rad::REMOTE_NAME) {
        Ok(_) => {
            return Err(error::op_err(format!(
                "repository already has a '{}' remote; refusing to initialize",
                *radicle::rad::REMOTE_NAME,
            )));
        }
        Err(err) if err.code() == git::raw::ErrorCode::NotFound => {}
        Err(err) => return Err(error::git_err(err)),
    }
    // init's upstream push does not check the subprocess exit status. Ensure
    // the requested branch really has a commit before allowing side effects.
    repo.find_reference(&format!("refs/heads/{default_branch}"))
        .and_then(|r| r.peel_to_commit())
        .map_err(error::git_err)?;
    let (id, _, _) = radicle::rad::init(
        &repo,
        name,
        description,
        branch,
        visibility,
        &*signer.inner,
        &storage.inner.raw,
    )
    .map_err(|e| error::op_err(format!("initialize project: {e}")))?;
    Ok(id.to_string())
}

#[pyfunction]
pub fn fork_project(storage: &PyStorage, signer: &PySigner, repo_id: &str) -> PyResult<()> {
    radicle::rad::fork(rid(repo_id)?, &*signer.inner, &storage.inner.raw)
        .map_err(|e| error::op_err(format!("fork project: {e}")))
}

#[pyfunction]
#[pyo3(signature = (storage, repo_id, remote, path, *, bare=false))]
pub fn checkout_project(
    storage: &PyStorage,
    repo_id: &str,
    remote: &str,
    path: PathBuf,
    bare: bool,
) -> PyResult<PathBuf> {
    let remote = remote
        .parse()
        .map_err(|e| error::value_err(format!("invalid NodeId: {e}")))?;
    radicle::rad::checkout(rid(repo_id)?, &remote, &path, &storage.inner.raw, bare)
        .map_err(|e| error::op_err(format!("checkout project: {e}")))?;
    Ok(path)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ProjectLocation>()?;
    m.add_function(wrap_pyfunction!(project_at, m)?)?;
    m.add_function(wrap_pyfunction!(init_project, m)?)?;
    m.add_function(wrap_pyfunction!(fork_project, m)?)?;
    m.add_function(wrap_pyfunction!(checkout_project, m)?)?;
    Ok(())
}
