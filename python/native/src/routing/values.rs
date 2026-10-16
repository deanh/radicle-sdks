//! Routing snapshots; unix epoch milliseconds.
use crate::hashing::snapshot_hash;
use pyo3::prelude::*;

/// One routing-table entry: which node advertises which repo, last-seen.
///
/// `timestamp` is **unix epoch milliseconds** (upstream `node::Timestamp`
/// is a millis `u64` newtype).
#[pyclass(name = "RoutingEntry", module = "radiclepy", frozen, eq)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoutingEntry {
    pub(crate) rid: String,
    pub(crate) node_id: String,
    pub(crate) timestamp: Option<u64>,
}

#[pymethods]
impl RoutingEntry {
    fn __hash__(&self) -> u64 {
        snapshot_hash(self)
    }

    #[getter]
    fn rid(&self) -> &str {
        &self.rid
    }
    #[getter]
    fn node_id(&self) -> &str {
        &self.node_id
    }
    /// Last-seen time (unix epoch **milliseconds**).
    #[getter]
    fn timestamp(&self) -> Option<u64> {
        self.timestamp
    }

    fn __repr__(&self) -> String {
        format!(
            "RoutingEntry({}, {}, {:?})",
            self.rid,
            self.node_id,
            self.timestamp.map(|t| t.to_string())
        )
    }
}
