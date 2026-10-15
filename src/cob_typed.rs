//! Typed COB stores (Phase 4 readers, Phase 5 writes): `Issues`/`Patches`.
//!
//! Reads wrap upstream stores in read-only mode (`ReadOnly`); writes open
//! with `WriteAs(signer)` — the signer lives in the *store access mode*,
//! never in the mutating methods' arguments (mirrors upstream exactly: no
//! signer-taking write methods; PLAN §3). Materialization is eager —
//! `.all()` returns a `list`, never a lazy iterator holding a git handle
//! open (PLAN §5.6).

use pyo3::prelude::*;
use pyo3::types::PyType;
use radicle::cob::cache::NoCache;
use radicle::cob::issue::State as IssueState;
use radicle::cob::issue::{self, Issue as RawIssue, Issues as RawIssues};
use radicle::cob::patch::{self, Patch as RawPatch, Patches as RawPatches};
use radicle::cob::store::access::{ReadOnly, WriteAs};
use radicle::cob::thread::CommentId;
use radicle::cob::{Label, ObjectId, Title};
use radicle::identity::Did;

use std::str::FromStr;

use crate::cob_values::{self, Comment, Revision};
use crate::error;
use crate::profile::PySigner;
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
#[pyclass(name = "Issue", module = "radiclepy", frozen, eq, hash)]
#[derive(Clone, PartialEq, Eq, Hash)]
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
    pub(crate) comments: Vec<Comment>,
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

    #[getter]
    fn comments(&self) -> Vec<Comment> {
        self.comments.clone()
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
        issue::State::Closed { reason } => ("closed".to_string(), Some(close_reason_str(reason))),
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
        comments: issue
            .comments()
            .map(|(id, c)| cob_values::comment(id, c))
            .collect(),
    }
}

// ─── Patch value class ───────────────────────────────────────────────────────

/// A materialized patch (read-only snapshot of upstream `cob::patch::Patch`).
///
/// `state`: `"draft" | "open" | "archived" | "merged"`. `base`/`head` are the
/// latest patch-author revision's base and tip commit OIDs; `latest_revision`
/// identifies that revision. `target` is the merge target as
/// a string (`"delegates"` or `"branch:<name>"`).
#[pyclass(name = "Patch", module = "radiclepy", frozen, eq, hash)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Patch {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) author: String,
    pub(crate) state: String,
    pub(crate) target: String,
    pub(crate) base: String,
    pub(crate) head: String,
    pub(crate) latest_revision: String,
    pub(crate) version: usize,
    pub(crate) labels: Vec<String>,
    pub(crate) revision_count: usize,
    pub(crate) revisions: Vec<Revision>,
    pub(crate) assignees: Vec<String>,
    pub(crate) merged_revision: Option<String>,
    pub(crate) merged_commit: Option<String>,
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
    /// Latest patch-author revision description.
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
    /// Base commit OID of the latest patch-author revision.
    #[getter]
    fn base(&self) -> &str {
        &self.base
    }
    /// Tip commit OID of the latest patch-author revision.
    #[getter]
    fn head(&self) -> &str {
        &self.head
    }
    /// ID of the patch-author revision described by base/head/description.
    #[getter]
    fn latest_revision(&self) -> &str {
        &self.latest_revision
    }
    /// Upstream revision-slot count minus one, including other authors and
    /// redacted slots; not an index into the non-redacted revisions snapshot.
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

    #[getter]
    fn revisions(&self) -> Vec<Revision> {
        self.revisions.clone()
    }
    #[getter]
    fn assignees(&self) -> Vec<String> {
        self.assignees.clone()
    }
    #[getter]
    fn merged_revision(&self) -> Option<&str> {
        self.merged_revision.as_deref()
    }
    #[getter]
    fn merged_commit(&self) -> Option<&str> {
        self.merged_commit.as_deref()
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
        // `TargetBranch` is guaranteed `refs/heads/`-qualified (upstream
        // constructor). Output is canonical *bare*: exactly one
        // `refs/heads/` prefix is removed (`strip_prefix`, NOT
        // `trim_start_matches`, which would eat repeated prefixes).
        // Input accepts bare or fully-qualified names (upstream
        // constructor); qualified input canonicalizes to bare output.
        patch::MergeTarget::Branch(branch) => {
            let bare = branch
                .as_str()
                .strip_prefix("refs/heads/")
                .unwrap_or(branch.as_str());
            format!("branch:{bare}")
        }
    };
    let mut labels: Vec<String> = patch.labels().map(|l| l.name().to_owned()).collect();
    labels.sort();
    let (latest_revision, revision) = patch.latest();

    Patch {
        id: id.to_string(),
        title: patch.title().to_owned(),
        description: revision.description().to_owned(),
        author: patch.author().id().to_string(),
        state: state.to_string(),
        target,
        base: revision.base().to_string(),
        head: revision.head().to_string(),
        latest_revision: latest_revision.to_string(),
        version: patch.version(),
        labels,
        revision_count: patch.revisions().count(),
        revisions: patch
            .revisions()
            .map(|(_, r)| cob_values::revision(r))
            .collect(),
        assignees: patch.assignees().map(|d| d.to_string()).collect(),
        merged_revision: match patch.state() {
            patch::State::Merged { revision, .. } => Some(revision.to_string()),
            _ => None,
        },
        merged_commit: match patch.state() {
            patch::State::Merged { commit, .. } => Some(commit.to_string()),
            _ => None,
        },
    }
}

