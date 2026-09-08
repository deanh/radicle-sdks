//! Typed COB readers (Phase 4): `Issues` and `Patches` stores.
//!
//! These wrap upstream `radicle::cob::issue::Issues` / `patch::Patches` in
//! **read-only** mode (`Store::open(repo, ReadOnly)`). COB *mutations* (which
//! require a signer) are Phase 5. Materialization is eager — `.all()` returns
//! a `list`, never a lazy iterator holding a git handle open (PLAN §5.6).

use pyo3::prelude::*;
use pyo3::types::PyType;
use radicle::cob::issue::{self, Issue as RawIssue, Issues as RawIssues};
use radicle::cob::patch::{self, Patch as RawPatch, Patches as RawPatches};
use radicle::cob::store::access::ReadOnly;
use radicle::cob::ObjectId;

use std::str::FromStr;

use crate::error;
use crate::storage::PyRepository;

/// ValueError for a malformed ObjectId, CobError for a store failure.
fn parse_object_id(value: &str) -> PyResult<ObjectId> {
    ObjectId::from_str(value).map_err(|err| error::value_err(format!("invalid ObjectId: {err}")))
}

fn cob_err(err: impl std::fmt::Display) -> PyErr {
    error::cob_err(err)
}

// ─── Issue value class ───────────────────────────────────────────────────────

/// A materialized issue (read-only snapshot of upstream `cob::issue::Issue`).
///
/// Field mapping (PLAN §5.4): title/description/labels are `String` upstream
/// (already valid UTF-8), so exposed as `str`. `author`/`assignees` are DIDs.
/// Comment bodies are `str` (upstream `String`); comment ids are COB
/// `ObjectId` hex strings. `state` is `"open" | "closed"`; a closed issue's
/// close reason (if any) is `close_reason`.
#[pyclass(name = "Issue", module = "radiclepy")]
#[derive(Clone)]
pub struct Issue {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) author: String,
    pub(crate) state: String,
    pub(crate) close_reason: Option<String>,
    pub(crate) assignees: Vec<String>,
    pub(crate) labels: Vec<String>,
    pub(crate) comment_count: usize,
    pub(crate) comment_ids: Vec<String>,
}

#[pymethods]
impl Issue {
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }
    #[getter]
    fn title(&self) -> &str {
        &self.title
    }
    /// Root comment body.
    #[getter]
    fn description(&self) -> &str {
        &self.description
    }
    /// Issue author (root comment author), as a DID.
    #[getter]
    fn author(&self) -> &str {
        &self.author
    }
    /// `"open" | "closed"`.
    #[getter]
    fn state(&self) -> &str {
        &self.state
    }
    /// Close reason (e.g. `"solved"`, `"unspecified"`) when closed, else None.
    #[getter]
    fn close_reason(&self) -> Option<&str> {
        self.close_reason.as_deref()
    }
    /// Assignee DIDs (sorted).
    #[getter]
    fn assignees(&self) -> Vec<String> {
        self.assignees.clone()
    }
    /// Labels (sorted).
    #[getter]
    fn labels(&self) -> Vec<String> {
        self.labels.clone()
    }
    /// Number of comments (including the root).
    #[getter]
    fn comment_count(&self) -> usize {
        self.comment_count
    }
    /// Comment ids in thread order (root first).
    #[getter]
    fn comment_ids(&self) -> Vec<String> {
        self.comment_ids.clone()
    }

    fn __repr__(&self) -> String {
        format!("Issue({}, {:?}, {})", self.id, self.title, self.state)
    }
}

fn close_reason_str(reason: &issue::CloseReason) -> String {
    // Upstream Display gives "solved" / "unspecified".
    reason.to_string()
}

