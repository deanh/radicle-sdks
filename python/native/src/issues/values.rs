//! Immutable issue snapshots and eager upstream materialization.
use crate::comment::{self, Comment};
use pyo3::prelude::*;
use radicle::cob::{
    ObjectId,
    issue::{self, Issue as RawIssue},
};

/// A materialized issue (read-only snapshot of upstream `cob::issue::Issue`).
///
/// Field mapping: title/description/labels are `String` upstream
/// (already valid UTF-8), so exposed as `str`. `author`/`assignees` are DIDs.
/// Comment bodies are `str` (upstream `String`); comment ids are COB
/// `ObjectId` hex strings. `state` is `"open" | "closed"`; a closed issue's
/// close reason (if any) is `close_reason`.
#[pyclass(name = "Issue", module = "radiclepy", frozen, eq)]
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
    fn __hash__(&self) -> u64 {
        crate::hashing::snapshot_hash(self)
    }

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

pub(super) fn materialize_issue(id: &ObjectId, issue: &RawIssue) -> Issue {
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
            .map(|(id, c)| comment::comment(id, c))
            .collect(),
    }
}
