//! Canonical project identity snapshot.
use crate::values::snapshot_hash;
use pyo3::prelude::*;

// ─── ProjectDoc ──────────────────────────────────────────────────────────────

/// The project identity document (canonical identity head), verified at load.
#[pyclass(name = "ProjectDoc", module = "radiclepy", frozen, eq)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ProjectDoc {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) default_branch: String,
    pub(crate) version: u32,
    pub(crate) delegates: Vec<String>,
    pub(crate) threshold: usize,
    pub(crate) visibility: String,
    pub(crate) commit: String,
    pub(crate) blob: String,
}

#[pymethods]
impl ProjectDoc {
    fn __hash__(&self) -> u64 {
        snapshot_hash(self)
    }

    #[getter]
    fn name(&self) -> &str {
        &self.name
    }
    #[getter]
    fn description(&self) -> &str {
        &self.description
    }
    /// Default branch as recorded in the project payload (short name).
    #[getter]
    fn default_branch(&self) -> &str {
        &self.default_branch
    }
    /// Identity document version number.
    #[getter]
    fn version(&self) -> u32 {
        self.version
    }
    #[getter]
    fn delegates(&self) -> Vec<String> {
        self.delegates.clone()
    }
    #[getter]
    fn threshold(&self) -> usize {
        self.threshold
    }
    /// `"public"` or `"private"`.
    #[getter]
    fn visibility(&self) -> &str {
        &self.visibility
    }
    /// Identity commit OID.
    #[getter]
    fn commit(&self) -> &str {
        &self.commit
    }
    /// Identity document blob OID.
    #[getter]
    fn blob(&self) -> &str {
        &self.blob
    }

    fn __repr__(&self) -> String {
        format!(
            "ProjectDoc({}, delegates={} commits to v{})",
            self.name,
            self.delegates.len(),
            self.version
        )
    }
}