// ─── Issues store ────────────────────────────────────────────────────────────

/// Typed issue store for one repository (upstream `cob::issue::Issues`).
/// `open()` is read-only; `open_as(repository, signer)` enables writes
/// (upstream `WriteAs(signer)` access mode — writes never take a signer).
#[pyclass(name = "Issues", module = "radiclepy", unsendable)]
pub struct PyIssues {
    repo: PyRepository,
    signer: Option<PySigner>,
}

impl PyIssues {
    fn write_signer(&self) -> PyResult<PySigner> {
        self.signer.clone().ok_or_else(|| {
            error::value_err(
                "issue store is read-only; open with Issues.open_as(repository, signer)",
            )
        })
    }
}

#[pymethods]
impl PyIssues {
    /// Open the issue store of `repository` (read-only).
    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, repository: PyRepository) -> Self {
        Self {
            repo: repository,
            signer: None,
        }
    }

    /// Open the issue store for **writes** (upstream `WriteAs(signer)`).
    /// Mutating methods still take no signer — it is fixed here, once.
    #[classmethod]
    fn open_as(_cls: &Bound<'_, PyType>, repository: PyRepository, signer: &PySigner) -> Self {
        Self {
            repo: repository,
            signer: Some(signer.clone()),
        }
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

    /// Create an issue; returns a mutable handle (the issue is immediately
    /// persisted — each mutator is its own COB transaction, upstream).
    #[pyo3(signature = (title, description, labels=None, assignees=None))]
    fn create(
        &self,
        title: &str,
        description: &str,
        labels: Option<Vec<String>>,
        assignees: Option<Vec<String>>,
    ) -> PyResult<PyIssueMut> {
        let signer = self.write_signer()?;
        let title = parse_title(title)?;
        let labels = parse_labels(labels.unwrap_or_default())?;
        let assignees = parse_dids(assignees.unwrap_or_default())?;
        let repo = self.repo.open()?;
        let mut store = RawIssues::open(&repo, WriteAs::new(&*signer.inner)).map_err(cob_err)?;
        let mut cache = NoCache;
        let created = store
            .create(title, description, &labels, &assignees, [], &mut cache)
            .map_err(cob_err)?;
        let id = created.id().to_owned();
        let snapshot = materialize_issue(&id, &created);
        Ok(PyIssueMut {
            repo: self.repo.clone(),
            signer,
            id: id.to_string(),
            snapshot,
        })
    }

    /// Mutable handle on an existing issue (write stores only).
    fn get_mut(&self, id: &str) -> PyResult<PyIssueMut> {
        let signer = self.write_signer()?;
        let id = parse_object_id(id)?;
        let repo = self.repo.open()?;
        let mut store = RawIssues::open(&repo, WriteAs::new(&*signer.inner)).map_err(cob_err)?;
        let mut cache = NoCache;
        let m = store.get_mut(&id, &mut cache).map_err(cob_err)?;
        let snapshot = materialize_issue(&id, &m);
        Ok(PyIssueMut {
            repo: self.repo.clone(),
            signer,
            id: id.to_string(),
            snapshot,
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "Issues({}{})",
            self.repo.rid_str(),
            if self.signer.is_some() {
                ", write"
            } else {
                ", read-only"
            }
        )
    }
}

// ─── IssueMut (Phase 5 writes) ───────────────────────────────────────────────

/// Mutable handle on an issue; every method commits one COB operation
/// (upstream transaction) and refreshes the `issue` snapshot.
#[pyclass(name = "IssueMut", module = "radiclepy", unsendable)]
pub struct PyIssueMut {
    repo: PyRepository,
    signer: PySigner,
    id: String,
    snapshot: Issue,
}

/// One COB transaction against the issue behind the handle: open the write
/// store, load the entry mutably, run `$body` (which binds `m: &mut IssueMut`,
/// must evaluate to `PyResult<String>`), then refresh the snapshot from the
/// upstream-updated entry. Commit-per-call mirrors upstream transactions.
macro_rules! issue_tx {
    ($slf:expr, |$m:ident| $body:expr) => {{
        let repo = $slf.repo.open()?;
        let mut store =
            RawIssues::open(&repo, WriteAs::new(&*$slf.signer.inner)).map_err(cob_err)?;
        let mut cache = NoCache;
        let id = parse_object_id(&$slf.id)?;
        let mut loaded = store.get_mut(&id, &mut cache).map_err(cob_err)?;
        let result: PyResult<String> = {
            let $m = &mut loaded;
            $body
        };
        $slf.snapshot = materialize_issue(&id, &loaded);
        result
    }};
}

#[pymethods]
impl PyIssueMut {
    /// Issue id (ObjectId hex).
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }
    /// Snapshot of the issue after the latest change through this handle.
    #[getter]
    fn issue(&self) -> Issue {
        self.snapshot.clone()
    }
    /// Re-read the issue from storage (upstream reload semantics).
    fn reload(&mut self) -> PyResult<Issue> {
        issue_tx!(self, |m| {
            let _ = &*m;
            Ok(String::new())
        })?;
        Ok(self.snapshot.clone())
    }
    /// Comment on the issue; returns the new change-entry id.
    /// `reply_to` defaults to the root comment (upstream requires a target).
    #[pyo3(signature = (body, reply_to=None))]
    fn comment(&mut self, body: &str, reply_to: Option<&str>) -> PyResult<String> {
        let reply_to = match reply_to {
            Some(id) => parse_comment_id(id)?,
            None => {
                let root = self
                    .snapshot
                    .comment_ids
                    .first()
                    .ok_or_else(|| error::op_err("issue has no root comment"))?;
                parse_comment_id(root)?
            }
        };
        let body = body.to_owned();
        issue_tx!(self, |m| {
            let entry = m.comment(body, reply_to, []).map_err(cob_err)?;
            Ok(entry.to_string())
        })
    }
    /// Set the issue title; returns the new change-entry id.
    fn edit(&mut self, title: &str) -> PyResult<String> {
        let title = parse_title(title)?;
        issue_tx!(self, |m| {
            let entry = m.edit(title).map_err(cob_err)?;
            Ok(entry.to_string())
        })
    }
    /// Set the issue description (edits the root comment); returns entry id.
    fn edit_description(&mut self, description: &str) -> PyResult<String> {
        let description = description.to_owned();
        issue_tx!(self, |m| {
            let entry = m.edit_description(description, []).map_err(cob_err)?;
            Ok(entry.to_string())
        })
    }
    /// SET the issue's label set; returns the new change-entry id.
    /// Upstream `Action::Label` REPLACES the labels (not additive): this
    /// handle's previous labels are gone after the call.
    fn label(&mut self, labels: Vec<String>) -> PyResult<String> {
        let labels = parse_labels(labels)?;
        issue_tx!(self, |m| {
            let entry = m.label(labels).map_err(cob_err)?;
            Ok(entry.to_string())
        })
    }
    /// SET the issue's assignee set; returns the new change-entry id.
    /// Upstream `Action::Assign` REPLACES the assignees (not additive,
    /// exactly like `label`); an empty list clears the set.
    fn assign(&mut self, assignees: Vec<String>) -> PyResult<String> {
        let assignees = parse_dids(assignees)?;
        issue_tx!(self, |m| {
            let entry = m.assign(assignees).map_err(cob_err)?;
            Ok(entry.to_string())
        })
    }
    #[pyo3(signature = (reason="unspecified"))]
    fn close(&mut self, reason: &str) -> PyResult<String> {
        self.lifecycle("closed", Some(reason))
    }

    fn reopen(&mut self) -> PyResult<String> {
        self.lifecycle("open", None)
    }

    /// Open/close the issue; returns the new change-entry id.
    /// `state` is `"open"` or `"closed"`; `reason` (when closing) is
    /// `"unspecified"` (default) or `"solved"` (upstream `CloseReason`).
    #[pyo3(signature = (state, reason=None))]
    fn lifecycle(&mut self, state: &str, reason: Option<&str>) -> PyResult<String> {
        let state = match state {
            "open" => {
                if reason.is_some() {
                    return Err(error::value_err("reason is only valid for state='closed'"));
                }
                IssueState::Open
            }
            "closed" => IssueState::Closed {
                reason: match reason.unwrap_or("unspecified") {
                    "solved" => issue::CloseReason::Solved,
                    "unspecified" => issue::CloseReason::Other,
                    other => {
                        return Err(error::value_err(format!(
                            "invalid close reason {other:?}; expected 'unspecified' or 'solved'"
                        )));
                    }
                },
            },
            other => {
                return Err(error::value_err(format!(
                    "invalid state {other:?}; expected 'open' or 'closed'"
                )));
            }
        };
        issue_tx!(self, |m| {
            let entry = m.lifecycle(state).map_err(cob_err)?;
            Ok(entry.to_string())
        })
    }
    fn __repr__(&self) -> String {
        format!("IssueMut({})", self.id)
    }
}

