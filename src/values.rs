//! Value classes returned by the repository reads.
//!
//! Conversion schemas live in API.md (§5.4): bytes-first fields with
//! explicitly-lossy `*_str` conveniences; OIDs are lowercase hex strings;
//! timestamps are unix epoch seconds with offset minutes.

use pyo3::prelude::*;

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

// ─── HeadInfo ────────────────────────────────────────────────────────────────

/// The repository head: qualified ref name + target OID
/// (upstream `ReadRepository::head() -> (Qualified, Oid)`).
#[pyclass(name = "HeadInfo", module = "radiclepy", frozen)]
#[derive(Clone)]
pub struct HeadInfo {
    pub(crate) name: String,
    pub(crate) oid: String,
}

#[pymethods]
impl HeadInfo {
    /// Qualified ref name, e.g. `refs/heads/master`.
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    /// Target OID.
    #[getter]
    fn oid(&self) -> &str {
        &self.oid
    }

    fn __repr__(&self) -> String {
        format!("HeadInfo({}, {})", self.name, self.oid)
    }
}

impl HeadInfo {
    pub(crate) fn new(name: String, oid: String) -> Self {
        Self { name, oid }
    }
}

// ─── Commit ──────────────────────────────────────────────────────────────────

/// A git commit. All identity/message fields are bytes-first; `*_str`
/// accessors are explicitly lossy (PLAN §5.4).
#[pyclass(name = "Commit", module = "radiclepy")]
#[derive(Clone)]
pub struct Commit {
    pub(crate) oid: String,
    pub(crate) tree: String,
    pub(crate) parents: Vec<String>,
    pub(crate) author_name: Vec<u8>,
    pub(crate) author_email: Vec<u8>,
    pub(crate) author_time: i64,
    pub(crate) author_offset_min: i32,
    pub(crate) committer_name: Vec<u8>,
    pub(crate) committer_email: Vec<u8>,
    pub(crate) committer_time: i64,
    pub(crate) committer_offset_min: i32,
    pub(crate) message: Vec<u8>,
    pub(crate) encoding: Option<String>,
}

#[pymethods]
impl Commit {
    #[getter]
    fn oid(&self) -> &str {
        &self.oid
    }
    /// OID of the root tree.
    #[getter]
    fn tree(&self) -> &str {
        &self.tree
    }
    #[getter]
    fn parents(&self) -> Vec<String> {
        self.parents.clone()
    }

    #[getter]
    fn author_name(&self) -> Vec<u8> {
        self.author_name.clone()
    }
    /// Lossy view of `author_name` (may contain U+FFFD).
    #[getter]
    fn author_name_str(&self) -> String {
        lossy(&self.author_name)
    }
    #[getter]
    fn author_email(&self) -> Vec<u8> {
        self.author_email.clone()
    }
    #[getter]
    fn author_email_str(&self) -> String {
        lossy(&self.author_email)
    }
    /// Author time, unix epoch seconds.
    #[getter]
    fn author_time(&self) -> i64 {
        self.author_time
    }
    /// Author timezone offset from UTC, in minutes.
    #[getter]
    fn author_offset_min(&self) -> i32 {
        self.author_offset_min
    }

    #[getter]
    fn committer_name(&self) -> Vec<u8> {
        self.committer_name.clone()
    }
    #[getter]
    fn committer_name_str(&self) -> String {
        lossy(&self.committer_name)
    }
    #[getter]
    fn committer_email(&self) -> Vec<u8> {
        self.committer_email.clone()
    }
    #[getter]
    fn committer_email_str(&self) -> String {
        lossy(&self.committer_email)
    }
    #[getter]
    fn committer_time(&self) -> i64 {
        self.committer_time
    }
    #[getter]
    fn committer_offset_min(&self) -> i32 {
        self.committer_offset_min
    }

