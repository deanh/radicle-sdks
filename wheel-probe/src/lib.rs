//! Phase 0A wheel probe: proves radicle links into an abi3 Python extension.
use pyo3::prelude::*;
use std::str::FromStr;

use radicle::identity::RepoId;

#[pyfunction]
fn parse_repo_id(value: &str) -> PyResult<String> {
    RepoId::from_str(value)
        .map(|id| id.to_string())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("invalid RepoId: {e}")))
}

#[pyfunction]
fn version() -> String {
    // Compile-time proof of the exact radicle version linked in.
    let _ = radicle::prelude::Profile::load; // touch the API surface
    "radicle=0.25.1".to_string()
}

#[pymodule]
fn wheel_probe(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_repo_id, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