fn materialize_issue(id: &ObjectId, issue: &RawIssue) -> Issue {
    let (state, close_reason) = match issue.state() {
        issue::State::Open => ("open".to_string(), None),
        issue::State::Closed { reason } => {
            ("closed".to_string(), Some(close_reason_str(reason)))
        }
    };
    let mut assignees: Vec<String> = issue.assignees().map(|d| d.to_string()).collect();
    assignees.sort();
    let mut labels: Vec<String> = issue.labels().map(|l| l.name().to_owned()).collect();
    labels.sort();
    let comment_ids: Vec<String> = issue.comments().map(|(cid, _)| cid.to_string()).collect();
    let comment_count = comment_ids.len();

    Issue {
        id: id.to_string(),
        title: issue.title().to_owned(),
        description: issue.description().to_owned(),
        author: issue.author().id().to_string(),
        state,
        close_reason,
        assignees,
        labels,
        comment_count,
        comment_ids,
    }
}

// ─── Patch value class ───────────────────────────────────────────────────────

/// A materialized patch (read-only snapshot of upstream `cob::patch::Patch`).
///
/// `state`: `"draft" | "open" | "archived" | "merged"`. `base`/`head` are the
/// latest revision's base and tip commit OIDs; `target` is the merge target as
/// a string (`"delegates"` or `"branch:<name>"`).
#[pyclass(name = "Patch", module = "radiclepy")]
#[derive(Clone)]
pub struct Patch {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) author: String,
    pub(crate) state: String,
    pub(crate) target: String,
    pub(crate) base: String,
    pub(crate) head: String,
    pub(crate) version: usize,
    pub(crate) labels: Vec<String>,
    pub(crate) revision_count: usize,
}

#[pymethods]
impl Patch {
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }
    #[getter]
    fn title(&self) -> &str {
        &self.title
    }
    /// Latest revision description.
    #[getter]
    fn description(&self) -> &str {
        &self.description
    }
    /// Patch author (DID).
    #[getter]
    fn author(&self) -> &str {
        &self.author
    }
    /// `"draft" | "open" | "archived" | "merged"`.
    #[getter]
    fn state(&self) -> &str {
        &self.state
    }
    /// Merge target: `"delegates"` or `"branch:<name>"`.
    #[getter]
    fn target(&self) -> &str {
        &self.target
    }
    /// Base commit OID of the latest revision.
    #[getter]
    fn base(&self) -> &str {
        &self.base
    }
    /// Tip commit OID of the latest revision.
    #[getter]
    fn head(&self) -> &str {
        &self.head
    }
    /// Revision index of the latest revision (0-based).
    #[getter]
    fn version(&self) -> usize {
        self.version
    }
    /// Labels (sorted).
    #[getter]
    fn labels(&self) -> Vec<String> {
        self.labels.clone()
    }
    /// Number of (non-redacted) revisions.
    #[getter]
    fn revision_count(&self) -> usize {
        self.revision_count
    }

    fn __repr__(&self) -> String {
        format!("Patch({}, {:?}, {})", self.id, self.title, self.state)
    }
}

fn materialize_patch(id: &ObjectId, patch: &RawPatch) -> Patch {
    let state = if patch.is_draft() {
        "draft"
    } else if patch.is_open() {
        "open"
    } else if patch.is_archived() {
        "archived"
    } else if patch.is_merged() {
        "merged"
    } else {
        "unknown"
    };
    let target = match patch.target() {
        patch::MergeTarget::Delegates => "delegates".to_string(),
        patch::MergeTarget::Branch(branch) => format!("branch:{}", branch.as_str()),
    };
    let mut labels: Vec<String> = patch.labels().map(|l| l.name().to_owned()).collect();
    labels.sort();
    let (_rid, revision) = patch.latest();

    Patch {
        id: id.to_string(),
        title: patch.title().to_owned(),
        description: revision.description().to_owned(),
        author: patch.author().id().to_string(),
        state: state.to_string(),
        target,
        base: revision.base().to_string(),
        head: revision.head().to_string(),
        version: patch.version(),
        labels,
        revision_count: patch.revisions().count(),
    }
}

// ─── Issues store ────────────────────────────────────────────────────────────

/// Read-only typed issue store for one repository (upstream `cob::issue::Issues`).
#[pyclass(name = "Issues", module = "radiclepy")]
pub struct PyIssues {
    repo: PyRepository,
}

