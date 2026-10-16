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

use pyo3::types::{PyBytes, PyDict, PyList, PyTuple, PyType};
use radicle::cob::change::store::Storage as _; // load
use radicle::cob::object::Storage as _; // types / objects
use radicle::cob::{self, TypeName};
use radicle::git::{self, Oid};

use crate::error;
use crate::repository::PyRepository;

fn cob_err(err: impl std::fmt::Display) -> PyErr {
    error::cob_err(err)
}

fn git_err(err: impl std::fmt::Display) -> PyErr {
    error::git_err(err)
}

fn parse_typename(value: &str) -> PyResult<TypeName> {
    TypeName::from_str(value)
        .map_err(|err| error::value_err(format!("invalid COB typename {value:?}: {err}")))
}

fn parse_cob_object_id(value: &str) -> PyResult<cob::ObjectId> {
    cob::ObjectId::from_str(value)
        .map_err(|err| error::value_err(format!("invalid ObjectId: {err}")))
}

// ─── Value classes ───────────────────────────────────────────────────────────

/// A COB ref: a git ref pointing at a change commit (authenticity comes from
/// `refs/rad/sigrefs`, not from the COB ref itself).
#[pyclass(name = "CobRef", module = "radiclepy", frozen, eq)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CobRef {
    pub(crate) name: String,
    pub(crate) target: String,
}

#[pymethods]
impl CobRef {
    fn __hash__(&self) -> u64 {
        crate::hashing::snapshot_hash(self)
    }

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
/// tuple of action payloads.
#[pyclass(name = "RawOp", module = "radiclepy", frozen, eq)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RawOp {
    pub(crate) id: String,
    pub(crate) actor: String,
    pub(crate) timestamp: u64,
    pub(crate) actions: Vec<Vec<u8>>,
    pub(crate) parents: Vec<String>,
    pub(crate) related: Vec<String>,
    pub(crate) identity: Option<String>,
    pub(crate) manifest: CobManifest,
}

/// The manifest of a change entry: what kind of COB it is, and which COB
/// schema version produced it (upstream `change::Manifest`).
#[pyclass(name = "CobManifest", module = "radiclepy", frozen, eq)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CobManifest {
    pub(crate) type_name: String,
    pub(crate) version: usize,
}

#[pymethods]
impl CobManifest {
    fn __hash__(&self) -> u64 {
        crate::hashing::snapshot_hash(self)
    }

    /// COB typename, e.g. `xyz.radicle.issue`.
    #[getter]
    fn type_name(&self) -> &str {
        &self.type_name
    }
    /// COB schema version.
    #[getter]
    fn version(&self) -> usize {
        self.version
    }
    fn __repr__(&self) -> String {
        format!("CobManifest({}, v{})", self.type_name, self.version)
    }
}

#[pymethods]
impl RawOp {
    fn __hash__(&self) -> u64 {
        crate::hashing::snapshot_hash(self)
    }

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
    /// Timestamp in **unix epoch seconds** (upstream `cob::change::Timestamp`
    /// is seconds since epoch; the binding exposes the value unchanged).
    #[getter]
    fn timestamp(&self) -> u64 {
        self.timestamp
    }
    /// Raw action payloads (JSON bytes), one per action; decode independently.
    #[getter]
    fn actions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(
            py,
            self.actions.iter().map(|b| PyBytes::new(py, b).into_any()),
        )
    }
    /// Causal parents (the ONLY traversal edge).
    #[getter]
    fn parents<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.parents.iter())
    }
    /// Related object links (NOT traversed as history).
    #[getter]
    fn related<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.related.iter())
    }
    /// Identity resource link (NOT traversed as history).
    #[getter]
    fn identity(&self) -> Option<&str> {
        self.identity.as_deref()
    }
    /// The change entry's manifest (typename + COB version).
    #[getter]
    fn manifest(&self) -> CobManifest {
        self.manifest.clone()
    }

    /// Each action parsed from JSON into plain Python objects
    /// (`None|bool|int|float|str|list|dict`; dict keys are `str`). Raises
    /// `CobError` if any action is not valid JSON.
    fn actions_json<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let items = self
            .actions
            .iter()
            .enumerate()
            .map(|(i, blob)| {
                let value: serde_json::Value = serde_json::from_slice(blob).map_err(|err| {
                    error::cob_err(format!(
                        "action #{i} of entry {} is not valid JSON: {err}",
                        self.id
                    ))
                })?;
                json_to_py(py, value)
            })
            .collect::<PyResult<Vec<_>>>()?;
        PyTuple::new(py, items)
    }

    fn __repr__(&self) -> String {
        format!(
            "RawOp({}, actor={}, {} action(s))",
            self.id,
            self.actor,
            self.actions.len()
        )
    }
}