    /// Raw message bytes (headers stripped). Use your own decoder.
    #[getter]
    fn message(&self) -> Vec<u8> {
        self.message.clone()
    }
    /// Lossy view of `message` (may contain U+FFFD).
    #[getter]
    fn message_str(&self) -> String {
        lossy(&self.message)
    }
    /// Commit object encoding header, if present.
    #[getter]
    fn encoding(&self) -> Option<&str> {
        self.encoding.as_deref()
    }

    fn __repr__(&self) -> String {
        format!("Commit({})", self.oid)
    }
}

// ─── TreeEntry / Tree ────────────────────────────────────────────────────────

/// One entry of a git tree.
///
/// `mode` is the raw unix filemode (0o100644 blob, 0o100755 executable,
/// 0o120000 symlink, 0o040000 tree, 0o160000 gitlink). NB: libgit2 reports
/// symlinks with kind `"blob"` (they are blob objects); use the mode to
/// distinguish them.
///
/// Ordering contract (PLAN §5.4 / FINDINGS-02 S5): lists of entries preserve
/// libgit2's canonical git tree order (directories sort as `<name>/`). The
/// binding never re-sorts.
#[pyclass(name = "TreeEntry", module = "radiclepy")]
#[derive(Clone)]
pub struct TreeEntry {
    pub(crate) name: Vec<u8>,
    pub(crate) kind: Option<String>,
    pub(crate) mode: u32,
    pub(crate) oid: String,
}

#[pymethods]
impl TreeEntry {
    /// Raw entry name bytes (may be invalid UTF-8).
    #[getter]
    fn name(&self) -> Vec<u8> {
        self.name.clone()
    }
    /// Lossy view of `name` (may contain U+FFFD).
    #[getter]
    fn name_str(&self) -> String {
        lossy(&self.name)
    }
    /// `"blob" | "tree" | "commit" | None` (symlinks are `"blob"` with mode
    /// 0o120000; `None` only for exotic/unknown object types).
    #[getter]
    fn kind(&self) -> Option<&str> {
        self.kind.as_deref()
    }
    /// Raw unix filemode.
    #[getter]
    fn mode(&self) -> u32 {
        self.mode
    }
    #[getter]
    fn oid(&self) -> &str {
        &self.oid
    }

    fn __repr__(&self) -> String {
        format!(
            "TreeEntry({:?}, {:?}, {:#o}, {})",
            String::from_utf8_lossy(&self.name),
            self.kind,
            self.mode,
            self.oid
        )
    }
}

/// A git tree: ordered entries in canonical git order (see `TreeEntry`).
#[pyclass(name = "Tree", module = "radiclepy")]
#[derive(Clone)]
pub struct Tree {
    pub(crate) oid: String,
    pub(crate) entries: Vec<TreeEntry>,
}

#[pymethods]
impl Tree {
    #[getter]
    fn oid(&self) -> &str {
        &self.oid
    }
    /// Entries in canonical git tree order.
    #[getter]
    fn entries(&self) -> Vec<TreeEntry> {
        self.entries.clone()
    }

    fn __len__(&self) -> usize {
        self.entries.len()
    }

    fn __repr__(&self) -> String {
        format!("Tree({}, {} entries)", self.oid, self.entries.len())
    }
}

// ─── TagInfo ─────────────────────────────────────────────────────────────────

/// A git tag with explicit raw/peeled distinction (PLAN §5.4, FINDINGS-02 S6).
///
/// Peeling is performed only on tag objects; for lightweight tags the target
/// *is* the peeled object (no silent commit→tree peel).
#[pyclass(name = "TagInfo", module = "radiclepy")]
#[derive(Clone)]
pub struct TagInfo {
    pub(crate) name: String,
    pub(crate) target: String,
    pub(crate) annotated: bool,
    pub(crate) peeled: String,
    pub(crate) peeled_kind: String,
}

