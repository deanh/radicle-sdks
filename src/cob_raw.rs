//! Raw COB inspection (Phase 4): `Raw` — typename/entity/ref enumeration and
//! causal history walk, no domain type required.
//!
//! Critical rules (FINDINGS-0A / PLAN §5.5):
//! * Raw action payloads come from `entry.contents().iter()` — one raw JSON
//!   blob per action — **never** `Op<Vec<u8>>::try_from` (fails at runtime).
//! * History traversal follows `entry.parents` **only** (causal). Git
//!   `parent_ids()` conflates causal parents, `related`, and the identity
//!   resource link — walking them pulls the identity COB's entries into an
//!   unrelated entity's history. `related` and `resource()` (identity) are
//!   surfaced as *links*, never traversed.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use pyo3::prelude::*;
use pyo3::types::PyType;
use radicle::cob::change::store::Storage as _; // load
use radicle::cob::object::Storage as _; // types / objects
use radicle::cob::{self, TypeName};
use radicle::git::{self, Oid};

use crate::error;
use crate::storage::PyRepository;

fn cob_err(err: impl std::fmt::Display) -> PyErr {
    error::cob_err(err)
}

fn git_err(err: impl std::fmt::Display) -> PyErr {
    error::git_err(err)
}

fn parse_typename(value: &str) -> PyResult<TypeName> {
    TypeName::from_str(value).map_err(|err| {
        error::value_err(format!("invalid COB typename {value:?}: {err}"))
    })
}

fn parse_cob_object_id(value: &str) -> PyResult<cob::ObjectId> {
    cob::ObjectId::from_str(value)
        .map_err(|err| error::value_err(format!("invalid ObjectId: {err}")))
}

// ─── Value classes ───────────────────────────────────────────────────────────

/// A COB ref: a git ref pointing at a change commit (authenticity comes from
/// `refs/rad/sigrefs`, not from the COB ref itself).
#[pyclass(name = "CobRef", module = "radiclepy")]
#[derive(Clone)]
pub struct CobRef {
    pub(crate) name: String,
    pub(crate) target: String,
}

#[pymethods]
impl CobRef {
    /// Full git ref name (e.g.
    /// `refs/namespaces/<nid>/refs/cobs/<typename>/<entity>/<rest>`).
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }
    /// Change commit OID this ref points at.
    #[getter]
    fn target(&self) -> &str {
        &self.target
    }
    fn __repr__(&self) -> String {
        format!("CobRef({}, {})", self.name, self.target)
    }
}

/// A single raw COB operation (change entry). One op carries a **non-empty**
/// list of action payloads.
#[pyclass(name = "RawOp", module = "radiclepy")]
#[derive(Clone)]
pub struct RawOp {
    pub(crate) id: String,
    pub(crate) actor: String,
    pub(crate) timestamp: u64,
    pub(crate) actions: Vec<Vec<u8>>,
    pub(crate) parents: Vec<String>,
    pub(crate) related: Vec<String>,
    pub(crate) identity: Option<String>,
    pub(crate) type_name: String,
    pub(crate) version: usize,
}