fn parse_title(value: &str) -> PyResult<Title> {
    Title::new(value).map_err(|e| error::value_err(format!("invalid title: {e}")))
}

fn parse_labels(values: Vec<String>) -> PyResult<Vec<Label>> {
    values
        .iter()
        .map(|l| {
            Label::new(l.as_str())
                .map_err(|e| error::value_err(format!("invalid label {l:?}: {e}")))
        })
        .collect()
}

fn parse_dids(values: Vec<String>) -> PyResult<Vec<Did>> {
    values
        .iter()
        .map(|d| {
            d.parse::<Did>()
                .map_err(|e| error::value_err(format!("invalid DID {d:?}: {e}")))
        })
        .collect()
}

fn parse_comment_id(value: &str) -> PyResult<CommentId> {
    value
        .parse::<CommentId>()
        .map_err(|e| error::value_err(format!("invalid comment id {value:?}: {e}")))
}

// ─── Patches store ───────────────────────────────────────────────────────────

/// Typed patch store for one repository (upstream `cob::patch::Patches`).
/// `open()` is read-only; `open_as(repository, signer)` enables writes
/// (upstream `WriteAs(signer)` access mode).
#[pyclass(name = "Patches", module = "radiclepy", unsendable)]
pub struct PyPatches {
    repo: PyRepository,
    signer: Option<PySigner>,
}

