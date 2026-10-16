//! Immutable policy snapshots.
use crate::values::snapshot_hash;
use pyo3::prelude::*;

/// A node follow policy (upstream `policy::FollowPolicy`).
#[pyclass(name = "FollowPolicyInfo", module = "radiclepy", frozen, eq)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FollowPolicyInfo {
    pub(crate) node_id: String,
    pub(crate) alias: Option<String>,
    pub(crate) policy: String,
}

#[pymethods]
impl FollowPolicyInfo {
    fn __hash__(&self) -> u64 {
        snapshot_hash(self)
    }

    /// The node this policy applies to.
    #[getter]
    fn node_id(&self) -> &str {
        &self.node_id
    }
    /// Optional alias recorded when following.
    #[getter]
    fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
    /// `"allow" | "block"`.
    #[getter]
    fn policy(&self) -> &str {
        &self.policy
    }

    fn __repr__(&self) -> String {
        format!(
            "FollowPolicyInfo({}, {}, {})",
            self.node_id,
            self.alias.as_deref().unwrap_or("-"),
            self.policy
        )
    }
}

/// A repository seed policy (upstream `policy::SeedPolicy`).
///
/// `scope` (`"all" | "followed"`) is present only when policy is `"allow"` —
/// blocked repos have no scope.
#[pyclass(name = "SeedPolicyInfo", module = "radiclepy", frozen, eq)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SeedPolicyInfo {
    pub(crate) rid: String,
    pub(crate) policy: String,
    pub(crate) scope: Option<String>,
}

#[pymethods]
impl SeedPolicyInfo {
    fn __hash__(&self) -> u64 {
        snapshot_hash(self)
    }

    #[getter]
    fn rid(&self) -> &str {
        &self.rid
    }
    /// `"allow" | "block"`.
    #[getter]
    fn policy(&self) -> &str {
        &self.policy
    }
    /// `"all" | "followed"` when allowed; `None` when blocked.
    #[getter]
    fn scope(&self) -> Option<&str> {
        self.scope.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "SeedPolicyInfo({}, {}{})",
            self.rid,
            self.policy,
            self.scope
                .as_deref()
                .map(|s| format!(", scope={s}"))
                .unwrap_or_default()
        )
    }
}