#[pymethods]
impl RawOp {
    /// Change commit OID (hex).
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }
    /// Author's canonical NodeId (upstream stores a PublicKey).
    #[getter]
    fn actor(&self) -> &str {
        &self.actor
    }
    /// Timestamp, unix epoch **milliseconds**.
    #[getter]
    fn timestamp(&self) -> u64 {
        self.timestamp
    }
    /// Raw action payloads (JSON bytes), decoded independently per action.
    #[getter]
    fn actions(&self) -> Vec<Vec<u8>> {
        self.actions.clone()
    }
    /// Causal parents (the ONLY traversal edge).
    #[getter]
    fn parents(&self) -> Vec<String> {
        self.parents.clone()
    }
    /// Related object links (NOT traversed as history).
    #[getter]
    fn related(&self) -> Vec<String> {
        self.related.clone()
    }
    /// Identity resource link (NOT traversed as history).
    #[getter]
    fn identity(&self) -> Option<&str> {
        self.identity.as_deref()
    }
    /// COB typename from the entry's manifest.
    #[getter]
    fn type_name(&self) -> &str {
        &self.type_name
    }
    /// COB version from the entry's manifest.
    #[getter]
    fn version(&self) -> usize {
        self.version
    }

    /// Each action parsed as JSON (via the `json` stdlib). Raises `CobError`
    /// if any action is not valid JSON. Returns raw `bytes` to Python and
    /// defers parsing to the caller's `json.loads` — the binding keeps no
    /// private JSON↔Python type mapping (PLAN §5.5).
    fn action_json_types(&self) -> PyResult<Vec<String>> {
        self.actions
            .iter()
            .enumerate()
            .map(|(i, blob)| {
                let value: serde_json::Value = serde_json::from_slice(blob).map_err(|err| {
                    error::cob_err(format!("action #{i} of entry {} is not valid JSON: {err}", self.id))
                })?;
                Ok(json_type_name(&value).to_owned())
            })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "RawOp({}, actor={}{}…)",
            self.id,
            self.actor,
            if self.actions.len() == 1 { "" } else { "s" }
        )
    }
}

fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "str",
        serde_json::Value::Array(_) => "list",
        serde_json::Value::Object(_) => "dict",
    }
}

// ─── Raw store ───────────────────────────────────────────────────────────────

/// Raw COB inspection for one repository — runtime-generic, no domain type.
#[pyclass(name = "Raw", module = "radiclepy")]
pub struct PyRaw {
    repo: PyRepository,
}

impl PyRaw {
    /// Load an entry and convert it to a `RawOp`.
    fn load_op(
        repo: &radicle::storage::git::Repository,
        oid: Oid,
    ) -> PyResult<RawOp> {
        let entry = repo
            .load(oid)
            .map_err(|err| cob_err(format!("load entry {oid}: {err}")))?;
        Ok(RawOp {
            id: oid.to_string(),
            actor: entry.author().to_string(),
            timestamp: entry.timestamp,
            actions: entry
                .contents()
                .iter()
                .map(|b| b.to_vec())
                .collect(),
            parents: entry.parents.iter().map(|p| p.to_string()).collect(),
            related: entry.related.iter().map(|r| r.to_string()).collect(),
            identity: entry.resource().map(|r| r.to_string()),
            type_name: entry.manifest.type_name.to_string(),
            version: usize::from(entry.manifest.version),
        })
    }
}

