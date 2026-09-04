use pyo3::prelude::*;

mod error;
mod identity;
mod profile;

#[pymodule]
fn radiclepy(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    error::register(py, m)?;
    identity::register(py, m)?;
    profile::register(py, m)?;
    Ok(())
}
