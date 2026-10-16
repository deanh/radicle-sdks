//! Identifier validators. Identifiers cross the boundary as strings
//! (PLAN §5.1); these functions validate and canonicalize them.
//!
//! Failure mode: `ValueError` — these are our own syntactic validators,
//! not operational failures (PLAN §5.8).

use std::str::FromStr;

use pyo3::prelude::*;
use radicle::cob::{thread::CommentId, Label, ObjectId, Title};
use radicle::git::Oid;
use radicle::identity::{Did, RepoId};
use radicle::node::NodeId;

use crate::error;

/// Validate a Radicle repository id (`rad:z…`) and return its canonical form.
#[pyfunction]
pub fn parse_repo_id(value: &str) -> PyResult<String> {
    RepoId::from_str(value)
        .map(|v| v.to_string())
        .map_err(|err| error::value_err(format!("invalid RepoId: {err}")))
}

/// Validate a DID (`did:key:z…`) and return its canonical form.
#[pyfunction]
pub fn parse_did(value: &str) -> PyResult<String> {
    Did::from_str(value)
        .map(|v| v.to_string())
        .map_err(|err| error::value_err(format!("invalid DID: {err}")))
}

/// Validate a node id / public key (`z6Mk…`) and return its canonical form.
#[pyfunction]
pub fn parse_node_id(value: &str) -> PyResult<String> {
    NodeId::from_str(value)
        .map(|v| v.to_string())
        .map_err(|err| error::value_err(format!("invalid NodeId: {err}")))
}

/// Validate a git OID (40-char hex) and return its canonical lowercase form.
#[pyfunction]
pub fn parse_oid(value: &str) -> PyResult<String> {
    Oid::from_str(value)
        .map(|v| v.to_string())
        .map_err(|err| error::value_err(format!("invalid Oid: {err}")))
}

/// Validate a COB object id (40-char hex) and return its canonical form.
#[pyfunction]
pub fn parse_object_id(value: &str) -> PyResult<String> {
    ObjectId::from_str(value)
        .map(|v| v.to_string())
        .map_err(|err| error::value_err(format!("invalid ObjectId: {err}")))
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Report the public package as the defining module, not the compiled
    // extension submodule (introspection / help() quality).
    for func in [
        wrap_pyfunction!(parse_repo_id, m)?,
        wrap_pyfunction!(parse_did, m)?,
        wrap_pyfunction!(parse_node_id, m)?,
        wrap_pyfunction!(parse_oid, m)?,
        wrap_pyfunction!(parse_object_id, m)?,
    ] {
        func.setattr("__module__", "radiclepy")?;
        m.add_function(func)?;
    }
    Ok(())
}

/// ValueError for a malformed ObjectId, CobError for a store failure.
pub(crate) fn parse_cob_id(value: &str) -> PyResult<ObjectId> {
    ObjectId::from_str(value).map_err(|err| error::value_err(format!("invalid ObjectId: {err}")))
}

pub(crate) fn parse_title(value: &str) -> PyResult<Title> {
    Title::new(value).map_err(|e| error::value_err(format!("invalid title: {e}")))
}

pub(crate) fn parse_labels(values: Vec<String>) -> PyResult<Vec<Label>> {
    values
        .iter()
        .map(|l| {
            Label::new(l.as_str())
                .map_err(|e| error::value_err(format!("invalid label {l:?}: {e}")))
        })
        .collect()
}

pub(crate) fn parse_dids(values: Vec<String>) -> PyResult<Vec<Did>> {
    values
        .iter()
        .map(|d| {
            d.parse::<Did>()
                .map_err(|e| error::value_err(format!("invalid DID {d:?}: {e}")))
        })
        .collect()
}

pub(crate) fn parse_comment_id(value: &str) -> PyResult<CommentId> {
    value
        .parse::<CommentId>()
        .map_err(|e| error::value_err(format!("invalid comment id {value:?}: {e}")))
}

/// Validate a git commit OID string (used by COB write APIs for base/head).
pub(crate) fn parse_git_oid(value: &str) -> PyResult<radicle::git::Oid> {
    radicle::git::Oid::from_str(value)
        .map_err(|err| error::value_err(format!("invalid git OID {value:?}: {err}")))
}
