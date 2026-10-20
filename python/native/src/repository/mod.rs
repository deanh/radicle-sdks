//! Storage and repository access.
//!
//! `PyRepository` holds `Arc<PyStorageInner>` + a `RepoId` and reopens the
//! repository per operation: no long-lived git handles, no
//! borrow-lifetime pressure across the GIL.

mod values;
mod write;
use crate::project::ProjectDoc;
use values::{Commit, HeadInfo, HistoryPage, NamespaceTip, RemoteInfo, TagInfo, Tree, TreeEntry};
pub(crate) use values::{PySetHead, PySignedRefs};

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyType;
use radicle::git::{self, Oid};
use radicle::identity::RepoId;
use radicle::node::Alias;
use radicle::storage::git::Storage as RawStorage;
use radicle::storage::{ReadRepository, ReadStorage};

use crate::error;
use crate::profile::PyProfile;

/// Default/maximum page sizes — history walks are always bounded.
const DEFAULT_HISTORY_LIMIT: i64 = 100;
const MAX_HISTORY_LIMIT: i64 = 1000;

fn to_raw(oid: Oid) -> PyResult<git::raw::Oid> {
    git::raw::Oid::from_str(&oid.to_string())
        .map_err(|err| error::op_err(format!("oid conversion: {err}")))
}

fn parse_oid(value: &str) -> PyResult<Oid> {
    Oid::from_str(value).map_err(|err| error::value_err(format!("invalid Oid: {err}")))
}

/// Concrete object kind behind `TreeEntry.kind` and `TagInfo.peeled_kind`.
/// Only blob, tree and commit can occur there: tree entries never point at tag
/// objects, and a fully peeled tag target is by construction not a tag. The
/// libgit2 wildcard `Any` and `Tag` map to `None` so the public domain stays
/// exactly `"blob" | "tree" | "commit"`.
fn kind_str(kind: Option<git::raw::ObjectType>) -> Option<String> {
    match kind? {
        git::raw::ObjectType::Blob => Some("blob".to_owned()),
        git::raw::ObjectType::Tree => Some("tree".to_owned()),
        git::raw::ObjectType::Commit => Some("commit".to_owned()),
        git::raw::ObjectType::Any | git::raw::ObjectType::Tag => None,
    }
}

// ─── Storage ─────────────────────────────────────────────────────────────────

pub(crate) struct StorageInner {
    pub(crate) raw: RawStorage,
}

#[pyclass(name = "Storage", module = "radiclepy")]
#[derive(Clone)]
pub struct PyStorage {
    pub(crate) inner: Arc<StorageInner>,
}

impl PyStorage {
    pub(crate) fn from_profile_ref(profile: &radicle::Profile) -> Self {
        Self {
            inner: Arc::new(StorageInner {
                raw: profile.storage.clone(),
            }),
        }
    }
}

#[pymethods]
impl PyStorage {
    /// Open storage for a profile (primary constructor — UserInfo comes from
    /// the profile, matching `profile.storage`).
    #[classmethod]
    fn from_profile(_cls: &Bound<'_, PyType>, profile: PyRef<'_, PyProfile>) -> Self {
        Self::from_profile_ref(&profile.inner)
    }

