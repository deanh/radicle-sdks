//! `RoutingStore` — named after upstream `radicle::node::routing::Store`
//! (Phase 3).
//!
//! Upstream facts (verified against `=0.25.1`, see docs/archive/FINDINGS-03.md):
//! * `Profile::routing(config)` is *literally* `Home::database(config)` — the
//!   routing table lives in the node database (`<home>/node/node.db`), and
//!   `node::Database` implements `routing::Store`. We therefore hold the
//!   named `node::Database` type, opened read-only via `Profile::database()`
//!   (multiple concurrent readers; upstream busy timeout). Read-only open
//!   does **not** create the file: a missing `node.db` raises
//!   `DatabaseError` (upstream reader runs no migrations).
//! * `node::Timestamp` is a newtype over **unix epoch milliseconds**
//!   (`Deref<Target = u64>`); the binding surfaces the raw millis int.
//! * The trait's `entries()` yields only `(RepoId, NodeId)` pairs; the
//!   per-entry timestamp requires `entry(rid, node)`. `entries()` here is an
//!   eager list that performs that lookup per pair (N+1 queries on a local
//!   sqlite file — documented in docs/API.md).
//! * Inventory mutations (`add_inventory` / `remove_inventory`) need the
//!   mutable database; they are deferred to Phase 5 (writes).

mod values;
pub(crate) use values::RoutingEntry;

use std::str::FromStr;

use pyo3::prelude::*;
use pyo3::types::PyType;
use radicle::identity::RepoId;
use radicle::node::routing::Store as _;
use radicle::node::Database;
use radicle::node::NodeId;

use crate::error;
use crate::profile::PyProfile;

fn db_err(err: impl std::fmt::Display) -> PyErr {
    error::database_err(err)
}

fn parse_node_id(value: &str) -> PyResult<NodeId> {
    NodeId::from_str(value).map_err(|err| error::value_err(format!("invalid NodeId: {err}")))
}

fn parse_rid(value: &str) -> PyResult<RepoId> {
    RepoId::from_str(value).map_err(|err| error::value_err(format!("invalid RepoId: {err}")))
}

/// Read-only view of the node's routing table (upstream `node::routing::Store`
/// over the node database).
#[pyclass(name = "RoutingStore", module = "radiclepy")]
pub struct PyRoutingStore {
    db: Database,
}

impl PyRoutingStore {
    /// Shared implementation behind `RoutingStore.from_profile` and
    /// `Profile.routing_store`.
    pub(crate) fn from_profile_ref(profile: &radicle::Profile) -> PyResult<Self> {
        let db = profile.database().map_err(db_err)?;
        Ok(Self { db })
    }
}

#[pymethods]
impl PyRoutingStore {
    /// Primary constructor: read-only node database of `profile`'s home
    /// (SQLite config comes from the profile, matching `Profile::routing()`).
    /// Raises `DatabaseError` if the node database does not exist —
    /// upstream `Database::reader` opens with read-only flags and performs
    /// no schema creation (a fresh `Profile.init`/`load` home does have a
    /// migrated `node.db`, hence freshly-initialized profiles open fine with
    /// an empty routing table; FINDINGS-03).
    #[classmethod]
    fn from_profile(_cls: &Bound<'_, PyType>, profile: PyRef<'_, PyProfile>) -> PyResult<Self> {
        Self::from_profile_ref(&profile.inner)
    }

    /// Nodes seeding `rid` (sorted).
    fn get(&self, rid: &str) -> PyResult<Vec<String>> {
        let rid = parse_rid(rid)?;
        let mut ids: Vec<String> = self
            .db
            .get(&rid)
            .map_err(db_err)?
            .into_iter()
            .map(|n| n.to_string())
            .collect();
        ids.sort();
        Ok(ids)
    }

    /// Inventory (seeded repos) advertised by `node_id` (sorted).
    fn get_inventory(&self, node_id: &str) -> PyResult<Vec<String>> {
        let nid = parse_node_id(node_id)?;
        let mut ids: Vec<String> = self
            .db
            .get_inventory(&nid)
            .map_err(db_err)?
            .into_iter()
            .map(|r| r.to_string())
            .collect();
        ids.sort();
        Ok(ids)
    }

    /// Last-seen timestamp (**unix epoch milliseconds**; upstream `Timestamp`
    /// derefs to a millis `u64`) for one `(rid, node_id)` entry, or `None`.
    fn entry(&self, rid: &str, node_id: &str) -> PyResult<Option<u64>> {
        let rid = parse_rid(rid)?;
        let nid = parse_node_id(node_id)?;
        Ok(self.db.entry(&rid, &nid).map_err(db_err)?.map(|t| *t))
    }

    /// All routing entries (eager list). Each entry's timestamp is looked up
    /// per pair (N+1 queries; routing tables are small).
    fn entries(&self) -> PyResult<Vec<RoutingEntry>> {
        let raws = self.db.entries().map_err(db_err)?;
        let mut out = Vec::new();
        for (rid, nid) in raws {
            let timestamp = self.db.entry(&rid, &nid).map_err(db_err)?.map(|t| *t);
            out.push(RoutingEntry {
                rid: rid.to_string(),
                node_id: nid.to_string(),
                timestamp,
            });
        }
        out.sort();
        Ok(out)
    }

    /// Number of nodes seeding `rid`.
    fn count(&self, rid: &str) -> PyResult<usize> {
        let rid = parse_rid(rid)?;
        Ok(self.db.get(&rid).map_err(db_err)?.len())
    }

    /// Total number of routing entries.
    fn total(&self) -> PyResult<usize> {
        self.db.len().map_err(db_err)
    }

    /// Whether the routing table is empty.
    fn is_empty(&self) -> PyResult<bool> {
        self.db.is_empty().map_err(db_err)
    }

    fn __repr__(&self) -> String {
        "RoutingStore(node.db)".to_owned()
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRoutingStore>()?;
    Ok(())
}