#[pymethods]
impl TagInfo {
    /// Short tag name (without `refs/tags/`).
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }
    /// OID the tag ref points at (tag object for annotated tags).
    #[getter]
    fn target(&self) -> &str {
        &self.target
    }
    /// `"annotated"` or `"lightweight"`.
    #[getter]
    fn kind(&self) -> &'static str {
        if self.annotated { "annotated" } else { "lightweight" }
    }
    /// Fully-peeled target OID ("peeled" = resolved through any tag chain to
    /// the final object — commit, tree, or blob; NOT commit-specific). For
    /// lightweight tags this equals `target`.
    #[getter]
    fn peeled(&self) -> &str {
        &self.peeled
    }
    /// Concrete object kind of `peeled`: `"commit" | "tree" | "blob"` (a
    /// fully-peeled target is by construction not a tag object).
    #[getter]
    fn peeled_kind(&self) -> &str {
        &self.peeled_kind
    }

    fn __repr__(&self) -> String {
        format!(
            "TagInfo({}, {}, peeled={} [{}])",
            self.name,
            self.kind(),
            self.peeled,
            self.peeled_kind
        )
    }
}

// ─── RemoteInfo ──────────────────────────────────────────────────────────────

/// A remote tracked by the repository (upstream signed refs).
#[pyclass(name = "RemoteInfo", module = "radiclepy")]
#[derive(Clone)]
pub struct RemoteInfo {
    pub(crate) fetcher: String,
    pub(crate) heads: Vec<String>,
}

#[pymethods]
impl RemoteInfo {
    /// The remote's fetcher node id.
    #[getter]
    fn fetcher(&self) -> &str {
        &self.fetcher
    }
    /// Ref names present in the remote's signed refs (e.g. `refs/heads/main`).
    #[getter]
    fn heads(&self) -> Vec<String> {
        self.heads.clone()
    }

    fn __repr__(&self) -> String {
        format!("RemoteInfo({})", self.fetcher)
    }
}

// ─── ProjectDoc ──────────────────────────────────────────────────────────────

/// The project identity document (canonical identity head), verified at load.
#[pyclass(name = "ProjectDoc", module = "radiclepy")]
#[derive(Clone)]
pub struct ProjectDoc {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) default_branch: String,
    pub(crate) version: u32,
    pub(crate) delegates: Vec<String>,
    pub(crate) threshold: usize,
    pub(crate) visibility: String,
    pub(crate) commit: String,
    pub(crate) blob: String,
}

#[pymethods]
impl ProjectDoc {
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }
    #[getter]
    fn description(&self) -> &str {
        &self.description
    }
    /// Default branch as recorded in the project payload (short name).
    #[getter]
    fn default_branch(&self) -> &str {
        &self.default_branch
    }
    /// Identity document version number.
    #[getter]
    fn version(&self) -> u32 {
        self.version
    }
    #[getter]
    fn delegates(&self) -> Vec<String> {
        self.delegates.clone()
    }
    #[getter]
    fn threshold(&self) -> usize {
        self.threshold
    }
    /// `"public"` or `"private"`.
    #[getter]
    fn visibility(&self) -> &str {
        &self.visibility
    }
    /// Identity commit OID.
    #[getter]
    fn commit(&self) -> &str {
        &self.commit
    }
    /// Identity document blob OID.
    #[getter]
    fn blob(&self) -> &str {
        &self.blob
    }

    fn __repr__(&self) -> String {
        format!(
            "ProjectDoc({}, delegates={} commits to v{})",
            self.name,
            self.delegates.len(),
            self.version
        )
    }
}

// ─── HistoryPage ─────────────────────────────────────────────────────────────

/// One page of commit history.
///
/// Ordering contract (FINDINGS-02 S7): libgit2 revwalk default order; **not**
/// explicitly topological/time-sorted (the pinned upstream does not expose
/// `Sort`). Callers needing strict topology must verify or cap depth.
#[pyclass(name = "HistoryPage", module = "radiclepy")]
#[derive(Clone)]
pub struct HistoryPage {
    pub(crate) name: String,
    pub(crate) head: String,
    pub(crate) oids: Vec<String>,
    pub(crate) offset: usize,
    pub(crate) has_more: bool,
}