    /// Low-level constructor: open (or create) storage at `path` with an
    /// explicit git user identity. Most callers want `Storage.from_profile`.
    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, path: PathBuf, alias: &str, node_id: &str) -> PyResult<Self> {
        let alias = Alias::from_str(alias)
            .map_err(|err| error::value_err(format!("invalid Alias: {err}")))?;
        let key = node_id
            .parse::<radicle::node::NodeId>()
            .map_err(|err| error::value_err(format!("invalid NodeId: {err}")))?;
        let raw =
            RawStorage::open(path, git::UserInfo { alias, key }).map_err(error::storage_err)?;
        Ok(Self {
            inner: Arc::new(StorageInner { raw }),
        })
    }

    /// Filesystem path of this storage.
    #[getter]
    fn path(&self) -> PyResult<&str> {
        self.inner
            .raw
            .path()
            .to_str()
            .ok_or_else(|| error::value_err("storage path is not valid UTF-8"))
    }

    /// Open a repository for reading. Raises `RepositoryError` if absent.
    fn repository(&self, rid: &str) -> PyResult<PyRepository> {
        let rid = RepoId::from_str(rid)
            .map_err(|err| error::value_err(format!("invalid RepoId: {err}")))?;
        // Validate now; operations reopen per call.
        self.inner
            .raw
            .repository(rid)
            .map_err(error::repository_err)?;
        Ok(PyRepository {
            storage: self.clone(),
            rid,
        })
    }

    /// Whether a repository with this id exists in this storage.
    fn contains(&self, rid: &str) -> PyResult<bool> {
        let rid = RepoId::from_str(rid)
            .map_err(|err| error::value_err(format!("invalid RepoId: {err}")))?;
        self.inner.raw.contains(&rid).map_err(error::repository_err)
    }

    fn __contains__(&self, rid: &str) -> PyResult<bool> {
        self.contains(rid)
    }

    /// Repository ids present in this storage (unsorted; storage order).
    /// Raises `StorageError` on storage-level failures (NOT an empty list —
    /// real failures must not masquerade as an empty store).
    fn repositories(&self) -> PyResult<Vec<String>> {
        Ok(self
            .inner
            .raw
            .repositories()
            .map_err(error::storage_err)?
            .into_iter()
            .map(|i| i.rid.to_string())
            .collect())
    }

    fn __repr__(&self) -> String {
        format!("Storage({})", self.inner.raw.path().display())
    }
}

// ─── Repository (read-only) ──────────────────────────────────────────────────

#[pyclass(name = "Repository", module = "radiclepy")]
#[derive(Clone)]
pub struct PyRepository {
    storage: PyStorage,
    rid: RepoId,
}

impl PyRepository {
    /// Repository id for crate-internal use; the `rid` getter below is the
    /// Python surface.
    pub(crate) fn rid_str(&self) -> String {
        self.rid.to_string()
    }

    pub(crate) fn open(&self) -> PyResult<radicle::storage::git::Repository> {
        self.storage
            .inner
            .raw
            .repository(self.rid)
            .map_err(error::repository_err)
    }
}

#[pymethods]
impl PyRepository {
    /// The repository id.
    #[getter]
    fn rid(&self) -> String {
        self.rid.to_string()
    }

    /// Head as `HeadInfo(name, oid)` (upstream `head() -> (Qualified, Oid)`).
    #[getter]
    fn head(&self) -> PyResult<HeadInfo> {
        let repo = self.open()?;
        let (name, oid) = ReadRepository::head(&repo).map_err(error::repository_err)?;
        Ok(HeadInfo::new(name.as_str().to_owned(), oid.to_string()))
    }

    /// The repository's project identity document (verified canonical doc).
    /// Raises `IdentityError` if missing or unverifiable.
    fn identity(&self) -> PyResult<ProjectDoc> {
        let repo = self.open()?;
        let doc = repo
            .canonical_identity_doc()
            .map_err(|err| error::identity_err(format!("{err}")))?;
        let project = doc
            .project()
            .map_err(|err| error::identity_err(format!("{err}")))?;
        let visibility = match doc.visibility() {
            radicle::identity::Visibility::Public => "public",
            radicle::identity::Visibility::Private { .. } => "private",
        };
        Ok(ProjectDoc {
            name: project.name().to_string(),
            description: project.description().to_owned(),
            default_branch: project.default_branch().as_str().to_owned(),
            version: doc.version().number().into(),
            delegates: doc.delegates().iter().map(|d| d.to_string()).collect(),
            threshold: doc.threshold(),
            visibility: visibility.to_owned(),
            commit: doc.commit.to_string(),
            blob: doc.blob.to_string(),
        })
    }

    /// The project description (from the identity document).
    ///
    /// The project payload requires a description, so there is no legitimate
    /// "missing" value: identity failures (corruption, verification, I/O)
    /// propagate as `IdentityError` instead of masquerading as `None`.
    fn description(&self) -> PyResult<String> {
        Ok(self.identity()?.description.clone())
    }