impl PyPatches {
    fn write_signer(&self) -> PyResult<PySigner> {
        self.signer.clone().ok_or_else(|| {
            error::value_err(
                "patch store is read-only; open with Patches.open_as(repository, signer)",
            )
        })
    }
    #[allow(clippy::too_many_arguments)]
    fn _create_or_draft(
        &self,
        title: &str,
        description: &str,
        target: &str,
        base: &str,
        head: &str,
        labels: Option<Vec<String>>,
        draft: bool,
    ) -> PyResult<PyPatchMut> {
        let signer = self.write_signer()?;
        let title = parse_title(title)?;
        let target = parse_merge_target(target)?;
        let labels = parse_labels(labels.unwrap_or_default())?;
        let base = crate::write::parse_git_oid(base)?;
        let head = crate::write::parse_git_oid(head)?;
        let repo = self.repo.open()?;
        let mut store = RawPatches::open(&repo, WriteAs::new(&*signer.inner)).map_err(cob_err)?;
        let mut cache = NoCache;
        let m = if draft {
            store
                .draft(title, description, target, base, head, &labels, &mut cache)
                .map_err(cob_err)?
        } else {
            store
                .create(title, description, target, base, head, &labels, &mut cache)
                .map_err(cob_err)?
        };
        let id = m.id().to_owned();
        let snapshot = materialize_patch(&id, &m);
        Ok(PyPatchMut {
            repo: self.repo.clone(),
            signer,
            id: id.to_string(),
            snapshot,
        })
    }
}

