use pyo3::prelude::*;

mod error;
mod identity;
mod profile;
mod storage;
#[cfg(feature = "test-helpers")]
mod testhelpers;
mod values;

#[pymodule]
fn radiclepy(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    error::register(py, m)?;
    identity::register(py, m)?;
    profile::register(py, m)?;
    storage::register(py, m)?;
    storage::register_values(py, m)?;
    #[cfg(feature = "test-helpers")]
    testhelpers::register(py, m)?;
    Ok(())
}
