//! Identifier validators. Identifiers cross the boundary as strings
//! (PLAN §5.1); these functions validate and canonicalize them.
//!
//! Failure mode: `ValueError` — these are our own syntactic validators,
//! not operational failures (PLAN §5.8).

use std::str::FromStr;

use pyo3::prelude::*;
use radicle::cob::ObjectId;
use radicle::git::Oid;
use radicle::identity::{Did, RepoId};
use radicle::node::NodeId;

use crate::error;

/// Validate a Radicle repository id (`rad:z…`) and return its canonical form.
#[pyfunction]
pub fn parse_repo_id(value: &str) -> PyResult<String> {
    RepoId::from_str(value).map(|v| v.to_string()).map_err(|err| error::value_err(format!("invalid RepoId: {err}")))
}

/// Validate a DID (`did:key:z…`) and return its canonical form.
#[pyfunction]
pub fn parse_did(value: &str) -> PyResult<String> {
    Did::from_str(value).map(|v| v.to_string()).map_err(|err| error::value_err(format!("invalid DID: {err}")))
}

/// Validate a node id / public key (`z6Mk…`) and return its canonical form.
#[pyfunction]
pub fn parse_node_id(value: &str) -> PyResult<String> {
    NodeId::from_str(value).map(|v| v.to_string()).map_err(|err| error::value_err(format!("invalid NodeId: {err}")))
}

/// Validate a git OID (40-char hex) and return its canonical lowercase form.
#[pyfunction]
pub fn parse_oid(value: &str) -> PyResult<String> {
    Oid::from_str(value).map(|v| v.to_string()).map_err(|err| error::value_err(format!("invalid Oid: {err}")))
}

/// Validate a COB object id (40-char hex) and return its canonical form.
#[pyfunction]
pub fn parse_object_id(value: &str) -> PyResult<String> {
    ObjectId::from_str(value).map(|v| v.to_string()).map_err(|err| error::value_err(format!("invalid ObjectId: {err}")))
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
