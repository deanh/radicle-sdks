//! Python exception hierarchy and Rust-to-Python error mapping.
//!
//! Convention (PLAN §5.8): our own validators (`parse_*`) raise `ValueError`;
//! operational failures use this hierarchy. Every mapper documents the upstream
//! error enum it covers. Children not yet raised in Phase 1 are registered so
//! the hierarchy is stable from the first release.

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;

create_exception!(radiclepy, Error, PyException);
create_exception!(radiclepy, StorageError, Error);
create_exception!(radiclepy, RepositoryError, Error);
create_exception!(radiclepy, GitError, Error);
create_exception!(radiclepy, DatabaseError, Error);
create_exception!(radiclepy, IdentityError, Error);
create_exception!(radiclepy, SignerError, Error);
create_exception!(radiclepy, PolicyError, Error);
create_exception!(radiclepy, CobError, Error);

/// Our own syntactic validators (`parse_*`, seed length, …).
pub(crate) fn value_err(message: impl Into<String>) -> PyErr {
    PyValueError::new_err(message.into())
}

/// `radicle::profile::Error` — profile load/init failures.
pub(crate) fn profile_err(err: radicle::profile::Error) -> PyErr {
    Error::new_err(err.to_string())
}

/// `radicle::profile::SignerError` — signer resolution failures.
pub(crate) fn signer_err(err: radicle::profile::SignerError) -> PyErr {
    SignerError::new_err(err.to_string())
}

/// `std::io::Error` at the Home/profile boundary.
pub(crate) fn io_err(err: std::io::Error) -> PyErr {
    Error::new_err(err.to_string())
}

/// Operational failures with no more specific class (e.g. OS RNG
/// unavailability) — NOT `ValueError` (PLAN §5.8).
pub(crate) fn op_err(message: impl Into<String>) -> PyErr {
    Error::new_err(message.into())
}

pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("Error", py.get_type::<Error>())?;
    m.add("StorageError", py.get_type::<StorageError>())?;
    m.add("RepositoryError", py.get_type::<RepositoryError>())?;
    m.add("GitError", py.get_type::<GitError>())?;
    m.add("DatabaseError", py.get_type::<DatabaseError>())?;
    m.add("IdentityError", py.get_type::<IdentityError>())?;
    m.add("SignerError", py.get_type::<SignerError>())?;
    m.add("PolicyError", py.get_type::<PolicyError>())?;
    m.add("CobError", py.get_type::<CobError>())?;
    Ok(())
}
