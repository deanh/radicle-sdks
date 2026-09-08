//! `PolicyStore` — named after upstream `radicle::node::policy::Store` (Phase 3).
//!
//! Upstream facts (verified against `=0.25.1`, see FINDINGS-03.md):
//! * The policy store is its **own sqlite file** (`<home>/node/policies.db`),
//!   separate from the node database.
//! * `Store<Read>` opens read-only (multiple concurrent readers, upstream
//!   busy timeout); `Store<Write>` opens read-write.
//! * Upstream `Profile::policies()` returns a *config-combining* reader
//!   (`policy::config::Config<Read>`); this binding deliberately mirrors the
//!   **raw `Store` trait surface** instead — every method maps 1:1, with no
//!   implicit file-policy fallbacks (documented in API.md).
//! * `nodes_by_alias` is a **fuzzy** (substring, case-insensitive) lookup
//!   upstream — preserved, documented.

use std::path::PathBuf;
use std::str::FromStr;

use pyo3::prelude::*;
use pyo3::types::PyType;
use radicle::identity::RepoId;
use radicle::node::policy::{self, Policy, Scope};
use radicle::node::{Alias, NodeId};
use radicle::node::{self as node_mod};

use crate::error;
use crate::profile::PyProfile;
use crate::values::{FollowPolicyInfo, SeedPolicyInfo};

type RawStore = policy::store::Store<policy::store::Write>;
type RawReader = policy::store::Store<policy::store::Read>;

pub(crate) fn policy_err(err: policy::store::Error) -> PyErr {
    error::policy_err(err)
}

fn parse_node_id(value: &str) -> PyResult<NodeId> {
    NodeId::from_str(value).map_err(|err| error::value_err(format!("invalid NodeId: {err}")))
}

fn parse_rid(value: &str) -> PyResult<RepoId> {
    RepoId::from_str(value).map_err(|err| error::value_err(format!("invalid RepoId: {err}")))
}

fn parse_alias(value: &str) -> PyResult<Alias> {
    Alias::from_str(value).map_err(|err| error::value_err(format!("invalid Alias: {err}")))
}

fn parse_policy(value: &str) -> PyResult<Policy> {
    match value {
        "allow" => Ok(Policy::Allow),
        "block" => Ok(Policy::Block),
        other => Err(error::value_err(format!(
            "invalid policy {other:?}: expected \"allow\" or \"block\""
        ))),
    }
}

fn parse_scope(value: &str) -> PyResult<Scope> {
    match value {
        "all" => Ok(Scope::All),
        "followed" => Ok(Scope::Followed),
        other => Err(error::value_err(format!(
            "invalid scope {other:?}: expected \"all\" or \"followed\""
        ))),
    }
}

fn policy_str(policy: Policy) -> &'static str {
    match policy {
        Policy::Allow => "allow",
        Policy::Block => "block",
    }
}

enum Backend {
    Rw(RawStore),
    Ro(RawReader),
}

impl Backend {
    fn write_store(&mut self) -> PyResult<&mut RawStore> {
        match self {
            Self::Rw(store) => Ok(store),
            Self::Ro(_) => Err(error::storage_err_msg(
                "policy store is read-only; open with PolicyStore.from_profile / PolicyStore.open",
            )),
        }
    }
}

/// Follow/seed policy store (upstream `node::policy::Store`).
///
/// Concurrency: sqlite serializes writers with upstream busy timeouts; a
/// read-only handle (`PolicyStore.reader`) allows many concurrent readers.
#[pyclass(name = "PolicyStore", module = "radiclepy", unsendable)]
pub struct PyPolicyStore {
    backend: Backend,
    path: PathBuf,
}

impl PyPolicyStore {
    fn rw(&mut self) -> PyResult<&mut RawStore> {
        self.backend.write_store()
    }
}

impl PyPolicyStore {
    /// Shared implementation behind `PolicyStore.from_profile` and
    /// `Profile.policy_store`.
    pub(crate) fn from_profile_ref(profile: &radicle::Profile) -> PyResult<Self> {
        let path = profile.node().join(node_mod::POLICIES_DB_FILE);
        let store = RawStore::open(&path).map_err(policy_err)?;
        Ok(Self {
            backend: Backend::Rw(store),
            path,
        })
    }
}

#[pymethods]
impl PyPolicyStore {
    /// Primary constructor: read-write policy store of `profile`'s home
    /// (path and file name come from upstream layout).
    #[classmethod]
    fn from_profile(_cls: &Bound<'_, PyType>, profile: PyRef<'_, PyProfile>) -> PyResult<Self> {
        Self::from_profile_ref(&profile.inner)
    }