#[pymethods]
impl HistoryPage {
    /// The revision expression as passed by the caller (e.g. `"master"`,
    /// `"HEAD~3"`); not normalized to a qualified ref.
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }
    /// Resolved head OID of the walk.
    #[getter]
    fn head(&self) -> &str {
        &self.head
    }
    /// Commit OIDs in this page (walk order).
    #[getter]
    fn oids(&self) -> Vec<String> {
        self.oids.clone()
    }
    /// Number of entries skipped before this page.
    #[getter]
    fn offset(&self) -> usize {
        self.offset
    }
    /// Whether more commits exist beyond this page.
    #[getter]
    fn has_more(&self) -> bool {
        self.has_more
    }

    fn __repr__(&self) -> String {
        format!(
            "HistoryPage({}, {} of {}, has_more={})",
            self.name,
            self.oids.len(),
            self.offset,
            self.has_more
        )
    }
}

// ─── Policy & routing value classes (Phase 3) ────────────────────────────────

/// A node follow policy (upstream `policy::FollowPolicy`).
#[pyclass(name = "FollowPolicyInfo", module = "radiclepy")]
#[derive(Clone)]
pub struct FollowPolicyInfo {
    pub(crate) node_id: String,
    pub(crate) alias: Option<String>,
    pub(crate) policy: String,
}

#[pymethods]
impl FollowPolicyInfo {
    /// The node this policy applies to.
    #[getter]
    fn node_id(&self) -> &str {
        &self.node_id
    }
    /// Optional alias recorded when following.
    #[getter]
    fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
    /// `"allow" | "block"`.
    #[getter]
    fn policy(&self) -> &str {
        &self.policy
    }

    fn __repr__(&self) -> String {
        format!(
            "FollowPolicyInfo({}, {}, {})",
            self.node_id,
            self.alias.as_deref().unwrap_or("-"),
            self.policy
        )
    }
}

/// A repository seed policy (upstream `policy::SeedPolicy`).
///
/// `scope` (`"all" | "followed"`) is present only when policy is `"allow"` —
/// blocked repos have no scope.
#[pyclass(name = "SeedPolicyInfo", module = "radiclepy")]
#[derive(Clone)]
pub struct SeedPolicyInfo {
    pub(crate) rid: String,
    pub(crate) policy: String,
    pub(crate) scope: Option<String>,
}

#[pymethods]
impl SeedPolicyInfo {
    #[getter]
    fn rid(&self) -> &str {
        &self.rid
    }
    /// `"allow" | "block"`.
    #[getter]
    fn policy(&self) -> &str {
        &self.policy
    }
    /// `"all" | "followed"` when allowed; `None` when blocked.
    #[getter]
    fn scope(&self) -> Option<&str> {
        self.scope.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "SeedPolicyInfo({}, {}{})",
            self.rid,
            self.policy,
            self.scope
                .as_deref()
                .map(|s| format!(", scope={s}"))
                .unwrap_or_default()
        )
    }
}

/// One routing-table entry: which node advertises which repo, last-seen.
///
/// `timestamp` is **unix epoch milliseconds** (upstream `node::Timestamp`
/// is a millis `u64` newtype).
#[pyclass(name = "RoutingEntry", module = "radiclepy")]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RoutingEntry {
    pub(crate) rid: String,
    pub(crate) node_id: String,
    pub(crate) timestamp: Option<u64>,
}

#[pymethods]
impl RoutingEntry {
    #[getter]
    fn rid(&self) -> &str {
        &self.rid
    }
    #[getter]
    fn node_id(&self) -> &str {
        &self.node_id
    }
    /// Last-seen time (unix epoch seconds).
    #[getter]
    fn timestamp(&self) -> Option<u64> {
        self.timestamp
    }

    fn __repr__(&self) -> String {
        format!(
            "RoutingEntry({}, {}, {:?})",
            self.rid,
            self.node_id,
            self.timestamp.map(|t| t.to_string())
        )
    }
}
