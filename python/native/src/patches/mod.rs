//! Local patch store and mutable handles; preserve upstream transaction boundaries.
mod values;
use crate::error::cob_err;
use crate::ids::{
    parse_cob_id as parse_object_id, parse_comment_id, parse_dids, parse_labels, parse_title,
};
use crate::{error, profile::PySigner, repository::PyRepository};
use pyo3::prelude::*;
use pyo3::types::PyType;
use radicle::cob::cache::NoCache;
use radicle::cob::patch::{self, Patches as RawPatches};
use radicle::cob::store::access::{ReadOnly, WriteAs};
use values::materialize_patch;
pub(crate) use values::{Patch, Review, Revision};

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
            error::read_only_err(
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
        let base = crate::ids::parse_git_oid(base)?;
        let head = crate::ids::parse_git_oid(head)?;
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
    #[pyo3(signature = (title, description, *, base, head, target="delegates", labels=None))]
    fn create(
        &self,
        title: &str,
        description: &str,
        base: &str,
        head: &str,
        target: &str,
        labels: Option<Vec<String>>,
    ) -> PyResult<PyPatchMut> {
        self._create_or_draft(title, description, target, base, head, labels, false)
    }

    /// Create a **draft** patch (upstream `Patches::draft`); use ready()
    /// on the returned handle to promote it.
    #[pyo3(signature = (title, description, *, base, head, target="delegates", labels=None))]
    fn draft(
        &self,
        title: &str,
        description: &str,
        base: &str,
        head: &str,
        target: &str,
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
    #[pyo3(signature = (description, *, base, head))]
    fn update(&mut self, description: &str, base: &str, head: &str) -> PyResult<String> {
        let base = crate::ids::parse_git_oid(base)?;
        let head = crate::ids::parse_git_oid(head)?;
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
        let commit = crate::ids::parse_git_oid(commit)?;
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
