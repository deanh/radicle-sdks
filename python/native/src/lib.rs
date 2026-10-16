use pyo3::prelude::*;

mod comment;
mod error;
mod ids;
mod issues;
mod patches;
mod policy;
mod profile;
mod project;
mod raw;
mod repository;
mod routing;
#[cfg(feature = "test-helpers")]
mod testhelpers;
mod hashing; // Shared process-local snapshot hashing only; DTOs are domain-owned.

#[pymodule]
fn radiclepy(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    error::register(py, m)?;
    ids::register(py, m)?;
    profile::register(py, m)?;
    project::register(m)?;
    m.add_class::<comment::Comment>()?;
    m.add_class::<patches::Review>()?;
    m.add_class::<patches::Revision>()?;
    repository::register(py, m)?;
    repository::register_values(py, m)?;
    policy::register(py, m)?;
    routing::register(py, m)?;
    m.add_class::<policy::FollowPolicyInfo>()?;
    m.add_class::<policy::SeedPolicyInfo>()?;
    m.add_class::<routing::RoutingEntry>()?;
    m.add_class::<repository::PySetHead>()?;
    m.add_class::<repository::PySignedRefs>()?;
    m.add_class::<issues::Issue>()?;
    m.add_class::<patches::Patch>()?;
    m.add_class::<issues::PyIssues>()?;
    m.add_class::<patches::PyPatches>()?;
    m.add_class::<issues::PyIssueMut>()?;
    m.add_class::<patches::PyPatchMut>()?;
    raw::register(py, m)?;
    #[cfg(feature = "test-helpers")]
    testhelpers::register(py, m)?;
    Ok(())
}