    /// Read-only handle on the same path (upstream `Store::reader`: no
    /// locking, many concurrent readers). Write methods raise `StorageError`.
    #[classmethod]
    fn reader(_cls: &Bound<'_, PyType>, profile: PyRef<'_, PyProfile>) -> PyResult<Self> {
        let path = profile.inner.node().join(node_mod::POLICIES_DB_FILE);
        let store = RawReader::reader(&path).map_err(policy_err)?;
        Ok(Self {
            backend: Backend::Ro(store),
            path,
        })
    }

    /// Low-level constructor: read-write store at an explicit path.
    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, path: &str) -> PyResult<Self> {
        let store = RawStore::open(path).map_err(policy_err)?;
        Ok(Self {
            backend: Backend::Rw(store),
            path: PathBuf::from(path),
        })
    }

    #[getter]
    fn path(&self) -> PyResult<&str> {
        self.path
            .to_str()
            .ok_or_else(|| error::value_err("policy store path is not valid UTF-8"))
    }

    // ── Writes (upstream Store<Write>; return true when the store changed) ──

    /// Follow `node_id` (optionally with an alias). `true` if newly followed.
    #[pyo3(signature = (node_id, *, alias = None))]
    fn follow(&mut self, node_id: &str, alias: Option<&str>) -> PyResult<bool> {
        let nid = parse_node_id(node_id)?;
        let alias = alias.map(parse_alias).transpose()?;
        let alias_ref = alias.as_ref();
        self.rw()?
            .follow(&nid, alias_ref)
            .map_err(policy_err)
    }

    /// Stop following `node_id`. `true` if it was followed.
    fn unfollow(&mut self, node_id: &str) -> PyResult<bool> {
        let nid = parse_node_id(node_id)?;
        self.rw()?.unfollow(&nid).map_err(policy_err)
    }

    /// Seed `rid` with scope `"all"` (default) or `"followed"`.
    #[pyo3(signature = (rid, *, scope = "all"))]
    fn seed(&mut self, rid: &str, scope: &str) -> PyResult<bool> {
        let rid = parse_rid(rid)?;
        let scope = parse_scope(scope)?;
        self.rw()?.seed(&rid, scope).map_err(policy_err)
    }

    /// Stop seeding `rid`. `true` if it was seeded.
    fn unseed(&mut self, rid: &str) -> PyResult<bool> {
        let rid = parse_rid(rid)?;
        self.rw()?.unseed(&rid).map_err(policy_err)
    }

    /// Set the follow policy (`"allow"`/`"block"`) for `node_id`.
    fn set_follow_policy(&mut self, node_id: &str, policy: &str) -> PyResult<bool> {
        let nid = parse_node_id(node_id)?;
        let policy = parse_policy(policy)?;
        self.rw()?.set_follow_policy(&nid, policy).map_err(policy_err)
    }

    /// Set the seed policy (`"allow"`/`"block"`) for `rid`.
    fn set_seed_policy(&mut self, rid: &str, policy: &str) -> PyResult<bool> {
        let rid = parse_rid(rid)?;
        let policy = parse_policy(policy)?;
        self.rw()?.set_seed_policy(&rid, policy).map_err(policy_err)
    }

    /// Remove a *block* on `rid`. `true` if one existed.
    fn unblock_rid(&mut self, rid: &str) -> PyResult<bool> {
        let rid = parse_rid(rid)?;
        self.rw()?.unblock_rid(&rid).map_err(policy_err)
    }

    /// Remove a *block* on `node_id`. `true` if one existed.
    fn unblock_nid(&mut self, node_id: &str) -> PyResult<bool> {
        let nid = parse_node_id(node_id)?;
        self.rw()?.unblock_nid(&nid).map_err(policy_err)
    }

    // ── Reads ────────────────────────────────────────────────────────────────

    fn is_following(&self, node_id: &str) -> PyResult<bool> {
        let nid = parse_node_id(node_id)?;
        match &self.backend {
            Backend::Rw(s) => s.is_following(&nid),
            Backend::Ro(s) => s.is_following(&nid),
        }
        .map_err(policy_err)
    }

    fn is_seeding(&self, rid: &str) -> PyResult<bool> {
        let rid = parse_rid(rid)?;
        match &self.backend {
            Backend::Rw(s) => s.is_seeding(&rid),
            Backend::Ro(s) => s.is_seeding(&rid),
        }
        .map_err(policy_err)
    }

    /// Whether `node_id` is explicitly *blocked* (distinct from unfollowed).
    fn is_blocked(&self, node_id: &str) -> PyResult<bool> {
        let nid = parse_node_id(node_id)?;
        match &self.backend {
            Backend::Rw(s) => s.is_blocked(&nid),
            Backend::Ro(s) => s.is_blocked(&nid),
        }
        .map_err(policy_err)
    }

    /// Explicit follow policy for `node_id`, or `None`.
    fn follow_policy(&self, node_id: &str) -> PyResult<Option<FollowPolicyInfo>> {
        let nid = parse_node_id(node_id)?;
        let fetched = match &self.backend {
            Backend::Rw(s) => s.follow_policy(&nid),
            Backend::Ro(s) => s.follow_policy(&nid),
        }
        .map_err(policy_err)?;
        Ok(fetched.map(|p| FollowPolicyInfo {
            node_id: p.nid.to_string(),
            alias: p.alias.map(|a| a.to_string()),
            policy: policy_str(p.policy).to_owned(),
        }))
    }

    /// Explicit seed policy for `rid`, or `None`.
    fn seed_policy(&self, rid: &str) -> PyResult<Option<SeedPolicyInfo>> {
        let rid_t = parse_rid(rid)?;
        let fetched = match &self.backend {
            Backend::Rw(s) => s.seed_policy(&rid_t),
            Backend::Ro(s) => s.seed_policy(&rid_t),
        }
        .map_err(policy_err)?;
        Ok(fetched.map(|p| {
            let (policy, scope) = match p.policy {
                policy::SeedingPolicy::Allow { scope } => (
                    "allow",
                    Some(match scope {
                        Scope::All => "all",
                        Scope::Followed => "followed",
                    }),
                ),
                policy::SeedingPolicy::Block => ("block", None),
            };
            SeedPolicyInfo {
                rid: p.rid.to_string(),
                policy: policy.to_owned(),
                scope: scope.map(str::to_owned),
            }
        }))
    }

    /// All follow policies (eager; storage order).
    fn follow_policies(&self) -> PyResult<Vec<FollowPolicyInfo>> {
        let raws: Vec<_> = match &self.backend {
            Backend::Rw(s) => s.follow_policies(),
            Backend::Ro(s) => s.follow_policies(),
        }
        .map_err(policy_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(policy_err)?;
        Ok(raws
            .into_iter()
            .map(|p| FollowPolicyInfo {
                node_id: p.nid.to_string(),
                alias: p.alias.map(|a| a.to_string()),
                policy: policy_str(p.policy).to_owned(),
            })
            .collect())
    }

    /// All seed policies (eager; storage order).
    fn seed_policies(&self) -> PyResult<Vec<SeedPolicyInfo>> {
        let raws: Vec<_> = match &self.backend {
            Backend::Rw(s) => s.seed_policies(),
            Backend::Ro(s) => s.seed_policies(),
        }
        .map_err(policy_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(policy_err)?;
        Ok(raws
            .into_iter()
            .map(|p| {
                let (policy, scope) = match p.policy {
                    policy::SeedingPolicy::Allow { scope } => (
                        "allow",
                        Some(match scope {
                            Scope::All => "all",
                            Scope::Followed => "followed",
                        }),
                    ),
                    policy::SeedingPolicy::Block => ("block", None),
                };
                SeedPolicyInfo {
                    rid: p.rid.to_string(),
                    policy: policy.to_owned(),
                    scope: scope.map(str::to_owned),
                }
            })
            .collect())
    }

    /// Nodes whose alias **contains** `alias` (upstream fuzzy substring
    /// match, case-insensitive) — `(node_id, alias)` pairs.
    fn nodes_by_alias(&self, alias: &str) -> PyResult<Vec<(String, String)>> {
        let alias_t = parse_alias(alias)?;
        let raws: Vec<_> = match &self.backend {
            Backend::Rw(s) => s.nodes_by_alias(&alias_t),
            Backend::Ro(s) => s.nodes_by_alias(&alias_t),
        }
        .map_err(policy_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(policy_err)?;
        Ok(raws
            .into_iter()
            .map(|(nid, alias)| (nid.to_string(), alias.to_string()))
            .collect())
    }

    fn __repr__(&self) -> String {
        format!(
            "PolicyStore({}, {})",
            self.path.display(),
            if matches!(self.backend, Backend::Rw(_)) { "rw" } else { "ro" }
        )
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPolicyStore>()?;
    Ok(())
}