#[pymethods]
impl PyPatches {
    /// Open the patch store of `repository` (read-only).
    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, repository: PyRepository) -> Self {
        Self {
            repo: repository,
            signer: None,
        }
    }

    /// Open the patch store for **writes** (upstream `WriteAs(signer)`).
    #[classmethod]
    fn open_as(_cls: &Bound<'_, PyType>, repository: PyRepository, signer: &PySigner) -> Self {
        Self {
            repo: repository,
            signer: Some(signer.clone()),
        }
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

    /// Mutable handle on an existing patch (write stores only).
    fn get_mut(&self, id: &str) -> PyResult<PyPatchMut> {
        let signer = self.write_signer()?;
        let id = parse_object_id(id)?;
        let repo = self.repo.open()?;
        let mut store = RawPatches::open(&repo, WriteAs::new(&*signer.inner)).map_err(cob_err)?;
        let mut cache = NoCache;
        let m = store.get_mut(&id, &mut cache).map_err(cob_err)?;
        let snapshot = materialize_patch(&id, &m);
        Ok(PyPatchMut {
            repo: self.repo.clone(),
            signer,
            id: id.to_string(),
            snapshot,
        })
    }

    /// Create an **open** patch (upstream `Patches::create`). `target` is
    /// `"delegates"` or `"branch:<name>"`; `base`/`head` are git commit OIDs
    /// that must exist in the repository.
    #[pyo3(signature = (title, description, target, base, head, labels=None))]
    fn create(
        &self,
        title: &str,
        description: &str,
        target: &str,
        base: &str,
        head: &str,
        labels: Option<Vec<String>>,
    ) -> PyResult<PyPatchMut> {
        self._create_or_draft(title, description, target, base, head, labels, false)
    }

    /// Create a **draft** patch (upstream `Patches::draft`); use ready()
    /// on the returned handle to promote it.
    #[pyo3(signature = (title, description, target, base, head, labels=None))]
    fn draft(
        &self,
        title: &str,
        description: &str,
        target: &str,
        base: &str,
        head: &str,
        labels: Option<Vec<String>>,
    ) -> PyResult<PyPatchMut> {
        self._create_or_draft(title, description, target, base, head, labels, true)
    }

    fn __repr__(&self) -> String {
        format!(
            "Patches({}{})",
            self.repo.rid_str(),
            if self.signer.is_some() {
                ", write"
            } else {
                ", read-only"
            }
        )
    }
}

// ─── PatchMut (Phase 5) ──────────────────────────────────────────────────────

/// Mutable patch handle; reopen/load per call, one upstream transaction.
#[pyclass(name = "PatchMut", module = "radiclepy", unsendable)]
pub struct PyPatchMut {
    repo: PyRepository,
    signer: PySigner,
    id: String,
    snapshot: Patch,
}

macro_rules! patch_tx {
    ($slf:expr, |$m:ident| $body:expr) => {{
        let repo = $slf.repo.open()?;
        let mut store =
            RawPatches::open(&repo, WriteAs::new(&*$slf.signer.inner)).map_err(cob_err)?;
        let mut cache = NoCache;
        let id = parse_object_id(&$slf.id)?;
        let mut loaded = store.get_mut(&id, &mut cache).map_err(cob_err)?;
        let result = {
            let $m = &mut loaded;
            $body
        };
        $slf.snapshot = materialize_patch(&id, &loaded);
        result
    }};
}

fn parse_revision(value: &str) -> PyResult<patch::RevisionId> {
    parse_comment_id(value).map(patch::RevisionId::from)
}

