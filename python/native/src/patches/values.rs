//! Immutable patch/revision/review snapshots and eager materialization.
use crate::comment::{comment, Comment};
use pyo3::prelude::*;
use radicle::cob::{
    patch::{self, Patch as RawPatch},
    ObjectId,
};

#[pyclass(name = "Review", module = "radiclepy", get_all, frozen, eq)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Review {
    pub id: String,
    pub author: String,
    pub verdict: Option<String>,
    pub summary: String,
    pub timestamp: u64,
}

#[pymethods]
impl Review {
    fn __hash__(&self) -> u64 {
        crate::values::snapshot_hash(self)
    }

    fn __repr__(&self) -> String {
        format!("Review({}, {}, {:?})", self.id, self.author, self.verdict)
    }
}

#[pyclass(name = "Revision", module = "radiclepy", get_all, frozen, eq)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Revision {
    pub id: String,
    pub author: String,
    pub description: String,
    pub base: String,
    pub head: String,
    pub timestamp: u64,
    pub comments: Vec<Comment>,
    pub reviews: Vec<Review>,
}

#[pymethods]
impl Revision {
    fn __hash__(&self) -> u64 {
        crate::values::snapshot_hash(self)
    }

    fn __repr__(&self) -> String {
        format!("Revision({}, {})", self.id, self.head)
    }
}

pub fn revision(r: &patch::Revision) -> Revision {
    Revision {
        id: r.id().to_string(),
        author: r.author().id().to_string(),
        description: r.description().to_owned(),
        base: r.base().to_string(),
        head: r.head().to_string(),
        timestamp: r.timestamp().as_millis(),
        comments: r.replies().map(|(id, c)| comment(id, c)).collect(),
        reviews: r
            .reviews()
            .map(|(_, v)| Review {
                id: v.id().to_string(),
                author: v.author().id().to_string(),
                verdict: v.verdict().map(|v| v.to_string()),
                summary: v.summary().to_owned(),
                timestamp: v.timestamp().as_millis(),
            })
            .collect(),
    }
}

/// A materialized patch (read-only snapshot of upstream `cob::patch::Patch`).
///
/// `state`: `"draft" | "open" | "archived" | "merged"`. `base`/`head` are the
/// latest patch-author revision's base and tip commit OIDs; `latest_revision`
/// identifies that revision. `target` is the merge target as
/// a string (`"delegates"` or `"branch:<name>"`).
#[pyclass(name = "Patch", module = "radiclepy", frozen, eq)]
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
    fn __hash__(&self) -> u64 {
        crate::values::snapshot_hash(self)
    }

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

pub(super) fn materialize_patch(id: &ObjectId, patch: &RawPatch) -> Patch {
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
        revisions: patch.revisions().map(|(_, r)| self::revision(r)).collect(),
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