/// `serde_json::Value` → plain Python objects. Integers prefer i64, then
/// u64; everything else numeric is f64.
fn json_to_py<'py>(py: Python<'py>, value: serde_json::Value) -> PyResult<Bound<'py, PyAny>> {
    use serde_json::Value;
    Ok(match value {
        Value::Null => py.None().into_bound(py),
        Value::Bool(b) => b.into_pyobject(py)?.to_owned().into_any(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_pyobject(py)?.into_any()
            } else if let Some(u) = n.as_u64() {
                u.into_pyobject(py)?.into_any()
            } else if let Some(f) = n.as_f64() {
                f.into_pyobject(py)?.into_any()
            } else {
                return Err(error::cob_err("unsupported JSON number"));
            }
        }
        Value::String(s) => s.into_pyobject(py)?.into_any(),
        Value::Array(items) => {
            let pyitems = items
                .into_iter()
                .map(|v| json_to_py(py, v))
                .collect::<PyResult<Vec<_>>>()?;
            PyList::new(py, pyitems)?.into_any()
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map {
                dict.set_item(k, json_to_py(py, v)?)?;
            }
            dict.into_any()
        }
    })
}

/// Parse a git ref name into a COB `TypedId`, strictly: after stripping any
/// namespaces the ref must be `refs/cobs/<typename>/<id>`. Everything else is
/// `None`: non-COB refs (including tag/branch refs that merely share the
/// `refs/<category>/<typename>/<oid>` shape) and COB-shaped refs whose typename
/// or id does not parse. Peers control the ref names in their namespaces, so one
/// malformed ref must not make enumeration fail for the whole repository.
fn typed_id_of(name: &str) -> Option<cob::TypedId> {
    let refstr = git::fmt::RefStr::try_from_str(name).ok()?;
    match refstr.to_namespaced() {
        Some(ns) => cob::TypedId::from_namespaced(&ns).ok().flatten(),
        None => refstr
            .qualified()
            .and_then(|q| cob::TypedId::from_qualified(&q).ok().flatten()),
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
    fn load_op(repo: &radicle::storage::git::Repository, oid: Oid) -> PyResult<RawOp> {
        let entry = repo
            .load(oid)
            .map_err(|err| cob_err(format!("load entry {oid}: {err}")))?;
        Ok(RawOp {
            id: oid.to_string(),
            actor: entry.author().to_string(),
            timestamp: entry.timestamp,
            actions: entry.contents().iter().map(|b| b.to_vec()).collect(),
            parents: entry.parents.iter().map(|p| p.to_string()).collect(),
            related: entry.related.iter().map(|r| r.to_string()).collect(),
            identity: entry.resource().map(|r| r.to_string()),
            manifest: CobManifest {
                type_name: entry.manifest.type_name.to_string(),
                version: usize::from(entry.manifest.version),
            },
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
            if let Some(typed) = typed_id_of(name) {
                names.insert(typed.type_name.to_string());
            }
        }
        Ok(names.into_iter().collect())
    }

    /// Entity ids (ObjectId hex) that have COB refs under `typename` (sorted).
    ///
    /// Enumeration is binding-side and strict (`TypedId`), so COB-shaped
    /// non-COB refs (e.g. `refs/tags/<typename>/<oid>`) never appear —
    /// upstream `types()` inherits `parse_refstr`'s loose category matching.
    fn entities(&self, typename: &str) -> PyResult<Vec<String>> {
        let typename = parse_typename(typename)?;
        let repo = self.repo.open()?;
        let mut out = BTreeSet::new();
        for reference in repo.backend.references().map_err(git_err)? {
            let reference = reference.map_err(git_err)?;
            let name = reference.name().map_err(git_err)?;
            match typed_id_of(name) {
                Some(typed) if typed.type_name == typename => {
                    out.insert(typed.id.to_string());
                }
                _ => {}
            }
        }
        Ok(out.into_iter().collect())
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

        topological_sort(ops)
    }

    fn __repr__(&self) -> String {
        format!("Raw({})", self.repo.rid_str())
    }
}

/// Sort ops so every op appears after all of its (loaded) parents.
/// Kahn's algorithm; deterministic (lexicographic tie-breaks). Returns a Vec
/// in emission order — insertion order IS the contract. This simple variant
/// rescans pending nodes/parents per emission (quadratic for typical sparse
/// histories, worse for dense graphs); acceptable for typical COB sizes.
/// A ready-queue/indegree implementation is deferred until profiling warrants it.
///
/// Content-addressed causal history is acyclic by construction; if the walk
/// cannot emit every op, the data is corrupt (cycle) and we return
/// `CobError` rather than silently degrade the ordering contract.
fn topological_sort(ops: BTreeMap<String, RawOp>) -> PyResult<Vec<RawOp>> {
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
            let outstanding: BTreeSet<&str> = parents
                .iter()
                .copied()
                .filter(|p| !emitted.contains(p))
                .collect();
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
    if emitted.len() != ops.len() {
        return Err(error::cob_err(format!(
            "corrupt COB history: causal cycle among {} entries (emitted {}/{})",
            ops.len(),
            emitted.len(),
            ops.len()
        )));
    }
    Ok(ordered)
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CobRef>()?;
    m.add_class::<CobManifest>()?;
    m.add_class::<RawOp>()?;
    m.add_class::<PyRaw>()?;
    Ok(())
}
