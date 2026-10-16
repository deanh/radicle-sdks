//! Local issue store and mutable handles; eager snapshots, one transaction per call.
mod values;
use crate::error::cob_err;
use crate::ids::{
    parse_cob_id as parse_object_id, parse_comment_id, parse_dids, parse_labels, parse_title,
};
use crate::{error, profile::PySigner, repository::PyRepository};
use pyo3::prelude::*;
use pyo3::types::PyType;
use radicle::cob::cache::NoCache;
use radicle::cob::issue::{self, Issues as RawIssues, State as IssueState};
use radicle::cob::store::access::{ReadOnly, WriteAs};
pub(crate) use values::Issue;
use values::materialize_issue;

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
            error::read_only_err(
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

// ─── IssueMut ───────────────────────────────────────────────

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
