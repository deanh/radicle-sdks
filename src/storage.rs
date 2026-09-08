//! Storage and read-only repository access (Phase 2).
//!
//! `PyRepository` holds `Arc<PyStorageInner>` + a `RepoId` and reopens the
//! repository per operation (PLAN §5.3): no long-lived git handles, no
//! borrow-lifetime pressure across the GIL.

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
use crate::values::{Commit, HeadInfo, HistoryPage, ProjectDoc, RemoteInfo, TagInfo, Tree, TreeEntry};

/// Default/maximum page sizes — history walks are always bounded (FINDINGS-02 S7).
const DEFAULT_HISTORY_LIMIT: usize = 100;
const MAX_HISTORY_LIMIT: usize = 1000;

fn to_raw(oid: Oid) -> PyResult<git::raw::Oid> {
    git::raw::Oid::from_str(&oid.to_string())
        .map_err(|err| error::op_err(format!("oid conversion: {err}")))
}

fn parse_oid(value: &str) -> PyResult<Oid> {
    Oid::from_str(value).map_err(|err| error::value_err(format!("invalid Oid: {err}")))
}

fn kind_str(kind: Option<git::raw::ObjectType>) -> Option<String> {
    kind.map(|k| {
        match k {
            git::raw::ObjectType::Any => "any",
            git::raw::ObjectType::Blob => "blob",
            git::raw::ObjectType::Commit => "commit",
            git::raw::ObjectType::Tree => "tree",
            git::raw::ObjectType::Tag => "tag",
        }
        .to_owned()
    })
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

#[pymethods]
impl PyStorage {
    /// Open storage for a profile (primary constructor — UserInfo comes from
    /// the profile, matching `profile.storage`).
    #[classmethod]
    fn from_profile(_cls: &Bound<'_, PyType>, profile: PyRef<'_, PyProfile>) -> Self {
        Self {
            inner: Arc::new(StorageInner {
                raw: profile.inner.storage.clone(),
            }),
        }
    }

    /// Low-level constructor: open (or create) storage at `path` with an
    /// explicit git user identity. Most callers want `Storage.from_profile`.
    #[classmethod]
    fn open(
        _cls: &Bound<'_, PyType>,
        path: &str,
        alias: &str,
        node_id: &str,
    ) -> PyResult<Self> {
        let alias = Alias::from_str(alias)
            .map_err(|err| error::value_err(format!("invalid Alias: {err}")))?;
        let key = node_id
            .parse::<radicle::node::NodeId>()
            .map_err(|err| error::value_err(format!("invalid NodeId: {err}")))?;
        let raw = RawStorage::open(
            path,
            git::UserInfo { alias, key },
        )
        .map_err(error::storage_err)?;
        Ok(Self {
            inner: Arc::new(StorageInner { raw }),
        })
    }

    /// Filesystem path of this storage.
    #[getter]
    fn path(&self) -> PyResult<&str> {
        self.inner.raw
            .path()
            .to_str()
            .ok_or_else(|| error::value_err("storage path is not valid UTF-8"))
    }

    /// Open a repository for reading. Raises `RepositoryError` if absent.
    fn repository(&self, rid: &str) -> PyResult<PyRepository> {
        let rid = RepoId::from_str(rid)
            .map_err(|err| error::value_err(format!("invalid RepoId: {err}")))?;
        // Validate now; operations reopen per call.
        self.inner.raw
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
    fn open(&self) -> PyResult<radicle::storage::git::Repository> {
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
            let name = r
                .name()
                .map_err(error::git_err)?
                .trim_start_matches("refs/heads/")
                .to_owned();
            if r.name().map_err(error::git_err)?.starts_with("refs/heads/") {
                names.push(name);
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
            // let-chains are MSRV-forbidden (1.88 stable), so nested if-let.
            #[allow(clippy::collapsible_if)]
            if let Some(short) = name.strip_prefix("refs/tags/") {
                if let Some(target) = r.target() {
                    raws.push((short.to_owned(), target));
                }
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
            // its tree (FINDINGS-02 S6 trap). For lightweight tags the target
            // already is the peeled object.
            let (peeled, peeled_kind) = if annotated {
                let peeled = obj.peel(git::raw::ObjectType::Any).map_err(error::git_err)?;
                (
                    peeled.id().to_string(),
                    kind_str(peeled.kind()).unwrap_or_else(|| "unknown".into()),
                )
            } else {
                (
                    target.to_string(),
                    kind_str(obj.kind()).unwrap_or_else(|| "unknown".into()),
                )
            };
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

    /// Resolve a git revision expression to an OID.
    /// Raises `GitError` for both invalid and unresolved revisions
    /// (revspec semantics are git's, not ours — PLAN §5.8).
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
    /// the pinned upstream — see FINDINGS-02 S7).
    ///
    /// Pagination contract: `limit` must satisfy `1 <= limit <= 1000`
    /// (a larger limit is clamped to 1000; `limit == 0` raises `ValueError`
    /// rather than silently becoming 1). Negative limits fail argument
    /// conversion (TypeError) before reaching this method.
    #[pyo3(signature = (rev, *, limit = DEFAULT_HISTORY_LIMIT, offset = 0))]
    fn history(&self, rev: &str, limit: usize, offset: usize) -> PyResult<HistoryPage> {
        if limit == 0 {
            return Err(error::value_err("limit must be >= 1"));
        }
        let limit = limit.min(MAX_HISTORY_LIMIT);
        let repo = self.open()?;
        let obj = repo
            .backend
            .revparse_single(rev)
            .map_err(|err| error::git_err(format!("rev-parse {rev:?}: {err}")))?;
        let head = obj.id();
        let walk = ReadRepository::revwalk(&repo, Oid::from(head))
            .map_err(error::git_err)?;
        // Fetch limit+1 to compute has_more without a second walk.
        let mut page: Vec<String> = Vec::with_capacity(limit + 1);
        for oid in walk.skip(offset).take(limit + 1) {
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
    m.add_class::<ProjectDoc>()?;
    m.add_class::<HistoryPage>()?;
    Ok(())
}