#[pymethods]
impl PyIssues {
    /// Open the issue store of `repository` (read-only).
    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, repository: PyRepository) -> Self {
        Self { repo: repository }
    }

    #[getter]
    fn rid(&self) -> String {
        self.repo.rid_str()
    }

    /// All issues (eager list, sorted by id).
    fn all(&self) -> PyResult<Vec<Issue>> {
        let repo = self.repo.open()?;
        let store = RawIssues::open(&repo, ReadOnly).map_err(cob_err)?;
        let mut out = Vec::new();
        for item in store.all().map_err(cob_err)? {
            let (id, issue) = item.map_err(cob_err)?;
            out.push(materialize_issue(&id, &issue));
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// One issue by id, or `None`.
    fn get(&self, id: &str) -> PyResult<Option<Issue>> {
        let id = parse_object_id(id)?;
        let repo = self.repo.open()?;
        let store = RawIssues::open(&repo, ReadOnly).map_err(cob_err)?;
        let issue = store.get(&id).map_err(cob_err)?;
        Ok(issue.map(|i| materialize_issue(&id, &i)))
    }

    /// Number of issues.
    fn count(&self) -> PyResult<usize> {
        let repo = self.repo.open()?;
        let store = RawIssues::open(&repo, ReadOnly).map_err(cob_err)?;
        store.count().map_err(cob_err)
    }

    /// Whether there are no issues.
    fn is_empty(&self) -> PyResult<bool> {
        let repo = self.repo.open()?;
        let store = RawIssues::open(&repo, ReadOnly).map_err(cob_err)?;
        store.is_empty().map_err(cob_err)
    }

    fn __repr__(&self) -> String {
        format!("Issues({})", self.repo.rid_str())
    }
}

// ─── Patches store ───────────────────────────────────────────────────────────

/// Read-only typed patch store for one repository (upstream `cob::patch::Patches`).
#[pyclass(name = "Patches", module = "radiclepy")]
pub struct PyPatches {
    repo: PyRepository,
}

#[pymethods]
impl PyPatches {
    /// Open the patch store of `repository` (read-only).
    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, repository: PyRepository) -> Self {
        Self { repo: repository }
    }

    #[getter]
    fn rid(&self) -> String {
        self.repo.rid_str()
    }

    /// All patches (eager list, sorted by id).
    fn all(&self) -> PyResult<Vec<Patch>> {
        let repo = self.repo.open()?;
        let store = RawPatches::open(&repo, ReadOnly).map_err(cob_err)?;
        let mut out = Vec::new();
        for item in store.all().map_err(cob_err)? {
            let (id, patch) = item.map_err(cob_err)?;
            out.push(materialize_patch(&id, &patch));
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// One patch by id, or `None`.
    fn get(&self, id: &str) -> PyResult<Option<Patch>> {
        let id = parse_object_id(id)?;
        let repo = self.repo.open()?;
        let store = RawPatches::open(&repo, ReadOnly).map_err(cob_err)?;
        let patch = store.get(&id).map_err(cob_err)?;
        Ok(patch.map(|p| materialize_patch(&id, &p)))
    }

    /// Number of patches.
    fn count(&self) -> PyResult<usize> {
        let repo = self.repo.open()?;
        let store = RawPatches::open(&repo, ReadOnly).map_err(cob_err)?;
        store.count().map_err(cob_err)
    }

    /// Whether there are no patches.
    fn is_empty(&self) -> PyResult<bool> {
        let repo = self.repo.open()?;
        let store = RawPatches::open(&repo, ReadOnly).map_err(cob_err)?;
        store.is_empty().map_err(cob_err)
    }

    fn __repr__(&self) -> String {
        format!("Patches({})", self.repo.rid_str())
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Issue>()?;
    m.add_class::<Patch>()?;
    m.add_class::<PyIssues>()?;
    m.add_class::<PyPatches>()?;
    Ok(())
}
