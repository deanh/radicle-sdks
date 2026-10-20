//! Narrow, upstream-shaped repository writes.
//!
//! Each write maps 1:1 to an upstream
//! method on `storage::WriteRepository` / `storage::SignRepository`:
//!
//! * `set_default_branch_to_canonical_head` → `WriteRepository`
//! * `set_identity_head`                    → `WriteRepository`
//! * `set_remote_identity_root`             → `WriteRepository`
//! * `sign_refs(signer)`                    → `SignRepository` — the ONLY
//!   signer-taking write; the other three take no signer upstream and take
//!   none here. No hidden "write then sign" composition: sequencing is the
//!   caller's explicit choice.
//!
//! All of these raise `RepositoryError` on failure (upstream
//! `storage::RepositoryError`), like every other repository operation.

use pyo3::prelude::*;

use radicle::storage::{SignRepository, WriteRepository};

use super::{
    PyRepository,
    values::{PySetHead, PySignedRefs},
};
use crate::error;
use crate::profile::PySigner;

// ─── Thin helpers called by repository pymethods ────────────────────────────

use radicle::crypto::PublicKey;

pub(crate) fn set_default_branch_to_canonical_head(
    repo_handle: &PyRepository,
) -> PyResult<PySetHead> {
    let repo = repo_handle.open()?;
    let head = repo
        .set_default_branch_to_canonical_head()
        .map_err(error::repository_err)?;
    Ok(PySetHead {
        old: head.old.map(|o| o.to_string()),
        new: head.new.to_string(),
    })
}

pub(crate) fn set_identity_head(repo_handle: &PyRepository) -> PyResult<String> {
    let repo = repo_handle.open()?;
    let oid = repo.set_identity_head().map_err(error::repository_err)?;
    Ok(oid.to_string())
}

pub(crate) fn set_remote_identity_root(
    repo_handle: &PyRepository,
    remote: &str,
) -> PyResult<String> {
    let remote = remote
        .parse::<PublicKey>()
        .map_err(|err| error::value_err(format!("invalid NodeId {remote:?}: {err}")))?;
    let repo = repo_handle.open()?;
    let oid = repo
        .set_remote_identity_root(&remote)
        .map_err(error::repository_err)?;
    Ok(oid.to_string())
}

pub(crate) fn sign_refs(repo_handle: &PyRepository, signer: &PySigner) -> PyResult<PySignedRefs> {
    let repo = repo_handle.open()?;
    let signed = repo
        .sign_refs(&*signer.inner)
        .map_err(error::repository_err)?;
    Ok(signed.into())
}
