//! Phase 5 (narrow, upstream-shaped writes) — PLAN §3.
//!
//! Exactly the writes the PLAN promises, each mapping 1:1 to an upstream
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
use std::str::FromStr;

use radicle::storage::{SignRepository, WriteRepository};

use crate::error;
use crate::profile::PySigner;
use crate::storage::PyRepository;

/// Output of `Repository.set_default_branch_to_canonical_head`
/// (upstream `storage::SetHead`).
#[pyclass(name = "SetHead", module = "radiclepy")]
#[derive(Clone)]
pub struct PySetHead {
    pub(crate) old: Option<String>,
    pub(crate) new: String,
}

#[pymethods]
impl PySetHead {
    /// Previous default-branch head (`None` if the branch did not exist).
    #[getter]
    fn old(&self) -> Option<&str> {
        self.old.as_deref()
    }
    /// New default-branch head.
    #[getter]
    #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)] // upstream field name `new`
    fn new(&self) -> &str {
        &self.new
    }
    /// Whether the head actually moved (upstream `is_updated`).
    fn is_updated(&self) -> bool {
        self.old.as_deref() != Some(self.new.as_str())
    }
    fn __repr__(&self) -> String {
        format!(
            "SetHead(old={}, new={})",
            self.old.as_deref().unwrap_or("None"),
            self.new
        )
    }
}

/// Result of `Repository.sign_refs` (upstream `storage::refs::SignedRefs`).
///
/// The cryptographic signature itself is deliberately NOT surfaced: it is
/// verified (and re-derived) by upstream on every read path; the binding
/// exposes the identity and content of the signed set.
#[pyclass(name = "SignedRefs", module = "radiclepy")]
#[derive(Clone)]
pub struct PySignedRefs {
    pub(crate) node_id: String,
    pub(crate) at: String,
    pub(crate) parent: Option<String>,
    pub(crate) feature_level: String,
    /// Sorted (refname -> target) snapshot of the signed set.
    pub(crate) refs: Vec<(String, String)>,
}

#[pymethods]
impl PySignedRefs {
    /// Node id of the signer / remote namespace these refs live under.
    #[getter]
    fn node_id(&self) -> &str {
        &self.node_id
    }
    /// Commit on `refs/rad/sigrefs` carrying this signed set.
    #[getter]
    fn at(&self) -> &str {
        &self.at
    }
    /// Parent sigrefs commit (`None` at the root).
    #[getter]
    fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }
    /// Feature level of the signed refs:
    /// `"none" | "root" | "parent" | "canonical"` (upstream `FeatureLevel`).
    #[getter]
    fn feature_level(&self) -> &str {
        &self.feature_level
    }
    /// The signed refs as `{refname: target-oid}` (BTreeMap order).
    fn refs(&self) -> std::collections::BTreeMap<String, String> {
        self.refs.iter().cloned().collect()
    }
    fn __repr__(&self) -> String {
        format!(
            "SignedRefs(node_id={}, at={}, {} ref(s))",
            self.node_id,
            self.at,
            self.refs.len()
        )
    }
}

// ─── PyRepository write operations (thin; called from storage.rs pymethods) ──

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
    let remote = remote.parse::<PublicKey>()
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
    Ok(PySignedRefs {
        node_id: signed.id().to_string(),
        at: signed.at.to_string(),
        parent: signed.parent().map(|p| p.to_string()),
        feature_level: signed.feature_level().to_string(),
        refs: signed
            .refs()
            .iter()
            .map(|(name, oid)| (name.to_string(), oid.to_string()))
            .collect(),
    })
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySetHead>()?;
    m.add_class::<PySignedRefs>()?;
    Ok(())
}

/// Validate a git commit OID string (used by COB write APIs for base/head).
pub(crate) fn parse_git_oid(value: &str) -> PyResult<radicle::git::Oid> {
    radicle::git::Oid::from_str(value)
        .map_err(|err| error::value_err(format!("invalid git OID {value:?}: {err}")))
}
