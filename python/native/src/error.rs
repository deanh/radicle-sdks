//! Python exception hierarchy and Rust-to-Python error mapping.
//!
//! Convention: our own validators (`parse_*`) raise `ValueError`;
//! operational failures use this hierarchy. Every mapper documents the upstream
//! error enum it covers. Every child is registered even where no code path
//! raises it yet, so the hierarchy is stable across releases.

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
create_exception!(radiclepy, ReadOnlyError, Error);

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
/// unavailability) — never `ValueError`, which is reserved for invalid input.
pub(crate) fn op_err(message: impl Into<String>) -> PyErr {
    Error::new_err(message.into())
}

/// `radicle::storage::Error` — storage-level operational failures.
pub(crate) fn storage_err(err: radicle::storage::Error) -> PyErr {
    StorageError::new_err(err.to_string())
}

/// `radicle::storage::RepositoryError` — repository open/operation failures.
pub(crate) fn repository_err(err: radicle::storage::RepositoryError) -> PyErr {
    RepositoryError::new_err(err.to_string())
}

/// COB store / raw-COB failures.
pub(crate) fn cob_err(err: impl std::fmt::Display) -> PyErr {
    CobError::new_err(err.to_string())
}

/// Low-level libgit2 failures crossing the boundary.
pub(crate) fn git_err(err: impl std::fmt::Display) -> PyErr {
    GitError::new_err(err.to_string())
}

/// Identity document load/verification failures.
pub(crate) fn identity_err(err: impl std::fmt::Display) -> PyErr {
    IdentityError::new_err(err.to_string())
}

/// `node::policy::store::Error` — policy store failures.
pub(crate) fn policy_err(err: impl std::fmt::Display) -> PyErr {
    PolicyError::new_err(err.to_string())
}

/// SQLite / node-database failures crossing the boundary.
pub(crate) fn database_err(err: impl std::fmt::Display) -> PyErr {
    DatabaseError::new_err(err.to_string())
}

/// Writes through any read-only handle (binding access-mode validation).
pub(crate) fn read_only_err(message: impl Into<String>) -> PyErr {
    ReadOnlyError::new_err(message.into())
}

pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("Error", py.get_type::<Error>())?;
    m.add("ReadOnlyError", py.get_type::<ReadOnlyError>())?;
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