#[pymethods]
impl PyPatchMut {
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }
    #[getter]
    fn patch(&self) -> Patch {
        self.snapshot.clone()
    }

    fn reload(&mut self) -> PyResult<Patch> {
        patch_tx!(self, |m| {
            let _ = &*m;
            Ok::<(), PyErr>(())
        })?;
        Ok(self.snapshot.clone())
    }
    fn edit(&mut self, title: &str, target: &str) -> PyResult<String> {
        let title = parse_title(title)?;
        let target = parse_merge_target(target)?;
        patch_tx!(self, |m| m
            .edit(title, target)
            .map(|id| id.to_string())
            .map_err(cob_err))
    }
    fn label(&mut self, labels: Vec<String>) -> PyResult<String> {
        let labels = parse_labels(labels)?;
        patch_tx!(self, |m| m
            .label(labels)
            .map(|id| id.to_string())
            .map_err(cob_err))
    }
    fn assign(&mut self, assignees: Vec<String>) -> PyResult<String> {
        let assignees = parse_dids(assignees)?.into_iter().collect();
        patch_tx!(self, |m| m
            .assign(assignees)
            .map(|id| id.to_string())
            .map_err(cob_err))
    }
    fn update(&mut self, description: &str, base: &str, head: &str) -> PyResult<String> {
        let base = crate::write::parse_git_oid(base)?;
        let head = crate::write::parse_git_oid(head)?;
        patch_tx!(self, |m| m
            .update(description, base, head)
            .map(|id| id.to_string())
            .map_err(cob_err))
    }
    fn ready(&mut self) -> PyResult<bool> {
        patch_tx!(self, |m| m.ready().map_err(cob_err))
    }
    fn unready(&mut self) -> PyResult<bool> {
        patch_tx!(self, |m| m.unready().map_err(cob_err))
    }
    fn archive(&mut self) -> PyResult<bool> {
        patch_tx!(self, |m| m.archive().map_err(cob_err))
    }
    fn unarchive(&mut self) -> PyResult<bool> {
        patch_tx!(self, |m| m.unarchive().map_err(cob_err))
    }

    #[pyo3(signature = (revision, body, reply_to=None))]
    fn comment(&mut self, revision: &str, body: &str, reply_to: Option<&str>) -> PyResult<String> {
        let revision = parse_revision(revision)?;
        let reply_to = reply_to.map(parse_comment_id).transpose()?;
        patch_tx!(self, |m| m
            .comment(revision, body, reply_to, None, [])
            .map(|id| id.to_string())
            .map_err(cob_err))
    }
    #[pyo3(signature = (revision, verdict=None, summary=None))]
    fn review(
        &mut self,
        revision: &str,
        verdict: Option<&str>,
        summary: Option<String>,
    ) -> PyResult<String> {
        let revision = parse_revision(revision)?;
        let verdict = match verdict {
            Some("accept") => Some(patch::Verdict::Accept),
            Some("reject") => Some(patch::Verdict::Reject),
            None => None,
            _ => {
                return Err(error::value_err(
                    "verdict must be 'accept', 'reject' or None",
                ));
            }
        };
        patch_tx!(self, |m| m
            .review(revision, verdict, summary, vec![])
            .map(|id| id.to_string())
            .map_err(cob_err))
    }
    /// Record a merge; does not merge Git, move branches or clean up refs.
    fn merge(&mut self, revision: &str, commit: &str) -> PyResult<String> {
        let revision = parse_revision(revision)?;
        let commit = crate::write::parse_git_oid(commit)?;
        patch_tx!(self, |m| m
            .merge(revision, commit)
            .map(|merged| merged.entry.to_string())
            .map_err(cob_err))
    }
    fn __repr__(&self) -> String {
        format!("PatchMut({})", self.id)
    }
}

fn parse_merge_target(value: &str) -> PyResult<patch::MergeTarget> {
    match value {
        "delegates" => Ok(patch::MergeTarget::Delegates),
        other => match other.strip_prefix("branch:") {
            Some(name) => {
                let branch = patch::TargetBranch::try_from(name)
                    .map_err(|e| error::value_err(format!("invalid branch name {name:?}: {e}")))?;
                Ok(patch::MergeTarget::Branch(branch))
            }
            None => Err(error::value_err(
                "target must be 'delegates' or 'branch:<name>'",
            )),
        },
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Issue>()?;
    m.add_class::<Patch>()?;
    m.add_class::<PyIssues>()?;
    m.add_class::<PyPatches>()?;
    m.add_class::<PyIssueMut>()?;
    m.add_class::<PyPatchMut>()?;
    Ok(())
}
