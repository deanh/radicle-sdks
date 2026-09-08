use pyo3::prelude::*;

mod cob_raw;
mod cob_typed;
mod error;
mod identity;
mod policy;
mod profile;
mod routing;
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
    policy::register(py, m)?;
    routing::register(py, m)?;
    m.add_class::<values::FollowPolicyInfo>()?;
    m.add_class::<values::SeedPolicyInfo>()?;
    m.add_class::<values::RoutingEntry>()?;
    cob_typed::register(py, m)?;
    cob_raw::register(py, m)?;
    #[cfg(feature = "test-helpers")]
    testhelpers::register(py, m)?;
    Ok(())
}