#[pymethods]
impl PyRaw {
    /// Open raw COB inspection for `repository`.
    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, repository: PyRepository) -> Self {
        Self { repo: repository }
    }

    #[getter]
    fn rid(&self) -> String {
        self.repo.rid_str()
    }

    /// All COB typenames present in this repository (from COB git refs, sorted).
    fn typenames(&self) -> PyResult<Vec<String>> {
        let repo = self.repo.open()?;
        let mut names = BTreeSet::new();
        for reference in repo.backend.references().map_err(git_err)? {
            let reference = reference.map_err(git_err)?;
            let name = reference.name().map_err(git_err)?;
            // let-chains are MSRV-forbidden (edition 2021 on 1.88), so nested if-let.
            #[allow(clippy::collapsible_if)]
            if let Ok(refstr) = git::fmt::RefStr::try_from_str(name) {
                if let Some((typename, _oid)) = cob::object::parse_refstr(&refstr) {
                    names.insert(typename.to_string());
                }
            }
        }
        Ok(names.into_iter().collect())
    }

    /// Entity ids (ObjectId hex) that have COB refs under `typename` (sorted).
    fn entities(&self, typename: &str) -> PyResult<Vec<String>> {
        let typename = parse_typename(typename)?;
        let repo = self.repo.open()?;
        let entities = repo.types(&typename).map_err(cob_err)?;
        Ok(entities.keys().map(|id| id.to_string()).collect())
    }

    /// COB refs for one `(typename, entity_id)`.
    fn refs(&self, typename: &str, entity_id: &str) -> PyResult<Vec<CobRef>> {
        let typename = parse_typename(typename)?;
        let entity = parse_cob_object_id(entity_id)?;
        let repo = self.repo.open()?;
        let objects = repo.objects(&typename, &entity).map_err(cob_err)?;
        let mut out = Vec::new();
        for reference in objects.iter() {
            out.push(CobRef {
                name: reference.name.to_string(),
                target: reference.target.id.to_string(),
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Causal history for `(typename, entity_id)`: walks `entry.parents`
    /// only, deduplicated **globally across all COB refs of the entity**, and
    /// returned in causal-topological order (each op after its parents).
    ///
    /// `related` and the identity `resource` link are surfaced on each `RawOp`
    /// but never traversed (FINDINGS-0A / PLAN §5.5).
    fn history(&self, typename: &str, entity_id: &str) -> PyResult<Vec<RawOp>> {
        let typename = parse_typename(typename)?;
        let entity = parse_cob_object_id(entity_id)?;
        let repo = self.repo.open()?;
        let objects = repo.objects(&typename, &entity).map_err(cob_err)?;

        // Collect all tips (change commit OIDs) that COB refs point at.
        let mut tips: Vec<Oid> = objects.iter().map(|r| r.target.id).collect();
        tips.sort();
        tips.dedup();

        // Global DFS over causal parents, memoized by entry id. We load each
        // entry once; `order` accumulates ids in discovery order and is fixed
        // into topological order below.
        let mut ops: BTreeMap<String, RawOp> = BTreeMap::new();
        let mut stack: Vec<Oid> = tips.clone();
        while let Some(oid) = stack.pop() {
            let key = oid.to_string();
            if ops.contains_key(&key) {
                continue;
            }
            let op = PyRaw::load_op(&repo, oid)?;
            for parent in op.parents.clone() {
                #[allow(clippy::collapsible_if)]
                if let Ok(poid) = Oid::from_str(&parent) {
                    if !ops.contains_key(&poid.to_string()) {
                        stack.push(poid);
                    }
                }
            }
            ops.insert(key, op);
        }

        Ok(topological_sort(ops))
    }

    fn __repr__(&self) -> String {
        format!("Raw({})", self.repo.rid_str())
    }
}

/// Sort ops so every op appears after all of its (loaded) parents.
/// Kahn's algorithm; deterministic (lexicographic tie-breaks). Returns a Vec
/// in emission order — insertion order IS the contract.
fn topological_sort(ops: BTreeMap<String, RawOp>) -> Vec<RawOp> {
    // pending: op id -> set of not-yet-emitted parents (ids).
    let mut pending: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (id, op) in &ops {
        let parents: BTreeSet<&str> = op
            .parents
            .iter()
            .map(|p| p.as_str())
            .filter(|p| ops.contains_key(*p) && *p != id.as_str())
            .collect();
        pending.insert(id.as_str(), parents);
    }

    let mut ordered: Vec<RawOp> = Vec::with_capacity(ops.len());
    let mut emitted = std::collections::HashSet::new();
    while ordered.len() < ops.len() {
        // Emit the lexicographically-smallest not-yet-emitted id whose
        // remaining (not-yet-emitted) parents are all done.
        let next = pending.iter().find_map(|(id, parents)| {
            if emitted.contains(*id) {
                return None;
            }
            let outstanding: BTreeSet<&str> =
                parents.iter().copied().filter(|p| !emitted.contains(p)).collect();
            if outstanding.is_empty() {
                Some(*id)
            } else {
                None
            }
        });
        let Some(id) = next else { break };
        ordered.push(ops.get(id).cloned().unwrap());
        emitted.insert(id);
    }
    // Append any leftover (cyclic) ops in id order — never drop data.
    for (id, op) in &ops {
        if !emitted.contains(id.as_str()) {
            ordered.push(op.clone());
        }
    }
    ordered
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CobRef>()?;
    m.add_class::<RawOp>()?;
    m.add_class::<PyRaw>()?;
    Ok(())
}