    /// Branch (head ref) short names.
    fn branches(&self) -> PyResult<Vec<String>> {
        let repo = self.open()?;
        let mut names = Vec::new();
        for r in repo.backend.references().map_err(error::git_err)? {
            let r = r.map_err(error::git_err)?;
            if let Some(name) = r
                .name()
                .map_err(error::git_err)?
                .strip_prefix("refs/heads/")
            {
                names.push(name.to_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    /// The OID a branch points at, or `None` if the branch does not exist.
    ///
    /// Raises `ValueError` for binding-invalid branch names and `GitError`
    /// for any other libgit2 failure — only `NotFound` maps to `None`.
    fn branch_head(&self, name: &str) -> PyResult<Option<String>> {
        let full = format!("refs/heads/{name}");
        if name.is_empty() || !git::raw::Reference::is_valid_name(&full) {
            return Err(error::value_err(format!("invalid branch name: {name:?}")));
        }
        let repo = self.open()?;
        match repo.backend.refname_to_id(&full) {
            Ok(oid) => Ok(Some(oid.to_string())),
            Err(err) if err.code() == git::raw::ErrorCode::NotFound => Ok(None),
            Err(err) => Err(error::git_err(err)),
        }
    }

    /// Tags with explicit raw/peeled distinction (see `TagInfo`).
    fn tags(&self) -> PyResult<Vec<TagInfo>> {
        let repo = self.open()?;
        let mut raws: Vec<(String, git::raw::Oid)> = Vec::new();
        for r in repo.backend.references().map_err(error::git_err)? {
            let r = r.map_err(error::git_err)?;
            let name = r.name().map_err(error::git_err)?;
            // Symbolic tag refs have no direct target and are skipped.
            if let Some(short) = name.strip_prefix("refs/tags/")
                && let Some(target) = r.target()
            {
                raws.push((short.to_owned(), target));
            }
        }
        raws.sort();

        let mut out = Vec::with_capacity(raws.len());
        for (short, target) in raws {
            let obj = repo
                .backend
                .find_object(target, None)
                .map_err(error::git_err)?;
            let annotated = obj.kind() == Some(git::raw::ObjectType::Tag);
            // Peel ONLY tag objects: peel() on a commit silently resolves to
            // its tree. For lightweight tags the target
            // already is the peeled object.
            let (peeled, peeled_obj) = if annotated {
                let peeled = obj
                    .peel(git::raw::ObjectType::Any)
                    .map_err(error::git_err)?;
                (peeled.id().to_string(), peeled)
            } else {
                (target.to_string(), obj)
            };
            // A peeled target is a commit, tree or blob; anything else is
            // corrupt data and must not leak as a made-up kind string.
            let peeled_kind = kind_str(peeled_obj.kind()).ok_or_else(|| {
                error::git_err(format!("tag {short:?} peels to an unexpected object kind"))
            })?;
            out.push(TagInfo {
                name: short,
                target: target.to_string(),
                annotated,
                peeled,
                peeled_kind,
            });
        }
        Ok(out)
    }

    /// Reflexive plain-Git commit ancestry, not COB causal ancestry.
    fn is_ancestor(&self, ancestor: &str, descendant: &str) -> PyResult<bool> {
        let ancestor = parse_oid(ancestor)?;
        let descendant = parse_oid(descendant)?;
        let repo = self.open()?;
        // Validate both endpoints even for equality: libgit2's strict descendant
        // check returns false for equal OIDs without looking up either object.
        repo.backend
            .find_commit(to_raw(ancestor)?)
            .map_err(error::git_err)?;
        repo.backend
            .find_commit(to_raw(descendant)?)
            .map_err(error::git_err)?;
        if ancestor == descendant {
            return Ok(true);
        }
        repo.is_ancestor_of(ancestor, descendant)
            .map_err(error::git_err)
    }

    /// Resolve a git revision expression to an OID.
    /// Raises `GitError` for both invalid and unresolved revisions
    /// (revspec semantics are git's, not ours).
    fn rev_parse(&self, rev: &str) -> PyResult<String> {
        let repo = self.open()?;
        let obj = repo
            .backend
            .revparse_single(rev)
            .map_err(|err| error::git_err(format!("rev-parse {rev:?}: {err}")))?;
        Ok(obj.id().to_string())
    }

    /// Load a commit by OID. Raises `GitError` if absent or not a commit.
    fn commit(&self, oid: &str) -> PyResult<Commit> {
        let oid = parse_oid(oid)?;
        let repo = self.open()?;
        let c = repo
            .backend
            .find_commit(to_raw(oid)?)
            .map_err(|err| error::git_err(format!("commit {oid}: {err}")))?;
        let sig = |s: git::raw::Signature| {
            (
                s.name_bytes().to_vec(),
                s.email_bytes().to_vec(),
                s.when().seconds(),
                s.when().offset_minutes(),
            )
        };
        let (an, ae, at, ao) = sig(c.author());
        let (cn, ce, ct, co) = sig(c.committer());
        Ok(Commit {
            oid: oid.to_string(),
            tree: c.tree_id().to_string(),
            parents: c.parent_ids().map(|p| p.to_string()).collect(),
            author_name: an,
            author_email: ae,
            author_time: at,
            author_offset_min: ao,
            committer_name: cn,
            committer_email: ce,
            committer_time: ct,
            committer_offset_min: co,
            message: c.message_bytes().to_vec(),
            encoding: match c.message_encoding() {
                Ok(Some(s)) => Some(s.to_owned()),
                _ => None,
            },
        })
    }

    /// Load a tree by OID (entries in canonical git order).
    fn tree(&self, oid: &str) -> PyResult<Tree> {
        let oid = parse_oid(oid)?;
        let repo = self.open()?;
        let tree = repo
            .backend
            .find_tree(to_raw(oid)?)
            .map_err(|err| error::git_err(format!("tree {oid}: {err}")))?;
        let entries = tree
            .iter()
            .map(|e| TreeEntry {
                name: e.name_bytes().to_vec(),
                kind: kind_str(e.kind()),
                mode: e.filemode() as u32,
                oid: e.id().to_string(),
            })
            .collect();
        Ok(Tree {
            oid: oid.to_string(),
            entries,
        })
    }

    /// Load a blob's raw bytes.
    fn blob(&self, oid: &str) -> PyResult<Vec<u8>> {
        let oid = parse_oid(oid)?;
        let repo = self.open()?;
        let blob = ReadRepository::blob(&repo, oid).map_err(error::git_err)?;
        Ok(blob.content().to_vec())
    }

    /// Bounded commit history page starting at `rev`.
    ///
    /// Ordering: libgit2 revwalk default order (no explicit sort is exposed by
    /// the pinned upstream).
    ///
    /// Pagination contract (binding-validated, `ValueError`):
    /// `1 <= limit <= 1000` (larger limits are clamped to 1000 — only values
    /// below 1 raise, never silently bumped to 1); `offset >= 0`. Signed
    /// parameters are validated explicitly, so negatives raise `ValueError`
    /// — not a conversion `OverflowError`.
    #[pyo3(signature = (rev, *, limit = DEFAULT_HISTORY_LIMIT, offset = 0))]
    fn history(&self, rev: &str, limit: i64, offset: i64) -> PyResult<HistoryPage> {
        if limit < 1 {
            return Err(error::value_err(format!("limit must be >= 1, got {limit}")));
        }
        if offset < 0 {
            return Err(error::value_err(format!(
                "offset must be >= 0, got {offset}"
            )));
        }
        let limit = limit.min(MAX_HISTORY_LIMIT) as usize;
        let offset = offset as usize;
        let repo = self.open()?;
        let obj = repo
            .backend
            .revparse_single(rev)
            .map_err(|err| error::git_err(format!("rev-parse {rev:?}: {err}")))?;
        let head = obj.id();
        let mut walk = ReadRepository::revwalk(&repo, Oid::from(head)).map_err(error::git_err)?;
        // Check the skipped entries too: `Iterator::skip` would discard a walk
        // error (e.g. a missing parent) and end the walk, so a corrupt history
        // past `offset` would masquerade as an empty, final page.
        for skipped in walk.by_ref().take(offset) {
            skipped.map_err(error::git_err)?;
        }
        // Fetch limit+1 to compute has_more without a second walk.
        let mut page: Vec<String> = Vec::with_capacity(limit + 1);
        for oid in walk.take(limit + 1) {
            page.push(oid.map_err(error::git_err)?.to_string());
        }
        let has_more = page.len() > limit;
        page.truncate(limit);
        Ok(HistoryPage {
            name: rev.to_owned(),
            head: head.to_string(),
            oids: page,
            offset,
            has_more,
        })
    }

    /// Namespace sigrefs tips, sorted by node id; reads refs, not signed sets.
    fn namespace_tips(&self) -> PyResult<Vec<NamespaceTip>> {
        let repo = self.open()?;
        let mut tips = Vec::new();
        for reference in repo
            .backend
            .references_glob(radicle::storage::git::SIGREFS_GLOB.as_str())
            .map_err(error::git_err)?
        {
            let reference = reference.map_err(error::git_err)?;
            let name = reference.name().map_err(error::git_err)?;
            let (node_id, _) =
                git::parse_ref_namespaced::<radicle::node::NodeId>(name).map_err(error::git_err)?;
            let resolved = reference.resolve().map_err(error::git_err)?;
            let target = resolved
                .target()
                .ok_or_else(|| error::git_err("sigrefs ref has no target"))?;
            tips.push(NamespaceTip {
                node_id: node_id.to_string(),
                target: target.to_string(),
            });
        }
        tips.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        Ok(tips)
    }

    /// Load and verify a signed set at an explicit sigrefs commit via upstream.
    fn signed_refs_at(&self, node_id: &str, oid: &str) -> PyResult<PySignedRefs> {
        let remote = node_id
            .parse::<radicle::node::NodeId>()
            .map_err(|err| error::value_err(format!("invalid NodeId: {err}")))?;
        let at = parse_oid(oid)?;
        let repo = self.open()?;
        let signed = radicle::storage::refs::SignedRefs::load_at(at, remote, &repo)
            .map_err(|err| error::git_err(format!("sigrefs {oid} for {node_id}: {err}")))?
            .ok_or_else(|| error::git_err(format!("no sigrefs at {oid} for {node_id}")))?;
        Ok(signed.into())
    }

    /// Remotes tracked by this repository (signed refs).
    fn remotes(&self) -> PyResult<Vec<RemoteInfo>> {
        let repo = self.open()?;
        let mut out = Vec::new();
        for remote in repo.remotes().map_err(error::git_err)? {
            let (fetcher, remote) = remote.map_err(error::git_err)?;
            let heads = remote
                .refs
                .iter()
                .map(|(name, _)| name.as_str().to_owned())
                .collect();
            out.push(RemoteInfo {
                fetcher: fetcher.to_string(),
                heads,
            });
        }
        out.sort_by(|a, b| a.fetcher.cmp(&b.fetcher));
        Ok(out)
    }

    // ─── Writes ─────────────────────────────────────────
    // Each maps 1:1 to an upstream `WriteRepository`/`SignRepository` method.
    // The first three take NO signer upstream and take none here; `sign_refs`
    // is the only signer-taking write. No hidden write-then-sign composition.

    /// Compute the default-branch head from the delegate set and set it
    /// (upstream `set_default_branch_to_canonical_head`).
    fn set_default_branch_to_canonical_head(&self) -> PyResult<PySetHead> {
        write::set_default_branch_to_canonical_head(self)
    }

    /// Set the repository's `rad/id` to the canonical identity head agreed
    /// by quorum (upstream `set_identity_head`). Returns the new head.
    fn set_identity_head(&self) -> PyResult<String> {
        write::set_identity_head(self)
    }

    /// Set remote `remote`'s identity-root ref to the repository's canonical
    /// identity root (upstream `set_remote_identity_root`). Returns the root.
    fn set_remote_identity_root(&self, remote: &str) -> PyResult<String> {
        write::set_remote_identity_root(self, remote)
    }

    /// Sign the repository's canonical refs under `refs/rad/sigrefs`
    /// (upstream `sign_refs`). The ONLY signer-taking write in the binding.
    fn sign_refs(&self, signer: &crate::profile::PySigner) -> PyResult<PySignedRefs> {
        write::sign_refs(self, signer)
    }

    fn __repr__(&self) -> String {
        format!("Repository({})", self.rid)
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStorage>()?;
    m.add_class::<PyRepository>()?;
    Ok(())
}

pub fn register_values(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<HeadInfo>()?;
    m.add_class::<Commit>()?;
    m.add_class::<TreeEntry>()?;
    m.add_class::<Tree>()?;
    m.add_class::<TagInfo>()?;
    m.add_class::<RemoteInfo>()?;
    m.add_class::<NamespaceTip>()?;
    m.add_class::<ProjectDoc>()?;
    m.add_class::<HistoryPage>()?;
    Ok(())
}
